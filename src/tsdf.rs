//! Modul Rekonstruksi Volumetrik Spasial berbasis Hashing Voxel TSDF (Truncated Signed Distance Function).
//! Menyediakan representasi 3D padat dari lingkungan fisik dengan alokasi memori dinamis hemat RAM.

use std::collections::HashMap;
use nalgebra::{Vector3, Matrix3, Rotation3};

/// Ukuran lebar satu voxel block dalam jumlah voxel (8x8x8 = 512 voxels per block).
pub const BLOCK_SIZE: usize = 8;
pub const VOXELS_PER_BLOCK: usize = BLOCK_SIZE * BLOCK_SIZE * BLOCK_SIZE;

/// Elemen voxel dasar penampung nilai jarak bertanda.
#[derive(Debug, Copy, Clone)]
pub struct Voxel {
    /// Nilai SDF (Signed Distance Function) ternormalisasi dalam rentang [-1.0, 1.0].
    /// Melambangkan jarak voxel ke permukaan fisik terdekat.
    pub sdf: f32,
    /// Bobot akumulasi kepercayaan integrasi.
    pub weight: f32,
}

impl Default for Voxel {
    fn default() -> Self {
        Self { sdf: 1.0, weight: 0.0 }
    }
}

/// Koordinat kubik 3D untuk mengindeks satu voxel block dalam ruang global.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlockCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Struktur data voxel block penampung 512 voxel lokal.
pub struct VoxelBlock {
    pub voxels: [Voxel; VOXELS_PER_BLOCK],
}

impl VoxelBlock {
    pub fn new() -> Self {
        Self {
            voxels: [Voxel::default(); VOXELS_PER_BLOCK],
        }
    }
}

/// Struktur utama volume TSDF berbasis Spatial Voxel Hashing.
pub struct TsdfVolume {
    /// Ukuran fisik satu voxel dalam meter (misal 0.01m = 1cm resolution)
    pub voxel_size: f32,
    /// Jarak pemangkasan (truncation distance) SDF (misal 4.0 * voxel_size)
    pub truncation_dist: f32,
    /// Tabel hash spasial yang memetakan koordinat block ke alokasi memori voxel block
    pub hash_table: HashMap<BlockCoords, VoxelBlock>,
}

impl TsdfVolume {
    pub fn new(voxel_size: f32, truncation_dist: f32) -> Self {
        Self {
            voxel_size,
            truncation_dist,
            hash_table: HashMap::new(),
        }
    }

    /// Mendapatkan koordinat voxel dalam ruang global berdasarkan koordinat block dan indeks lokal.
    pub fn get_global_voxel_pos(&self, block_coords: &BlockCoords, local_idx: usize) -> Vector3<f32> {
        let lx = (local_idx % BLOCK_SIZE) as f32;
        let ly = ((local_idx / BLOCK_SIZE) % BLOCK_SIZE) as f32;
        let lz = (local_idx / (BLOCK_SIZE * BLOCK_SIZE)) as f32;

        let block_offset_x = (block_coords.x * BLOCK_SIZE as i32) as f32;
        let block_offset_y = (block_coords.y * BLOCK_SIZE as i32) as f32;
        let block_offset_z = (block_coords.z * BLOCK_SIZE as i32) as f32;

        Vector3::new(
            (block_offset_x + lx) * self.voxel_size,
            (block_offset_y + ly) * self.voxel_size,
            (block_offset_z + lz) * self.voxel_size,
        )
    }

    /// Konversi posisi 3D kontinu global (meter) menjadi koordinat block integer 3D.
    pub fn global_pos_to_block_coords(&self, pos: &Vector3<f32>) -> BlockCoords {
        let block_width = self.voxel_size * BLOCK_SIZE as f32;
        BlockCoords {
            x: (pos.x / block_width).floor() as i32,
            y: (pos.y / block_width).floor() as i32,
            z: (pos.z / block_width).floor() as i32,
        }
    }

    /// Konversi posisi 3D kontinu global (meter) ke indeks lokal voxel (0..512) di dalam block terkait.
    pub fn global_pos_to_local_index(&self, pos: &Vector3<f32>, block_coords: &BlockCoords) -> usize {
        let block_width = self.voxel_size * BLOCK_SIZE as f32;
        let local_x = ((pos.x - (block_coords.x as f32 * block_width)) / self.voxel_size).floor() as usize;
        let local_y = ((pos.y - (block_coords.y as f32 * block_width)) / self.voxel_size).floor() as usize;
        let local_z = ((pos.z - (block_coords.z as f32 * block_width)) / self.voxel_size).floor() as usize;

        // Batasi indeks dalam rentang [0, BLOCK_SIZE-1]
        let local_x = local_x.clamp(0, BLOCK_SIZE - 1);
        let local_y = local_y.clamp(0, BLOCK_SIZE - 1);
        let local_z = local_z.clamp(0, BLOCK_SIZE - 1);

        local_x + local_y * BLOCK_SIZE + local_z * BLOCK_SIZE * BLOCK_SIZE
    }

