//! fizgravity-ar-engine
//! Core engine implementation in Rust for Fizard Studio's Fizgravity AR.
//! Handles sensor data fusion, tracking, scene understanding, and lighting estimation.

pub mod face;
pub mod hand;
pub mod physics;
pub mod splatting;
pub mod p2p;
pub mod math;
pub mod imu;
pub mod msckf;
pub mod tsdf;
pub mod lighting;
pub mod extrapolator;
pub mod segmentation;
pub mod skin_analyzer;
pub mod color_harmonizer;
pub mod pbr_makeup;
pub mod eye_contacts;
pub mod stabilizer;
pub mod makeup_triangulator;
pub mod texture_analyzer;

use std::ffi::c_void;
use std::os::raw::{c_float, c_int};
use std::sync::{Arc, RwLock};
use std::sync::mpsc::{SyncSender, Receiver, sync_channel};

use face::{ArFaceMesh, ArTexCoord2D, ArFaceVertexInterleaved, FaceTracker};
use hand::{ArHandJoints, HandTracker};
use physics::{PhysicsSolver};
use splatting::{ArGaussianSplat, SplatManager};
use p2p::{ArVoxelHashKey, P2PManager};

/// Titik 3D vertikal dalam ruang koordinat lokal (X, Y, Z).
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArVertex3D {
    pub x: c_float,
    pub y: c_float,
    pub z: c_float,
}

/// Representasi 3D pose kamera/perangkat melintasi antarmuka FFI C-ABI.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArPose {
    /// Posisi 3D (X, Y, Z) dalam meter.
    pub position: [c_float; 3],
    /// Orientasi kuaternion (W, X, Y, Z).
    pub rotation: [c_float; 4],
}

/// Struktur data untuk menyimpan 9 koefisien Harmonik Sferis (SH) Orde 2
/// per saluran warna RGB, dengan total 27 floats.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArSphericalHarmonics {
    /// Koefisien warna merah (9 floats).
    pub coefficients_r: [c_float; 9],
    /// Koefisien warna hijau (9 floats).
    pub coefficients_g: [c_float; 9],
    /// Koefisien warna biru (9 floats).
    pub coefficients_b: [c_float; 9],
}

/// Keadaan internal terenkapsulasi dari mesin pelacak Fizgravity AR.
pub struct FizgravityEngine {
    // VIO Tracker state (MSCKF)
    pub is_initialized: bool,
    pub current_pose: ArPose,
    pub current_lighting: ArSphericalHarmonics,
    
    // Status wajah & tangan terbagi secara aman lintas-thread (Thread-Safe Shared State)
    pub face_mesh_shared: Arc<RwLock<ArFaceMesh>>,
    pub hand_joints_shared: Arc<RwLock<(ArHandJoints, ArHandJoints, bool, bool)>>, // left, right, left_detected, right_detected

    // Saluran komunikasi untuk mengirim frame gambar kamera ke background worker thread (Bounded Queue)
    pub ml_sender: SyncSender<Vec<u8>>,
    // Saluran daur ulang buffer (Buffer Recycler) untuk menghindari memory churn heap allocation
    pub recycle_receiver: Receiver<Vec<u8>>,
    pub recycle_sender: SyncSender<Vec<u8>>,

    // Solver Fisika & Pengelola Rekonstruksi 3DGS
    pub physics_solver: PhysicsSolver,
    pub splat_manager: SplatManager,
    pub p2p_manager: P2PManager,
    
    // Modul pencahayaan & ekstrapolasi (Late Latching) baru
    pub lighting_estimator: lighting::LightingEstimator,
    pub extrapolator: extrapolator::MotionExtrapolator,
    
    // Stabilizer adaptif untuk meniadakan jitter wajah
    pub face_stabilizer: Arc<RwLock<stabilizer::ArFaceMeshStabilizer>>,
    
    // Fase akumulasi rotasi giroskop untuk shimmer gliter fisika
    pub glitter_phase: RwLock<(f32, f32)>,
}

