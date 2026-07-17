//! Modul Auto-Kalibrasi Kamera Intrinsik Online menggunakan Fitur Geometri Wajah Antropometri.

use crate::ArVertex3D;

pub struct CameraAutoCalibrator {
    pub estimated_focal_length: f32, // f = (fx + fy) / 2
    pub estimated_cx: f32,
    pub estimated_cy: f32,
    alpha: f32, // Parameter redaman low-pass filter (0.01 - 0.05)
}

impl CameraAutoCalibrator {
    pub fn new() -> Self {
        Self {
            estimated_focal_length: 500.0, // Nilai default awal (asumsi kamera depan standar)
            estimated_cx: 320.0,
            estimated_cy: 240.0,
            alpha: 0.02, // Redaman sangat lambat dan stabil
        }
    }

    /// Melakukan kalibrasi online dinamis untuk mengestimasi parameter intrinsik kamera.
    ///
    /// * `face_vertices`: Koordinat 3D jaring wajah terdeteksi.
    /// * `image_w` & `image_h`: Dimensi bingkai gambar kamera.
    /// * `estimated_depth_z`: Kedalaman wajah terestimasi dari VIO/sensor dalam meter.
    pub fn update_calibration(
        &mut self,
        face_vertices: &[crate::face::ArFaceVertexInterleaved; 468],
        image_w: f32,
        image_h: f32,
        estimated_depth_z: f32,
    ) {
        // Tentukan pusat gambar secara optik sebagai tebakan awal
        self.estimated_cx = image_w * 0.5;
        self.estimated_cy = image_h * 0.5;

        if estimated_depth_z < 0.1 {
            return; // Hindari kedalaman invalid dekat dengan lensa
        }

        // Ambil jarak piksel horizontal antara pelipis kiri (index 127) dan kanan (index 356)
        let p_left = face_vertices[127].position;
        let p_right = face_vertices[356].position;
        
        // Ambil tinggi dahi-dagu (index 10 ke 152) untuk koreksi penolehan (yaw)
        let p_top = face_vertices[10].position;
        let p_bottom = face_vertices[152].position;

        // Proyeksikan lebar pelipis terukur
        let dx = p_left.x - p_right.x;
        let dy = p_left.y - p_right.y;
        let w_measured = (dx*dx + dy*dy).sqrt();

        // Proyeksikan tinggi vertikal terukur
        let dx_v = p_top.x - p_bottom.x;
        let dy_v = p_top.y - p_bottom.y;
        let h_measured = (dx_v*dx_v + dy_v*dy_v).sqrt();

        if w_measured < 10.0 || h_measured < 10.0 {
            return;
        }

        // Rasio aspek dahi-pelipis fisik rata-rata (W_phys / H_phys = 13.5cm / 19.0cm = 0.71)
        let base_ratio = 0.71f32;

        // Hitung faktor kosinus sudut yaw berdasarkan rasio penyusutan lebar terhadap tinggi
        let cos_yaw = (w_measured / (h_measured * base_ratio)).clamp(0.5, 1.0);

        // Koreksi lebar piksel untuk mengompensasi penolehan kepala (yaw)
        let d_pixels_corrected = w_measured / cos_yaw;

        // Lebar pelipis fisik rata-rata manusia secara antropometri (W_phys = 13.5 cm)
        let w_phys = 0.135f32; 

        // Selesaikan formula pinhole: f = (d_pixels_corrected * Z) / W_phys
        let f_measured = (d_pixels_corrected * estimated_depth_z) / w_phys;

        // Lakukan pemulusan menggunakan filter low-pass
        self.estimated_focal_length = self.alpha * f_measured + (1.0 - self.alpha) * self.estimated_focal_length;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::ArTexCoord2D;

    #[test]
    fn test_auto_calibration_convergence() {
        let mut calibrator = CameraAutoCalibrator::new();
        
        let mut vertices = [ArFaceVertexInterleaved {
            position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
            normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
            uv: ArTexCoord2D { u: 0.0, v: 0.0 },
        }; 468];

        // Buat jarak pelipis 100 piksel di layar
        vertices[127].position = ArVertex3D { x: 100.0, y: 0.0, z: 0.0 };
        vertices[356].position = ArVertex3D { x: 0.0, y: 0.0, z: 0.0 };

        // Atur tinggi dahi-dagu agar rasio = 0.71 (100.0 / 0.71 = 140.84)
        vertices[10].position = ArVertex3D { x: 0.0, y: 140.84, z: 0.0 };
        vertices[152].position = ArVertex3D { x: 0.0, y: 0.0, z: 0.0 };

        // Lakukan update berkali-kali untuk melihat konvergensi filter
        // Z = 0.675 meter (jarak wajar tangan memegang HP)
        // f_target = (100 * 0.675) / 0.135 = 500
        for _ in 0..100 {
            calibrator.update_calibration(&vertices, 640.0, 480.0, 0.675);
        }

        // Hasil estimasi harus konvergen ke ~500
        assert!((calibrator.estimated_focal_length - 500.0).abs() < 5.0);
    }
}
use crate::face::ArFaceVertexInterleaved;
