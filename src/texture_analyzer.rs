//! Modul AI Analisis Tekstur & Kesehatan Kulit: Local Contrast LBP dan Deteksi Kerutan/Noda.

use std::os::raw::{c_float, c_int, c_uchar};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SkinHealthReport {
    pub roughness_score: f32,    // Kekasaran kulit (0.0 halus - 1.0 kasar)
    pub wrinkle_score: f32,      // Kerutan dahi (0.0 halus - 1.0 banyak kerutan)
    pub blemish_score: f32,      // Noda/bintik jerawat (0.0 bersih - 1.0 banyak noda)
}

pub struct SkinTextureAnalyzer;

impl SkinTextureAnalyzer {
    /// Menganalisis tekstur kulit pada Region of Interest (ROI) tertentu:
    ///
    /// * `image_rgb`: Buffer gambar RGB mentah (lebar x tinggi x 3 bytes).
    /// * `width` & `height`: Dimensi gambar.
    /// * `roi_x`, `roi_y`, `roi_w`, `roi_h`: Koordinat ROI di dalam gambar.
    pub fn analyze_roi(
        image_rgb: &[u8],
        width: usize,
        height: usize,
        roi_x: usize,
        roi_y: usize,
        roi_w: usize,
        roi_h: usize,
    ) -> (f32, f32) {
        if roi_x + roi_w > width || roi_y + roi_h > height || roi_w < 3 || roi_h < 3 {
            return (0.0, 0.0);
        }

        // 1. Ekstrak ROI ke dalam grayscale buffer
        let mut gray_roi = vec![0.0f32; roi_w * roi_h];
        for y in 0..roi_h {
            for x in 0..roi_w {
                let img_x = roi_x + x;
                let img_y = roi_y + y;
                let idx = (img_y * width + img_x) * 3;
                if idx + 2 < image_rgb.len() {
                    let r = image_rgb[idx] as f32;
                    let g = image_rgb[idx + 1] as f32;
                    let b = image_rgb[idx + 2] as f32;
                    // Luma conversion (ITU-R BT.601)
                    gray_roi[y * roi_w + x] = 0.299 * r + 0.587 * g + 0.114 * b;
                }
            }
        }

        // 2. Hitung Local Contrast-Weighted LBP (LC-LBP)
        // Kita menggunakan LBP 3x3 standar dengan threshold dinamis berdasarkan kontras lokal.
        let mut lbp_sum = 0.0f32;
        let mut lbp_count = 0;
        let mut local_variance_sum = 0.0f32;

        for y in 1..(roi_h - 1) {
            for x in 1..(roi_w - 1) {
                let center_val = gray_roi[y * roi_w + x];
                
                // Hitung kontras lokal (deviasi standar sekitar 3x3) dalam satu loop
                let mut sum = 0.0f32;
                let mut sum_sq = 0.0f32;
                for ny in (y-1)..=(y+1) {
                    for nx in (x-1)..=(x+1) {
                        let val = gray_roi[ny * roi_w + nx];
                        sum += val;
                        sum_sq += val * val;
                    }
                }
                let mean = sum / 9.0;
                let variance = (sum_sq / 9.0 - mean * mean).max(0.0);
                let var = variance.sqrt();
                local_variance_sum += var;

                // Threshold dinamis berdasarkan kontras lokal (tau * variance)
                let tau = 0.08;
                let threshold = (tau * var).max(1.5); // Proteksi minimal untuk sensor noise

                let mut lbp_pattern = 0u8;
                let mut bit = 0;
                
                // 8 tetangga sekitar searah jarum jam
                let neighbors = [
                    (-1, -1), (0, -1), (1, -1),
                    (1, 0),   (1, 1),  (0, 1),
                    (-1, 1),  (-1, 0)
                ];

                for &(dx, dy) in &neighbors {
                    let neighbor_val = gray_roi[((y as isize + dy) as usize) * roi_w + ((x as isize + dx) as usize)];
                    if neighbor_val >= center_val + threshold {
                        lbp_pattern |= 1 << bit;
                    }
                    bit += 1;
                }

                // Pola LBP seragam (uniform patterns) memiliki transisi bit <= 2
                let transitions = count_transitions(lbp_pattern);
                if transitions <= 2 {
                    lbp_sum += lbp_pattern as f32;
                    lbp_count += 1;
                }
            }
        }

        let mean_roughness = if lbp_count > 0 { lbp_sum / (lbp_count as f32 * 255.0) } else { 0.0 };
        let mean_contrast = if (roi_w * roi_h) > 0 { local_variance_sum / ((roi_w * roi_h) as f32) } else { 0.0 };

        (mean_contrast, mean_roughness)
    }