/// Menginisialisasi instansi baru dari Fizgravity AR Engine.
/// Mengembalikan pointer mentah ke struktur mesin internal.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_init() -> *mut c_void {
    // Bounded queue sebesar 1 frame saja untuk mencegah akumulasi lag frame usang
    let (ml_sender, ml_receiver) = sync_channel::<Vec<u8>>(1);
    // Bounded queue daur ulang buffer sebesar 3 frame (Triple Buffering)
    let (recycle_sender, recycle_receiver) = sync_channel::<Vec<u8>>(3);

    // Alokasikan 3 buffer berukuran 640x480 RGB di awal (Zero-Allocation Pipeline)
    let frame_bytes_size = 640 * 480 * 3;
    for _ in 0..3 {
        let _ = recycle_sender.send(vec![0u8; frame_bytes_size]);
    }

    let face_mesh_shared = Arc::new(RwLock::new(ArFaceMesh {
        vertices: [ArFaceVertexInterleaved {
            position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
            normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
            uv: ArTexCoord2D { u: 0.0, v: 0.0 },
        }; face::FACE_MESH_VERTICES_COUNT],
        blendshapes: [0.0; face::FACE_BLENDSHAPES_COUNT],
    }));

    let hand_joints_shared = Arc::new(RwLock::new((
        ArHandJoints {
            joints: [ArVertex3D { x: 0.0, y: 0.0, z: 0.0 }; hand::HAND_JOINTS_COUNT],
            confidence: 0.0,
            is_right_hand: 0,
        },
        ArHandJoints {
            joints: [ArVertex3D { x: 0.0, y: 0.0, z: 0.0 }; hand::HAND_JOINTS_COUNT],
            confidence: 0.0,
            is_right_hand: 1,
        },
        false, // left_detected
        false, // right_detected
    )));

    // Spawn Thread Pekerja Kombinasi ML (Combined ML Tracker Worker Thread)
    // Penggabungan ini mereduksi frekuensi penyalinan memori data kamera dari 2 kali menjadi 1 kali copy
    let face_mesh_clone = face_mesh_shared.clone();
    let hand_joints_clone = hand_joints_shared.clone();
    let recycle_clone = recycle_sender.clone();
    
    std::thread::spawn(move || {
        let mut face_tracker = FaceTracker::new();
        let mut hand_tracker = HandTracker::new();
        
        while let Ok(image_buffer) = ml_receiver.recv() {
            let ptr = image_buffer.as_ptr() as *const c_void;
            
            // Jalankan kedua model inferensi secara berurutan di background thread
            let _ = face_tracker.update(ptr);
            let _ = hand_tracker.update(ptr);

            // Update jaring wajah
            if let Ok(mut mesh) = face_mesh_clone.write() {
                *mesh = face_tracker.current_mesh;
            }

            // Update sendi tangan
            if let Ok(mut joints) = hand_joints_clone.write() {
                *joints = (
                    hand_tracker.left_hand,
                    hand_tracker.right_hand,
                    hand_tracker.left_hand_detected,
                    hand_tracker.right_hand_detected,
                );
            }

            // Daur ulang buffer kosong ke antrean agar bisa digunakan kembali oleh thread utama
            let _ = recycle_clone.send(image_buffer);
        }
    });

    let engine = Box::new(FizgravityEngine {
        is_initialized: false,
        current_pose: ArPose {
            position: [0.0, 0.0, 0.0],
            rotation: [1.0, 0.0, 0.0, 0.0], // Kuaternion identitas
        },
        current_lighting: ArSphericalHarmonics {
            coefficients_r: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // Nilai ambient default
            coefficients_g: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            coefficients_b: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        },
        face_mesh_shared,
        hand_joints_shared,
        ml_sender,
        recycle_receiver,
        recycle_sender,
        physics_solver: PhysicsSolver::new(),
        splat_manager: SplatManager::new(),
        p2p_manager: P2PManager::new(),
        lighting_estimator: lighting::LightingEstimator::new(),
        extrapolator: extrapolator::MotionExtrapolator::new(0.016),
        face_stabilizer: Arc::new(RwLock::new(stabilizer::ArFaceMeshStabilizer::new(1.5, 0.15))),
        glitter_phase: RwLock::new((0.0, 0.0)),
    });

    Box::into_raw(engine) as *mut c_void
}

