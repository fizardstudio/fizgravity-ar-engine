//! Modul pelacakan Wajah (Face Mesh Tracking) berbasis AI.
//! Mengintegrasikan inferensi model ONNX untuk mengekstrak 468 koordinat wajah 3D
//! dan 52 parameter blendshapes ekspresi wajah secara real-time.

use std::os::raw::{c_float, c_int};
use crate::ArVertex3D;
use std::path::Path;

/// Ukuran konstanta jaring wajah (468 vertices standar MediaPipe/ARKit).
pub const FACE_MESH_VERTICES_COUNT: usize = 468;
/// Jumlah parameter blendshapes wajah standar (52 ARKit blendshapes).
pub const FACE_BLENDSHAPES_COUNT: usize = 52;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArTexCoord2D {
    pub u: c_float,
    pub v: c_float,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArFaceVertexInterleaved {
    pub position: ArVertex3D,
    pub normal: ArVertex3D,
    pub uv: ArTexCoord2D,
}

/// Struktur data eksposisi FFI untuk representasi jaring wajah 3D interleaved.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArFaceMesh {
    /// Array vertex ter-interleave kontigu (Posisi 3D + UV 2D = 20 bytes per vertex)
    pub vertices: [ArFaceVertexInterleaved; FACE_MESH_VERTICES_COUNT],
    /// Koefisien kekuatan blendshape untuk ekspresi wajah (52 parameter).
    pub blendshapes: [c_float; FACE_BLENDSHAPES_COUNT],
}

/// Struktur Manajemen Sesi Model ONNX Wajah.
/// Mengenkapsulasi alokasi memori tensor input/output untuk model FaceMesh.
pub struct FaceModelSession {
    pub model_path: String,
    pub is_loaded: bool,
    pub input_shape: [usize; 4], // [batch, height, width, channels] -> [1, 256, 256, 3]
    pub session: Option<ort::session::Session>,
}

#[cfg(target_os = "android")]
extern "C" {
    fn __android_log_write(prio: i32, tag: *const u8, text: *const u8) -> i32;
}

#[cfg(target_os = "android")]
fn android_log(msg: &str) {
    let tag = b"FizgravityRust\0";
    let text = format!("{}\0", msg);
    unsafe {
        __android_log_write(3, tag.as_ptr(), text.as_ptr());
    }
}

#[cfg(not(target_os = "android"))]
fn android_log(msg: &str) {
    println!("{}", msg);
}

impl FaceModelSession {
    pub fn new(path: &str) -> Self {
        Self {
            model_path: path.to_string(),
            is_loaded: false,
            input_shape: [1, 256, 256, 3],
            session: None,
        }
    }

    /// Memuat file model ONNX ke dalam memori.
    /// Pustaka libonnxruntime.so harus sudah dimuat terlebih dahulu di sisi Kotlin (System.loadLibrary).
    pub fn load_session(&mut self) -> Result<(), String> {
        if !std::path::Path::new(&self.model_path).exists() {
            let err = format!("Model file not found at: {}", self.model_path);
            android_log(&err);
            return Err(err);
        }

        let _ = ort::init()
            .with_name("FizgravityFaceTracker")
            .commit();

        let session = ort::session::Session::builder()
            .map_err(|e| {
                let err = format!("Gagal memuat builder: {:?}", e);
                android_log(&err);
                err
            })?
            .with_intra_threads(1) // Batasi ke 1 thread untuk menghemat CPU seluler
            .map_err(|e| {
                let err = format!("Gagal set thread: {:?}", e);
                android_log(&err);
                err
            })?
            .commit_from_file(&self.model_path)
            .map_err(|e| {
                let err = format!("Gagal memuat model: {:?}", e);
                android_log(&err);
                err
            })?;

        self.session = Some(session);
        self.is_loaded = true;
        android_log("Sesi model ONNX Wajah berhasil dimuat dengan sukses!");
        Ok(())
    }

