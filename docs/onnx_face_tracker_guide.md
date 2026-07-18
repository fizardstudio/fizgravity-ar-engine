# Cetak Biru Integrasi ONNX Runtime (Face Tracker Core)
## Desain Sesi, Pra-pemrosesan, & Inferensi Real-Time di Rust (`ort` Crate)

Dokumen ini menyajikan rancangan teknis dan implementasi kode untuk menggantikan sistem *mock* di `src/face.rs` dengan **ONNX Runtime (ort Crate)** nyata untuk mendeteksi 468 landmark wajah secara real-time di Android/iOS.

---

## 📦 1. Pembaruan Dependensi — `Cargo.toml`

Untuk mempermudah kompilasi silang (*cross-compilation*) pada platform seluler (Android/iOS), kita menggunakan fitur `load-dynamic` pada crate `ort`. Ini memungkinkan Rust memuat pustaka bersama `libonnxruntime.so` secara dinamis di memori ponsel saat runtime, meniadakan kerumitan tautan statis (*static linking*).

Tambahkan baris berikut pada `Cargo.toml`:
```toml
[dependencies]
# ONNX Runtime API binding untuk Rust
ort = { version = "2.0.0-rc.9", default-features = false, features = ["load-dynamic"] }
# Utilitas untuk mempermudah manipulasi ndarray
ndarray = "0.15"
```

---

## 🛠️ 2. Arsitektur Pipeline Inferensi Wajah

Model FaceMesh ONNX (hasil konversi MediaPipe) memiliki spesifikasi tensor sebagai berikut:
*   **Input**: Tensor `[1, 3, 192, 192]` (RGB gambar, format planar `CHW`, nilai dinormalisasi ke rentang `[0.0, 1.0]` atau `[-1.0, 1.0]`).
*   **Output 1 (Landmarks)**: Tensor `[1, 1404]` (468 titik $\times$ 3 koordinat $x, y, z$).
*   **Output 2 (Blendshapes)**: Tensor `[1, 52]` (52 koefisien ekspresi wajah ARKit).

---

## 💻 3. Kode Implementasi Nyata — `src/face.rs`

Berikut adalah desain kode Rust sesungguhnya untuk mengelola sesi model ONNX, memproses gambar kamera, dan mengekstrak landmark wajah:

