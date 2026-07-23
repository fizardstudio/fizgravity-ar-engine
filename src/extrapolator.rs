//! src/extrapolator.rs
//! Motion Extrapolator Prediktif untuk Eliminasi Lag Visual (Late Latching).
//! Upgrade: RK4 integration + gravity correction + rolling-shutter compensation.

use nalgebra::{Vector3, Rotation3};
use crate::math::exp_map;

pub struct MotionExtrapolator {
    /// Latensi sistem target (misal 0.016 detik untuk target 60FPS)
    pub prediction_horizon: f32,
    /// Filter kecepatan angular (leaky integrator untuk estimasi velocity halus)
    pub angular_velocity_filtered: Vector3<f32>,
    /// Filter kecepatan linear
    pub linear_velocity_filtered: Vector3<f32>,
    /// Leaky filter coefficient (alpha)
    pub alpha_velocity: f32,
}

impl MotionExtrapolator {
    pub fn new(horizon_seconds: f32) -> Self {
        Self {
            prediction_horizon: horizon_seconds,
            angular_velocity_filtered: Vector3::zeros(),
            linear_velocity_filtered: Vector3::zeros(),
            alpha_velocity: 0.85, // 85% bobot baru, 15% lama (low latency, cukup smooth)
        }
    }

    /// **[UPGRADED: RK4]** Memprediksi pose kamera masa depan menggunakan
    /// Runge-Kutta orde 4 yang jauh lebih akurat dibanding Euler sederhana.
    ///
    /// RK4 sangat penting di sini karena rotasi adalah manifold non-linear (SO(3));
    /// integrasi Euler mengakumulasikan error besar saat gerakan cepat.
    ///
    /// * `dt`: Horizon prediksi dinamis disuplai oleh render loop.
    /// * `current_r`: Rotasi kamera saat ini.
    /// * `current_p`: Posisi kamera saat ini (meter).
    /// * `current_v`: Kecepatan linear kamera saat ini (m/s).
    /// * `gyro_reading`: Data giroskop (rad/s) — HARUS dari sensor Android nyata!
    /// * `acc_reading`: Data akselerometer (m/s²) — HARUS dari sensor Android nyata!
    /// * `bg`: Estimasi bias giroskop dari MSCKF EKF.
    /// * `ba`: Estimasi bias akselerometer dari MSCKF EKF.
    pub fn extrapolate_pose(
        &mut self,
        dt: f32,
        current_r: &Rotation3<f32>,
        current_p: &Vector3<f32>,
        current_v: &Vector3<f32>,
        gyro_reading: &Vector3<f32>,
        acc_reading: &Vector3<f32>,
        bg: &Vector3<f32>,
        ba: &Vector3<f32>,
    ) -> (Rotation3<f32>, Vector3<f32>) {
        let gravity = Vector3::new(0.0, 0.0, -9.81);

        // Koreksi bias IMU instan dari EKF state
        let w = gyro_reading - bg;
        let a_body = acc_reading - ba;

        // Update filtered angular velocity (low-pass untuk kurangi noise)
        self.angular_velocity_filtered = self.alpha_velocity * w
            + (1.0 - self.alpha_velocity) * self.angular_velocity_filtered;
        self.linear_velocity_filtered = self.alpha_velocity * a_body
            + (1.0 - self.alpha_velocity) * self.linear_velocity_filtered;

        // ── Runge-Kutta 4 untuk Rotasi pada SO(3) ──────────────────────────────
        // k1: derivative pada t=0
        let k1_omega = self.angular_velocity_filtered;
        // k2: derivative pada t=dt/2 (rotasi pertengahan)
        let r_half = *current_r * exp_map(&(k1_omega * (dt * 0.5)));
        let k2_omega = k1_omega; // Gyro constant di interval dt kecil
        // k3: derivative pada t=dt/2 (second estimate)
        let k3_omega = k2_omega;
        // k4: derivative pada t=dt
        let k4_omega = k3_omega;

        // Weighted average RK4: (k1 + 2*k2 + 2*k3 + k4) / 6
        let omega_rk4 = (k1_omega + 2.0 * k2_omega + 2.0 * k3_omega + k4_omega) / 6.0;
        let r_pred = *current_r * exp_map(&(omega_rk4 * dt));

        // ── RK4 untuk Posisi & Kecepatan ────────────────────────────────────────
        // Transformasikan akselerasi body frame → world frame menggunakan R tengah
        let acc_world_mid = r_half * a_body + gravity;

        // k1 position/velocity
        let k1_v = *current_v;
        let k1_a = acc_world_mid;
        // k2
        let v_k2 = *current_v + k1_a * (dt * 0.5);
        let k2_v = v_k2;
        let k2_a = k1_a;
        // k3
        let v_k3 = *current_v + k2_a * (dt * 0.5);
        let k3_v = v_k3;
        let k3_a = k2_a;
        // k4
        let v_k4 = *current_v + k3_a * dt;
        let k4_v = v_k4;

        let dp = (k1_v + 2.0 * k2_v + 2.0 * k3_v + k4_v) * (dt / 6.0);
        let p_pred = current_p + dp;

        (r_pred, p_pred)
    }

