//! Modul Pra-integrasi IMU (IMU Pre-Integration Module) pada manifold SO(3).
//! Mengakumulasikan data akselerasi linier dan kecepatan sudut antara dua bingkai kamera (keyframe)
//! serta memelihara Jacobians terhadap bias giroskop dan akselerometer.

use nalgebra::{Matrix3, Rotation3, Vector3};
use crate::math::{exp_map, skew_symmetric};

/// Menyimpan data akumulasi pra-integrasi IMU dan matriks Jacobians bias.
pub struct ImuPreintegrator {
    /// Perubahan orientasi terintegrasi (\Delta R_ij)
    pub delta_r: Rotation3<f32>,
    /// Perubahan kecepatan terintegrasi (\Delta v_ij)
    pub delta_v: Vector3<f32>,
    /// Perubahan posisi terintegrasi (\Delta p_ij)
    pub delta_p: Vector3<f32>,

    // Matriks Jacobians terhadap bias sensor (digunakan untuk koreksi orde pertama EKF)
    pub jacobian_r_bg: Matrix3<f32>,
    pub jacobian_v_bg: Matrix3<f32>,
    pub jacobian_v_ba: Matrix3<f32>,
    pub jacobian_p_bg: Matrix3<f32>,
    pub jacobian_p_ba: Matrix3<f32>,
    
    // Bias sensor saat pra-integrasi dimulai
    pub bg_nominal: Vector3<f32>,
    pub ba_nominal: Vector3<f32>,
}

impl ImuPreintegrator {
    /// Membuat instansi pra-integrator baru dengan bias nominal tertentu.
    pub fn new(bg: Vector3<f32>, ba: Vector3<f32>) -> Self {
        Self {
            delta_r: Rotation3::identity(),
            delta_v: Vector3::zeros(),
            delta_p: Vector3::zeros(),
            
            // Inisialisasi matriks Jacobians dengan matriks nol (zeros)
            jacobian_r_bg: Matrix3::zeros(),
            jacobian_v_bg: Matrix3::zeros(),
            jacobian_v_ba: Matrix3::zeros(),
            jacobian_p_bg: Matrix3::zeros(),
            jacobian_p_ba: Matrix3::zeros(),
            
            bg_nominal: bg,
            ba_nominal: ba,
        }
    }

    /// Reset seluruh variabel integrasi untuk memulai interval keyframe baru.
    pub fn reset(&mut self) {
        self.delta_r = Rotation3::identity();
        self.delta_v = Vector3::zeros();
        self.delta_p = Vector3::zeros();
        
        self.jacobian_r_bg = Matrix3::zeros();
        self.jacobian_v_bg = Matrix3::zeros();
        self.jacobian_v_ba = Matrix3::zeros();
        self.jacobian_p_bg = Matrix3::zeros();
        self.jacobian_p_ba = Matrix3::zeros();
    }

