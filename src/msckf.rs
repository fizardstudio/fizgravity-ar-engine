//! Modul Utama Filter Navigasi MSCKF (Multi-State Constraint Kalman Filter).
//! Mengelola vektor keadaan (state vector), matriks kovariansi (covariance P),
//! augmentasi status kamera, dan pembaruan pengukuran berbasis Proyeksi Ruang Nol.

use nalgebra::{Matrix3, OMatrix, OVector, Rotation3, Vector3, Dynamic};
use crate::math::{exp_map, skew_symmetric};
use crate::imu::ImuPreintegrator;

/// Keadaan inersia kinematik saat ini (IMU State).
#[derive(Debug, Clone)]
pub struct ImuState {
    /// Orientasi tubuh IMU terhadap ruang referensi global (rotation SO(3))
    pub r_gi: Rotation3<f32>,
    /// Posisi IMU dalam meter (3D)
    pub p_g: Vector3<f32>,
    /// Kecepatan linier IMU dalam m/s (3D)
    pub v_g: Vector3<f32>,
    /// Bias giroskop (3D)
    pub bg: Vector3<f32>,
    /// Bias akselerometer (3D)
    pub ba: Vector3<f32>,
}

/// Representasi klon pose kamera masa lalu dalam sliding window.
#[derive(Debug, Copy, Clone)]
pub struct CameraState {
    /// ID temporal frame
    pub frame_id: u32,
    /// Orientasi kamera terhadap ruang referensi global
    pub r_gc: Rotation3<f32>,
    /// Posisi kamera dalam koordinat global
    pub p_c: Vector3<f32>,
}

/// Penyimpan seluruh variabel keadaan filter MSCKF.
pub struct MsckfState {
    /// Status IMU aktif saat ini
    pub imu_state: ImuState,
    /// Antrean sliding window pose kamera historis (maksimum N)
    pub camera_states: Vec<CameraState>,
    /// Matriks kovariansi kesalahan EKF (dimensi: 15 + 6 * N)
    /// 15 status IMU awal + 6 status per klon kamera (3 rotasi, 3 posisi)
    pub covariance: OMatrix<f32, Dynamic, Dynamic>,
}

impl MsckfState {
    /// Membuat keadaan inersia awal dengan matriks kovariansi identitas skala kecil.
    pub fn new(window_size: usize) -> Self {
        let imu = ImuState {
            r_gi: Rotation3::identity(),
            p_g: Vector3::zeros(),
            v_g: Vector3::zeros(),
            bg: Vector3::zeros(),
            ba: Vector3::zeros(),
        };

        // Vektor kesalahan IMU adalah 15 dimensi: delta_theta (3), delta_p (3), delta_v (3), delta_bg (3), delta_ba (3)
        let dim = 15;
        let cov = OMatrix::<f32, Dynamic, Dynamic>::identity_generic(
            Dynamic::new(dim),
            Dynamic::new(dim),
        ) * 1e-3;

        Self {
            imu_state: imu,
            camera_states: Vec::with_capacity(window_size),
            covariance: cov,
        }
    }
}

/// Struktur solver filter Kalman MSCKF.
pub struct MsckfFilter {
    pub state: MsckfState,
    pub max_window_size: usize,
    /// Matriks rotasi ekstrinsik kamera ke IMU (R_c_i)
    pub r_c_i: Rotation3<f32>,
    /// Translasi ekstrinsik kamera ke IMU (p_c_i)
    pub p_c_i: Vector3<f32>,
}

impl MsckfFilter {
    pub fn new(max_window: usize) -> Self {
        Self {
            state: MsckfState::new(max_window),
            max_window_size: max_window,
            r_c_i: Rotation3::identity(), // Kalibrasi default
            p_c_i: Vector3::zeros(),
        }
    }

