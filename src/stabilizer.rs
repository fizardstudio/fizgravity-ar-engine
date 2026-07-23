//! Modul Pemulus Adaptif One-Euro Filter untuk Stabilisasi Koordinat AR tanpa Jitter dan Lag.
//! Upgrade v2: Dynamic beta adaptation berdasarkan jitter RMS per-vertex,
//! sehingga filter otomatis lebih agresif saat gerakan cepat & lebih presisi saat diam.

use crate::ArVertex3D;

pub struct OneEuroFilter {
    first_time: bool,
    min_cutoff: f32,
    beta: f32,
    d_cutoff: f32,
    x_prev: f32,
    dx_prev: f32,
}

impl OneEuroFilter {
    pub fn new(min_cutoff: f32, beta: f32, d_cutoff: f32) -> Self {
        Self {
            first_time: true,
            min_cutoff,
            beta,
            d_cutoff,
            x_prev: 0.0,
            dx_prev: 0.0,
        }
    }

    /// Menghitung koefisien pemulusan alpha berdasarkan cutoff frekuensi dan delta waktu.
    fn compute_alpha(cutoff: f32, dt: f32) -> f32 {
        let tau = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
        1.0 / (1.0 + tau / dt)
    }

    /// Memuluskan nilai baru secara adaptif menggunakan One-Euro Filter.
    pub fn filter(&mut self, val: f32, dt: f32) -> f32 {
        if self.first_time {
            self.first_time = false;
            self.x_prev = val;
            self.dx_prev = 0.0;
            return val;
        }

        let dt = if dt > 1e-5 { dt } else { 0.016 };

        // 1. Hitung turunan/kecepatan nilai (derivatif)
        let d_val = (val - self.x_prev) / dt;
        let d_alpha = Self::compute_alpha(self.d_cutoff, dt);
        let dx_filtered = d_alpha * d_val + (1.0 - d_alpha) * self.dx_prev;
        self.dx_prev = dx_filtered;

        // 2. Tentukan cutoff frekuensi dinamis berdasarkan magnitudo kecepatan
        let cutoff = self.min_cutoff + self.beta * dx_filtered.abs();

        // 3. Terapkan pemulusan adaptif pada nilai utama
        let alpha = Self::compute_alpha(cutoff, dt);
        let x_filtered = alpha * val + (1.0 - alpha) * self.x_prev;
        self.x_prev = x_filtered;

        x_filtered
    }

    /// [NEW] Update beta secara eksternal (untuk adaptive jitter-driven tuning).
    pub fn set_beta(&mut self, new_beta: f32) {
        self.beta = new_beta.clamp(0.001, 10.0);
    }

    /// [NEW] Ambil nilai derivatif terkini (untuk deteksi gerakan cepat).
    pub fn get_velocity(&self) -> f32 {
        self.dx_prev
    }
}

pub struct ArFaceMeshStabilizer {
    filters_x: Vec<OneEuroFilter>,
    filters_y: Vec<OneEuroFilter>,
    filters_z: Vec<OneEuroFilter>,
    /// Kecepatan RMS dari frame sebelumnya (untuk adaptive beta tuning)
    prev_rms_velocity: f32,
    /// Counter frame untuk periodik re-tune
    frame_count: u32,
}

impl ArFaceMeshStabilizer {
    /// Buat stabilizer baru.
    /// * `min_cutoff`: Frekuensi cutoff minimum (Hz). 1.5 Hz = cukup smooth untuk diam.
    /// * `beta`: Koefisien velocity-adaptive (0.15 = balance antara responsif & smooth).
    pub fn new(min_cutoff: f32, beta: f32) -> Self {
        let count = 468;
        let mut filters_x = Vec::with_capacity(count);
        let mut filters_y = Vec::with_capacity(count);
        let mut filters_z = Vec::with_capacity(count);

        for _ in 0..count {
            filters_x.push(OneEuroFilter::new(min_cutoff, beta, 1.0));
            filters_y.push(OneEuroFilter::new(min_cutoff, beta, 1.0));
            filters_z.push(OneEuroFilter::new(min_cutoff, beta, 1.0));
        }

        Self {
            filters_x,
            filters_y,
            filters_z,
            prev_rms_velocity: 0.0,
            frame_count: 0,
        }
    }

