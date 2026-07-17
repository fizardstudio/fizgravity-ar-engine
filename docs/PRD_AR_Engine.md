# Product Requirements Document (PRD) - Fizgravity AR Engine
## Pengembangan Mesin Augmented Reality (AR) Terintegrasi Generasi Baru dari Fizard Studio

---

## 1. Visi & Latar Belakang Produk

### 1.1 Latar Belakang
Pengalaman Augmented Reality (AR) yang imersif membutuhkan integrasi yang mulus antara elemen virtual dan dunia nyata. Kemampuan memadukan objek digital secara presisi bergantung pada pelacakan posisi perangkat (tracking), pemahaman geometri sekitar (scene reconstruction), dan penyesuaian pencahayaan real-time (global illumination estimation). 

Kerangka kerja industri seperti Apple ARKit menetapkan standar dengan fusi sensor berlatensi rendah. Untuk membangun mesin AR berkinerja tinggi dari awal yang dapat berjalan secara independen pada perangkat keras portabel (mobile/wearable) dengan anggaran termal dan komputasi yang sangat ketat, diperlukan arsitektur yang kuat, aman secara memori, dan efisien secara komputasi.

### 1.2 Visi Produk
Membangun AR Engine generasi baru yang:
- **Aman dan Berkinerja Tinggi:** Menggunakan Rust sebagai basis logika komputasi sensorik untuk mencegah *data races* dan kerentanan memori tanpa penalti kinerja (*zero-cost abstractions*).
- **Efisien Komputasi:** Mengoptimalkan algoritma pelacakan dengan pendekatan penyaringan (MSCKF) alih-alih optimisasi graf global yang boros daya, agar baterai perangkat seluler tidak cepat habis.
- **Fotorealistik & Responsif:** Menyediakan oklusi spasial real-time sub-milimeter dan estimasi pencahayaan global instan menggunakan kompresi matematis Harmonik Sferis (*Spherical Harmonics*).
- **Sarat Fitur AR Modern:** Mengintegrasikan kecerdasan buatan (AI/ML) untuk segmentasi semantik, pelacakan gestur tangan/wajah 3D, serta interaksi fisika deterministik layaknya Apple ARKit, Meta Spark, TikTok Effect House, dan Niantic Lightship.
- **Terobosan Teknologi Revolusioner:** Menyediakan visualisasi fotorealistik ruangan menggunakan real-time 3D Gaussian Splatting, kestabilan tracking ekstrim dengan Kalman Filter Adaptif (AEKF), pemetaan kolaboratif P2P tanpa internet, dan mekanisme neural tracking recovery instan.
- **Teknologi Orde-Selanjutnya (Next-Gen):** Menghasilkan pantulan specular logam fotorealistik via Neural Specular Reflection Fields (NSRF), oklusi batas tepian sub-piksel tajam via Bilateral Guided Depth-Completion, dan latensi visual 0 milidetik via prediktif Late Latching.

---

## 2. Arsitektur Sistem & Interoperabilitas

Mesin AR dirancang dengan model hibrida untuk memanfaatkan keunggulan sistem keamanan Rust dan ekosistem grafis C++.

```mermaid
graph TD
    SubGraph1[Modul Sensor & Inti AR (Rust)]
    SubGraph2[Jembatan Interoperabilitas (C-ABI FFI)]
    SubGraph3[Mesin Grafis / Render Pipeline (C++)]

    IMU[Sensor IMU @1000Hz] -->|Data Raw| SubGraph1
    Camera[Kamera @60Hz] -->|Bingkai Video| SubGraph1
    LiDAR[LiDAR / Depth Sensor] -->|Bilateral Depth Fusion| SubGraph1
    
    SubGraph1 -->|MSCKF VIO Tracker| Tracker[Estimasi Pose & Lintasan]
    SubGraph1 -->|TSDF Voxel Hashing| Mesh[Mes Rekonstruksi 3D]
    SubGraph1 -->|Spherical Harmonics| Light[Koefisien Iluminasi (27 Floats)]
    SubGraph1 -->|ONNX Runtime Inference| AI[Wajah, Tangan, Segmentasi & NSRF Reflection]
    SubGraph1 -->|Rapier3D Solver| Physics[Simulasi Tabrakan & Gravitasi]
    SubGraph1 -->|3D Gaussian Fitting| 3DGS[Peta Spasial Fotorealistik 3DGS]
    SubGraph1 -->|libp2p Channel| P2P[Desentralisasi Collaborative Map]
    SubGraph1 -->|Predictive Extrapolator| Warping[Late Latching Warp Matrix]
    
    Tracker -->|FFI StatusOr C++| SubGraph2
    Mesh -->|FFI C-ABI| SubGraph2
    Light -->|FFI Array| SubGraph2
    AI -->|FFI Structs/Texture| SubGraph2
    Physics -->|FFI Transform| SubGraph2
    3DGS -->|FFI Splat Array| SubGraph2
    P2P -->|FFI Sync Command| SubGraph2
    Warping -->|FFI Warping Matrix| SubGraph2
    
    SubGraph2 -->|Pointer & Wrapper| SubGraph3
    SubGraph3 -->|Unreal Engine / Custom Render| Render[Rendering Akhir Layar]
```