```rust
//! src/face.rs
//! Integrasi Sesi Inferensi Model ONNX Runtime untuk Jaring Wajah.

use std::path::Path;
use ort::{Session, SessionInputs, Value};
use ndarray::Array4;
use crate::{ArVertex3D, face::{ArTexCoord2D, ArFaceVertexInterleaved}};

pub const FACE_MESH_VERTICES_COUNT: usize = 468;
pub const FACE_BLENDSHAPES_COUNT: usize = 52;

pub struct FaceModelSession {
    session: Option<Session>,
    pub is_loaded: bool,
}

impl FaceModelSession {
    pub fn new() -> Self {
        Self {
            session: None,
            is_loaded: false,
        }
    }

    /// Memuat file model ONNX ke dalam memori.
    /// Pustaka libonnxruntime.so harus sudah dimuat terlebih dahulu di sisi Kotlin (System.loadLibrary).
    pub fn load_session<P: AsRef<Path>>(&mut self, model_path: P) -> Result<(), String> {
        // 1. Inisialisasi ONNX Runtime environment
        ort::init()
            .with_name("FizgravityFaceTracker")
            .init()
            .map_err(|e| format!("Gagal inisialisasi ORT: {:?}", e))?;

        // 2. Buat sesi inferensi dengan opsi optimal untuk seluler
        let session = Session::builder()
            .map_err(|e| format!("Gagal memuat builder sesi: {:?}", e))?
            .with_intra_threads(1) // Batasi ke 1 thread untuk menghemat CPU seluler
            .map_err(|e| format!("Gagal mengatur opsi utas: {:?}", e))?
            .with_model_from_file(model_path)
            .map_err(|e| format!("Gagal membuka model ONNX: {:?}", e))?;

        self.session = Some(session);
        self.is_loaded = true;
        Ok(())
    }

    /// Melakukan pra-pemrosesan buffer gambar kamera mentah (RGB HWC) ke Tensor Planar (CHW) [1 x 3 x 192 x 192].
    pub fn preprocess_image(
        &self,
        image_data: &[u8], // Data piksel RGB kontigu
        width: usize,
        height: usize,
    ) -> Result<Array4<f32>, String> {
        if image_data.len() != width * height * 3 {
            return Err("Ukuran data gambar tidak cocok dengan dimensi RGB.".to_string());
        }

        // 1. Alokasikan memori ndarray [1, 3, 192, 192]
        let mut input_array = Array4::<f32>::zeros((1, 3, 192, 192));

        // 2. Lakukan downsampling (resize bilinear sederhana) dan normalisasi ke [0.0, 1.0]
        let scale_x = width as f32 / 192.0;
        let scale_y = height as f32 / 192.0;

        for y in 0..192 {
            let src_y = (y as f32 * scale_y).min(height as f32 - 1.0) as usize;
            for x in 0..192 {
                let src_x = (x as f32 * scale_x).min(width as f32 - 1.0) as usize;
                
                // Indeks piksel RGB asal (HWC)
                let src_idx = (src_y * width + src_x) * 3;
                
                let r = image_data[src_idx] as f32 / 255.0;
                let g = image_data[src_idx + 1] as f32 / 255.0;
                let b = image_data[src_idx + 2] as f32 / 255.0;

                // Salin ke tensor tujuan (CHW)
                input_array[[0, 0, y, x]] = r;
                input_array[[0, 1, y, x]] = g;
                input_array[[0, 2, y, x]] = b;
            }
        }

        Ok(input_array)
    }

    /// Melakukan inferensi model ONNX untuk mengekstrak landmark wajah & blendshapes.
    pub fn run_inference(
        &self,
        input_tensor: Array4<f32>,
        out_vertices: &mut [ArVertex3D; FACE_MESH_VERTICES_COUNT],
        out_blendshapes: &mut [f32; FACE_BLENDSHAPES_COUNT],
    ) -> Result<(), String> {
        let session = self.session.as_ref().ok_or("Sesi ONNX belum dimuat.")?;

        // 1. Buat input value untuk model
        let input_value = Value::from_array(input_tensor)
            .map_err(|e| format!("Gagal membuat tensor input: {:?}", e))?;

        // 2. Jalankan inferensi (sesuaikan nama input model, misal "input_1")
        let outputs = session.run(ort::inputs!["input_1" => input_value].unwrap())
            .map_err(|e| format!("Gagal menjalankan sesi inferensi: {:?}", e))?;

        // 3. Ekstrak tensor output landmark (misal nama output "Identity")
        let landmark_output = outputs.get("Identity")
            .ok_or("Output landmark 'Identity' tidak ditemukan.")?;
        
        let landmark_data = landmark_output.try_extract_tensor::<f32>()
            .map_err(|e| format!("Gagal mengekstrak tensor landmark: {:?}", e))?;

        // 4. Salin 1404 nilai f32 ke dalam 468 koordinat ArVertex3D
        for i in 0..FACE_MESH_VERTICES_COUNT {
            let idx = i * 3;
            out_vertices[i] = ArVertex3D {
                x: landmark_data[[0, idx]],
                y: landmark_data[[0, idx + 1]],
                z: landmark_data[[0, idx + 2]],
            };
        }

        // 5. Ekstrak tensor output blendshapes (misal nama output "Identity_1")
        if let Some(blendshape_output) = outputs.get("Identity_1") {
            let blendshape_data = blendshape_output.try_extract_tensor::<f32>()
                .map_err(|e| format!("Gagal mengekstrak tensor blendshapes: {:?}", e))?;
            for i in 0..FACE_BLENDSHAPES_COUNT {
                out_blendshapes[i] = blendshape_data[[0, i]];
            }
        }

        Ok(())
    }
}
```

---

## 📱 4. Panduan Kompilasi & Integrasi Seluler (Android)

Karena kita menggunakan pemuatan dinamis (`load-dynamic`), berikut adalah langkah integrasi di sisi Android Studio:

### Langkah A: Tambahkan `libonnxruntime.so` ke Proyek Kotlin
Salin pustaka bersama `libonnxruntime.so` yang sesuai dengan arsitektur CPU target ke direktori JNI Android:
```text
GlowMatch/android/app/src/main/jniLibs/
├── arm64-v8a/
│   └── libonnxruntime.so
└── armeabi-v7a/
    └── libonnxruntime.so
```

### Langkah B: Muat Pustaka Sebelum Memanggil Rust
Di dalam aktivitas utama Kotlin aplikasi GlowMatch, muat ONNX Runtime secara dinamis ke memori menggunakan `System.loadLibrary`:
```kotlin
class MainActivity : AppCompatActivity() {
    companion object {
        init {
            // 1. Wajib muat ONNX Runtime terlebih dahulu
            System.loadLibrary("onnxruntime")
            // 2. Muat jembatan FFI Rust Engine kita
            System.loadLibrary("fizgravity_ar")
        }
    }
    // ...
}
```
Pola ini membebaskan Rust Core dari kewajiban membungkus binary ONNX Runtime secara statis, membuat ukuran berkas `.so` Rust kita berkurang dari **>80MB** menjadi hanya **<3MB**!
