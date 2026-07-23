# Dokumentasi Modifikasi Fizgravity AR Engine & Client JNI
Dokumen ini merangkum seluruh perubahan teknis yang telah diterapkan pada **Fizgravity AR Engine (Rust Core)** dan **GlowMatch Client Bridge (C++ JNI)** untuk mengaktifkan pelacakan wajah menggunakan model ONNX secara murni (*pure ONNX tracking*) tanpa memerlukan Google ML Kit.

---

## 1. Perubahan pada Rust Core (`New AR Engine`)

### 📄 File: [src/face.rs](file:///e:/APP%20PROJECT/New%20AR%20Engine/src/face.rs)

#### 🚀 A. Penyesuaian Dimensi Tensor Input & Orientasi Potret (`[1, 3, 192, 192]` ➡️ `[1, 256, 256, 3]`)
*   **Masalah**: Kamera ponsel pada AR Try-On selalu berjalan pada orientasi portrait (`480x640`). Pemuatan/pra-pemrosesan frame kamera bawaan Rust menggunakan hardcode landscape `640x480`. Ini memicu pixel scrambling dan stretching sehingga deteksi meleset jauh dari proporsi wajah asli.
*   **Solusi**:
    *   Mengubah parameter input dimensi di dalam `FaceTracker::update` Rust dari `640, 480` menjadi `480, 640`.
    *   Mengubah properti `input_shape` di struct `FaceModelSession` menjadi `[1, 256, 256, 3]`.
    *   Mengubah alokasi buffer tensor di dalam `FaceTracker::update` menjadi `1 * 256 * 256 * 3` elemen `f32`.
    *   Memperbarui fungsi penyusunan tensor input `Value::from_array` agar mengirimkan shape `[1, 256, 256, 3]`.

#### 🎨 B. Algoritma Pra-Pemrosesan Gambar (`preprocess_image`)
*   **Masalah**: Pemrosesan gambar lama memetakan piksel kamera dari format RGB interleaved ke planar CHW (saluran merah dipisah dari hijau dan biru) pada grid `192x192`. Hal ini membuat data citra yang dikirim ke model ONNX acak-acakan dan tidak terbaca oleh neural network. Selain itu, terdapat error tipe inferensi Rust (`E0277`) saat pengaksesan slice array menggunakan tipe data `i32` yang salah.
*   **Solusi**:
    *   Mengubah grid pemetaan bilinear ke resolusi `256x256` piksel.
    *   Menghilangkan konversi planar dan menyalin data piksel secara berurutan (*interleaved* `[R, G, B, R, G, B...]`) ke dalam tensor input sesuai keinginan model.
    *   Melakukan *casting* loop counter dan dimensi `width`/`height` secara ketat ke `usize` saat pengaksesan indeks slice untuk mencegah kegagalan kompilasi.

#### 🛡️ C. Pencegahan Crash / Out-of-Bounds (Aman dari Index Bounds Panic)
*   **Masalah**: Model ONNX mengembalikan beberapa output tensor. Node `"Identity_1"` pada model ini ternyata merupakan klasifikasi bernilai tunggal (panjang `1`), bukan tensor 52 blendshapes. Ketika kode Rust mencoba memetakan output ini ke array blendshapes berukuran 52, thread mengalami *panic* `index out of bounds: the len is 1 but the index is 1` dan membuat aplikasi *force close*.
*   **Solusi**:
    *   Menerapkan pengecekan dinamis yang aman sebelum menyalin data tensor: `let copy_len = blendshape_slice.len().min(FACE_BLENDSHAPES_COUNT)`.
    *   Secara aktif mencoba mengambil output blendshape dari node `"Identity_2"` terlebih dahulu, dan menggunakan `"Identity_1"` hanya sebagai fallback dengan pembatas panjang slice agar aman 100% dari potensi *out of bounds*.

#### 🏷️ D. Perbaikan Nama Node Input Model (`"input_1"` ➡️ `"input"`)
*   **Masalah**: Grafik model ONNX mendefinisikan nama input layer utama dengan key `"input"`. Di sisi lain, kode Rust memanggil sesi inferensi dengan key `"input_1"`. ONNX Runtime membatalkan eksekusi grafik secara internal akibat ketidakcocokan nama ini, membuat landmark wajah bernilai `0.0` (nol) terus-menerus.
*   **Solusi**:
    *   Mengubah key pemanggilan sesi inferensi `session.run(ort::inputs!["input" => input_value])` agar sesuai dengan nama node model.