### 2.1 Modul Inti AR (Rust)
Seluruh logika yang berhubungan dengan fusi sensor, tracking EKF, rekonstruksi 3D (mesh & 3DGS), inferensi ML (NSRF), komputasi fisika, transmisi P2P, dan komputasi warping prediksi late latching dijalankan di atas Rust. Hal ini menjamin keamanan memori thread-safe melintasi batas runtime asynchronous.

### 2.2 Modul Jembatan Grafis (C++ Bridge)
Integrasi dengan perrender grafis dipertahankan menggunakan antarmuka C++ melalui C-ABI. Seluruh pertukaran buffer array (seperti mesh segitiga atau splat Gaussians) diekspos melalui alokasi pointer statis yang dikelola oleh siklus hidup Rust untuk menghindari double-free.

---

## 3. Spesifikasi Fitur Inti Teknologis

### 3.1 Visual-Inertial Odometry (VIO) berbasis MSCKF
Estimasi pose kamera dilakukan melalui pendekatan fusi sensor erat (tightly-coupled) antara kamera dengan unit IMU.
- **MSCKF:** Memiliki kompleksitas linier $O(N)$ terhadap jumlah fitur karena titik fitur 3D tidak dimasukkan ke dalam vektor keadaan EKF, melainkan diproyeksikan ke ruang nol (*null-space projection*), menjaga CPU tetap stabil pada 60Hz.
- **EKF State:**
  $$x_k = [x_I^T, x_{C_1}^T, x_{C_2}^T, \dots, x_{C_N}^T, x_W^T]^T$$
- **Pra-Integrasi IMU:** Dilakukan pada manifold SO(3) untuk memadatkan data inersia temporal menjadi delta pose relatif independen.

---

### 3.2 Pemahaman Spasial & Rekonstruksi Lingkungan (Scene Reconstruction)
- **RANSAC & MDL:** Ekstraksi bidang horizontal/vertical yang didukung filter Minimum Description Length (MDL) untuk mencegah visualisasi bidang tumpang tindih.
- **TSDF Voxel Hashing:** Pemetaan spasial 3D padat yang menggunakan tabel hash spasial untuk efisiensi memori, diubah menjadi mes poligon melalui GPU Marching Cubes.

---

### 3.3 Estimasi Iluminasi Global Berbasis Harmonik Sferis (Spherical Harmonics)
Mengekstraksi warna ambient ruangan nyata menjadi **koefisien Harmonik Sferis Orde 2** (9 koefisien warna per saluran RGB, total 27 floats). Ini mengonversi integral irradiance hemisfer yang rumit menjadi perkalian dot-product shader $O(1)$ di GPU.

---

### 3.4 Sinkronisasi Sensor & Kalibrasi Temporal
Menggunakan kalibrasi online dinamis untuk mengestimasi offset waktu sensor ($t_d$) serta menerapkan kompensasi interpolasi pose untuk rolling shutter kamera seluler.

---

### 3.5 Pipeline Inferensi ML & Segmentasi Semantik
Menggunakan **ONNX Runtime** dengan akselerasi perangkat keras seluler (NNAPI/CoreML). Menyediakan klasifikasi objek spasial (*semantic segmentation*) serta *People Occlusion* berlatensi rendah untuk merender konten AR di belakang manusia secara realistis.

---