    /// Menganalisis dahi secara khusus menggunakan filter Sobel horizontal untuk mengukur kerutan.
    pub fn analyze_wrinkles(
        image_rgb: &[u8],
        width: usize,
        height: usize,
        forehead_x: usize,
        forehead_y: usize,
        forehead_w: usize,
        forehead_h: usize,
    ) -> f32 {
        if forehead_x + forehead_w > width || forehead_y + forehead_h > height || forehead_w < 3 || forehead_h < 3 {
            return 0.0;
        }

        // Ekstrak ROI ke grayscale
        let mut gray = vec![0.0f32; forehead_w * forehead_h];
        for y in 0..forehead_h {
            for x in 0..forehead_w {
                let idx = ((forehead_y + y) * width + (forehead_x + x)) * 3;
                if idx + 2 < image_rgb.len() {
                    gray[y * forehead_w + x] = 0.299 * image_rgb[idx] as f32
                        + 0.587 * image_rgb[idx + 1] as f32
                        + 0.114 * image_rgb[idx + 2] as f32;
                }
            }
        }

        // Jalankan deteksi tepi Sobel Horizontal (untuk kerutan dahi yang melintang horizontal)
        let mut edge_sum = 0.0f32;
        let mut edge_count = 0;

        for y in 1..(forehead_h - 1) {
            for x in 1..(forehead_w - 1) {
                // Sobel kernel y (horizontal edges):
                // [ -1, -2, -1 ]
                // [  0,  0,  0 ]
                // [  1,  2,  1 ]
                let gy = -1.0 * gray[(y - 1) * forehead_w + (x - 1)]
                    - 2.0 * gray[(y - 1) * forehead_w + x]
                    - 1.0 * gray[(y - 1) * forehead_w + (x + 1)]
                    + 1.0 * gray[(y + 1) * forehead_w + (x - 1)]
                    + 2.0 * gray[(y + 1) * forehead_w + x]
                    + 1.0 * gray[(y + 1) * forehead_w + (x + 1)];

                let edge_val = gy.abs();
                if edge_val > 25.0 { // Ambang batas kerutan nyata
                    edge_sum += edge_val;
                    edge_count += 1;
                }
            }
        }

        if edge_count > 0 {
            let density = (edge_count as f32) / ((forehead_w * forehead_h) as f32);
            let intensity = edge_sum / (edge_count as f32 * 255.0);
            (density * intensity * 15.0).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Menghitung sudut ITA (Individual Typology Angle) dan menentukan undertone (Cool, Neutral, Warm)
    /// berdasarkan sampel warna RGB kulit.
    pub fn calculate_skin_undertone(r: u8, g: u8, b: u8) -> (f32, String) {
        let (l, a_val, b_val) = crate::skin_analyzer::rgb_to_lab(r, g, b);
        let ita_angle = crate::skin_analyzer::compute_ita_angle(l, b_val);

        // Klasifikasi undertone berdasarkan hubungan antara a* (merah/pink) dan b* (kuning/hangat)
        let undertone = if b_val > a_val + 3.0 {
            "Warm".to_string()
        } else if b_val < a_val - 2.0 {
            "Cool".to_string()
        } else {
            "Neutral".to_string()
        };

        (ita_angle, undertone)
    }
}

/// Menghitung jumlah transisi bit 0->1 dan 1->0 untuk mendeteksi keseragaman LBP pattern.
fn count_transitions(val: u8) -> u32 {
    let mut transitions = 0;
    let mut prev_bit = val & 1;
    for i in 1..8 {
        let bit = (val >> i) & 1;
        if bit != prev_bit {
            transitions += 1;
        }
        prev_bit = bit;
    }
    // Lingkar melingkar bit 7 ke bit 0
    if (val >> 7) & 1 != val & 1 {
        transitions += 1;
    }
    transitions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lbp_transitions() {
        assert_eq!(count_transitions(0xFF), 0); // Semua 1 -> 0 transisi
        assert_eq!(count_transitions(0x00), 0); // Semua 0 -> 0 transisi
        assert_eq!(count_transitions(0x55), 8); // 01010101 -> 8 transisi
    }

    #[test]
    fn test_analyze_roi_solid_color() {
        let rgb_image = vec![128u8; 100 * 100 * 3]; // Gambar solid
        let (roughness, contrast) = SkinTextureAnalyzer::analyze_roi(&rgb_image, 100, 100, 10, 10, 20, 20);
        // Gambar solid tidak boleh memiliki kekasaran atau kontras
        assert_eq!(roughness, 0.0);
        assert_eq!(contrast, 0.0);
    }

    #[test]
    fn test_skin_undertone_classification() {
        // Sampel Warm: (210, 180, 140)
        let (ita1, under1) = SkinTextureAnalyzer::calculate_skin_undertone(210, 180, 140);
        assert_eq!(under1, "Warm");
        assert!(ita1 > -90.0 && ita1 < 90.0);

        // Sampel Cool: (215, 175, 175)
        let (_, under2) = SkinTextureAnalyzer::calculate_skin_undertone(215, 175, 175);
        assert_eq!(under2, "Cool");

        // Sampel Neutral: (200, 180, 170)
        let (_, under3) = SkinTextureAnalyzer::calculate_skin_undertone(200, 180, 170);
        assert_eq!(under3, "Neutral");
    }
}