    /// Melakukan pra-pemrosesan gambar kamera mentah ke bentuk Tensor input normalisasi.
    ///
    /// * `image_data`: Pointer mentah ke data RGB buffer kamera.
    /// * `width` & `height`: Dimensi gambar kamera.
    /// * `out_tensor`: Buffer output tempat tensor input ternormalisasi [1 x 256 x 256 x 3] akan disalin.
    pub fn preprocess_image(
        &self,
        image_data: *const std::ffi::c_void,
        width: i32,
        height: i32,
        out_tensor: &mut [f32],
        face_box_opt: Option<[i32; 4]>,
    ) -> Result<(), &'static str> {
        if image_data.is_null() {
            android_log("preprocess_image: Data gambar null!");
            return Err("Data gambar null");
        }
        
        android_log(&format!("preprocess_image: Dimulai dengan ukuran {}x{}", width, height));

        let total_pixels = (width * height) as usize;
        let pixels = unsafe { std::slice::from_raw_parts(image_data as *const u8, total_pixels * 3) };

        // Tentukan area crop: jika tidak ada kotak, fallback ke full frame
        let (crop_x, crop_y, crop_w, crop_h) = match face_box_opt {
            Some([bx, by, bw, bh]) => {
                // Expand the bounding box slightly by 25% to ensure the whole head (chin to forehead) is included
                let pad_w = (bw as f32 * 0.25) as i32;
                let pad_h = (bh as f32 * 0.25) as i32;
                let nx = bx - pad_w / 2;
                let ny = by - pad_h / 2;
                let nw = bw + pad_w;
                let nh = bh + pad_h;
                
                // Clamp to screen bounds safely to prevent usize underflow panics
                let cx1 = nx.max(0);
                let cy1 = ny.max(0);
                let cx2 = (nx + nw).min(width).max(cx1);
                let cy2 = (ny + nh).min(height).max(cy1);
                
                (cx1 as usize, cy1 as usize, (cx2 - cx1) as usize, (cy2 - cy1) as usize)
            }
            None => (0, 0, width as usize, height as usize),
        };

        android_log(&format!("preprocess_image: crop params -> x:{}, y:{}, w:{}, h:{}", crop_x, crop_y, crop_w, crop_h));

        // Konversi area crop dari RGB Interleaved ke tensor input 256x256
        for y in 0..256 {
            let src_y = crop_y + ((y * crop_h) / 256);
            for x in 0..256 {
                let src_x = crop_x + ((x * crop_w) / 256);
                
                // Safety bound check just in case
                let safe_y = src_y.min(height as usize - 1);
                let safe_x = src_x.min(width as usize - 1);
                
                let src_idx = (safe_y * width as usize + safe_x) * 3;

                let r = pixels[src_idx + 0] as f32 / 255.0;
                let g = pixels[src_idx + 1] as f32 / 255.0;
                let b = pixels[src_idx + 2] as f32 / 255.0;

                let dest_idx = (y * 256 + x) * 3;
                out_tensor[dest_idx + 0] = r;
                out_tensor[dest_idx + 1] = g;
                out_tensor[dest_idx + 2] = b;
            }
        }