### 3.6 Pelacakan 3D Wajah (Face Mesh), Tangan (Hand), & Tubuh (Body)
- **3D Face Mesh:** Pelacakan 468 titik wajah dan estimasi 52 parameter blendshapes ekspresi wajah.
- **3D Hand Tracking:** Pelacakan 21 titik sendi jari tangan untuk gestur kontrol interaktif.
- **3D Body Tracking:** Estimasi pose kerangka tubuh 33 joints untuk fiting aset 3D (try-on).

---

### 3.7 Integrasi Engine Fisika Real-Time (Rapier3D)
Integrasi solver **Rapier3D** di Rust untuk mensimulasikan dinamika rigid-body, tabrakan, dan gravitasi secara deterministik di atas mes TSDF/bidang RANSAC yang dinamis.

---

### 3.8 Visual Positioning System (VPS) & Jangkar Persisten
Penyinkronan point cloud lokal hasil VIO dengan database peta global cloud (geospatial markers) untuk penempatan jangkar AR pada koordinat geospasial absolut (Lat, Long, Alt) dengan presisi sentimeter.

---

### 3.9 Pipeline Fusi Spasial 3D Gaussian Splatting (3DGS)
Untuk melampaui visualisasi mes kasar tradisional, mesin ini mengadopsi fusi volumetrik **3D Gaussian Splatting (3DGS)** secara real-time.
- **Gaussian Fitting:** Setiap titik spasial yang dipetakan oleh sensor fusi diekstrak parameter elipsoidnya: posisi $x$, skala $s$, rotasi $q$ (kuaternion), opasitas $\alpha$, dan koefisien warna Harmonik Sferis.
- **Fotorealisme Spasial:** Rendering 3DGS menangkap efek transparansi, refraksi, dan pantulan cahaya dari ruangan fisik asli (misalnya kaca atau logam) secara instan, menghasilkan kloning ruangan 3D yang jauh lebih nyata daripada mesh poligon biasa.

---

### 3.10 Kalman Filter Adaptif (AEKF) dengan Pengendali Kovarians Dinamis
- **Dynamic Covariance Control:** Mesin mengimplementasikan **Adaptive EKF (AEKF)**. Daripada menggunakan matriks noise konvarian $Q$ (process noise) dan $R$ (measurement noise) yang kaku, AEKF menghitung rasio inovasi residual secara berkala.
- **Suhu & Getaran:** Bila sensor giroskop mendeteksi getaran frekuensi tinggi atau fluktuasi termal pada chip, AEKF menaikkan kovarians $Q$ secara dinamis untuk menyaring noise, mencegah anomali gambar AR melompat (*glitching*).

---

### 3.11 Kolaborasi Spasial P2P Desentralisasi (Collaborative Map)
Mesin AR ini mendukung pemetaan kolaboratif antar-perangkat secara lokal tanpa bergantung pada koneksi internet.
- **P2P Communication:** Menggunakan pustaka **libp2p** di Rust melalui koneksi Wi-Fi Direct atau Bluetooth LE lokal.
- **Voxel Hash Key Exchange:** Perangkat hanya saling mengirimkan kunci voxel hash (*Voxel Hash Keys*) dan delta stempel pose teranyar. Perangkat penerima kemudian menggabungkan (*merge*) peta spasial lokalnya secara instan.

---

### 3.12 Neural Tracking Fallback (SuperPoint & LightGlue)
- **Feature Extraction & Matching:** Mesin mengaktifkan model inferensi ONNX **SuperPoint** (ekstraktor fitur neural) dan **LightGlue** (matcher neural) secara paralel.
- **Instant Relocalization:** Ketika frontend VIO KLT konvensional mendeteksi kegagalan pelacakan, sub-thread ML langsung memproses frame kamera terakhir dan mencocokkannya dengan peta referensi spasial abstrak di memori untuk relokalisasi pose instan.

---

### 3.13 Estimasi SV-HDR & Neural Specular Reflection Fields (NSRF)
Meningkatkan fusi pencahayaan difus Harmonik Sferis dengan estimasi pantulan cermin mengkilap (*specular highlights*):
- **Spatially-Varying HDR Inference:** Menggunakan model CNN encoder-decoder ringan di Rust core untuk memprediksi peta lingkungan SV-HDR dari frame kamera SDR tunggal secara real-time.
- **NSRF Rendering:** Mengonstruksi representasi medan refleksi spekular dinamis. Objek virtual berbahan krom, kaca, atau logam mengkilap dapat memantulkan objek di sekitarnya secara akurat mengikuti sudut pandang kamera (*view-dependent reflection*).

