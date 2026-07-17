//! Modul Triangulator Kosmetik Lokal untuk Riasan Presisi (Bibir Atas, Bibir Bawah, Eyelids).

use crate::face::ArFaceVertexInterleaved;
use std::os::raw::c_int;

// Indeks landmark kontur bibir atas (MediaPipe)
pub const UPPER_LIP_OUTER: [usize; 11] = [61, 185, 40, 39, 37, 0, 267, 269, 270, 409, 291];
pub const UPPER_LIP_INNER: [usize; 11] = [78, 191, 80, 81, 82, 13, 312, 311, 310, 415, 308];

// Indeks landmark kontur bibir bawah (MediaPipe)
pub const LOWER_LIP_OUTER: [usize; 11] = [291, 325, 307, 375, 321, 17, 91, 146, 61, 181, 84]; // 11 titik diselaraskan
pub const LOWER_LIP_INNER: [usize; 11] = [308, 324, 318, 402, 317, 14, 87, 178, 88, 95, 78];

pub struct MakeupTriangulator;

impl MakeupTriangulator {
    /// Menghasilkan indeks segitiga (triangulasi) untuk Bibir Atas.
    pub fn get_upper_lip_triangles() -> Vec<u32> {
        let mut triangles = Vec::new();
        // Hubungkan outer dan inner kontur menggunakan quad-strip
        for i in 0..10 {
            let o1 = UPPER_LIP_OUTER[i] as u32;
            let o2 = UPPER_LIP_OUTER[i + 1] as u32;
            let i1 = UPPER_LIP_INNER[i] as u32;
            let i2 = UPPER_LIP_INNER[i + 1] as u32;

            // Segitiga 1
            triangles.push(o1);
            triangles.push(o2);
            triangles.push(i1);

            // Segitiga 2
            triangles.push(i1);
            triangles.push(o2);
            triangles.push(i2);
        }
        triangles
    }

    /// Menghasilkan indeks segitiga (triangulasi) untuk Bibir Bawah.
    pub fn get_lower_lip_triangles() -> Vec<u32> {
        let mut triangles = Vec::new();
        for i in 0..10 {
            let o1 = LOWER_LIP_OUTER[i] as u32;
            let o2 = LOWER_LIP_OUTER[i + 1] as u32;
            let i1 = LOWER_LIP_INNER[i] as u32;
            let i2 = LOWER_LIP_INNER[i + 1] as u32;

            // Segitiga 1
            triangles.push(o1);
            triangles.push(o2);
            triangles.push(i1);

            // Segitiga 2
            triangles.push(i1);
            triangles.push(o2);
            triangles.push(i2);
        }
        triangles
    }

    /// Menghitung koefisien feathering (alpha 0.0 - 1.0) untuk kelembutan batas luar kosmetik.
    /// Verteks di luar bibir (outer contour) akan memudar halus mendekati 0.0,
    /// sedangkan bagian tengah (inner contour) tetap tebal mendekati 1.0.
    pub fn calculate_lip_feathering(
        face_vertices: &[ArFaceVertexInterleaved; 468],
        out_alphas: &mut [f32; 468],
    ) {
        // Set default ke 1.0 (tidak diubah) untuk seluruh wajah
        for alpha in out_alphas.iter_mut() {
            *alpha = 1.0;
        }

        // Terapkan gradasi transisi linear (feathering) pada bibir atas
        for &outer_idx in &UPPER_LIP_OUTER {
            // Tepi luar memudar halus ke 0.15
            out_alphas[outer_idx] = 0.15;
        }
        for &inner_idx in &UPPER_LIP_INNER {
            // Tepi dalam tebal penuh 1.0
            out_alphas[inner_idx] = 1.0;
        }

        // Terapkan hal yang sama pada bibir bawah
        for &outer_idx in &LOWER_LIP_OUTER {
            out_alphas[outer_idx] = 0.15;
        }
        for &inner_idx in &LOWER_LIP_INNER {
            out_alphas[inner_idx] = 1.0;
        }

        // Hitung pemulusan tambahan pada landmark di antara outer dan inner (opsional)
        // MediaPipe memiliki landmark bibir sekunder di antara outer & inner yang diberi kekuatan alpha menengah (0.6)
        let middle_lip_indices = [37, 0, 267, 81, 82, 13, 312, 311, 14, 87, 178, 317];
        for &mid_idx in &middle_lip_indices {
            if mid_idx < 468 {
                out_alphas[mid_idx] = 0.65;
            }
        }
    }

