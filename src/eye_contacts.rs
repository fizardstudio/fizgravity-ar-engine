//! Modul Pelacakan Gerakan Mata & Proyeksi Lensa Kontak Warna (Softlens Try-On) secara Sferis.

use crate::ArVertex3D;
use std::os::raw::c_float;

pub const IRIS_VERTICES_COUNT: usize = 5; // 1 pusat, 4 tepi (kiri, kanan, atas, bawah)

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArEyeContactsState {
    pub left_iris: [ArVertex3D; IRIS_VERTICES_COUNT],
    pub right_iris: [ArVertex3D; IRIS_VERTICES_COUNT],
    pub gaze_direction_x: c_float, // Sudut pandang horizontal mata
    pub gaze_direction_y: c_float, // Sudut pandang vertikal mata
}

impl ArEyeContactsState {
    /// Membuat koordinat polar sferis (theta, phi) untuk pemetaan tekstur lensa kontak yang melengkung.
    /// Ini mengoreksi distorsi 2D datar di tepi iris mata.
    ///
    /// * `vertex`: Titik landmark pada permukaan iris.
    /// * `center`: Koordinat titik pusat iris 3D.
    /// * `radius`: Radius bola bola mata (elipsoid).
    pub fn project_spherical_uv(
        &self,
        vertex: &ArVertex3D,
        center: &ArVertex3D,
        radius: f32,
    ) -> (f32, f32) {
        if radius <= 1e-6 {
            return (0.5, 0.5);
        }

        // 1. Transformasikan landmark ke sistem koordinat relatif terhadap pusat iris
        let dx = (vertex.x - center.x) / radius;
        let dy = (vertex.y - center.y) / radius;
        
        // Asumsi sumbu Z dihitung berdasarkan proyeksi permukaan bola: x^2 + y^2 + z^2 = 1
        let r2 = dx*dx + dy*dy;
        let dz = if r2 < 1.0 { (1.0 - r2).sqrt() } else { 0.0 };

        // 2. Konversikan arah unit vector (dx, dy, dz) ke koordinat bola sferis (theta, phi)
        // theta = sudut azimuth [-PI, PI]
        // phi = sudut polar [0, PI]
        let theta = dy.atan2(dx);
        let phi = dz.acos();

        // 3. Normalisasikan sudut ke rentang koordinat UV [0.0, 1.0]
        let u = (theta + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
        let v = phi / std::f32::consts::PI;

        (u, v)
    }

    /// Menghitung diameter pembukaan pupil secara dinamis berdasarkan nilai luminansi kecerahan cahaya ambient.
    /// Cahaya terang -> pupil menyempit; Cahaya redup -> pupil melebar.
    pub fn compute_dynamic_pupil_dilation(&self, ambient_brightness: f32) -> f32 {
        // Normalisasi kecerahan: 0.0 (gelap gulita) - 1.0 (sangat terang)
        let brightness_clamped = ambient_brightness.clamp(0.0, 1.0);
        
        // Pupil diameter rasio: 0.15 (sangat terang/dilatasi kecil) - 0.45 (gelap/dilatasi besar)
        0.45 - (0.30 * brightness_clamped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spherical_projection_center() {
        let state = ArEyeContactsState {
            left_iris: [ArVertex3D { x: 0.0, y: 0.0, z: 0.0 }; 5],
            right_iris: [ArVertex3D { x: 0.0, y: 0.0, z: 0.0 }; 5],
            gaze_direction_x: 0.0,
            gaze_direction_y: 0.0,
        };

        let center = ArVertex3D { x: 1.0, y: 1.0, z: 1.0 };
        // Landmark tepat di pusat iris
        let (u, v) = state.project_spherical_uv(&center, &center, 0.012);
        
        // Di pusat (dx=0, dy=0, dz=1 -> phi = acos(1) = 0)
        assert!((v - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_dynamic_pupil_dilation() {
        let state = ArEyeContactsState {
            left_iris: [ArVertex3D { x: 0.0, y: 0.0, z: 0.0 }; 5],
            right_iris: [ArVertex3D { x: 0.0, y: 0.0, z: 0.0 }; 5],
            gaze_direction_x: 0.0,
            gaze_direction_y: 0.0,
        };

        // Kecerahan tinggi -> pupil menyempit
        let scale_bright = state.compute_dynamic_pupil_dilation(1.0);
        assert!((scale_bright - 0.15).abs() < 1e-4);

        // Kecerahan rendah -> pupil melebar
        let scale_dark = state.compute_dynamic_pupil_dilation(0.0);
        assert!((scale_dark - 0.45).abs() < 1e-4);
    }
}