---

### 3.14 Fusi Bilateral Guided Depth-Completion (Oklusi Sub-Piksel)
Mengatasi batas oklusi pecah-pecah (*pixel bleeding*) di sekitar tepian benda tipis:
- **Guided Fusion Filter:** Rust core menggabungkan data titik kedalaman LiDAR yang renggang dengan kontur garis tepi beresolusi tinggi (*high-resolution RGB edges*) dari sensor kamera.
- **Bilateral Filter Solver:** Mengoperasikan filter bilateral spasial-temporal untuk menghasilkan peta kedalaman padat sub-piksel secara instan. Hal ini memastikan batas oklusi (misalnya objek AR terhalang oleh helai rambut manusia atau kaki meja) ter-render dengan sangat tajam tanpa artefak visual.

---

### 3.15 Late Latching & Motion Extrapolator Prediktif
Mengeliminasi jeda visual (*lagging*) objek virtual ketika kamera digeser dengan cepat:
- **Inertial Feed-Forward:** Menggunakan akselerometer dan giroskop IMU berkecepatan 1000Hz untuk mengekstrapolasi dan memproyeksikan lintasan gerakan kamera 16-33 ms ke depan (memprediksi waktu penyegaran piksel layar berikutnya).
- **Shader Warping (Late Latching):** Tepat sebelum layar ponsel menampilkan frame buffer, shader perrender C++ menerapkan warping koreksi proyeksi pada citra berdasarkan matriks pose masa depan yang diprediksi. Langkah ini memotong jeda latensi visual hingga mendekati **0 milidetik**, mengunci objek virtual di dunia nyata dengan kokoh.

---

## 4. Kebutuhan Non-Fungsional (Kinerja & Batasan)

| Metrik Kinerja | Kriteria Penerimaan | Keterangan |
| :--- | :--- | :--- |
| **Frekuensi Pelacakan VIO** | $\ge 60$ Hz | Harus sinkron dengan kecepatan pembaruan layar ponsel. |
| **Latensi Ujung-ke-Ujung** | $\le 10$ ms | Jeda waktu dari gerakan fisik nyata hingga pembaruan grafis di layar. |
| **Akurasi Pelacakan Spasial** | Drift $\le 1.5\%$ | Akumulasi kesalahan pergeseran posisi terhadap jarak tempuh linier. |
| **Siklus Konsumsi CPU** | $\le 15\%$ | Batas alokasi penggunaan CPU pada chipset mobile kelas menengah (misal Snapdragon 7 Gen 1). |
| **Batas Anggaran Memori** | $\le 120$ MB | Alokasi RAM konstan untuk pipeline pelacakan VIO dan voxel hashing. |
| **Latensi Inferensi ML** | $\le 16$ ms | Waktu yang dialokasikan untuk pemrosesan deteksi wajah/tangan di GPU/NPU per frame. |
| **Apparent Latency (Late Latching)** | $\approx 0$ ms | Latensi persepsi visual yang dirasakan pengguna saat menggeser kamera dengan cepat. |

---

## 5. Tumpukan Teknologi (Tech Stack)

- **Bahasa Pemrograman Utama:** Rust (Edisi 2021) untuk modul inti pelacakan (`ar-core-engine`).
- **Antarmuka Binding:** C++ (standar C++20) untuk integrasi perrender grafis.
- **Aljabar Linier & Matematika:** Pustaka `nalgebra` di Rust (dengan optimisasi SIMD enabled).
- **Akselerasi Grafis & Compute:** WebGPU / Vulkan Compute shader untuk optimisasi Marching Cubes dan TSDF voxel hashing.
- **Mesin ML Inference:** ONNX Runtime Rust bindings dengan akselerasi TensorRT/CoreML/NNAPI.
- **Engine Fisika:** `rapier3d` (Rust-native physics library).
- **Komunikasi Lokal P2P:** `libp2p` Rust library.
- **Kompilasi Silang (Cross-Compilation):** Dukungan toolchain `cargo-ndk` untuk target Android (aarch64-linux-android) dan target iOS (aarch64-apple-ios).