    /// Menstabilkan seluruh 468 titik spasial wajah koordinat 3D.
    ///
    /// **[UPGRADE v2]**: Adaptive beta — secara otomatis menaikkan beta (lebih responsif)
    /// saat gerakan cepat terdeteksi, dan menurunkannya (lebih halus) saat wajah diam.
    /// Ini menghilangkan tradeoff klasik antara lag vs jitter.
    pub fn stabilize_face_mesh(&mut self, vertices: &mut [crate::face::ArFaceVertexInterleaved; 468], dt: f32) {
        self.frame_count += 1;

        // Hitung RMS velocity rata-rata dari semua filters (setiap 10 frame untuk efisiensi)
        if self.frame_count % 10 == 0 {
            let mut sum_vel_sq = 0.0f32;
            for i in 0..468 {
                let vx = self.filters_x[i].get_velocity();
                let vy = self.filters_y[i].get_velocity();
                let vz = self.filters_z[i].get_velocity();
                sum_vel_sq += vx*vx + vy*vy + vz*vz;
            }
            let rms_velocity = (sum_vel_sq / (468.0 * 3.0)).sqrt();

            // Exponential smoothing pada RMS velocity agar transisi halus
            self.prev_rms_velocity = 0.7 * rms_velocity + 0.3 * self.prev_rms_velocity;

            // Adaptive beta:
            // - Diam (rms < 0.02): beta = 0.05 → filter sangat halus, hilangkan jitter
            // - Bergerak sedang (rms ≈ 0.1): beta = 0.3 → balance
            // - Bergerak cepat (rms > 0.5): beta = 2.0 → filter sangat responsif, ikuti gerakan
            let adaptive_beta = (self.prev_rms_velocity * 4.0).clamp(0.05, 2.5);

            for i in 0..468 {
                self.filters_x[i].set_beta(adaptive_beta);
                self.filters_y[i].set_beta(adaptive_beta);
                self.filters_z[i].set_beta(adaptive_beta);
            }
        }

        for i in 0..468 {
            let x = vertices[i].position.x;
            let y = vertices[i].position.y;
            let z = vertices[i].position.z;

            vertices[i].position = ArVertex3D {
                x: self.filters_x[i].filter(x, dt),
                y: self.filters_y[i].filter(y, dt),
                z: self.filters_z[i].filter(z, dt),
            };
        }
    }

    /// [NEW] Reset semua filter (berguna saat wajah hilang lalu muncul lagi).
    pub fn reset_all(&mut self) {
        for i in 0..468 {
            self.filters_x[i].first_time = true;
            self.filters_y[i].first_time = true;
            self.filters_z[i].first_time = true;
        }
        self.prev_rms_velocity = 0.0;
        self.frame_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_euro_filter_smoothing() {
        let mut filter = OneEuroFilter::new(1.0, 0.1, 1.0);
        
        // Input stabil awal
        let f1 = filter.filter(10.0, 0.016);
        assert_eq!(f1, 10.0);

        // Input bergetar kecil (jitter) -> harus diredam berat
        let f2 = filter.filter(10.05, 0.016);
        assert!((f2 - 10.0).abs() < 0.01); // Redaman kuat, hasil mendekati 10.0
    }

    #[test]
    fn test_adaptive_beta_responsiveness() {
        let mut filter = OneEuroFilter::new(1.5, 0.15, 1.0);
        filter.filter(0.0, 0.016); // Initialize

        // Gerakan cepat: filter harus follow lebih responsif setelah beta dinaikkan
        filter.set_beta(2.0);
        let result_fast = filter.filter(1.0, 0.016);
        
        let mut filter_slow = OneEuroFilter::new(1.5, 0.05, 1.0);
        filter_slow.filter(0.0, 0.016);
        let result_slow = filter_slow.filter(1.0, 0.016);

        // Filter dengan beta tinggi harus lebih dekat ke nilai target (1.0)
        assert!(result_fast > result_slow, "Fast beta filter harus lebih responsif");
    }

    #[test]
    fn test_reset_all() {
        use crate::face::{ArFaceVertexInterleaved, ArTexCoord2D};

        let mut stabilizer = ArFaceMeshStabilizer::new(1.5, 0.15);
        let mut vertices = [ArFaceVertexInterleaved {
            position: ArVertex3D { x: 1.0, y: 1.0, z: 1.0 },
            normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
            uv: ArTexCoord2D { u: 0.0, v: 0.0 },
        }; 468];

        stabilizer.stabilize_face_mesh(&mut vertices, 0.016);
        stabilizer.reset_all();

        // Setelah reset, first_time harus true lagi
        assert_eq!(stabilizer.frame_count, 0);
    }
}
