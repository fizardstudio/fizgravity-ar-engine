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
        // Ekstrak angular velocity dan acceleration rata-rata dari integrasi untuk menyusun Phi
        let w_corrected = if dt > 1e-6 { crate::math::log_map(&dr) / dt } else { Vector3::zeros() };
        let a_corrected = if dt > 1e-6 { dv / dt } else { Vector3::zeros() };

        let mut phi = OMatrix::<f32, Dynamic, Dynamic>::identity_generic(Dynamic::new(15), Dynamic::new(15));
        let w_skew = skew_symmetric(&w_corrected);
        let a_skew = skew_symmetric(&a_corrected);
        let r_mat = imu.r_gi.matrix();

        phi.slice_mut((0, 0), (3, 3)).copy_from(&(Matrix3::identity() - w_skew * dt));
        phi.slice_mut((0, 9), (3, 3)).copy_from(&(-Matrix3::identity() * dt));
        phi.slice_mut((3, 6), (3, 3)).copy_from(&(Matrix3::identity() * dt));
        phi.slice_mut((6, 0), (3, 3)).copy_from(&(-r_mat * a_skew * dt));
        phi.slice_mut((6, 12), (3, 3)).copy_from(&(-r_mat * dt));

        let p_ii = self.state.covariance.slice((0, 0), (15, 15));
        let q_imu = OMatrix::<f32, Dynamic, Dynamic>::identity_generic(Dynamic::new(15), Dynamic::new(15)) * 1e-4;
        let p_ii_new = &phi * p_ii * phi.transpose() + q_imu;
        
        // Tulis kembali P_ii_new ke covariance (top-left)
        self.state.covariance.slice_mut((0, 0), (15, 15)).copy_from(&p_ii_new);

        // Propagasikan korelasi silang kamera (cross-covariance) P_ic = Phi * P_ic
        let num_cameras = self.state.camera_states.len();
        if num_cameras > 0 {
            let p_ic = self.state.covariance.slice((0, 15), (15, 6 * num_cameras));
            let p_ic_new = &phi * p_ic;
            
            // Salin P_ic_new ke covariance (top-right)
            self.state.covariance.slice_mut((0, 15), (15, 6 * num_cameras)).copy_from(&p_ic_new);
            // Salin P_ci_new ke covariance (bottom-left)
            self.state.covariance.slice_mut((15, 0), (6 * num_cameras, 15)).copy_from(&p_ic_new.transpose());
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
        j_c.slice_mut((0, 0), (3, 3)).copy_from(&Matrix3::identity()); // Turunan rotasi terhadap rotasi IMU
        j_c.slice_mut((3, 3), (3, 3)).copy_from(&Matrix3::identity()); // Turunan posisi terhadap posisi IMU
        
        // Turunan posisi kamera terhadap rotasi IMU: -R_gi * [p_c_i]_x (Cross-term ekstrinsik)
        let p_ci_skew = skew_symmetric(&self.p_c_i);
        j_c.slice_mut((3, 0), (3, 3)).copy_from(&(-imu.r_gi.matrix() * p_ci_skew));

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

        // Batasi ukuran sliding window dengan membuang klon kamera tertua (marginalization)
        if self.state.camera_states.len() > self.max_window_size {
            self.marginalize_oldest();
        }
    }

    /// Membuang klon kamera tertua (indeks 0) dari sliding window dan covariance matrix P
    /// untuk mencegah overhead komputasi akibat dimensi matriks yang membesar tanpa batas.
    pub fn marginalize_oldest(&mut self) {
        if self.state.camera_states.is_empty() {
            return;
        }
        self.state.camera_states.remove(0);

        let old_dim = self.state.covariance.nrows();
        let new_dim = old_dim - 6;

        let mut new_cov = OMatrix::<f32, Dynamic, Dynamic>::zeros_generic(
            Dynamic::new(new_dim),
            Dynamic::new(new_dim),
        );

        // Salin blok kovariansi IMU internal (15x15)
        new_cov.slice_mut((0, 0), (15, 15)).copy_from(&self.state.covariance.slice((0, 0), (15, 15)));

        // Salin blok klon kamera sisa (melewati indeks 15..21)
        if new_dim > 15 {
            // Salin kolom korelasi silang (top-right)
            new_cov.slice_mut((0, 15), (15, new_dim - 15))
                .copy_from(&self.state.covariance.slice((0, 21), (15, old_dim - 21)));

            // Salin baris korelasi silang (bottom-left)
            new_cov.slice_mut((15, 0), (new_dim - 15, 15))
                .copy_from(&self.state.covariance.slice((21, 0), (old_dim - 21, 15)));

            // Salin kovariansi kamera-kamera (bottom-right)
            new_cov.slice_mut((15, 15), (new_dim - 15, new_dim - 15))
                .copy_from(&self.state.covariance.slice((21, 21), (old_dim - 21, old_dim - 21)));
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msckf_augmentation_and_marginalization() {
        // Inisialisasi filter dengan max window size 3
        let mut filter = MsckfFilter::new(3);
        assert_eq!(filter.state.camera_states.len(), 0);
        assert_eq!(filter.state.covariance.nrows(), 15);

        // 1. Augmentasi frame 1
        filter.augment_state(1);
        assert_eq!(filter.state.camera_states.len(), 1);
        assert_eq!(filter.state.covariance.nrows(), 21);

        // 2. Augmentasi frame 2
        filter.augment_state(2);
        assert_eq!(filter.state.camera_states.len(), 2);
        assert_eq!(filter.state.covariance.nrows(), 27);

        // 3. Augmentasi frame 3
        filter.augment_state(3);
        assert_eq!(filter.state.camera_states.len(), 3);
        assert_eq!(filter.state.covariance.nrows(), 33);

        // 4. Augmentasi frame 4 (harus memicu marginalisasi klon tertua frame 1)
        filter.augment_state(4);
        assert_eq!(filter.state.camera_states.len(), 3);
        // Dimensi covariance harus kembali ke 33 (15 + 6 * 3) bukan 39
        assert_eq!(filter.state.covariance.nrows(), 33);
        
        // Kamera tertua harus bernilai frame_id = 2
        assert_eq!(filter.state.camera_states[0].frame_id, 2);
    }
}
