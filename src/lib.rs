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

use std::ffi::c_void;
use std::os::raw::{c_float, c_int};
use std::sync::{Arc, RwLock};
use std::sync::mpsc::{SyncSender, Receiver, sync_channel};

use face::{ArFaceMesh, FaceTracker};
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
        vertices: [ArVertex3D { x: 0.0, y: 0.0, z: 0.0 }; face::FACE_MESH_VERTICES_COUNT],
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
        
        // Kirim secara non-blocking. Jika worker thread sedang sibuk, segera kembalikan buffer ke recycler.
        if let Err(std::sync::mpsc::TrySendError::Full(buf)) = engine.ml_sender.try_send(image_buffer) {
            let _ = engine.recycle_sender.send(buf);
        }
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

/// Melepaskan alokasi memori internal Fizgravity AR Engine.
/// Harus dipanggil saat aplikasi AR ditutup untuk mencegah kebocoran memori (memory leaks).
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_release(engine_ptr: *mut c_void) {
    if !engine_ptr.is_null() {
        // Ambil kembali kepemilikan box untuk deallokasi otomatis oleh Rust
        let _ = Box::from_raw(engine_ptr as *mut FizgravityEngine);
    }
}
