//! Modul Pengaturan Riasan Fisik PBR (Gloss/Matte Lipstick, Metallic Eyeshadow) dengan Schlick's Fresnel.

use std::os::raw::c_float;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArMakeupPbrProperties {
    pub roughness: c_float,       // 0.0 (Glossy/Lip gloss) - 1.0 (Matte/Matte lipstick)
    pub metallic: c_float,        // 0.0 (Non-metallic) - 1.0 (Eyeshadow metallic/glitter)
    pub sheer_blending: c_float,  // 0.0 (Transparan tipis) - 1.0 (Warna pekat)
}

impl ArMakeupPbrProperties {
    pub fn new(roughness: f32, metallic: f32, sheer: f32) -> Self {
        Self {
            roughness,
            metallic,
            sheer_blending: sheer,
        }
    }

    /// Menghitung refleksivitas Fresnel menggunakan Aproksimasi Schlick untuk optimasi GPU seluler.
    ///
    /// * `cos_theta`: Cosinus sudut antara arah pandang (V) dan normal setengah (H).
    /// * `f0`: Reflektansi dasar pada insiden normal (biasanya 0.04 untuk dielektrik, atau warna albedo untuk logam).
    pub fn compute_schlick_fresnel(&self, cos_theta: f32, f0: f32) -> f32 {
        let clamped_cos = cos_theta.clamp(0.0, 1.0);
        let factor = 1.0 - clamped_cos;
        let factor5 = factor * factor * factor * factor * factor; // (1 - cos)^5
        
        f0 + (1.0 - f0) * factor5
    }

    /// Menghitung fungsi distribusi mikro-permukaan GGX (GGX Normal Distribution Function).
    ///
    /// * `n_dot_h`: Dot product antara normal permukaan (N) dan half-vector (H).
    pub fn compute_ggx_ndf(&self, n_dot_h: f32) -> f32 {
        let alpha = self.roughness * self.roughness;
        let alpha2 = alpha * alpha;
        
        let nh2 = (n_dot_h * n_dot_h).clamp(0.0, 1.0);
        let denom_part = nh2 * (alpha2 - 1.0) + 1.0;
        let denom = std::f32::consts::PI * denom_part * denom_part;
        
        if denom > 1e-6 {
            alpha2 / denom
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schlick_fresnel() {
        let props = ArMakeupPbrProperties::new(0.3, 0.0, 0.8);
        
        // Pada sudut 90 derajat (cos_theta = 0), Fresnel harus mendekati 1.0 (refleksi total)
        let f_90 = props.compute_schlick_fresnel(0.0, 0.04);
        assert!((f_90 - 1.0).abs() < 1e-4);

        // Pada insiden normal (cos_theta = 1), Fresnel harus sama dengan f0
        let f_normal = props.compute_schlick_fresnel(1.0, 0.04);
        assert!((f_normal - 0.04).abs() < 1e-4);
    }

    #[test]
    fn test_ggx_ndf() {
        let props = ArMakeupPbrProperties::new(0.5, 0.0, 0.8);
        let ndf = props.compute_ggx_ndf(1.0); // N == H
        assert!(ndf > 0.0);
    }
}