/// Memperbarui keadaan mesin pelacakan dengan bingkai video baru dan tumpukan data inersia (IMU).
/// Fungsi ini dirancang untuk dipanggil dari perender pada loop rendering utama.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_update_frame(
    engine_ptr: *mut c_void,
    _timestamp: c_float,
    camera_data: *const c_void,
    _imu_data: *const c_void,
    out_pose: *mut ArPose,
    out_lighting: *mut ArSphericalHarmonics,
) -> c_int {
    if engine_ptr.is_null() {
        return -1; // Pointer mesin null
    }

    let engine = &mut *(engine_ptr as *mut FizgravityEngine);

    // Perbarui tracker internal (VIO)
    engine.is_initialized = true;
    engine.current_pose.position[0] += 0.001; // Simulasi gerakan linier lambat
    
    // Perbarui Wajah & Tangan secara asinkron jika camera_data valid
    if !camera_data.is_null() {
        let frame_bytes_size = 640 * 480 * 3;
        
        // Ambil buffer daur ulang dari recycler. Jika kosong (jarang terjadi), alokasikan buffer baru.
        let mut image_buffer = match engine.recycle_receiver.try_recv() {
            Ok(buf) => buf,
            Err(_) => vec![0u8; frame_bytes_size],
        };

        // Salin piksel kamera secara langsung tanpa alokasi heap baru (Zero Memory Allocation)
        std::ptr::copy_nonoverlapping(
            camera_data as *const u8,
            image_buffer.as_mut_ptr(),
            frame_bytes_size,
        );

        // Estima pencahayaan global (ambient SH) secara real-time dari frame kamera
        engine.lighting_estimator.estimate_ambient_sh(camera_data, 640, 480);
        engine.current_lighting = engine.lighting_estimator.current_sh;
        
        // Kirim secara non-blocking. Jika worker thread sedang sibuk atau mati (disconnected), tangani secara aman.
        if let Err(err) = engine.ml_sender.try_send(image_buffer) {
            match err {
                std::sync::mpsc::TrySendError::Full(buf) => {
                    let _ = engine.recycle_sender.send(buf);
                }
                std::sync::mpsc::TrySendError::Disconnected(_buf) => {
                    // Terjadi jika thread pekerja ML mengalami kepanikan (panic) atau tertutup.
                    eprintln!("[Fizgravity AR Warning] ML worker thread has disconnected or panicked!");
                }
            }
        }
    }

    // Jika data IMU tersedia, kita lakukan prediksi pose ke depan menggunakan ekstrapolasi kinematik (Late Latching)
    if !_imu_data.is_null() {
        // Ambil orientasi dan posisi nominal dari tracker VIO saat ini
        // Ekstrak orientasi aktif dari kuaternion pose saat ini (menghilangkan bias rotasi identitas)
        let current_q = nalgebra::UnitQuaternion::new_normalize(nalgebra::Quaternion::new(
            engine.current_pose.rotation[0], // W
            engine.current_pose.rotation[1], // X
            engine.current_pose.rotation[2], // Y
            engine.current_pose.rotation[3], // Z
        ));
        let current_r = current_q.to_rotation_matrix();
        let current_p = nalgebra::Vector3::new(engine.current_pose.position[0], engine.current_pose.position[1], engine.current_pose.position[2]);
        let current_v = nalgebra::Vector3::zeros();

        // Data inersia mentah disimulasikan dari _imu_data
        let gyro = nalgebra::Vector3::new(0.01, 0.02, 0.0);
        let acc = nalgebra::Vector3::new(0.0, 0.0, 9.81);
        let bg = nalgebra::Vector3::new(0.0, 0.0, 0.0);
        let ba = nalgebra::Vector3::new(0.0, 0.0, 0.0);

        let (r_pred, p_pred) = engine.extrapolator.extrapolate_pose(
            0.016, // Horizon prediksi dinamis disuplai di sini
            &current_r,
            &current_p,
            &current_v,
            &gyro,
            &acc,
            &bg,
            &ba
        );

        // Perbarui pose dengan estimasi prediktif untuk meniadakan lag visual pan kamera
        engine.current_pose.position = [p_pred.x, p_pred.y, p_pred.z];
        let q = nalgebra::UnitQuaternion::from_rotation_matrix(&r_pred);
        engine.current_pose.rotation = [q.w, q.i, q.j, q.k];
    }
    
    if !out_pose.is_null() {
        *out_pose = engine.current_pose;
    }
    
    if !out_lighting.is_null() {
        *out_lighting = engine.current_lighting;
    }

    0 // Sukses
}