    /// Menghitung tingkat blending hairline dahi agar foundation memudar halus saat mendekati rambut.
    /// out_alphas diisi dengan nilai transparansi [0.0 - 1.0].
    pub fn calculate_hairline_blending(
        face_vertices: &[ArFaceVertexInterleaved; 468],
        out_alphas: &mut [f32; 468],
    ) {
        // Tentukan batas landmark dahi paling atas (hairline)
        let hairline_indices = [103, 67, 109, 10, 338, 297, 332, 284, 251, 389, 356];

        for i in 0..468 {
            let v_pos = face_vertices[i].position;
            let mut min_dist_sq = f32::MAX;
            for &hair_idx in &hairline_indices {
                if hair_idx < 468 {
                    let h_pos = face_vertices[hair_idx].position;
                    let dx = v_pos.x - h_pos.x;
                    let dy = v_pos.y - h_pos.y;
                    let dz = v_pos.z - h_pos.z;
                    let dist_sq = dx*dx + dy*dy + dz*dz;
                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                    }
                }
            }

            let min_dist = min_dist_sq.sqrt();

            // Jika jarak ke hairline sangat dekat (< 3.5cm), mulailah memudar secara non-linear (sigmoid/smoothstep)
            let blend_radius = 0.035;
            if min_dist < blend_radius {
                let factor = min_dist / blend_radius;
                let alpha = factor * factor * (3.0 - 2.0 * factor);
                out_alphas[i] = out_alphas[i].min(alpha.clamp(0.0, 1.0));
            }
        }
    }

    /// Menghitung koefisien ambient occlusion (AO) dinamis untuk setiap dari 468 vertices.
    /// out_ao diisi dengan faktor [0.0 (gelap/terhalang) - 1.0 (terang)].
    pub fn calculate_dynamic_ao(
        blendshapes: &[f32; 52],
        out_ao: &mut [f32; 468],
    ) {
        // 1. Set default AO ke 1.0 (terang penuh)
        for ao in out_ao.iter_mut() {
            *ao = 1.0;
        }

        // 2. Terapkan AO statis pada daerah lipatan wajah anatomis (hidung & mata)
        let nose_creases = [102, 331, 294, 64, 278, 98, 327, 2, 94, 323];
        for &idx in &nose_creases {
            out_ao[idx] = 0.55;
        }

        let eye_corners = [133, 155, 173, 362, 382, 398, 33, 263];
        for &idx in &eye_corners {
            out_ao[idx] = 0.50;
        }

        // 3. Modulasikan AO area bibir dalam secara dinamis berdasarkan blendshape mouthOpen (index 25)
        let mouth_open_coeff = blendshapes[25];

        let mouth_ao = 0.15 + 0.70 * mouth_open_coeff.clamp(0.0, 1.0);

        for &idx in &UPPER_LIP_INNER {
            out_ao[idx] = mouth_ao;
        }
        for &idx in &LOWER_LIP_INNER {
            out_ao[idx] = mouth_ao;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upper_lip_triangulation() {
        let tris = MakeupTriangulator::get_upper_lip_triangles();
        // 10 segmen * 2 segitiga * 3 indeks = 60 indeks total
        assert_eq!(tris.len(), 60);
        // Indeks pertama harus cocok dengan UPPER_LIP_OUTER[0] = 61
        assert_eq!(tris[0], 61);
    }

    #[test]
    fn test_lower_lip_triangulation() {
        let tris = MakeupTriangulator::get_lower_lip_triangles();
        assert_eq!(tris.len(), 60);
    }

    #[test]
    fn test_hairline_blending() {
        use crate::ArVertex3D;
        use crate::face::ArTexCoord2D;
        
        let mut vertices = [ArFaceVertexInterleaved {
            position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
            normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
            uv: ArTexCoord2D { u: 0.0, v: 0.0 },
        }; 468];

        vertices[10].position = ArVertex3D { x: 0.0, y: 0.0, z: 0.0 };
        vertices[0].position = ArVertex3D { x: 1.0, y: 1.0, z: 1.0 };

        let mut alphas = [1.0f32; 468];
        MakeupTriangulator::calculate_hairline_blending(&vertices, &mut alphas);

        assert!(alphas[10] < 0.05);
        assert_eq!(alphas[0], 1.0);
    }

    #[test]
    fn test_dynamic_ambient_occlusion() {
        let mut blendshapes = [0.0f32; 52];
        let mut ao = [1.0f32; 468];

        blendshapes[25] = 0.0;
        MakeupTriangulator::calculate_dynamic_ao(&blendshapes, &mut ao);
        assert!((ao[UPPER_LIP_INNER[0]] - 0.15).abs() < 1e-4);

        blendshapes[25] = 1.0;
        MakeupTriangulator::calculate_dynamic_ao(&blendshapes, &mut ao);
        assert!((ao[UPPER_LIP_INNER[0]] - 0.85).abs() < 1e-4);
    }
}