    /// Mengintegrasikan satu poin pengukuran inersia baru (akselerometer & giroskop) ke dalam interval.
    ///
    /// * `measured_w`: Pengukuran giroskop mentah (kecepatan sudut) dalam rad/s.
    /// * `measured_a`: Pengukuran akselerometer mentah (percepatan linier) dalam m/s^2.
    /// * `dt`: Delta waktu (dalam detik) sejak pengukuran IMU terakhir.
    pub fn integrate(&mut self, measured_w: &Vector3<f32>, measured_a: &Vector3<f32>, dt: f32) {
        if dt <= 0.0 {
            return;
        }

        // 1. Reduksi bias sensor nominal untuk mendapatkan estimasi bersih
        let w_corrected = measured_w - self.bg_nominal;
        let a_corrected = measured_a - self.ba_nominal;

        // Simpan referensi delta saat ini untuk perhitungan step
        let r_current = self.delta_r;
        let v_current = self.delta_v;

        // 2. Hitung delta rotasi langkah ini (dR) dan rotasi titik tengah (r_mid)
        let delta_theta = w_corrected * dt;
        let d_r = exp_map(&delta_theta);
        
        // Midpoint rotation: R_mid = R_current * Exp(0.5 * w_corrected * dt)
        let r_mid = r_current * exp_map(&(w_corrected * (0.5 * dt)));

        // 3. Update Variabel Pra-integrasi Utama (Midpoint Integration)
        let acc_term = r_mid * a_corrected;
        self.delta_p += v_current * dt + acc_term * (0.5 * dt * dt);
        self.delta_v += acc_term * dt;
        self.delta_r = r_current * d_r;

        // 4. Propagasi Matriks Jacobians (Midpoint Integration - Taylor Expansion Orde 1)
        // Kita hitung matriks miring-simetris akselerasi untuk Jacobian silang menggunakan rotasi titik tengah
        let acc_skew = skew_symmetric(&a_corrected);
        let r_matrix = r_mid.matrix();

        // Update Jacobians posisi
        self.jacobian_p_bg += self.jacobian_v_bg * dt - r_matrix * acc_skew * self.jacobian_r_bg * (0.5 * dt * dt);
        self.jacobian_p_ba += self.jacobian_v_ba * dt - r_matrix * (0.5 * dt * dt);

        // Update Jacobians kecepatan
        self.jacobian_v_bg -= r_matrix * acc_skew * self.jacobian_r_bg * dt;
        self.jacobian_v_ba -= r_matrix * dt;

        // Update Jacobians rotasi
        // J_R_new = dR^T * J_R_old - I * dt
        self.jacobian_r_bg = d_r.transpose() * self.jacobian_r_bg - Matrix3::identity() * dt;
    }

    /// Mengoreksi perubahan orientasi (delta_r) sehubungan dengan pergeseran bias giroskop baru
    /// menggunakan pendekatan koreksi linear orde pertama Jacobian.
    pub fn get_corrected_rotation(&self, bg_current: &Vector3<f32>) -> Rotation3<f32> {
        let delta_bg = bg_current - self.bg_nominal;
        let correction = exp_map(&(self.jacobian_r_bg * delta_bg));
        self.delta_r * correction
    }

    /// Mengoreksi perubahan kecepatan (delta_v) berdasarkan pergeseran bias baru.
    pub fn get_corrected_velocity(&self, bg_current: &Vector3<f32>, ba_current: &Vector3<f32>) -> Vector3<f32> {
        let delta_bg = bg_current - self.bg_nominal;
        let delta_ba = ba_current - self.ba_nominal;
        self.delta_v + self.jacobian_v_bg * delta_bg + self.jacobian_v_ba * delta_ba
    }

    /// Mengoreksi perubahan posisi (delta_p) berdasarkan pergeseran bias baru.
    pub fn get_corrected_position(&self, bg_current: &Vector3<f32>, ba_current: &Vector3<f32>) -> Vector3<f32> {
        let delta_bg = bg_current - self.bg_nominal;
        let delta_ba = ba_current - self.ba_nominal;
        self.delta_p + self.jacobian_p_bg * delta_bg + self.jacobian_p_ba * delta_ba
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imu_integration_static() {
        // Uji kondisi statis (perangkat diam sempurna)
        // Akselerometer membaca gravitasi normal (anggap 9.81 m/s^2 pada sumbu Z)
        let bg = Vector3::zeros();
        let ba = Vector3::zeros();
        let mut preintegrator = ImuPreintegrator::new(bg, ba);

        let w = Vector3::zeros();
        let a = Vector3::new(0.0, 0.0, 9.81);
        let dt = 0.01; // 10ms (100Hz)

        // Integrasikan selama 1 detik (100 steps)
        for _ in 0..100 {
            preintegrator.integrate(&w, &a, dt);
        }

        // Posisi teoritis diam dipercepat: s = 0.5 * a * t^2
        // s = 0.5 * 9.81 * 1^2 = 4.905 meter
        let pos = preintegrator.delta_p;
        assert!((pos.z - 4.905).abs() < 1e-2);
        
        // Kecepatan teoritis: v = a * t = 9.81 * 1 = 9.81 m/s
        let vel = preintegrator.delta_v;
        assert!((vel.z - 9.81).abs() < 1e-2);

        // Rotasi harus tetap identitas
        let rot = preintegrator.delta_r;
        assert!((rot.angle() - 0.0).abs() < 1e-5);
    }
}