/// Mengekstrak estimasi geometri jaring wajah terupdate.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_get_face_mesh(
    engine_ptr: *mut c_void,
    out_mesh: *mut ArFaceMesh,
) -> c_int {
    if engine_ptr.is_null() || out_mesh.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *mut FizgravityEngine);
    
    // Membaca status mesh wajah teranyar secara thread-safe non-blocking
    if let Ok(mesh) = engine.face_mesh_shared.read() {
        *out_mesh = *mesh;
        0
    } else {
        -2 // Gagal memperoleh lock
    }
}

/// Mengekstrak jaring leher virtual hasil ekstrapolasi.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_get_neck_mesh(
    engine_ptr: *mut c_void,
    out_neck: *mut face::ArNeckMesh,
) -> c_int {
    if engine_ptr.is_null() || out_neck.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *mut FizgravityEngine);
    
    if let Ok(mesh) = engine.face_mesh_shared.read() {
        *out_neck = face::ArNeckExtender::extrapolate_neck(&mesh.vertices);
        0
    } else {
        -2
    }
}

/// Mengekstrak sendi koordinat tangan 3D yang dilacak.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_get_hand_joints(
    engine_ptr: *mut c_void,
    is_right: c_int,
    out_joints: *mut ArHandJoints,
) -> c_int {
    if engine_ptr.is_null() || out_joints.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *mut FizgravityEngine);
    
    // Membaca status tangan teranyar secara thread-safe non-blocking
    if let Ok(joints_data) = engine.hand_joints_shared.read() {
        let (left_hand, right_hand, left_detected, right_detected) = *joints_data;
        if is_right == 1 {
            if right_detected {
                *out_joints = right_hand;
                return 0;
            }
        } else {
            if left_detected {
                *out_joints = left_hand;
                return 0;
            }
        }
    }
    -2 // Tangan tidak terdeteksi atau lock failed
}

/// Menambahkan plane collider lantai / meja statis ke simulator fisika.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_add_physics_plane(
    engine_ptr: *mut c_void,
    id: c_int,
    x: c_float,
    y: c_float,
    z: c_float,
) -> c_int {
    if engine_ptr.is_null() {
        return -1;
    }
    let engine = &mut *(engine_ptr as *mut FizgravityEngine);
    engine.physics_solver.add_plane_collider(id, ArVertex3D { x, y, z });
    0
}

/// Melakukan simulasi satu langkah maju (step) fisika dengan delta waktu tertentu.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_step_physics(
    engine_ptr: *mut c_void,
    delta_time: c_float,
) -> c_int {
    if engine_ptr.is_null() {
        return -1;
    }
    let engine = &mut *(engine_ptr as *mut FizgravityEngine);
    engine.physics_solver.step_simulation(delta_time)
}

/// Menyalin koordinat 3D Gaussian Splats terupdate ke buffer perrender.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_get_gaussian_splats(
    engine_ptr: *mut c_void,
    out_splats: *mut ArGaussianSplat,
    max_count: c_int,
) -> c_int {
    if engine_ptr.is_null() || out_splats.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *mut FizgravityEngine);
    let count = std::cmp::min(max_count as usize, engine.splat_manager.splats.len());
    
    let splat_slice = std::slice::from_raw_parts_mut(out_splats, count);
    for i in 0..count {
        splat_slice[i] = engine.splat_manager.splats[i];
    }
    
    count as c_int
}

/// 	Mengepaskan (fitting) awan titik mentah ke dalam representasi elipsoid 3D Gaussians.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_fit_point_cloud_to_gaussians(
    engine_ptr: *mut c_void,
    points: *const ArVertex3D,
    count: c_int,
) -> c_int {
    if engine_ptr.is_null() || points.is_null() {
        return -1;
    }
    let engine = &mut *(engine_ptr as *mut FizgravityEngine);
    engine.splat_manager.fit_gaussians_from_point_cloud(points, count)
}

/// Memulai pemindaian perangkat terdekat untuk kolaborasi spasial P2P lokal.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_p2p_start_discovery(engine_ptr: *mut c_void) -> c_int {
    if engine_ptr.is_null() {
        return -1;
    }
    let engine = &mut *(engine_ptr as *mut FizgravityEngine);
    engine.p2p_manager.start_discovery()
}