        Ok(())
    }

    pub fn run_inference(
        &mut self,
        input_tensor: &[f32],
        out_mesh: &mut ArFaceMesh,
    ) -> Result<(), &'static str> {
        if !self.is_loaded || self.session.is_none() {
            android_log("run_inference: Model belum dimuat, menggunakan FALLBACK mock face!");
            // Mode Fallback: Jika model atau libonnxruntime tidak ada (misal pada test environment)
            for i in 0..FACE_MESH_VERTICES_COUNT {
                let angle = (i as f32) * std::f32::consts::PI / 234.0;
                let pos = ArVertex3D {
                    x: angle.cos() * 0.1,
                    y: angle.sin() * 0.15,
                    z: (i as f32 * 0.0001) - 0.05,
                };
                out_mesh.vertices[i] = ArFaceVertexInterleaved {
                    position: pos,
                    normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
                    uv: crate::canonical_uv::CANONICAL_UV[i],
                };
            }
            compute_face_normals(&mut out_mesh.vertices);
            for i in 0..FACE_BLENDSHAPES_COUNT {
                out_mesh.blendshapes[i] = 0.0;
            }
            out_mesh.blendshapes[0] = 0.85; // leftEyeBlink mock
            return Ok(());
        }

        android_log("run_inference: Mulai persiapan input");
        let session = self.session.as_mut().unwrap();

        let input_value = ort::value::Value::from_array((vec![1, 256, 256, 3], input_tensor.to_vec()))
            .map_err(|e| {
                let err = format!("Gagal membuat tensor input: {:?}", e);
                android_log(&err);
                "Gagal membuat tensor input."
            })?;

        android_log("run_inference: Mulai session.run ONNX");
        let outputs = session.run(ort::inputs!["input" => input_value])
            .map_err(|e| {
                let err = format!("Gagal menjalankan sesi inferensi: {:?}", e);
                android_log(&err);
                "Gagal menjalankan sesi inferensi."
            })?;

        android_log("run_inference: Selesai session.run, mengambil output");
        let landmark_output = outputs.get("Identity")
            .ok_or_else(|| {
                android_log("Output landmark 'Identity' tidak ditemukan.");
                "Output landmark 'Identity' tidak ditemukan."
            })?;
        
        let (_shape, landmark_slice) = landmark_output.try_extract_tensor::<f32>()
            .map_err(|e| {
                let err = format!("Gagal mengekstrak tensor landmark: {:?}", e);
                android_log(&err);
                "Gagal mengekstrak tensor landmark."
            })?;

        android_log("run_inference: Mulai salin output ke out_mesh");
        let num_vertices = (landmark_slice.len() / 3).min(FACE_MESH_VERTICES_COUNT);
        for i in 0..num_vertices {
            let idx = i * 3;
            out_mesh.vertices[i].position = ArVertex3D {
                x: landmark_slice[idx],
                y: landmark_slice[idx + 1],
                z: landmark_slice[idx + 2],
            };
            out_mesh.vertices[i].uv = crate::canonical_uv::CANONICAL_UV[i];
        }

        // Jika data landmark yang diperoleh lebih sedikit dari jumlah mesh standar, bersihkan sisanya
        for i in num_vertices..FACE_MESH_VERTICES_COUNT {
            out_mesh.vertices[i].position = ArVertex3D { x: 0.0, y: 0.0, z: 0.0 };
        }

        compute_face_normals(&mut out_mesh.vertices);

        // Ekstrak tensor output blendshapes secara aman (coba Identity_2 dahulu, fallback ke Identity_1)
        let blendshape_node = outputs.get("Identity_2").or_else(|| outputs.get("Identity_1"));
        if let Some(blendshape_output) = blendshape_node {
            if let Ok((_bs_shape, blendshape_slice)) = blendshape_output.try_extract_tensor::<f32>() {
                let copy_len = blendshape_slice.len().min(FACE_BLENDSHAPES_COUNT);
                for i in 0..copy_len {
                    out_mesh.blendshapes[i] = blendshape_slice[i];
                }
                for i in copy_len..FACE_BLENDSHAPES_COUNT {
                    out_mesh.blendshapes[i] = 0.0;
                }
            }
        }

        Ok(())
    }
}

/// Struktur manajemen internal pelacak wajah.
pub struct FaceTracker {
    pub current_mesh: ArFaceMesh,
    pub session: FaceModelSession,
}

impl FaceTracker {
    pub fn new(model_path: &str) -> Self {
        let mut session = FaceModelSession::new(model_path);
        let _ = session.load_session();

        Self {
            current_mesh: ArFaceMesh {
                vertices: [ArFaceVertexInterleaved {
                    position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
                    normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
                    uv: ArTexCoord2D { u: 0.0, v: 0.0 },
                }; FACE_MESH_VERTICES_COUNT],
                blendshapes: [0.0; FACE_BLENDSHAPES_COUNT],
            },
            session,
        }
    }

