//! Modul Pemulus Adaptif One-Euro Filter untuk Stabilisasi Koordinat AR tanpa Jitter dan Lag.

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
}

pub struct ArFaceMeshStabilizer {
    filters_x: Vec<OneEuroFilter>,
    filters_y: Vec<OneEuroFilter>,
    filters_z: Vec<OneEuroFilter>,
}

impl ArFaceMeshStabilizer {
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
        }
    }

    /// Menstabilkan seluruh 468 titik spasial wajah koordinat 3D.
    pub fn stabilize_face_mesh(&mut self, vertices: &mut [crate::face::ArFaceVertexInterleaved; 468], dt: f32) {
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
}
