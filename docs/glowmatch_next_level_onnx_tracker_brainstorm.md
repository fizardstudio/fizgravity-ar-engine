# Brainstorming: Peningkatan ONNX Face Tracker ke Tingkat Dunia (Next-Level)
## Strategi Akselerasi Perangkat Keras, Pipeline ROI Dinamis, & Optimalisasi Banuba/Snapchat

Untuk bersaing langsung dengan mesin AR papan atas (seperti **Snapchat Lens Studio**, **Banuba**, dan **YouCam Makeup**), kita tidak boleh hanya sekadar memuat model ONNX dan menjalankan inferensi standar di CPU. Kita harus mengoptimalkan seluruh aspek matematika, alokasi memori, dan akselerasi perangkat keras ponsel.

Berikut adalah 4 pilar rekayasa tingkat lanjut (*next-level*) untuk menyempurnakan **ONNX Face Tracker** kita:

---

## ⚡ 1. Two-Stage Pipeline: BlazeFace (Detektor) & FaceMesh (Penjejak ROI)

Model FaceMesh `192x192` tidak bisa mendeteksi wajah langsung dari gambar kamera penuh yang lebar (misal $1920 \times 1080$), karena wajah akan terlihat gepeng saat di-resize, membuat deteksi gagal.

### Desain Pipeline yang Benar (Standar Snapchat):
1.  **Stage 1: BlazeFace (Face Detection) — Berjalan Lambat (O(10-30 FPS))**
    *   Hanya berjalan di awal aplikasi, atau saat pelacakan wajah hilang (*tracking lost*).
    *   BlazeFace (sangat kecil, input $128 \times 128$) memindai gambar kamera penuh untuk mencari kotak pembatas (*bounding box*) wajah dan 6 titik jangkar (mata, hidung, mulut, telinga).
2.  **Stage 2: FaceMesh (Region of Interest - ROI Tracking) — Berjalan Cepat (60 FPS)**
    *   Pada frame berikutnya, kita tidak menjalankan BlazeFace lagi! 
    *   Kita mengambil hasil koordinat wajah dari frame sebelumnya, menghitung kotak pembatas yang sedikit dilebarkan (margin $\approx 25\%$), lalu memutar kotak tersebut mengikuti sudut kemiringan mata (*roll angle*).
    *   Potong (*crop*) gambar kamera menggunakan kotak berputar tersebut, lalu kirim hasil crop ke model FaceMesh.
    *   **Keuntungan**: Menghemat daya CPU hingga **$70\%$** dan memotong latensi secara dramatis karena area piksel yang diproses sangat kecil!

---

## 🚀 2. Akselerasi Perangkat Keras Seluler (Execution Providers)

Inferensi di CPU ponsel akan membuat perangkat cepat panas dan baterai boros. Kita harus memaksa ONNX Runtime menggunakan NPU (Neural Processing Unit) atau GPU seluler.

### Implementasi Execution Providers di Rust (`ort` crate):
Kita mengonfigurasi Sessi ONNX di Rust untuk memprioritaskan akselerasi perangkat keras bawaan OS:
```rust
use ort::{Session, SessionInputs, Value};

let session = Session::builder()?
    .with_execution_providers([
        // 1. Android NPU (Neural Networks API - NNAPI)
        ort::NNAPIExecutionProvider::default().build(),
        
        // 2. iOS NPU (CoreML Delegate)
        ort::CoreMLExecutionProvider::default().build(),
        
        // 3. Fallback ke CPU Multi-threading
        ort::CPUExecutionProvider::default().build()
    ])?
    .with_model_from_file("facemesh_quantized.onnx")?;
```
Dengan NNAPI/CoreML, kalkulasi tensor dialihkan dari CPU ke chip akselerator khusus (seperti Apple A-series Neural Engine atau Qualcomm Hexagon NPU), menghasilkan inferensi sub-milidetik (**$< 5\text{ms}$** per frame).

---

## 📉 3. Model Quantization (FP32 ke INT8)

Model FaceMesh standar menggunakan floating-point 32-bit (`FP32`) dengan ukuran file $\sim 18\text{MB}$.

### Modifikasi:
*   Kita melakukan **Post-Training Quantization (PTQ)** untuk mengonversi bobot model dari `FP32` ke integer 8-bit (`INT8`).
*   **Dampak**:
    1.  Ukuran file model turun sebesar **$75\%$** (dari $18\text{MB}$ menjadi hanya **$\sim 4.5\text{MB}$**).
    2.  Konsumsi bandwidth memori berkurang jauh, mempercepat waktu eksekusi di GPU/NPU seluler hingga **$3\text{x}$ lipat** dengan penurunan akurasi yang hampir tidak terlihat ($< 0.5\%$).

---

## 🎯 4. Vertex-Dependent One-Euro Filter (Peredam Getaran Cerdas)

Filter One-Euro yang kita miliki saat ini menyamaratakan stabilisasi ke seluruh 468 vertex wajah dengan koefisien pemulusan ($\beta$) yang sama. Ini tidak optimal.

### Modifikasi Optimalisasi:
Kita membagi jaring wajah menjadi 3 zona filter dengan karakteristik dinamis yang berbeda:
1.  **Zona Riasan Sensitif (Eyeliner, Lips, Eyebrows)**:
    *   Memerlukan pemulusan tinggi untuk mencegah lipstik/eyeliner terlihat bergetar (*wobbly*).
    *   Set parameter $\beta \approx 0.001$, $f_c \approx 0.5\text{Hz}$ (pemulusan sangat halus).
2.  **Zona Pose Struktur (Nose Bridge, Cheekbones)**:
    *   Memerlukan responsivitas tinggi agar koordinat jaring langsung mengikuti gerakan kepala tanpa ada efek keterlambatan (*lag*).
    *   Set parameter $\beta \approx 0.1$, $f_c \approx 1.5\text{Hz}$ (respons super cepat).
3.  **Zona Dahi & Rahang (Hairline, Chin)**:
    *   Set parameter menengah.

Dengan menerapkan **Vertex-Dependent Filter**, kosmetik digital akan terlihat super stabil saat mata pengguna berkedip kecil, namun riasan tetap melekat kencang tanpa delay saat kepala bergerak cepat!