    /// Memperbarui jaring wajah berdasarkan frame video kamera teranyar.
    pub fn update(&mut self, image_data: *const std::ffi::c_void, width: i32, height: i32, face_box_opt: Option<[i32; 4]>) -> c_int {
        if image_data.is_null() {
            return -1;
        }

        // Inisialisasi buffer tensor input [1 x 256 x 256 x 3]
        let mut input_tensor = vec![0.0f32; 1 * 256 * 256 * 3];
        
        // 1. Jalankan pra-pemrosesan gambar
        if self.session.preprocess_image(image_data, width, height, &mut input_tensor, face_box_opt).is_err() {
            return -2;
        }

        // 2. Jalankan inferensi neural network
        if self.session.run_inference(&input_tensor, &mut self.current_mesh).is_err() {
            return -3;
        }

        // 3. Transformasi balik dari koordinat crop (0..256) ke koordinat normalisasi full frame (-1..1)
        if let Some([bx, by, bw, bh]) = face_box_opt {
            let pad_w = (bw as f32 * 0.25) as i32;
            let pad_h = (bh as f32 * 0.25) as i32;
            let nx = bx - pad_w / 2;
            let ny = by - pad_h / 2;
            let nw = bw + pad_w;
            let nh = bh + pad_h;
            
            // Clamp to screen bounds safely
            let cx1 = nx.max(0);
            let cy1 = ny.max(0);
            let cx2 = (nx + nw).min(width).max(cx1); // width is dynamic
            let cy2 = (ny + nh).min(height).max(cy1); // height is dynamic
            
            let nx_clamped = cx1 as f32;
            let ny_clamped = cy1 as f32;
            let nw_clamped = (cx2 - cx1) as f32;
            let nh_clamped = (cy2 - cy1) as f32;
            
            for i in 0..self.current_mesh.vertices.len() {
                let p = &mut self.current_mesh.vertices[i].position;
                // ONNX menghasilkan 0..256
                let x_crop_norm = p.x / 256.0;
                let y_crop_norm = p.y / 256.0;
                let z_crop_norm = p.z / 256.0;

                // Konversi ke piksel kamera asli
                let x_pixel = nx_clamped + (x_crop_norm * nw_clamped);
                let y_pixel = ny_clamped + (y_crop_norm * nh_clamped);
                let z_pixel = z_crop_norm * nw_clamped; // scale Z based on face width

                // Normalisasi ke 0..1 (relatif terhadap ukuran layar)
                // width dan height dinamis
                p.x = x_pixel / (width as f32);
                p.y = y_pixel / (height as f32);
                p.z = z_pixel / (width as f32);
            }
        }

        0 // Sukses
    }
}

/// Indeks batas rahang bawah MediaPipe untuk ekstrapolasi leher virtual
pub const JAWLINE_INDICES: [usize; 17] = [361, 288, 397, 365, 379, 378, 400, 377, 152, 148, 176, 149, 150, 136, 172, 58, 132];

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ArNeckMesh {
    pub vertices: [ArFaceVertexInterleaved; 34], // 17 points * 2 rows
    pub indices: [u32; 96], // 16 segments * 2 triangles * 3 indices
}

pub struct ArNeckExtender;

impl ArNeckExtender {
    pub fn extrapolate_neck(face_vertices: &[ArFaceVertexInterleaved; FACE_MESH_VERTICES_COUNT]) -> ArNeckMesh {
        let v_forehead = face_vertices[10].position;
        let v_chin = face_vertices[152].position;
        
        let dx = v_chin.x - v_forehead.x;
        let dy = v_chin.y - v_forehead.y;
        let dz = v_chin.z - v_forehead.z;
        let len = (dx*dx + dy*dy + dz*dz).sqrt();
        
        let u_down = if len > 1e-5 {
            ArVertex3D { x: dx / len, y: dy / len, z: dz / len }
        } else {
            ArVertex3D { x: 0.0, y: -1.0, z: 0.0 }
        };
        
        let mut neck_vertices = [ArFaceVertexInterleaved {
            position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
            normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
            uv: ArTexCoord2D { u: 0.0, v: 0.0 },
        }; 34];
        
        let l1 = 0.05; // Row 1 distance
        let l2 = 0.10; // Row 2 distance
        
        for idx in 0..17 {
            let jaw_idx = JAWLINE_INDICES[idx];
            let v_jaw = face_vertices[jaw_idx];
            
            // Row 1
            neck_vertices[idx] = ArFaceVertexInterleaved {
                position: ArVertex3D {
                    x: v_jaw.position.x + l1 * u_down.x,
                    y: v_jaw.position.y + l1 * u_down.y,
                    z: v_jaw.position.z + l1 * u_down.z,
                },
                normal: v_jaw.normal,
                uv: ArTexCoord2D {
                    u: v_jaw.uv.u,
                    v: v_jaw.uv.v + 0.1,
                },
            };
            
            // Row 2
            neck_vertices[idx + 17] = ArFaceVertexInterleaved {
                position: ArVertex3D {
                    x: v_jaw.position.x + l2 * u_down.x,
                    y: v_jaw.position.y + l2 * u_down.y,
                    z: v_jaw.position.z + l2 * u_down.z,
                },
                normal: v_jaw.normal,
                uv: ArTexCoord2D {
                    u: v_jaw.uv.u,
                    v: v_jaw.uv.v + 0.2,
                },
            };
        }
        
        let mut indices = [0u32; 96];
        let mut ptr = 0;
        
        for i in 0..16 {
            indices[ptr] = i as u32;
            indices[ptr + 1] = (i + 1) as u32;
            indices[ptr + 2] = (i + 17) as u32;
            
            indices[ptr + 3] = (i + 17) as u32;
            indices[ptr + 4] = (i + 1) as u32;
            indices[ptr + 5] = (i + 18) as u32;
            
            ptr += 6;
        }
        
        ArNeckMesh {
            vertices: neck_vertices,
            indices,
        }
    }
}

