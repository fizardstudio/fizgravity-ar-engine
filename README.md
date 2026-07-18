# Fizgravity AR Engine

Fizgravity AR Engine adalah kit pengembangan perangkat lunak (**Spatial Computing & Augmented Reality SDK**) lintas-platform berkinerja tinggi yang dikembangkan secara kolaboratif oleh **Fizard Studio** dan **Antigravity**. 

Mesin ini ditulis dalam bahasa **Rust** untuk menjamin keamanan memori (*thread-safe*), efisiensi baterai yang tinggi, dan performa tanpa *Garbage Collection*, serta diekspos melalui antarmuka FFI C-ABI standar ke **C++** untuk kemudahan integrasi dengan render engine modern (Unreal Engine, Unity, custom Vulkan/Metal/OpenGL app) dan aplikasi kecantikan **GlowMatch**.

---

## 🚀 Fitur Utama & Keunggulan Rekayasa

### 1. Pelacakan Inersia & Matematika VIO Tingkat Lanjut
*   **MSCKF VIO Konsisten Geometri:** Multi-State Constraint Kalman Filter yang melacak pergerakan kamera dengan kompleksitas waktu $O(N)$ linier terhadap jumlah fitur visual.
*   **Penyelarasan Frame Percepatan EKF:** Memproyeksikan percepatan rata-rata dari koordinat keyframe ($b_i$) ke body frame aktif saat ini ($b_k$) menggunakan transpose rotasi relatif:
    $$\mathbf{a}_{local} = \Delta R^T \cdot \mathbf{a}_{b_i}$$
    Ini memastikan blok matriks transisi kecepatan-orientasi $\Phi_{v, \theta} = -R_{gi} \cdot [\mathbf{a}_{local}]_\times \cdot dt$ konsisten secara spasial, mencegah drift/divergensi filter EKF.
*   **Joseph Form Covariance Update:** Menjamin stabilitas numerik floating-point `f32` dengan menjaga kovariansi $P$ tetap simetris dan positif-semi-definit.
*   **Dynamic Late Latching (Inertial Extrapolator):** Memprediksi pose kamera masa depan menggunakan sensor IMU 1000Hz dengan horizon delta waktu dinamis.
*   **Dynamic Time Delta Tracking:** Secara otomatis mengukur interval waktu frame-ke-frame nyata menggunakan jam sistem presisi tinggi (`std::time::Instant`) untuk menstabilkan jaring wajah secara adaptif meskipun terjadi pelambatan rendering GPU (frame drops).

### 2. Spesialisasi Kecantikan & Riasan Premium (GlowMatch)
*   **Canonical MediaPipe 2D UV Map Template:** Pemetaan tekstur 2D anatomis statis standard industri untuk mencegah distorsi riasan. Riasan digital seperti PNG eyeliner, lipstik, dan eye shadow akan melar-kerut secara dinamis mengikuti proporsi wajah unik setiap orang.
*   **Continuous Face-Width Auto-Calibration Solver:** Mengoreksi parameter fokal lensa secara dinamis untuk meniadakan distorsi lensa ultra-wide menggunakan rasio lebar-tinggi wajah antropometri ($13.5\text{ cm}$) dan sudut penolehan kepala (yaw-angle foreshortening compensation).
*   **Topology-Guided Dynamic Ambient Occlusion:** Memberikan shading kedalaman wajah pada celah bibir secara real-time yang dimodulasikan secara dinamis mengikuti blendshape `mouthOpen` ($B_{25}$) untuk menghindari visual datar.
*   **Skin Detail High-Pass Blending Shader:** Mempertahankan pori-pori kulit asli pengguna di bawah lapisan foundation virtual.
*   **ROI Local Contrast LBP & Sobel Wrinkles Analyzer:** Menganalisis tingkat kehalusan kulit (roughness) di area pipi dan intensitas kerutan (wrinkles) di dahi menggunakan filter LBP lokal satu-langkah (*one-pass loop*) dan operator gradien Sobel.
*   **McCamy Color Temperature Ambient Relighting:** Mengestimasi suhu warna lampu ruangan (Kelvin) menggunakan formula McCamy dan intensitas cahaya dari koefisien Spherical Harmonics untuk mengoreksi bias warna riasan.
*   **Leaky Integrator Specular Glitter Shimmer:** Menggeser koordinat noise voronoi highlighter berdasarkan orientasi layar HP (portrait/landscape) dan rotasi roll/pitch giroskop dengan peluruhan eksponensial berbasis waktu nyata (FPS-invariant decay).
*   **Forehead Geodesic Blending Mask:** Menghasilkan batas gradasi foundation yang memudar halus (smoothstep) ketika mendekati garis rambut dekat dahi (hairline) menggunakan optimalisasi perbandingan jarak kuadrat (*squared distance*).

---

## 📁 Struktur Repositori

