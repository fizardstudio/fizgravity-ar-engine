# Laporan Audit Integrasi: Fizgravity AR Engine di Aplikasi GlowMatch

Dokumen ini berisi hasil analisis mendalam (*deep analysis*) terhadap integrasi antara **Fizgravity AR Engine** (Rust) dan aplikasi **GlowMatch** (Flutter/Kotlin/C++).

---

## 🔍 1. Status Integrasi Saat Ini (Apa yang Sudah Digunakan)

Berdasarkan analisis file sumber `GLMeshEngine.kt`, `gl_mesh_engine.cpp`, dan `GlowMatchCameraView.kt`:

1.  **Inisialisasi Core Engine**:
    *   Fungsi `GLMeshEngine.initEngine()` berhasil memicu alokasi internal `fizgravity_engine_init()`.
2.  **Konversi Gambar Kamera**:
    *   `GLMeshEngine.updateFrameImage()` mengonversi buffer gambar kamera **YUV420 ke RGB888** secara optimal menggunakan aritmatika pointer C++ dan mengirimkannya ke `fizgravity_engine_update_frame`.
3.  **Penyaluran Koordinat ML Kit (Bypass)**:
    *   Aplikasi mendeteksi wajah menggunakan Google ML Kit, lalu menyalurkan koordinatnya ke Rust menggunakan `GLMeshEngine.setFaceMesh(landmarks, blendshapes)`.
4.  **Pengambilan Koordinat Ter-ekstrapolasi**:
    *   Aplikasi memanggil `GLMeshEngine.getFaceMesh()` untuk mengambil kembali koordinat wajah hasil olahan Rust dan menggambarnya ke layar melalui `overlayView.updateFaceMesh()`.

---

## 🚫 2. Apa yang BELUM Digunakan / Hilang (Penyebab Utama Filter Tertinggal)

Kami menemukan celah integrasi kritis yang menyebabkan filter AR GlowMatch mengalami lag/tertinggal:

### A. Aliran Data Sensor IMU Putus Total (Penyebab Utama Lag)
*   **Temuan di Kotlin (`GlowMatchCameraView.kt`)**: Tidak ada kodingan untuk mendengarkan sensor inersia telepon (`SensorManager` atau giroskop) di sisi Android.
*   **Temuan di C++ (`gl_mesh_engine.cpp#L196`)**: Fungsi update frame dipanggil dengan parameter IMU bernilai **`nullptr`**:
    ```cpp
    int32_t res = fizgravity_engine_update_frame(gFizEngine, 0.0f, rgb_buffer, nullptr, nullptr, nullptr);
    ```
*   **Akibat**: Blok ekstrapolator Late Latching (`MotionExtrapolator`) di Rust **tidak pernah berjalan** karena tidak menerima data giroskop fisik perangkat. Rust hanya mengembalikan koordinat mentah dari ML Kit tanpa prediksi pergerakan masa depan.

### B. Estimasi Pencahayaan Diabaikan (`out_lighting` = null)
*   **Temuan**: Parameter `out_lighting` di C++ diisi **`nullptr`**. 
*   **Akibat**: Hasil estimasi Harmonik Sferis dari Rust tidak pernah diambil dan tidak pernah dikirim ke shader OpenGL.

### C. Shader Grafis OpenGL Sangat Sederhana (Flat Shader)
*   **Temuan di C++ (`gl_renderer.cpp`)**: Shader makeup hanya menggunakan warna solid biasa tanpa pencahayaan:
    ```glsl
    fragColor = vec4(uColor, uOpacity);
    ```
*   **Akibat**: Riasan lipstik dan blush-on digambar rata tanpa efek 3D, pantulan cahaya (*glossy*), maupun GGX specular. Variabel `mBlushColor` dan `mFoundationColor` diset tetapi **tidak pernah dipakai** di rendering utama.

### D. Modul AI Diagnostik & Harmonisasi Belum Dipanggil
*   **Temuan**: Modul Rust `skin_analyzer.rs`, `color_harmonizer.rs`, dan `eye_contacts.rs` belum memiliki jembatan FFI di `src/lib.rs` dan belum dipanggil sama sekali oleh Flutter/Kotlin.

---

## 🛠️ 3. Panduan Perbaikan untuk Tim Pengembang GlowMatch

Untuk menghilangkan lag dan mengaktifkan fitur premium AR, tim pengembang GlowMatch harus melakukan langkah-langkah berikut:

### Langkah 1: Kirim Data Sensor Giroskop dari Kotlin ke C++
Implementasikan `SensorEventListener` di Kotlin untuk menangkap kecepatan sudut giroskop:
```kotlin
// Di GlowMatchCameraView.kt
val sensorManager = context.getSystemService(Context.SENSOR_SERVICE) as SensorManager
val gyroSensor = sensorManager.getDefaultSensor(Sensor.TYPE_GYROSCOPE)

// Dengarkan data giroskop dan salurkan ke JNI C++
```

### Langkah 2: Hubungkan Data IMU ke `fizgravity_engine_update_frame`
Ubah pemanggilan di `gl_mesh_engine.cpp` agar menyertakan data IMU giroskop ($x, y, z$) dan akselerometer ($x, y, z$) berukuran 6 float (24 bytes):
```cpp
float imu_data[6] = { gyro_x, gyro_y, gyro_z, acc_x, acc_y, acc_z };
fizgravity_engine_update_frame(gFizEngine, timestamp, rgb_buffer, imu_data, &out_pose, &out_lighting);
```

### Langkah 3: Gunakan Data Pencahayaan SH di Shader Makeup
Ambil koefisien `out_lighting` dari Rust, kirim sebagai serangkaian uniform floats ke shader, lalu hitung bayangan makeup berdasarkan arah cahaya lampu kamar nyata!