/// Menghitung vektor normal permukaan wajah secara radial ellipsoid
pub fn compute_face_normals(vertices: &mut [ArFaceVertexInterleaved; FACE_MESH_VERTICES_COUNT]) {
    let mut center = ArVertex3D { x: 0.0, y: 0.0, z: 0.0 };
    for v in vertices.iter() {
        center.x += v.position.x;
        center.y += v.position.y;
        center.z += v.position.z;
    }
    center.x /= FACE_MESH_VERTICES_COUNT as f32;
    center.y /= FACE_MESH_VERTICES_COUNT as f32;
    center.z /= FACE_MESH_VERTICES_COUNT as f32;

    for v in vertices.iter_mut() {
        let dx = v.position.x - center.x;
        let dy = v.position.y - center.y;
        let dz = v.position.z - center.z;
        let len = (dx*dx + dy*dy + dz*dz).sqrt();
        if len > 1e-5 {
            v.normal = ArVertex3D {
                x: dx / len,
                y: dy / len,
                z: dz / len,
            };
        } else {
            v.normal = ArVertex3D { x: 0.0, y: 0.0, z: 1.0 };
        }
    }
}

#[cfg(test)]
mod face_tests {
    use super::*;

    #[test]
    fn test_compute_normals_unit_length() {
        let mut vertices = [ArFaceVertexInterleaved {
            position: ArVertex3D { x: 1.0, y: 2.0, z: 3.0 },
            normal: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
            uv: ArTexCoord2D { u: 0.0, v: 0.0 },
        }; FACE_MESH_VERTICES_COUNT];

        // Buat satu vertex berbeda posisi agar center tidak sama dengan posisinya
        vertices[0].position = ArVertex3D { x: 10.0, y: 20.0, z: 30.0 };

        compute_face_normals(&mut vertices);

        // Verifikasi normal adalah unit vector (panjang = 1.0)
        let n = vertices[0].normal;
        let len = (n.x*n.x + n.y*n.y + n.z*n.z).sqrt();
        assert!((len - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_onnx_tracker_fallback() {
        let mut session = FaceModelSession::new("invalid_model_path.onnx");
        // Harus mengembalikan error secara aman karena file tidak ada
        let res = session.load_session();
        assert!(res.is_err());
        assert!(!session.is_loaded);

        // preprocess_image harus tetap aman digunakan
        let dummy_image = [128u8; 640 * 480 * 3];
        let mut out_tensor = vec![0.0f32; 256 * 256 * 3];
        let prep_res = session.preprocess_image(dummy_image.as_ptr() as *const std::ffi::c_void, 480, 640, &mut out_tensor, None);
        assert!(prep_res.is_ok());

        // run_inference harus fallback ke model mock
        let mut out_mesh = ArFaceMesh {
            vertices: [ArFaceVertexInterleaved {
                position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
                normal: ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
                uv: ArTexCoord2D { u: 0.0, v: 0.0 },
            }; FACE_MESH_VERTICES_COUNT],
            blendshapes: [0.0; FACE_BLENDSHAPES_COUNT],
        };
        let inf_res = session.run_inference(&out_tensor, &mut out_mesh);
        assert!(inf_res.is_ok());
        
        // Verifikasi landmark 0 dipetakan menggunakan Canonical UV
        assert!((out_mesh.vertices[0].uv.u - 0.427942).abs() < 1e-4);
    }
}