    /// Fase Penyebaran (Propagation Step):
    /// Memperbarui keadaan nominal IMU dan merambatkan kovarians menggunakan data pra-integrasi IMU.
    pub fn propagate(&mut self, preintegrator: &ImuPreintegrator, dt: f32) {
        let imu = &mut self.state.imu_state;

        // 1. Perbarui keadaan nominal menggunakan data delta pra-integrasi terkalibrasi bias
        let dp = preintegrator.get_corrected_position(&imu.bg, &imu.ba);
        let dv = preintegrator.get_corrected_velocity(&imu.bg, &imu.ba);
        let dr = preintegrator.get_corrected_rotation(&imu.bg);

        // Hukum gravitasi bumi (arah Z negatif)
        let gravity = Vector3::new(0.0, 0.0, -9.81);

        // Integrasi kinematik global:
        // p_g = p_g + v_g * dt + 0.5 * g * dt^2 + R_gi * delta_p
        let r_matrix = imu.r_gi.matrix();
        imu.p_g += imu.v_g * dt + gravity * (0.5 * dt * dt) + r_matrix * dp;
        
        // v_g = v_g + g * dt + R_gi * delta_v
        imu.v_g += gravity * dt + r_matrix * dv;
        
        // R_gi = R_gi * delta_r
        imu.r_gi = imu.r_gi * dr;

        // 2. Propagasi matriks kovariansi kesalahan EKF P = Phi * P * Phi^T + Q
        // Sesuai MSCKF, kita hanya memperbarui blok inersia 15x15 dan blok korelasi silang kamera.
        let dim = self.state.covariance.nrows();
        
        // Buat matriks transisi keadaan Phi (15x15 untuk bagian IMU)
        let _phi_imu = Matrix3::<f32>::identity(); // Secara teoretis Phi adalah matriks jacobian 15x15 penuh
        // Di sini kita formulasikan block transisi Jacobian rotasi-bias sederhana:
        // Phi_theta_bg = -R_gi * dt
        let _phi_theta_bg = -imu.r_gi.matrix() * dt;

        // Update blok kovariansi IMU internal (P_ii = Phi_i * P_ii * Phi_i^T + Q_i)
        // Untuk penyederhanaan implementasi, kita perbarui kovariansi dengan noise konstan Q:
        let q_imu = OMatrix::<f32, Dynamic, Dynamic>::identity_generic(
            Dynamic::new(15),
            Dynamic::new(15),
        ) * 1e-4;

        for i in 0..15 {
            for j in 0..15 {
                self.state.covariance[(i, j)] += q_imu[(i, j)];
            }
        }
    }

    /// Fase Augmentasi Keadaan (State Augmentation):
    /// Menangkap klon pose kamera baru saat frame video diperoleh dan memasukkannya ke state vector.
    pub fn augment_state(&mut self, frame_id: u32) {
        let imu = &self.state.imu_state;
        
        // 1. Hitung pose kamera dari ekstrinsik kamera-IMU
        // R_gc = R_gi * R_ci^T
        let r_gc = imu.r_gi * self.r_c_i.transpose();
        // p_c = p_g + R_gi * p_ci
        let p_c = imu.p_g + imu.r_gi.matrix() * self.p_c_i;

        let new_camera = CameraState {
            frame_id,
            r_gc,
            p_c,
        };

        // 2. Tambahkan klon kamera ke antrean sliding window
        self.state.camera_states.push(new_camera);

        // 3. Perluas dimensi matriks kovariansi P (tambahkan 6 baris/kolom baru)
        let old_dim = self.state.covariance.nrows();
        let new_dim = old_dim + 6;
        
        let mut new_cov = OMatrix::<f32, Dynamic, Dynamic>::zeros_generic(
            Dynamic::new(new_dim),
            Dynamic::new(new_dim),
        );
        
        // Salin kovariansi lama
        new_cov.slice_mut((0, 0), (old_dim, old_dim)).copy_from(&self.state.covariance);

        // Hitung baris/kolom korelasi silang (cross-covariance) Jacobian kamera terhadap bagian IMU:
        // J_c = [ J_r, J_p ] (matriks 6x15)
        let mut j_c = OMatrix::<f32, Dynamic, Dynamic>::zeros_generic(Dynamic::new(6), Dynamic::new(15));
        j_c.slice_mut((0, 0), (3, 3)).copy_from(&Matrix3::identity()); // Turunan rotasi
        j_c.slice_mut((3, 3), (3, 3)).copy_from(&Matrix3::identity()); // Turunan posisi

        // P_ic_new = P_ii * J_c^T
        let p_ii = self.state.covariance.slice((0, 0), (15, 15));
        let p_ic = p_ii * j_c.clone().transpose();

        // Salin korelasi baru ke matriks kovariansi yang diperluas
        new_cov.slice_mut((old_dim, 0), (6, 15)).copy_from(&p_ic.transpose());
        new_cov.slice_mut((0, old_dim), (15, 6)).copy_from(&p_ic);
        
        // P_cc = J_c * P_ii * J_c^T + R_camera_noise -> P_cc = J_c * P_ic
        let p_cc = &j_c * &p_ic;
        new_cov.slice_mut((old_dim, old_dim), (6, 6)).copy_from(&p_cc);

        self.state.covariance = new_cov;
    }