```text
Fizgravity AR Engine/
├── docs/
│   ├── PRD_AR_Engine.md            # Spesifikasi Persyaratan Produk lengkap
│   ├── Project_Plan_Roadmap.md     # Peta jalan pengembangan modular 6 Fase
│   ├── glowmatch_hyper_realistic_ar_makeup_research.md # Cetak biru formula optik premium
│   ├── glowmatch_integration_audit.md # Panduan FFI IMU untuk Kotlin JNI & C++
│   ├── glowmatch_zero_latency_tracking_brainstorm.md # Cetak biru pelacakan tanpa latensi
│   └── next_gen_ar_modules_roadmap.md # Daftar periksa modul AR kecantikan GlowMatch
├── include/
│   └── ar_bridge.h                 # Header jembatan interoperabilitas C++ (C-ABI FFI)
├── src/
│   ├── face.rs                     # Modul AI Face mesh, normal, & interleaved VBO
│   ├── canonical_uv.rs             # Template koordinat 2D wajah Canonical MediaPipe standar
│   ├── calibration.rs              # Modul auto-kalibrasi fokal dinamis terkompensasi yaw
│   ├── texture_analyzer.rs         # Modul analisis kesehatan kulit LBP satu-langkah & Sobel
│   ├── makeup_triangulator.rs      # Triangulasi Delaunay bibir, hairline & dynamic AO
│   ├── skin_analyzer.rs            # ITA° skin classifier klinis (atan) & Fitzpatrick
│   ├── lighting.rs                 # Estimator 9 koefisien SH & suhu warna McCamy
│   ├── lib.rs                      # Titik masuk ekspor FFI & tipe data FFI C-ABI
│   ├── stabilizer.rs               # Modul One-Euro filter dengan delta waktu adaptif
│   └── msckf.rs                    # Filter navigasi sliding window MSCKF EKF
└── Cargo.toml                      # Konfigurasi dependensi Cargo Rust
```

---

## 💄 Panduan Integrasi FFI Jembatan C-ABI (GlowMatch)

Berikut adalah panduan fungsi FFI C-ABI yang diekspos oleh `Fizgravity AR Engine` untuk digunakan di Kotlin JNI (Android) atau Objective-C/C++ (iOS).

### 1. Inisialisasi & Set Data Kamera
```cpp
// 1. Inisialisasi instansi mesin
void* engine = fizgravity_engine_init();

// 2. Set koordinat landmark wajah dari ML Kit (Auto-stabilisasi & estimasi normal VBO terpicu)
// vertices: 468 titik landmark wajah (dalam piksel)
// blendshapes: 52 koefisien blendshape ARKit
// FUNGSI INI OTOMATIS MENGUKUR ELAPSED TIME (dt) SYSTEM UNTUK ONE-EURO FILTER
int res = fizgravity_engine_set_face_mesh(engine, vertices_ptr, blendshapes_ptr);
```

### 2. Estimasi & Koreksi Cahaya Sekitar (Ambient Relighting)
```cpp
float out_temp = 0.0f;      // Suhu warna dalam Kelvin (misal: 3000K untuk hangat, 6500K untuk daylight)
float out_intensity = 0.0f; // Tingkat kecerahan (0.0 - 1.0)

// Mengambil warna lampu sekitar untuk diumpankan ke shader pencocokan riasan
fizgravity_engine_get_ambient_cct_and_intensity(engine, &out_temp, &out_intensity);
```

### 3. Analisis Kesehatan Kulit Scanner Kosmetik
```cpp
float roughness = 0.0f; // Tingkat kekasaran kulit pipi (0.0 = super mulus, 1.0 = kasar)
float wrinkles = 0.0f;  // Tingkat kerutan dahi (0.0 = kencang, 1.0 = keriput)

// image_rgb: pointer ke buffer gambar RGB kamera (640x480 piksel)
fizgravity_engine_analyze_skin_health(image_rgb, 640, 480, &roughness, &wrinkles);
```

### 4. Kilau Shimmer Fisik Berbasis Giroskop (IMU Specular Shimmer)
```cpp
float shift_x = 0.0f;
float shift_y = 0.0f;

// Hitung pergeseran koordinat tekstur gliter di dahi/pipi fragment shader
// gyro_x, gyro_y: data sensor kecepatan sudut giroskop
// dt: interval waktu frame render
// screen_rotation_degrees: rotasi orientasi layar HP (0, 90, 180, 270)
// SISTEM MENGGUNAKAN FPS-INVARIANT EXPONENTIAL DECAY UNTUK STABILITAS FISIK
fizgravity_engine_calculate_glitter_shimmer_shift(
    engine, gyro_x, gyro_y, gyro_z, dt, screen_rotation_degrees, &shift_x, &shift_y
);
```

### 5. Masking Batas Halus Tepi Rambut (Hairline Soft-Blending Mask)
```cpp
float alphas[468]; // Array alpha per-vertex wajah

// Mengisi nilai alpha (0.0 - 1.0) untuk ke-468 vertex wajah
// Vertex dekat garis hairline dahi akan memiliki alpha memudar halus mendekati 0.0
fizgravity_engine_calculate_hairline_blending(engine, alphas, 468);
```

### 6. Auto-Kalibrasi & Penyelaras Lensa HP (Lens Distortion Corrector)
```cpp
float focal_length = 0.0f; // Estimasi parameter focal length piksel terkalibrasi

// Menghitung focal length secara dinamis terkompensasi penolehan kepala (yaw)
// depth_z: estimasi jarak kepala dari kamera (dalam meter)
fizgravity_engine_update_auto_calibration(engine, 640.0f, 480.0f, depth_z, &focal_length);
```

### 7. Topology-Guided Dynamic Ambient Occlusion
```cpp
float ao[468]; // Array koefisien AO per-vertex wajah

// Menghitung oklusi celah bibir secara dinamis mengikuti blendshape membuka/menutup mulut
fizgravity_engine_calculate_dynamic_ao(engine, ao, 468);
```

---

## 🛠️ Cara Membangun Proyek (Build Guide)

### 1. Menjalankan Tes Unit & FFI
```bash
cargo test
```

### 2. Kompilasi Target Android (Shared Library)
Kompilasi pustaka `.so` untuk ARM64 Android menggunakan `cargo-ndk`:
```bash
cargo ndk --target aarch64-linux-android build --release
```

---

## 📝 Lisensi
Proyek ini dilisensikan dan dimiliki sepenuhnya oleh **Fizard Studio** & **Antigravity Developer**.