    /// Mengintegrasikan satu titik kedalaman sensor kamera (depth point) ke dalam volume TSDF.
    ///
    /// * `pt_camera`: Koordinat titik 3D dalam ruang kamera (X, Y, Z).
    /// * `r_gc`: Orientasi kamera ke ruang global.
    /// * `p_c`: Posisi kamera di koordinat global.
    pub fn integrate_point(&mut self, pt_camera: &Vector3<f32>, r_gc: &Rotation3<f32>, p_c: &Vector3<f32>) {
        // Transformasikan titik kedalaman ke ruang global
        let pt_global = p_c + r_gc.matrix() * pt_camera;

        // Sumbu ray penetrasi dari pusat kamera ke titik permukaan target
        let ray_dir = (pt_global - p_c).normalize();
        let ray_length = (pt_global - p_c).norm();

        // Kita telusuri voxel di sepanjang sinar raycast di dalam wilayah truncation band
        // Truncation band adalah area di sekitar permukaan fisik nyata [pt_global - t, pt_global + t]
        let start_dist = ray_length - self.truncation_dist;
        let end_dist = ray_length + self.truncation_dist;

        let step_size = self.voxel_size * 0.5; // Step penelusuran setengah ukuran voxel
        let mut dist = start_dist;

        while dist <= end_dist {
            let current_voxel_pos = p_c + ray_dir * dist;
            let b_coords = self.global_pos_to_block_coords(&current_voxel_pos);
            
            let local_idx = self.global_pos_to_local_index(&current_voxel_pos, &b_coords);
            
            // Dapatkan block voxel secara dinamis dari tabel hash spasial,
            // alokasikan block baru jika block belum pernah disentuh sensor (Voxel Hashing).
            let block = self.hash_table.entry(b_coords).or_insert_with(VoxelBlock::new);

            // Hitung nilai SDF aktual untuk voxel saat ini
            // sdf = jarak permukaan sejati - jarak voxel saat ini ke kamera
            let sdf_val = ray_length - dist;

            // Integrasi jika voxel berada di depan permukaan terpotong (truncation band)
            if sdf_val >= -self.truncation_dist {
                let normalized_sdf = (sdf_val / self.truncation_dist).clamp(-1.0, 1.0);
                
                let voxel = &mut block.voxels[local_idx];
                
                // Bobot baru (bisa dinormalisasi berdasarkan noise kamera)
                let w_new = 1.0;
                let w_old = voxel.weight;

                // Fusi integrasi rata-rata berbobot
                voxel.sdf = (w_old * voxel.sdf + w_new * normalized_sdf) / (w_old + w_new);
                voxel.weight = (w_old + w_new).min(100.0); // Batasi bobot maksimum untuk kestabilan dinamis
            }

            dist += step_size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voxel_hashing_allocation() {
        let mut volume = TsdfVolume::new(0.01, 0.04);
        
        let r_gc = Rotation3::identity();
        let p_c = Vector3::zeros();
        
        // Simulasikan titik permukaan pada jarak 1 meter lurus di depan kamera (Z positif)
        let pt_camera = Vector3::new(0.0, 0.0, 1.0);
        
        // Sebelum integrasi, tabel hash harus kosong
        assert_eq!(volume.hash_table.len(), 0);
        
        // Integrasikan titik
        volume.integrate_point(&pt_camera, &r_gc, &p_c);
        
        // Tabel hash harus mengalokasikan voxel block baru di sepanjang sinar raycast
        assert!(volume.hash_table.len() > 0);
        
        // Cek bahwa permukaan di sekitar 1.0 meter memiliki nilai SDF mendekati 0
        let surface_block_coords = volume.global_pos_to_block_coords(&pt_camera);
        let block = volume.hash_table.get(&surface_block_coords).unwrap();
        let idx = volume.global_pos_to_local_index(&pt_camera, &surface_block_coords);
        
        // Nilai SDF di permukaan harus sangat kecil (mendekati 0)
        assert!(block.voxels[idx].sdf.abs() < 0.2);
        assert!(block.voxels[idx].weight > 0.0);
    }
}