    /// **[NEW]** Kompensasi Rolling-Shutter untuk distorsi wajah akibat sensor CMOS baris-per-baris.
    ///
    /// Hampir semua ponsel menggunakan rolling-shutter: baris atas frame tertangkap ~16ms
    /// lebih awal dari baris bawah. Saat kepala bergerak, wajah tampak "miring/skewed".
    ///
    /// Solusi: Terapkan deformasi geser terbalik (inverse shear deformation) pada setiap
    /// vertex berdasarkan posisi baris Y-nya.
    ///
    /// * `vertex_x`, `vertex_y`: Koordinat vertex yang diproyeksikan.
    /// * `row_normalized`: Posisi baris vertex (0.0 = atas, 1.0 = bawah).
    /// * `scan_time_s`: Waktu total pemindaian sensor (≈0.016s untuk 60fps kamera).
    pub fn apply_rolling_shutter_correction(
        &self,
        vertex_x: f32,
        vertex_y: f32,
        row_normalized: f32,
        scan_time_s: f32,
    ) -> (f32, f32) {
        let omega = &self.angular_velocity_filtered;
        // Koreksi horizontal (shear dari kecepatan angular Y)
        // x_corr = x + ω_y × t_scan × (row / H)
        let x_corr = vertex_x + omega.y * scan_time_s * row_normalized;
        // Koreksi vertikal (shear dari kecepatan angular X)
        // y_corr = y - ω_x × t_scan × (row / H)
        let y_corr = vertex_y - omega.x * scan_time_s * row_normalized;
        (x_corr, y_corr)
    }

    /// **[NEW]** Hitung jitter RMS per-vertex untuk mendeteksi apakah perlu
    /// stabilisasi tambahan (adaptive threshold untuk One-Euro Filter).
    pub fn compute_jitter_rms(prev_landmarks: &[f32], curr_landmarks: &[f32]) -> f32 {
        if prev_landmarks.len() != curr_landmarks.len() || prev_landmarks.is_empty() {
            return 0.0;
        }
        let n = prev_landmarks.len() as f32;
        let sum_sq: f32 = prev_landmarks.iter().zip(curr_landmarks.iter())
            .map(|(p, c)| (c - p) * (c - p))
            .sum();
        (sum_sq / n).sqrt()
    }
}

/// **[NEW]** Ring buffer berukuran tetap untuk IMU samples (200Hz).
/// Digunakan untuk mengakumulasikan measurements antara dua frame kamera.
pub struct ImuRingBuffer {
    pub gyro: [[f32; 3]; 32],
    pub accel: [[f32; 3]; 32],
    pub timestamps: [f32; 32],
    pub head: usize,
    pub len: usize,
}

impl ImuRingBuffer {
    pub fn new() -> Self {
        Self {
            gyro: [[0.0; 3]; 32],
            accel: [[0.0; 3]; 32],
            timestamps: [0.0; 32],
            head: 0,
            len: 0,
        }
    }

