//! Modul Diagnostik Kulit AI: Fitzpatrick classification, CIELAB Conversion, dan White Balance.

use nalgebra::Vector3;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SkinAnalysisResult {
    pub fitzpatrick_type: i32,     // Tipe I - VI
    pub skin_tone_hex: [u8; 7],    // Contoh: "#D2B48C"
    pub dark_circles_score: f32,   // 0.0 (sehat) - 1.0 (kantung mata gelap)
    pub redness_score: f32,        // 0.0 - 1.0 (kemerahan/iritasi)
}

/// Mengonversi warna RGB (0..255) menjadi ruang warna CIELAB (L*, a*, b*).
pub fn rgb_to_lab(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    // 1. Normalisasi dan gamma correction (sRGB -> Linear RGB)
    let mut r_lin = (r as f32) / 255.0;
    let mut g_lin = (g as f32) / 255.0;
    let mut b_lin = (b as f32) / 255.0;

    r_lin = if r_lin > 0.04045 { ((r_lin + 0.055) / 1.055).powf(2.4) } else { r_lin / 12.92 };
    g_lin = if g_lin > 0.04045 { ((g_lin + 0.055) / 1.055).powf(2.4) } else { g_lin / 12.92 };
    b_lin = if b_lin > 0.04045 { ((b_lin + 0.055) / 1.055).powf(2.4) } else { b_lin / 12.92 };

    // 2. Linear RGB -> XYZ (D65 illuminant reference)
    let x = r_lin * 0.4124 + g_lin * 0.3576 + b_lin * 0.1805;
    let y = r_lin * 0.2126 + g_lin * 0.7152 + b_lin * 0.0722;
    let z = r_lin * 0.0193 + g_lin * 0.1192 + b_lin * 0.9505;

    // XYZ D65 reference white points
    let x_n = 0.95047;
    let y_n = 1.00000;
    let z_n = 1.08883;

    let xr = x / x_n;
    let yr = y / y_n;
    let zr = z / z_n;

    let f = |t: f32| -> f32 {
        if t > 0.008856 {
            t.powf(1.0 / 3.0)
        } else {
            7.787 * t + (16.0 / 116.0)
        }
    };

    let fx = f(xr);
    let fy = f(yr);
    let fz = f(zr);

    let l_star = 116.0 * fy - 16.0;
    let a_star = 500.0 * (fx - fy);
    let b_star = 200.0 * (fy - fz);

    (l_star, a_star, b_star)
}

pub struct SkinAnalyzer {
    pub neutral_light: Vector3<f32>, // Warna cahaya putih acuan RGB [1.0, 1.0, 1.0]
}

impl SkinAnalyzer {
    pub fn new() -> Self {
        Self {
            neutral_light: Vector3::new(1.0, 1.0, 1.0),
        }
    }

    /// Melakukan analisis diagnostik kulit dengan kompensasi warna cahaya ambient (White Balance).
    ///
    /// * `raw_skin_rgb`: Warna kulit mentah hasil sampling kamera.
    /// * `sh_ambient`: Warna ambient rata-rata terestimasi (misalnya koefisien SH pertama/ambient).
    pub fn analyze_skin(&self, raw_skin_rgb: &[u8; 3], sh_ambient: &Vector3<f32>) -> SkinAnalysisResult {
        // 1. Lakukan kompensasi White Balance
        // C_corrected = C_raw * (L_neutral / L_SH)
        let r_gain = if sh_ambient.x > 0.01 { self.neutral_light.x / sh_ambient.x } else { 1.0 };
        let g_gain = if sh_ambient.y > 0.01 { self.neutral_light.y / sh_ambient.y } else { 1.0 };
        let b_gain = if sh_ambient.z > 0.01 { self.neutral_light.z / sh_ambient.z } else { 1.0 };

        let r_corr = ((raw_skin_rgb[0] as f32) * r_gain).clamp(0.0, 255.0) as u8;
        let g_corr = ((raw_skin_rgb[1] as f32) * g_gain).clamp(0.0, 255.0) as u8;
        let b_corr = ((raw_skin_rgb[2] as f32) * b_gain).clamp(0.0, 255.0) as u8;

        // 2. Konversikan hasil white-balanced ke CIELAB
        let (l, a, b) = rgb_to_lab(r_corr, g_corr, b_corr);

        // 3. Deteksi tipe Fitzpatrick berdasarkan Luminansi L*
        // Fitzpatrick I: L* > 80, II: 70..80, III: 60..70, IV: 50..60, V: 40..50, VI: < 40
        let fitz = if l >= 80.0 {
            1
        } else if l >= 70.0 {
            2
        } else if l >= 60.0 {
            3
        } else if l >= 50.0 {
            4
        } else if l >= 40.0 {
            5
        } else {
            6
        };

        // Format warna ke string HEX
        let mut hex = [0u8; 7];
        let hex_str = format!("#{:02X}{:02X}{:02X}", r_corr, g_corr, b_corr);
        hex.copy_from_slice(hex_str.as_bytes());

        // Redness dihitung dari kanal a* (sumbu merah-hijau, a* tinggi = merah)
        // Normalisasi redness: a* berkisar dari -128 ke 127, daerah kemerahan kulit biasanya a* > 10
        let redness = ((a - 5.0) / 30.0).clamp(0.0, 1.0);

        // Dark circles dihitung dari penurunan kecerahan L* pada kelopak mata (disimulasikan di sini)
        let dark_circles = ((100.0 - l) / 100.0).clamp(0.0, 1.0);

        SkinAnalysisResult {
            fitzpatrick_type: fitz,
            skin_tone_hex: hex,
            dark_circles_score: dark_circles,
            redness_score: redness,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_lab_white() {
        let (l, a, b) = rgb_to_lab(255, 255, 255);
        // Putih sempurna harus memiliki L* mendekati 100 dan chromaticity (a*, b*) nol
        assert!((l - 100.0).abs() < 1.0);
        assert!(a.abs() < 0.5);
        assert!(b.abs() < 0.5);
    }

    #[test]
    fn test_skin_analysis_fitzpatrick() {
        let analyzer = SkinAnalyzer::new();
        let raw_skin = [210, 180, 140]; // Light brown
        let sh_ambient = Vector3::new(1.0, 1.0, 1.0); // Cahaya netral

        let result = analyzer.analyze_skin(&raw_skin, &sh_ambient);
        assert!(result.fitzpatrick_type >= 2 && result.fitzpatrick_type <= 4);
        assert_eq!(result.skin_tone_hex[0], b'#');
    }
}