/// Sinkronisasi voxel delta keys dengan perangkat terdekat.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_p2p_sync_voxels(
    engine_ptr: *mut c_void,
    keys: *const ArVoxelHashKey,
    count: c_int,
) -> c_int {
    if engine_ptr.is_null() || keys.is_null() {
        return -1;
    }
    let engine = &mut *(engine_ptr as *mut FizgravityEngine);
    engine.p2p_manager.send_voxel_delta(keys, count)
}

/// Mengimpor koordinat wajah nyata hasil deteksi Google ML Kit (Kotlin/Swift) ke dalam shared state Rust.
/// Ini memungkinkan Late Latching dan diagnostik AI berjalan menggunakan tracking hardware-accelerated.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_set_face_mesh(
    engine_ptr: *mut c_void,
    vertices_ptr: *const ArVertex3D,
    blendshapes_ptr: *const c_float,
) -> c_int {
    if engine_ptr.is_null() || vertices_ptr.is_null() || blendshapes_ptr.is_null() {
        return -1;
    }
    let engine = &mut *(engine_ptr as *mut FizgravityEngine);

    if let Ok(mut shared_mesh) = engine.face_mesh_shared.write() {
        let vertices_slice = std::slice::from_raw_parts(vertices_ptr, face::FACE_MESH_VERTICES_COUNT);
        for i in 0..face::FACE_MESH_VERTICES_COUNT {
            shared_mesh.vertices[i].position = vertices_slice[i];
            
            // Map static UV coordinates based on polar angles
            let angle = (i as f32) * std::f32::consts::PI / 234.0;
            shared_mesh.vertices[i].uv = face::ArTexCoord2D {
                u: (angle.cos() + 1.0) * 0.5,
                v: (angle.sin() + 1.0) * 0.5,
            };
        }

        // Hitung normal wajah 3D secara radial elipsoid
        face::compute_face_normals(&mut shared_mesh.vertices);

        let blendshapes_slice = std::slice::from_raw_parts(blendshapes_ptr, face::FACE_BLENDSHAPES_COUNT);
        shared_mesh.blendshapes.copy_from_slice(blendshapes_slice);

        // Stabilisasikan face mesh secara adaptif menggunakan One-Euro Filter
        if let Ok(mut stabilizer) = engine.face_stabilizer.write() {
            stabilizer.stabilize_face_mesh(&mut shared_mesh.vertices, 0.016);
        }
        
        0 // Sukses
    } else {
        -2 // Gagal write lock
    }
}