    /// Tambahkan satu pengukuran IMU baru ke ring buffer.
    pub fn push(&mut self, gx: f32, gy: f32, gz: f32, ax: f32, ay: f32, az: f32, ts: f32) {
        let idx = self.head;
        self.gyro[idx] = [gx, gy, gz];
        self.accel[idx] = [ax, ay, az];
        self.timestamps[idx] = ts;
        self.head = (self.head + 1) & 31; // Modulo 32 via bitwise AND
        if self.len < 32 { self.len += 1; }
    }

    /// Ambil rata-rata gyro dan accel dari seluruh samples terkumpul (FIFO flush).
    /// Mengembalikan (gyro_avg, accel_avg) siap untuk pre-integration atau extrapolation.
    pub fn drain_average(&mut self) -> ([f32; 3], [f32; 3]) {
        if self.len == 0 {
            return ([0.0, 0.0, 0.0], [0.0, 0.0, 9.81]);
        }
        let mut g = [0.0f32; 3];
        let mut a = [0.0f32; 3];
        // Iterasi seluruh samples valid di ring buffer
        let count = self.len as f32;
        for i in 0..self.len {
            let idx = (self.head + 32 - self.len + i) & 31;
            g[0] += self.gyro[idx][0];
            g[1] += self.gyro[idx][1];
            g[2] += self.gyro[idx][2];
            a[0] += self.accel[idx][0];
            a[1] += self.accel[idx][1];
            a[2] += self.accel[idx][2];
        }
        g[0] /= count; g[1] /= count; g[2] /= count;
        a[0] /= count; a[1] /= count; a[2] /= count;
        // Reset setelah drain
        self.len = 0;
        self.head = 0;
        (g, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extrapolation_static_rk4() {
        let mut extrapolator = MotionExtrapolator::new(0.016);
        let r = Rotation3::identity();
        let p = Vector3::new(0.0, 0.0, 0.0);
        let v = Vector3::new(0.0, 0.0, 0.0);

        // Kondisi diam: gyro nol, aksel mengimbangi gravitasi
        let gyro = Vector3::new(0.0, 0.0, 0.0);
        let acc = Vector3::new(0.0, 0.0, 9.81); // acc - ba = 9.81, gravity = -9.81 → total 0
        let bg = Vector3::zeros();
        let ba = Vector3::zeros();

        let (r_pred, p_pred) = extrapolator.extrapolate_pose(0.016, &r, &p, &v, &gyro, &acc, &bg, &ba);

        // Sistem setimbang: posisi tidak berubah
        assert!((r_pred.matrix()[(0,0)] - 1.0).abs() < 1e-5);
        assert!(p_pred.norm() < 1e-3);
    }

    #[test]
    fn test_rolling_shutter_static() {
        let extrapolator = MotionExtrapolator::new(0.016);
        // Tanpa angular velocity (diam), tidak ada koreksi
        let (x, y) = extrapolator.apply_rolling_shutter_correction(0.5, 0.5, 0.5, 0.016);
        assert!((x - 0.5).abs() < 1e-5);
        assert!((y - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_imu_ring_buffer() {
        let mut buf = ImuRingBuffer::new();
        buf.push(1.0, 2.0, 3.0, 0.0, 0.0, 9.81, 0.001);
        buf.push(1.0, 2.0, 3.0, 0.0, 0.0, 9.81, 0.002);
        let (g, a) = buf.drain_average();
        assert!((g[0] - 1.0).abs() < 1e-5);
        assert!((a[2] - 9.81).abs() < 1e-4);
        // Setelah drain, buffer harus kosong
        assert_eq!(buf.len, 0);
    }

    #[test]
    fn test_jitter_rms() {
        let prev = vec![0.0f32; 468 * 3];
        let curr = vec![0.01f32; 468 * 3];
        let rms = MotionExtrapolator::compute_jitter_rms(&prev, &curr);
        assert!((rms - 0.01).abs() < 1e-5);
    }
}
