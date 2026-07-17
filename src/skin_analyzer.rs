//! Modul Diagnostik Kulit AI: Fitzpatrick classification, CIELAB Conversion, ITA° Skin Undertone Classifier, dan White Balance.

use nalgebra::Vector3;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ItaSkinType {
    VeryLight = 0,
    Light = 1,
    Intermediate = 2,
    Tan = 3,
    Brown = 4,
    Dark = 5,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SkinAnalysisResult {
    pub fitzpatrick_type: i32,     // Tipe I - VI
    pub skin_tone_hex: [u8; 7],    // Contoh: "#D2B48C"
    pub dark_circles_score: f32,   // 0.0 (sehat) - 1.0 (kantung mata gelap)
    pub redness_score: f32,        // 0.0 - 1.0 (kemerahan/iritasi)
    pub ita_angle: f32,            // Sudut ITA° (-90..90 derajat)
    pub ita_skin_type: i32,        // Nilai ItaSkinType enum sebagai i32
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

/// Menghitung sudut Individual Typology Angle (ITA°) berdasarkan luminansi L* dan kekuningan b*
pub fn compute_ita_angle(l: f32, b: f32) -> f32 {
    let numerator = l - 50.0;
    // Hindari pembagian dengan nol dengan memberikan perlindungan kecil jika b mendekati nol
    let denominator = if b.abs() < 1e-5 { 1e-5 } else { b };
    let angle_rad = numerator.atan2(denominator);
    angle_rad * 180.0 / std::f32::consts::PI
}

/// Mengklasifikasikan tipe rona kulit secara klinis berdasarkan sudut ITA°
pub fn classify_ita_skin_type(ita_angle: f32) -> ItaSkinType {
    if ita_angle > 55.0 {
        ItaSkinType::VeryLight
    } else if ita_angle > 41.0 {
        ItaSkinType::Light
    } else if ita_angle > 28.0 {
        ItaSkinType::Intermediate
    } else if ita_angle > 10.0 {
        ItaSkinType::Tan
    } else if ita_angle > -30.0 {
        ItaSkinType::Brown
    } else {
        ItaSkinType::Dark
    }
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
        let r_gain = if sh_ambient.x > 0.01 { self.neutral_light.x / sh_ambient.x } else { 1.0 };
        let g_gain = if sh_ambient.y > 0.01 { self.neutral_light.y / sh_ambient.y } else { 1.0 };
        let b_gain = if sh_ambient.z > 0.01 { self.neutral_light.z / sh_ambient.z } else { 1.0 };

        let r_corr = ((raw_skin_rgb[0] as f32) * r_gain).clamp(0.0, 255.0) as u8;
        let g_corr = ((raw_skin_rgb[1] as f32) * g_gain).clamp(0.0, 255.0) as u8;
        let b_corr = ((raw_skin_rgb[2] as f32) * b_gain).clamp(0.0, 255.0) as u8;

        // 2. Konversikan hasil white-balanced ke CIELAB
        let (l, a, b) = rgb_to_lab(r_corr, g_corr, b_corr);

        // 3. Deteksi tipe Fitzpatrick berdasarkan Luminansi L*
        let fit = if l >= 80.0 {
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

        // Redness dihitung dari kanal a*
        let redness = ((a - 5.0) / 30.0).clamp(0.0, 1.0);

        // Dark circles dihitung dari penurunan kecerahan L*
        let dark_circles = ((100.0 - l) / 100.0).clamp(0.0, 1.0);

        // 4. Hitung sudut ITA° dan klasifikasikan tipenya
        let ita_angle = compute_ita_angle(l, b);
        let ita_skin = classify_ita_skin_type(ita_angle) as i32;

        SkinAnalysisResult {
            fitzpatrick_type: fit,
            skin_tone_hex: hex,
            dark_circles_score: dark_circles,
            redness_score: redness,
            ita_angle,
            ita_skin_type: ita_skin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_lab_white() {
        let (l, a, b) = rgb_to_lab(255, 255, 255);
        assert!((l - 100.0).abs() < 1.0);
        assert!(a.abs() < 0.5);
        assert!(b.abs() < 0.5);
    }

    #[test]
    fn test_skin_analysis_fitzpatrick() {
        let analyzer = SkinAnalyzer::new();
        let raw_skin = [210, 180, 140]; // Light brown
        let sh_ambient = Vector3::new(1.0, 1.0, 1.0);

        let result = analyzer.analyze_skin(&raw_skin, &sh_ambient);
        assert!(result.fitzpatrick_type >= 2 && result.fitzpatrick_type <= 4);
        assert_eq!(result.skin_tone_hex[0], b'#');
    }

    #[test]
    fn test_ita_classification() {
        // Uji Very Light (L* tinggi, b* rendah/netral)
        let angle1 = compute_ita_angle(80.0, 10.0);
        let t1 = classify_ita_skin_type(angle1);
        assert_eq!(t1, ItaSkinType::VeryLight);

        // Uji Dark (L* rendah, b* sedang)
        let angle2 = compute_ita_angle(30.0, 15.0);
        let t2 = classify_ita_skin_type(angle2);
        assert_eq!(t2, ItaSkinType::Dark);
    }
}
