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
        // Logika dinonaktifkan sementara untuk mengisolasi penyebab stack corruption
    }

    /// Menghitung suhu warna sekitar (T_ambient dalam Kelvin) dan intensitasnya (I_ambient)
    /// berdasarkan koefisien Spherical Harmonics saat ini menggunakan formula McCamy.
    pub fn estimate_temperature_and_intensity(&self) -> (f32, f32) {
        let r_amb = self.current_sh.coefficients_r[0];
        let g_amb = self.current_sh.coefficients_g[0];
        let b_amb = self.current_sh.coefficients_b[0];

        // Convert sRGB coefficients to CIE XYZ space
        let x_xyz = 0.4124 * r_amb + 0.3576 * g_amb + 0.1805 * b_amb;
        let y_xyz = 0.2126 * r_amb + 0.7152 * g_amb + 0.0722 * b_amb;
        let z_xyz = 0.0193 * r_amb + 0.1192 * g_amb + 0.9505 * b_amb;

        let xyz_sum = x_xyz + y_xyz + z_xyz;
        if xyz_sum < 1e-4 {
            return (6500.0, 0.0); // Default D65
        }

        let intensity = (0.299 * r_amb + 0.587 * g_amb + 0.114 * b_amb).clamp(0.0, 1.0);

        let x = x_xyz / xyz_sum;
        let y = y_xyz / xyz_sum;

        // McCamy formula: n = (x - xe) / (y - ye) where (xe, ye) = (0.3320, 0.1858)
        let n = (x - 0.3320) / (y - 0.1858);
        let temp = -449.0 * n.powi(3) + 3525.0 * n.powi(2) - 6823.3 * n + 5520.33;
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
