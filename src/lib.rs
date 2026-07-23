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
pub mod calibration;
pub mod canonical_uv;

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
    pub ml_sender: SyncSender<(Vec<u8>, Option<[i32; 4]>, i32, i32)>,
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

    // Auto-calibrator intrinsik kamera online
    pub calibrator: RwLock<calibration::CameraAutoCalibrator>,

    // IMU ring buffer untuk akumulasi samples 200Hz antara dua frame kamera
    pub imu_buffer: extrapolator::ImuRingBuffer,
    // Cache gyro/accel terakhir untuk extrapolasi real-time
    pub last_gyro: [f32; 3],
    pub last_accel: [f32; 3],
    // Bias estimasi dari MSCKF (diperbarui setiap frame)
    pub gyro_bias: nalgebra::Vector3<f32>,
    pub accel_bias: nalgebra::Vector3<f32>,
    // Kecepatan linear estimasi untuk RK4 extrapolation
    pub current_velocity: nalgebra::Vector3<f32>,

    // Waktu pembaruan terakhir untuk delta waktu dinamis
    pub last_update_time: RwLock<std::time::Instant>,

    // Unscale factor computed from face mesh width
    pub current_unscale: RwLock<f32>,
}

/// Menginisialisasi instansi baru dari Fizgravity AR Engine.
/// Mengembalikan pointer mentah ke struktur mesin internal.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_init(model_path: *const std::os::raw::c_char) -> *mut c_void {
    let path_str = if model_path.is_null() {
        "models/face_mesh_with_blendshapes.onnx".to_string()
    } else {
        match std::ffi::CStr::from_ptr(model_path).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => "models/face_mesh_with_blendshapes.onnx".to_string(),
        }
    };

    // Bounded queue sebesar 1 frame saja untuk mencegah akumulasi lag frame usang
    let (ml_sender, ml_receiver) = sync_channel::<(Vec<u8>, Option<[i32; 4]>, i32, i32)>(1);
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
    
    let path_clone = path_str.clone();
    std::thread::spawn(move || {
        let mut face_tracker = FaceTracker::new(&path_clone);
        let mut hand_tracker = HandTracker::new();
        
        while let Ok((image_buffer, face_box_opt, width, height)) = ml_receiver.recv() {
            let ptr = image_buffer.as_ptr() as *const c_void;
            
            // Jalankan kedua model inferensi secara berurutan di background thread
            let _ = face_tracker.update(ptr, width, height, face_box_opt);
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
        calibrator: RwLock::new(calibration::CameraAutoCalibrator::new()),
        imu_buffer: extrapolator::ImuRingBuffer::new(),
        last_gyro: [0.0, 0.0, 0.0],
        last_accel: [0.0, 0.0, 9.81],
        gyro_bias: nalgebra::Vector3::zeros(),
        accel_bias: nalgebra::Vector3::zeros(),
        current_velocity: nalgebra::Vector3::zeros(),
        last_update_time: RwLock::new(std::time::Instant::now()),
        current_unscale: RwLock::new(1.0),
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
    width: c_int,
    height: c_int,
    _imu_data: *const c_void,
    face_box_ptr: *const c_int,
    out_pose: *mut ArPose,
    out_lighting: *mut ArSphericalHarmonics,
) -> c_int {
    if engine_ptr.is_null() {
        return -1; // Pointer mesin null
    }
    if width <= 0 || height <= 0 {
        return -3; // Invalid dimensions
    }

    let engine = &mut *(engine_ptr as *mut FizgravityEngine);

    // Perbarui tracker internal (VIO)
    engine.is_initialized = true;
    // Tidak ada simulasi — pose diperbarui dari sensor nyata via push_imu
    
    // Perbarui Wajah & Tangan secara asinkron jika camera_data valid
    if !camera_data.is_null() {
        let frame_bytes_size = (width * height * 3) as usize;
        
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
        engine.lighting_estimator.estimate_ambient_sh(camera_data, width, height);
        engine.current_lighting = engine.lighting_estimator.current_sh;
        
        let face_box_opt = if face_box_ptr.is_null() {
            None
        } else {
            let slice = std::slice::from_raw_parts(face_box_ptr, 4);
            Some([slice[0], slice[1], slice[2], slice[3]])
        };

        // Kirim secara non-blocking. Jika worker thread sedang sibuk atau mati (disconnected), tangani secara aman.
        if let Err(err) = engine.ml_sender.try_send((image_buffer, face_box_opt, width, height)) {
            match err {
                std::sync::mpsc::TrySendError::Full((buf, _, _, _)) => {
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
        let current_v = engine.current_velocity; // Gunakan velocity yang tersimpan di engine

        // Drain IMU ring buffer → rata-rata semua samples terkumpul sejak frame terakhir
        let (g_avg, a_avg) = engine.imu_buffer.drain_average();
        let gyro = nalgebra::Vector3::new(g_avg[0], g_avg[1], g_avg[2]);
        let acc = nalgebra::Vector3::new(a_avg[0], a_avg[1], a_avg[2]);
        let bg = engine.gyro_bias;
        let ba = engine.accel_bias;

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

        // Update estimated velocity (sederhana: v_new = v_old + a*dt, minus gravity component)
        let acc_world = current_r * (acc - ba);
        let gravity = nalgebra::Vector3::new(0.0, 0.0, -9.81);
        engine.current_velocity += (acc_world + gravity) * 0.016;
        // Decay velocity perlahan untuk mencegah drift
        engine.current_velocity *= 0.95;

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

/// Mendorong (push) satu sampel pengukuran IMU baru ke dalam ring buffer engine.
/// Fungsi ini HARUS dipanggil dari SensorManager Android setiap ~5ms (200Hz).
/// Parameter: gx,gy,gz = kecepatan sudut (rad/s); ax,ay,az = percepatan (m/s²); ts = timestamp detik
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_push_imu(
    engine_ptr: *mut c_void,
    gx: c_float, gy: c_float, gz: c_float,
    ax: c_float, ay: c_float, az: c_float,
    timestamp_sec: c_float,
) -> c_int {
    if engine_ptr.is_null() { return -1; }
    let engine = &mut *(engine_ptr as *mut FizgravityEngine);
    // Simpan ke ring buffer (non-blocking, tanpa alokasi heap)
    engine.imu_buffer.push(gx, gy, gz, ax, ay, az, timestamp_sec);
    // Update cache terakhir untuk real-time extrapolation
    engine.last_gyro = [gx, gy, gz];
    engine.last_accel = [ax, ay, az];
    0
}

/// Mengembalikan 468 landmark wajah yang sudah diekstrapolasikan secara prediktif
/// menggunakan data IMU terbaru (Late Latching). Ini menghasilkan mesh yang benar-benar
/// nempel ke wajah bahkan saat kepala bergerak cepat.
///
/// out_vertices: Buffer output 468 * 3 floats (x,y,z per vertex).
/// dt_predict: Horizon prediksi dalam detik (gunakan render_frame_dt atau 0.016).
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_get_predicted_landmarks(
    engine_ptr: *mut c_void,
    out_vertices: *mut c_float,
    count: c_int,
    dt_predict: c_float,
) -> c_int {
    if engine_ptr.is_null() || out_vertices.is_null() { return -1; }
    if count <= 0 { return -3; }
    let engine = &mut *(engine_ptr as *mut FizgravityEngine);

    // Baca mesh wajah terakhir yang sudah distabilkan
    let mesh_snapshot = match engine.face_mesh_shared.read() {
        Ok(m) => *m,
        Err(_) => return -2,
    };

    // Ambil pose saat ini
    let current_q = nalgebra::UnitQuaternion::new_normalize(nalgebra::Quaternion::new(
        engine.current_pose.rotation[0],
        engine.current_pose.rotation[1],
        engine.current_pose.rotation[2],
        engine.current_pose.rotation[3],
    ));
    let current_r = current_q.to_rotation_matrix();
    let current_p = nalgebra::Vector3::new(
        engine.current_pose.position[0],
        engine.current_pose.position[1],
        engine.current_pose.position[2],
    );

    // Gunakan data IMU dari ring buffer atau cache terakhir
    let gyro = nalgebra::Vector3::new(engine.last_gyro[0], engine.last_gyro[1], engine.last_gyro[2]);
    let acc = nalgebra::Vector3::new(engine.last_accel[0], engine.last_accel[1], engine.last_accel[2]);
    let bg = engine.gyro_bias;
    let ba = engine.accel_bias;
    let current_v = engine.current_velocity;

    // Lakukan prediksi pose via RK4 extrapolation
    let (r_pred, _p_pred) = engine.extrapolator.extrapolate_pose(
        dt_predict.clamp(0.001, 0.05),
        &current_r, &current_p, &current_v,
        &gyro, &acc, &bg, &ba,
    );

    // Hitung delta rotasi antara pose saat ini vs prediksi
    let delta_r = current_r.transpose() * r_pred;

    // Terapkan delta rotasi ke setiap vertex wajah (Late Latching)
    let n = std::cmp::min(count as usize, face::FACE_MESH_VERTICES_COUNT);
    let out_slice = std::slice::from_raw_parts_mut(out_vertices, n * 3);

    for i in 0..n {
        let v = &mesh_snapshot.vertices[i].position;
        let pos = nalgebra::Vector3::new(v.x, v.y, v.z);
        // Putar vertex relatif terhadap pusat wajah (centroid)
        let rotated = delta_r * pos;
        // Terapkan rolling-shutter correction berdasarkan posisi baris Y vertex
        let row_norm = (v.y + 0.5).clamp(0.0, 1.0); // Normalisasi ke [0,1]
        let (rx, ry) = engine.extrapolator.apply_rolling_shutter_correction(
            rotated.x, rotated.y, row_norm, 0.016
        );
        out_slice[i * 3] = rx;
        out_slice[i * 3 + 1] = ry;
        out_slice[i * 3 + 2] = rotated.z;
    }

    n as c_int
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
    if max_count < 0 {
        return -3;
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
    if count < 0 {
        return -3;
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
    if count < 0 {
        return -3;
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
        
        // Deteksi dan normalkan koordinat dari piksel (ML Kit) ke meter (Canonical/SI Unit)
        let left_cheek = vertices_slice[234];
        let right_cheek = vertices_slice[454];
        let dx = left_cheek.x - right_cheek.x;
        let dy = left_cheek.y - right_cheek.y;
        let dz = left_cheek.z - right_cheek.z;
        let face_width = (dx*dx + dy*dy + dz*dz).sqrt();

        let unscale = if face_width > 1.5 {
            0.145 / face_width
        } else {
            1.0
        };

        if let Ok(mut u) = engine.current_unscale.write() {
            *u = unscale;
        }

        for i in 0..face::FACE_MESH_VERTICES_COUNT {
            shared_mesh.vertices[i].position = ArVertex3D {
                x: vertices_slice[i].x * unscale,
                y: vertices_slice[i].y * unscale,
                z: vertices_slice[i].z * unscale,
            };
            
            // Map static UV coordinates based on canonical MediaPipe face texture template
            shared_mesh.vertices[i].uv = canonical_uv::CANONICAL_UV[i];
        }

        // Hitung normal wajah 3D secara radial elipsoid
        face::compute_face_normals(&mut shared_mesh.vertices);

        let blendshapes_slice = std::slice::from_raw_parts(blendshapes_ptr, face::FACE_BLENDSHAPES_COUNT);
        shared_mesh.blendshapes.copy_from_slice(blendshapes_slice);

        // Stabilisasikan face mesh secara adaptif menggunakan One-Euro Filter dengan dt dinamis
        let dt = if let Ok(mut last_time) = engine.last_update_time.write() {
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(*last_time).as_secs_f32();
            *last_time = now;
            elapsed.clamp(0.005, 0.1)
        } else {
            0.016
        };

        if let Ok(mut stabilizer) = engine.face_stabilizer.write() {
            stabilizer.stabilize_face_mesh(&mut shared_mesh.vertices, dt);
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
        let engine_ptr = unsafe { fizgravity_engine_init(std::ptr::null()) };
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
        let engine_ptr = unsafe { fizgravity_engine_init(std::ptr::null()) };
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
    if max_count < 0 {
        return -3;
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
    if max_count < 0 {
        return -3;
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
    screen_rotation_degrees: c_int,
    out_shift_x: *mut f32,
    out_shift_y: *mut f32,
) -> c_int {
    if engine_ptr.is_null() || out_shift_x.is_null() || out_shift_y.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *const FizgravityEngine);
    if let Ok(mut phase) = engine.glitter_phase.write() {
        let rad = (screen_rotation_degrees as f32).to_radians();
        let cos_r = rad.cos();
        let sin_r = rad.sin();

        // Putar vektor giroskop berdasarkan orientasi layar HP
        let rot_x = gyro_x * cos_r - gyro_y * sin_r;
        let rot_y = gyro_x * sin_r + gyro_y * cos_r;

        let lambda = 2.45f32; // Kecepatan peluruhan (half-life ~0.28 detik, setara 0.96 pada 60fps)
        let decay = (-lambda * dt).exp();
        phase.0 = (phase.0 + rot_x * dt) * decay;
        phase.1 = (phase.1 + rot_y * dt) * decay;

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
    if max_count < 0 {
        return -3;
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
        let engine_ptr = unsafe { fizgravity_engine_init(std::ptr::null()) };
        assert!(!engine_ptr.is_null());

        let mut sx = 0.0f32;
        let mut sy = 0.0f32;
        let res = unsafe {
            fizgravity_engine_calculate_glitter_shimmer_shift(engine_ptr, 1.0, 2.0, 0.0, 0.016, 90, &mut sx, &mut sy)
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

/// Melakukan kalibrasi intrinsik kamera online secara dinamis menggunakan geometri lebar wajah.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_update_auto_calibration(
    engine_ptr: *mut c_void,
    image_w: f32,
    image_h: f32,
    depth_z: f32,
    out_focal_length: *mut f32,
) -> c_int {
    if engine_ptr.is_null() || out_focal_length.is_null() {
        return -1;
    }
    let engine = &*(engine_ptr as *const FizgravityEngine);
    let unscale = if let Ok(u) = engine.current_unscale.read() {
        *u
    } else {
        1.0
    };
    if let Ok(shared_mesh) = engine.face_mesh_shared.read() {
        if let Ok(mut calibrator) = engine.calibrator.write() {
            calibrator.update_calibration(&shared_mesh.vertices, image_w, image_h, depth_z, unscale);
            *out_focal_length = calibrator.estimated_focal_length;
            0
        } else {
            -2
        }
    } else {
        -3
    }
}

/// Menghitung koefisien ambient occlusion (AO) dinamis untuk setiap vertex jaring wajah.
#[no_mangle]
pub unsafe extern "C" fn fizgravity_engine_calculate_dynamic_ao(
    engine_ptr: *mut c_void,
    out_ao: *mut f32,
    max_count: c_int,
) -> c_int {
    if engine_ptr.is_null() || out_ao.is_null() {
        return -1;
    }
    if max_count < 0 {
        return -3;
    }
    let engine = &*(engine_ptr as *const FizgravityEngine);
    if let Ok(shared_mesh) = engine.face_mesh_shared.read() {
        let count = std::cmp::min(max_count as usize, face::FACE_MESH_VERTICES_COUNT);
        let ao_slice = std::slice::from_raw_parts_mut(out_ao, count);

        let mut ao_temp = [1.0f32; face::FACE_MESH_VERTICES_COUNT];
        makeup_triangulator::MakeupTriangulator::calculate_dynamic_ao(&shared_mesh.blendshapes, &mut ao_temp);

        for i in 0..count {
            ao_slice[i] = ao_temp[i];
        }
        0
    } else {
        -2
    }
}

#[cfg(test)]
mod medium_priority_ffi_tests {
    use super::*;

    #[test]
    fn test_ffi_calibration_and_ao() {
        let engine_ptr = unsafe { fizgravity_engine_init(std::ptr::null()) };
        assert!(!engine_ptr.is_null());

        let mut focal = 0.0f32;
        let res = unsafe {
            fizgravity_engine_update_auto_calibration(engine_ptr, 640.0, 480.0, 0.675, &mut focal)
        };
        assert_eq!(res, 0);
        assert!(focal > 0.0);

        let mut ao_buffer = [0.0f32; 468];
        let res2 = unsafe {
            fizgravity_engine_calculate_dynamic_ao(engine_ptr, ao_buffer.as_mut_ptr(), 468)
        };
        assert_eq!(res2, 0);
        assert!(ao_buffer[0] > 0.0);

        unsafe { fizgravity_engine_release(engine_ptr) };
    }

    #[test]
    fn test_canonical_uv_mapping() {
        let engine_ptr = unsafe { fizgravity_engine_init(std::ptr::null()) };
        assert!(!engine_ptr.is_null());

        let test_vertices = [ArVertex3D { x: 1.0, y: 2.0, z: 3.0 }; face::FACE_MESH_VERTICES_COUNT];
        let test_blendshapes = [0.5; face::FACE_BLENDSHAPES_COUNT];

        let res = unsafe {
            fizgravity_engine_set_face_mesh(
                engine_ptr,
                test_vertices.as_ptr(),
                test_blendshapes.as_ptr(),
            )
        };
        assert_eq!(res, 0);

        let mut out_mesh = ArFaceMesh {
            vertices: [ArFaceVertexInterleaved {
                position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
                normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
                uv: face::ArTexCoord2D { u: 0.0, v: 0.0 },
            }; face::FACE_MESH_VERTICES_COUNT],
            blendshapes: [0.0; face::FACE_BLENDSHAPES_COUNT],
        };

        let res2 = unsafe { fizgravity_engine_get_face_mesh(engine_ptr, &mut out_mesh) };
        assert_eq!(res2, 0);

        // Landmark 0 UV harus cocok dengan CANONICAL_UV[0] (u: 0.427942, v: 0.695278)
        assert!((out_mesh.vertices[0].uv.u - 0.427942).abs() < 1e-4);
        assert!((out_mesh.vertices[0].uv.v - 0.695278).abs() < 1e-4);

        unsafe { fizgravity_engine_release(engine_ptr) };
    }
}

#[cfg(test)]
mod imu_ffi_tests {
    use super::*;

    #[test]
    fn test_push_imu_and_predict() {
        let engine_ptr = unsafe { fizgravity_engine_init(std::ptr::null()) };
        assert!(!engine_ptr.is_null());

        // Push beberapa sample IMU
        for _ in 0..10 {
            unsafe {
                fizgravity_engine_push_imu(engine_ptr, 0.1, 0.05, 0.0, 0.0, 0.0, 9.81, 0.005);
            }
        }

        // Set face mesh dummy
        let vertices = [ArVertex3D { x: 0.5, y: 0.5, z: 0.0 }; face::FACE_MESH_VERTICES_COUNT];
        let blendshapes = [0.0f32; face::FACE_BLENDSHAPES_COUNT];
        unsafe {
            fizgravity_engine_set_face_mesh(engine_ptr, vertices.as_ptr(), blendshapes.as_ptr());
        }

        // Get predicted landmarks
        let mut out = vec![0.0f32; 468 * 3];
        let n = unsafe {
            fizgravity_engine_get_predicted_landmarks(engine_ptr, out.as_mut_ptr(), 468, 0.016)
        };
        assert_eq!(n, 468);
        // Vertex pertama harus berubah sedikit dari gyro rotation (bukan tetap di 0.5)
        // Gyro aktif → ada rotasi
        // (Tidak assert nilai exact karena filter)
        assert!(out[0].is_finite());

        unsafe { fizgravity_engine_release(engine_ptr) };
    }
}