/// Melepaskan alokasi memori internal Fizgravity AR Engine.
/// Harus dipanggil saat aplikasi AR ditutup untuk mencegah kebocoran memori (memory leaks).
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_release(engine_ptr: *mut c_void) {
    if !engine_ptr.is_null() {
        // Ambil kembali kepemilikan box untuk deallokasi otomatis oleh Rust
        let _ = Box::from_raw(engine_ptr as *mut FizgravityEngine);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_face_mesh_bypass() {
        // Inisialisasi engine
        let engine_ptr = unsafe { fizgravity_engine_init() };
        assert!(!engine_ptr.is_null());

        let test_vertices = [ArVertex3D { x: 1.0, y: 2.0, z: 3.0 }; face::FACE_MESH_VERTICES_COUNT];
        let test_blendshapes = [0.5; face::FACE_BLENDSHAPES_COUNT];

        // Set face mesh via FFI
        let res = unsafe {
            fizgravity_engine_set_face_mesh(
                engine_ptr,
                test_vertices.as_ptr(),
                test_blendshapes.as_ptr(),
            )
        };
        assert_eq!(res, 0);

        // Verifikasi shared state terupdate
        let engine = unsafe { &*(engine_ptr as *const FizgravityEngine) };
        if let Ok(mesh) = engine.face_mesh_shared.read() {
            assert_eq!(mesh.vertices[0].position.x, 1.0);
            assert_eq!(mesh.vertices[0].position.y, 2.0);
            assert_eq!(mesh.vertices[0].position.z, 3.0);
            assert_eq!(mesh.blendshapes[0], 0.5);
        } else {
            panic!("Gagal mengunci read lock");
        }

        // Release engine
        unsafe { fizgravity_engine_release(engine_ptr) };
    }

    #[test]
    fn test_get_neck_mesh() {
        let engine_ptr = unsafe { fizgravity_engine_init() };
        assert!(!engine_ptr.is_null());

        let mut neck_mesh = face::ArNeckMesh {
            vertices: [ArFaceVertexInterleaved {
                position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
                normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
                uv: ArTexCoord2D { u: 0.0, v: 0.0 },
            }; 34],
            indices: [0; 96],
        };

        let res = unsafe { fizgravity_engine_get_neck_mesh(engine_ptr, &mut neck_mesh) };
        assert_eq!(res, 0);

        // Verifikasi indices terisi dengan benar (tidak bernilai 0 semua)
        assert_eq!(neck_mesh.indices[0], 0);
        assert_eq!(neck_mesh.indices[1], 1);
        assert_eq!(neck_mesh.indices[2], 17);
        assert_eq!(neck_mesh.indices[3], 17);

        unsafe { fizgravity_engine_release(engine_ptr) };
    }
}

/// Mengambil indeks segitiga triangulasi untuk bibir atas (Upper Lip).
/// Menulis indeks ke buffer out_indices dan mengembalikan jumlah indeks yang ditulis.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_get_upper_lip_indices(
    out_indices: *mut u32,
    max_count: c_int,
) -> c_int {
    if out_indices.is_null() {
        return -1;
    }
    let triangles = makeup_triangulator::MakeupTriangulator::get_upper_lip_triangles();
    let count = std::cmp::min(max_count as usize, triangles.len());
    let slice = std::slice::from_raw_parts_mut(out_indices, count);
    for i in 0..count {
        slice[i] = triangles[i];
    }
    count as c_int
}

/// Mengambil indeks segitiga triangulasi untuk bibir bawah (Lower Lip).
/// Menulis indeks ke buffer out_indices dan mengembalikan jumlah indeks yang ditulis.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_get_lower_lip_indices(
    out_indices: *mut u32,
    max_count: c_int,
) -> c_int {
    if out_indices.is_null() {
        return -1;
    }
    let triangles = makeup_triangulator::MakeupTriangulator::get_lower_lip_triangles();
    let count = std::cmp::min(max_count as usize, triangles.len());
    let slice = std::slice::from_raw_parts_mut(out_indices, count);
    for i in 0..count {
        slice[i] = triangles[i];
    }
    count as c_int
}

#[cfg(test)]
mod makeup_tests {
    use super::*;

    #[test]
    fn test_ffi_get_lip_indices() {
        let mut upper_indices = [0u32; 60];
        let count = unsafe { fizgravity_engine_get_upper_lip_indices(upper_indices.as_mut_ptr(), 60) };
        assert_eq!(count, 60);
        assert_eq!(upper_indices[0], 61);

        let mut lower_indices = [0u32; 60];
        let count2 = unsafe { fizgravity_engine_get_lower_lip_indices(lower_indices.as_mut_ptr(), 60) };
        assert_eq!(count2, 60);
    }
}

use std::os::raw::c_uchar;

/// Mengambil estimasi suhu warna (Kelvin) dan intensitas cahaya sekitar menggunakan formula McCamy.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_get_ambient_cct_and_intensity(
    engine_ptr: *mut c_void,
    out_temp: *mut f32,
    out_intensity: *mut f32,
) -> c_int {
    if engine_ptr.is_null() || out_temp.is_null() || out_intensity.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *const FizgravityEngine);
    let (temp, intensity) = engine.lighting_estimator.estimate_temperature_and_intensity();
    *out_temp = temp;
    *out_intensity = intensity;
    0
}

/// Menghitung pergeseran koordinat specular gliter secara dinamis berdasarkan data sensor giroskop.
/// Menggunakan leaky integrator untuk meluruhkan offset drift rotasi secara berkala.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_calculate_glitter_shimmer_shift(
    engine_ptr: *mut c_void,
    gyro_x: f32,
    gyro_y: f32,
    gyro_z: f32,
    dt: f32,
    out_shift_x: *mut f32,
    out_shift_y: *mut f32,
) -> c_int {
    if engine_ptr.is_null() || out_shift_x.is_null() || out_shift_y.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *const FizgravityEngine);
    if let Ok(mut phase) = engine.glitter_phase.write() {
        let decay = 0.96;
        phase.0 = (phase.0 + gyro_x * dt) * decay;
        phase.1 = (phase.1 + gyro_y * dt) * decay;

        let sensitivity = 0.15;
        *out_shift_x = phase.1 * sensitivity;
        *out_shift_y = phase.0 * sensitivity;
        0
    } else {
        -2
    }
}