#### 🪵 E. Penambahan Jembatan FFI Logging (`__android_log_write`)
*   **Masalah**: Eksekusi thread latar belakang (*background thread worker*) di Rust berjalan secara asinkron. Jika terjadi error runtime (seperti kegagalan memuat model atau kegagalan inferensi), error tersebut ditelan secara senyap tanpa memuntahkan log ke Android Logcat.
*   **Solusi**:
    *   Mendeklarasikan FFI eksternal langsung ke fungsi bawaan Android NDK `__android_log_write`.
    *   Membuat fungsi pembantu `android_log(msg)` untuk mencetak log runtime Rust secara transparan ke Logcat dengan tag khusus **`"FizgravityRust"`**.

---

### 📄 File: [src/lib.rs](file:///e:/APP%20PROJECT/New%20AR%20Engine/src/lib.rs)

#### 🚀 F. Penyesuaian Dimensi Lighting Estimator (`640x480` ➡️ `480x640`)
*   **Masalah**: Modul pendeteksi kecerahan sekitar (Spherical Harmonics estimator) menerima parameter frame bertipe landscape hardcoded `640x480` di `fizgravity_engine_update_frame`.
*   **Solusi**:
    *   Mengubah parameter input dimensi lighting estimator menjadi `480, 640` agar estimasi pencahayaan sekitar tidak mengalami skew/pergeseran piksel dahi.

---

## 2. Perubahan pada Client JNI & Renderer (`GlowMatch`)

### 📄 File: [gl_mesh_engine.cpp](file:///e:/APP%20PROJECT/GlowMatch/android/app/src/main/cpp/gl_mesh_engine.cpp)

#### 🔄 A. Penskalaan Koordinat Potret Dinamis ke Layar
*   **Masalah**: JNI sebelumnya selalu mengasumsikan resolusi landscape (`rotatedWidth = 640`, `rotatedHeight = 480`) saat penskalaan viewport screen, yang mengakibatkan distorsi aspek rasio wajah (stretch horizontal ~1.33x) dan memosisikan makeup jauh dari koordinat fisik mata/bibir.
*   **Solusi**:
    *   Menambahkan variabel global `g_rotated_width` dan `g_rotated_height` di JNI.
    *   Mengubah `dest_width` dan `dest_height` di `updateFrameImage` secara dinamis sesuai rotasi layar (`480x640` untuk portrait, `640x480` untuk landscape) agar pixel tracking tegak lurus proporsional 1:1.
    *   Menggunakan `g_rotated_width` dan `g_rotated_height` dinamis sebagai basis perhitungan scale, dx, dan dy pada OpenGL orthographic projection.

---

### 📄 File: [gl_renderer.cpp](file:///e:/APP%20PROJECT/GlowMatch/android/app/src/main/cpp/gl_renderer.cpp)

#### 🧊 B. Penonaktifan Double-Transformation ModelView Matrix
*   **Masalah**: Ketika frame di-update, properti `mHasPoseAndLighting` bernilai `true`. Vertex shader riasan mengalikan koordinat piksel wajah kita dengan matriks translasi/rotasi kamera `modelView`. Akibatnya, wajah riasan mengalami pergeseran ganda dan melompat jauh keluar layar.
*   **Solusi**:
    *   Menonaktifkan manipulasi posisi vertex shader oleh matriks `modelView` ketika koordinat wajah piksel ortografis langsung digambar.

---

## 3. Alur Kompilasi Ulang (Rebuild Pipeline)

Apabila Anda melakukan perubahan di masa depan, ikuti langkah kompilasi berurutan ini:

1.  **Kompilasi Rust Core (arm64-v8a)**:
    Jalankan perintah berikut di folder `New AR Engine`:
    ```bash
    cargo ndk -t arm64-v8a build --release
    ```
2.  **Salin Library `.so`**:
    Salin file biner yang dihasilkan ke direktori JNI client:
    ```bash
    copy "New AR Engine\target\aarch64-linux-android\release\libfizgravity_ar.so" "GlowMatch\android\app\src\main\jniLibs\arm64-v8a\libfizgravity_ar.so"
    ```
3.  **Kompilasi Akhir Aplikasi (Gradle)**:
    Jalankan perintah berikut di folder `GlowMatch\android`:
    ```bash
    gradlew.bat assembleDebug
    ```
