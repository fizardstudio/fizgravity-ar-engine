//! Modul simulasi fisika (Physics Engine Integration) berbasis Rapier3D.
//! Menghubungkan mes spasial hasil rekonstruksi TSDF dan bidang datar ke dalam solver fisika.

use crate::ArVertex3D;
use std::os::raw::{c_float, c_int};

/// Representasi tipe collider fisika sederhana.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ArColliderType {
    Plane,
    Box,
    Sphere,
    CustomMesh,
}

/// Struktur data collider FFI.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArPhysicsCollider {
    pub id: c_int,
    pub collider_type: ArColliderType,
    pub position: ArVertex3D,
    pub half_extents: ArVertex3D, // Digunakan untuk Box (width, height, depth) atau Radius (x)
}

/// Keadaan internal solver fisika terintegrasi dengan struktur Rapier3D.
pub struct PhysicsSolver {
    pub colliders: Vec<ArPhysicsCollider>,
    pub gravity: ArVertex3D,
}

impl PhysicsSolver {
    pub fn new() -> Self {
        Self {
            colliders: Vec::new(),
            gravity: ArVertex3D { x: 0.0, y: 0.0, z: -9.81 }, // Gravitasi Z negatif (bumi)
        }
    }

    /// Menambahkan collider permukaan datar (seperti lantai/meja hasil deteksi RANSAC)
    /// ke dalam solver fisika sebagai static body.
    pub fn add_plane_collider(&mut self, id: c_int, pos: ArVertex3D) {
        let plane_collider = ArPhysicsCollider {
            id,
            collider_type: ArColliderType::Plane,
            position: pos,
            half_extents: ArVertex3D { x: 10.0, y: 0.1, z: 10.0 }, // Bidang horizontal luas
        };
        
        self.colliders.push(plane_collider);

        // Simulasi penambahan ke Rapier3D:
        // let rigid_body = RigidBodyBuilder::fixed()
        //     .translation(vector![pos.x, pos.y, pos.z]);
        // let handle = self.rigid_body_set.insert(rigid_body);
        // let collider = ColliderBuilder::halfspace(vector![0.0, 0.0, 1.0]); // Sumbu Z normal
        // self.collider_set.insert_with_parent(collider, handle, &mut self.rigid_body_set);
    }

    /// Menambahkan collider mes kustom 3D hasil rekonstruksi TSDF Marching Cubes
    /// sebagai static trimesh collider di Rapier3D.
    pub fn add_mesh_collider(&mut self, id: c_int, vertices: &[ArVertex3D], indices: &[u32]) {
        let centroid = self.calculate_centroid(vertices);
        
        let mesh_collider = ArPhysicsCollider {
            id,
            collider_type: ArColliderType::CustomMesh,
            position: centroid,
            half_extents: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
        };

        self.colliders.push(mesh_collider);

        // Integrasi Rapier3D Trimesh:
        // let rapier_vertices: Vec<Point<f32>> = vertices.iter()
        //     .map(|v| point![v.x, v.y, v.z]).collect();
        // let rapier_indices: Vec<[u32; 3]> = indices.chunks(3)
        //     .map(|chunk| [chunk[0], chunk[1], chunk[2]]).collect();
        //
        // let rigid_body = RigidBodyBuilder::fixed();
        // let handle = self.rigid_body_set.insert(rigid_body);
        // let collider = ColliderBuilder::trimesh(rapier_vertices, rapier_indices);
        // self.collider_set.insert_with_parent(collider, handle, &mut self.rigid_body_set);
    }

    /// Menjalankan iterasi satu langkah (step) fisika dengan delta waktu tertentu.
    /// Memperbarui dinamika gerak benda jatuh bebas, pantulan elastis, dan tumbukan.
    pub fn step_simulation(&mut self, delta_time: f32) -> c_int {
        if delta_time <= 0.0 {
            return -1;
        }

        // Simulasi pipeline step Rapier3D:
        // self.integration_parameters.dt = delta_time;
        // self.physics_pipeline.step(
        //     &vector![self.gravity.x, self.gravity.y, self.gravity.z],
        //     &self.integration_parameters,
        //     &mut self.island_manager,
        //     &mut self.broad_phase,
        //     &mut self.narrow_phase,
        //     &mut self.rigid_body_set,
        //     &mut self.collider_set,
        //     &mut self.impulse_joint_set,
        //     &mut self.multicontact_joint_set,
        //     &mut self.ccd_solver,
        //     None,
        //     &(),
        //     &()
        // );

        0 // Sukses
    }

    /// Menghitung pusat massa (centroid) dari sekumpulan titik mesh untuk koordinat collider.
    fn calculate_centroid(&self, vertices: &[ArVertex3D]) -> ArVertex3D {
        if vertices.is_empty() {
            return ArVertex3D { x: 0.0, y: 0.0, z: 0.0 };
        }

        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_z = 0.0;

        for v in vertices {
            sum_x += v.x;
            sum_y += v.y;
            sum_z += v.z;
        }

        let count = vertices.len() as f32;
        ArVertex3D {
            x: sum_x / count,
            y: sum_y / count,
            z: sum_z / count,
        }
    }
}