/// Menghitung pemulusan batas hairline pada dahi secara dinamis untuk foundation wajah.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_calculate_hairline_blending(
    engine_ptr: *mut c_void,
    out_alphas: *mut f32,
    max_count: c_int,
) -> c_int {
    if engine_ptr.is_null() || out_alphas.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *const FizgravityEngine);
    if let Ok(shared_mesh) = engine.face_mesh_shared.read() {
        let count = std::cmp::min(max_count as usize, face::FACE_MESH_VERTICES_COUNT);
        let alphas_slice = std::slice::from_raw_parts_mut(out_alphas, count);
        
        for alpha in alphas_slice.iter_mut() {
            *alpha = 1.0;
        }

        let mut alphas_temp = [1.0f32; face::FACE_MESH_VERTICES_COUNT];
        makeup_triangulator::MakeupTriangulator::calculate_hairline_blending(&shared_mesh.vertices, &mut alphas_temp);

        for i in 0..count {
            alphas_slice[i] = alphas_temp[i];
        }
        0
    } else {
        -2
    }
}

/// Menganalisis kondisi tekstur kulit, kerutan dahi, dan noda jerawat dari buffer gambar RGB kamera.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_analyze_skin_health(
    image_rgb_ptr: *const c_uchar,
    width: c_int,
    height: c_int,
    out_roughness: *mut f32,
    out_wrinkles: *mut f32,
) -> c_int {
    if image_rgb_ptr.is_null() || out_roughness.is_null() || out_wrinkles.is_null() || width <= 100 || height <= 100 {
        return -1;
    }
    let img_size = (width * height * 3) as usize;
    let image_slice = std::slice::from_raw_parts(image_rgb_ptr, img_size);

    let forehead_w = (width / 3) as usize;
    let forehead_h = (height / 6) as usize;
    let forehead_x = (width / 3) as usize;
    let forehead_y = (height / 8) as usize;

    let cheek_w = (width / 5) as usize;
    let cheek_h = (height / 5) as usize;
    let cheek_x = (width / 4) as usize;
    let cheek_y = (height / 2) as usize;

    let (roughness, _) = texture_analyzer::SkinTextureAnalyzer::analyze_roi(
        image_slice,
        width as usize,
        height as usize,
        cheek_x,
        cheek_y,
        cheek_w,
        cheek_h,
    );

    let wrinkles = texture_analyzer::SkinTextureAnalyzer::analyze_wrinkles(
        image_slice,
        width as usize,
        height as usize,
        forehead_x,
        forehead_y,
        forehead_w,
        forehead_h,
    );

    *out_roughness = roughness;
    *out_wrinkles = wrinkles;
    0
}

#[cfg(test)]
mod priority_ffi_tests {
    use super::*;

    #[test]
    fn test_ffi_glitter_shift() {
        let engine_ptr = unsafe { fizgravity_engine_init() };
        assert!(!engine_ptr.is_null());

        let mut sx = 0.0f32;
        let mut sy = 0.0f32;
        let res = unsafe {
            fizgravity_engine_calculate_glitter_shimmer_shift(engine_ptr, 1.0, 2.0, 0.0, 0.016, &mut sx, &mut sy)
        };
        assert_eq!(res, 0);
        assert!(sx.abs() > 0.0);
        assert!(sy.abs() > 0.0);

        unsafe { fizgravity_engine_release(engine_ptr) };
    }

    #[test]
    fn test_ffi_skin_health() {
        let rgb_data = vec![128u8; 320 * 240 * 3];
        let mut roughness = 0.0f32;
        let mut wrinkles = 0.0f32;
        let res = unsafe {
            fizgravity_engine_analyze_skin_health(rgb_data.as_ptr(), 320, 240, &mut roughness, &mut wrinkles)
        };
        assert_eq!(res, 0);
    }
}
