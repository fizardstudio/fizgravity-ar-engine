//! src/lighting.rs
//! Estimasi Pencahayaan Global (Ambient & Specular NSRF) Real-Time berbasis Spherical Harmonics.

use crate::ArSphericalHarmonics;
use std::os::raw::{c_float, c_void};

pub struct LightingEstimator {
    pub current_sh: ArSphericalHarmonics,
    pub nsrf_model_loaded: bool,
}

impl LightingEstimator {
    pub fn new() -> Self {
        Self {
            current_sh: ArSphericalHarmonics {
                coefficients_r: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                coefficients_g: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                coefficients_b: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
            nsrf_model_loaded: false,
        }
    }

    /// Memproyeksikan piksel kamera RGB yang didownscale ke dalam 9 koefisien SH Orde 2.
    /// Memetakan koordinat piksel (u, v) secara ortografis ke arah normal 3D pada hemisfer.
    pub fn estimate_ambient_sh(&mut self, camera_pixels: *const c_void, width: i32, height: i32) {
        if camera_pixels.is_null() || width <= 0 || height <= 0 {
            return;
        }

        let pixels = camera_pixels as *const u8;
        
        // Inisialisasi akumulator koefisien SH (9 floats untuk R, G, B)
        let mut accum_r = [0.0f32; 9];
        let mut accum_g = [0.0f32; 9];
        let mut accum_b = [0.0f32; 9];
        let mut weight_sum = 0.0f32;

        // Downsample grid sampling hemisferikal (misal 16x16 titik)
        // Parameter intrinsik kamera default (diestimasi secara empiris dari resolusi)
        let fx = (width as f32) * 0.8;
        let fy = (width as f32) * 0.8;
        let cx = (width as f32) * 0.5;
        let cy = (height as f32) * 0.5;

        let step_x = (width / 16).max(1);
        let step_y = (height / 16).max(1);

        for y_idx in (0..height).step_by(step_y as usize) {
            for x_idx in (0..width).step_by(step_x as usize) {
                // Rekonstruksi arah sinar datang berdasarkan model kamera perspektif
                let rx = (x_idx as f32 - cx) / fx;
                let ry = (y_idx as f32 - cy) / fy;
                let rz = 1.0f32;
                
                // Normalisasikan untuk mendapatkan unit arah normal datangnya cahaya (nx, ny, nz)
                let norm = (rx * rx + ry * ry + rz * rz).sqrt();
                let nx = rx / norm;
                let ny = ry / norm;
                let nz = rz / norm;

                // Ambil nilai piksel RGB (asumsi format input RGB 24-bit)
                let pixel_offset = ((y_idx * width + x_idx) * 3) as usize;
                let r_val = unsafe { *pixels.add(pixel_offset) } as f32 / 255.0;
                let g_val = unsafe { *pixels.add(pixel_offset + 1) } as f32 / 255.0;
                let b_val = unsafe { *pixels.add(pixel_offset + 2) } as f32 / 255.0;

                // 9 Basis Fungsi Harmonik Sferis Orde 2 (Spherical Harmonics Basis)
                let y00  = 0.282095;
                let y1_1 = 0.488603 * ny;
                let y10  = 0.488603 * nz;
                let y11  = 0.488603 * nx;
                let y2_2 = 1.092548 * nx * ny;
                let y2_1 = 1.092548 * ny * nz;
                let y20  = 0.315392 * (3.0 * nz * nz - 1.0);
                let y21  = 1.092548 * nx * nz;
                let y22  = 0.546274 * (nx * nx - ny * ny);

                let sh = [y00, y1_1, y10, y11, y2_2, y2_1, y20, y21, y22];

                // Akumulasikan proyeksi untuk masing-masing saluran warna
                for i in 0..9 {
                    accum_r[i] += r_val * sh[i];
                    accum_g[i] += g_val * sh[i];
                    accum_b[i] += b_val * sh[i];
                }
                weight_sum += 1.0;
            }
        }

        // Normalisasi dan perbarui koefisien SH saat ini
        if weight_sum > 0.0 {
            let norm_factor = 2.0 * std::f32::consts::PI / weight_sum;
            for i in 0..9 {
                self.current_sh.coefficients_r[i] = accum_r[i] * norm_factor;
                self.current_sh.coefficients_g[i] = accum_g[i] * norm_factor;
                self.current_sh.coefficients_b[i] = accum_b[i] * norm_factor;
            }
        }
    }

    /// Menghitung suhu warna sekitar (T_ambient dalam Kelvin) dan intensitasnya (I_ambient)
    /// berdasarkan koefisien Spherical Harmonics saat ini menggunakan formula McCamy.
    pub fn estimate_temperature_and_intensity(&self) -> (f32, f32) {
        let r_amb = self.current_sh.coefficients_r[0];
        let g_amb = self.current_sh.coefficients_g[0];
        let b_amb = self.current_sh.coefficients_b[0];

        let sum = r_amb + g_amb + b_amb;
        if sum < 1e-4 {
            return (6500.0, 0.0); // Default D65
        }

        let intensity = (0.299 * r_amb + 0.587 * g_amb + 0.114 * b_amb).clamp(0.0, 1.0);

        let x = r_amb / sum;
        let y = g_amb / sum;

        let n = (x - 0.3320) / (0.1858 - y);
        let temp = 449.0 * n.powi(3) + 3525.0 * n.powi(2) + 6823.3 * n + 5520.33;
        let temp = temp.clamp(2000.0, 10000.0);

        (temp, intensity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sh_projection_white() {
        let mut estimator = LightingEstimator::new();
        // Buat gambar putih 16x16 RGB
        let pixels = vec![255u8; 16 * 16 * 3];
        estimator.estimate_ambient_sh(pixels.as_ptr() as *const c_void, 16, 16);
        
        // Koefisien ambient utama (l=0, m=0) harus lebih besar dari 0
        assert!(estimator.current_sh.coefficients_r[0] > 0.0);
        assert!(estimator.current_sh.coefficients_g[0] > 0.0);
        assert!(estimator.current_sh.coefficients_b[0] > 0.0);
    }

    #[test]
    fn test_cct_and_intensity_estimation() {
        let mut estimator = LightingEstimator::new();
        estimator.current_sh.coefficients_r[0] = 0.282;
        estimator.current_sh.coefficients_g[0] = 0.282;
        estimator.current_sh.coefficients_b[0] = 0.282;

        let (temp, intensity) = estimator.estimate_temperature_and_intensity();
        assert!((temp - 6000.0).abs() < 1000.0);
        assert!((intensity - 0.282).abs() < 1e-4);
    }
}