    /// Langkah Pembaruan (Measurement Update) dengan Proyeksi Ruang Nol (Null-Space):
    /// Dipanggil ketika pelacakan satu set titik fitur telah selesai.
    pub fn update_features(&mut self, residuals: &[f32], jacobians_x: &[f32], jacobians_f: &[f32], num_observations: usize) {
        if num_observations == 0 || residuals.is_empty() {
            return;
        }

        // 1. Rekonstruksi matriks residual Y (dimensi: M x 1)
        let y = OVector::<f32, Dynamic>::from_column_slice(residuals);
        
        // Matriks Jacobian terhadap state (H_x, dimensi: M x (15 + 6N))
        let state_dim = self.state.covariance.nrows();
        let h_x = OMatrix::<f32, Dynamic, Dynamic>::from_row_slice_generic(
            Dynamic::new(num_observations),
            Dynamic::new(state_dim),
            jacobians_x,
        );

        // Matriks Jacobian terhadap posisi fitur 3D (H_f, dimensi: M x 3)
        let h_f = OMatrix::<f32, Dynamic, Dynamic>::from_row_slice_generic(
            Dynamic::new(num_observations),
            Dynamic::new(3),
            jacobians_f,
        );

        // 2. Proyeksi Ruang Nol (Null-Space Projection)
        // Kita lakukan dekomposisi QR pada H_f untuk memisah ruang ortogonal: H_f = [Q1 Q2] * [R; 0]
        let qr = h_f.qr();
        let q = qr.q();
        
        // Matriks V adalah kolom-kolom Q2 (ortogonal terhadap H_f, dimensi: M x (M - 3))
        // V^T * H_f = 0
        let m = num_observations;
        if m <= 3 {
            return; // Tidak cukup observasi untuk proyeksi ruang nol
        }
        
        let q_t = q.transpose();
        let v_t = q_t.slice((3, 0), (m - 3, m));

        // 3. Proyeksikan Residual dan Jacobian
        // r_o = V^T * y
        let r_o = &v_t * y;
        // H_o = V^T * H_x
        let h_o = &v_t * h_x;

        // 4. Kalman Gain Update
        // S = H_o * P * H_o^T + R_noise
        let p = &self.state.covariance;
        let r_noise = OMatrix::<f32, Dynamic, Dynamic>::identity_generic(Dynamic::new(m - 3), Dynamic::new(m - 3)) * 1e-4;
        let s = &h_o * p * h_o.transpose() + r_noise;

        // K = P * H_o^T * S^-1
        let s_inv = s.try_inverse().unwrap_or_else(|| OMatrix::<f32, Dynamic, Dynamic>::zeros_generic(Dynamic::new(m - 3), Dynamic::new(m - 3)));
        let k = p * h_o.transpose() * s_inv;

        // 5. Perbarui Vektor Keadaan (State Error Correction)
        let delta_x = &k * r_o;

        // Terapkan koreksi delta_x ke status IMU nominal
        let imu = &mut self.state.imu_state;
        let d_theta = Vector3::new(delta_x[0], delta_x[1], delta_x[2]);
        imu.r_gi = imu.r_gi * exp_map(&d_theta);
        imu.p_g += Vector3::new(delta_x[3], delta_x[4], delta_x[5]);
        imu.v_g += Vector3::new(delta_x[6], delta_x[7], delta_x[8]);
        imu.bg += Vector3::new(delta_x[9], delta_x[10], delta_x[11]);
        imu.ba += Vector3::new(delta_x[12], delta_x[13], delta_x[14]);

        // Perbarui Matriks Kovariansi P = (I - K * H_o) * P
        let identity = OMatrix::<f32, Dynamic, Dynamic>::identity_generic(Dynamic::new(state_dim), Dynamic::new(state_dim));
        self.state.covariance = (identity - &k * h_o) * p;
    }
}
