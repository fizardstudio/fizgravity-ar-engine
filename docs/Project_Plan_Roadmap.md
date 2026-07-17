# Peta Jalan & Rencana Proyek Fizgravity AR Engine

Dokumen ini memetakan fase-fase rekayasa untuk membangun mesin AR hybrid Rust-C++ baru sesuai spesifikasi teknis revolusioner dan next-gen (3DGS, AEKF, P2P collaborative map, Neural Tracking, NSRF, Depth-Completion, dan Late Latching) yang tertuang dalam Product Requirements Document (PRD).

---

## 1. Fase Pengembangan Utama & Milestones

Sesuai arahan instruksi terbaru, estimasi batasan waktu pengerjaan diabaikan untuk mengedepankan kualitas dan keandalan arsitektur modular. Proyek dibagi menjadi 6 fase sekuensial logis berikut:

```
Fase 1: Inti Matematika, Manifold SO(3) & IMU Pre-Integration
  └── Fase 2: Filter Navigasi MSCKF, Kalman Adaptif (AEKF) & Warping Prediktif
        └── Fase 3: Rekonstruksi TSDF, Bidang MDL, Fusi 3DGS & Guided Depth-Completion
              └── Fase 4: Integrasi ONNX Runtime, Face/Hand Mesh & Neural Tracking Fallback
                    └── Fase 5: Estimasi SV-HDR (NSRF), Fisika Rapier3D & Kolaborasi P2P
                          └── Fase 6: C++ FFI Bridge, Custom Shader SH & Optimasi Ponsel
```

---

## 2. Rincian Fase Pengembangan & Kriteria Keberhasilan

### Fase 1: Inti Matematika, Manifold SO(3) & IMU Pre-Integration
* **Tujuan:** Membangun fondasi manipulasi matriks aljabar dan pra-integrasi sensor inersia.
* **Tugas Utama:**
  1. Setup Cargo workspace proyek dan dependensi aljabar linier `nalgebra`.
  2. Implementasi representasi manifold SO(3) dan grup Lie untuk perhitungan rotasi 3D spasial presisi.
  3. Membangun model pra-integrasi IMU diskrit (Picard integration) untuk memproses percepatan sudut dan linier sensor.
  4. Pengujian unit (unit tests) matematika untuk membantikan hilangnya drift inersial di bawah bias fabrikasi konstan.
* **Milestone 1:** Pustaka Rust inti dapat mengintegrasikan data akselerometer dan giroskop mentah menjadi delta posisi-orientasi relatif secara deterministik.

### Fase 2: Filter Navigasi MSCKF, Kalman Adaptif (AEKF) & Warping Prediktif
* **Tujuan:** Merealisasikan estimasi lintasan kamera VIO yang tangguh dan memotong jeda visual hingga 0ms.
* **Tugas Utama:**
  1. Integrasi frontend pelacak fitur optik KLT (Kanade-Lucas-Tomasi) pada thread optik terpisah.
  2. Implementasi EKF state vector dengan sliding window pose kamera masa lalu dan parameter ekstrinsik online.
  3. Formulasi kontrol kovarians dinamis (**Adaptive EKF**) untuk menyesuaikan matriks derau $Q$ dan $R$ terhadap getaran fisik dan noise visual.
  4. Implementasi **Predictive Motion Extrapolator** berbasis feed-forward IMU 1000Hz untuk estimasi pose kamera 16-33 ms ke depan (Late Latching).
* **Milestone 2:** Modul pelacakan MSCKF-AEKF dapat melacak lintasan secara presisi, meminimalisir drift, dan menyuplai matriks proyeksi warping prediksi masa depan.

### Fase 3: Rekonstruksi TSDF, Bidang MDL, Fusi 3DGS & Guided Depth-Completion
* **Tujuan:** Menyediakan peta rekonstruksi fotorealistik dan oklusi tepian objek yang sangat tajam.
* **Tugas Utama:**
  1. Deteksi bidang datar geometris menggunakan evaluasi RANSAC dan penskalaan kompresi Minimum Description Length (MDL).
  2. Implementasi **Spatial Voxel Hashing** untuk alokasi voxel TSDF hemat RAM.
  3. Implementasi fusi volumetrik **3D Gaussian Splatting (3DGS)** dari awan titik spasial.
  4. Implementasi filter **Bilateral Guided Depth-Completion** untuk memadukan data kedalaman LiDAR renggang dengan garis tepi kamera RGB beresolusi tinggi.
* **Milestone 3:** Sistem mampu mendeteksi bidang datar, menghasilkan peta kedalaman padat sub-piksel dengan tepi oklusi tajam, serta merender klon ruangan 3DGS fotorealistik.

### Fase 4: Integrasi ONNX Runtime, Face/Hand Mesh & Neural Tracking Fallback
* **Tujuan:** Menambahkan fungsionalitas inferensi AI/ML untuk interaksi pengguna dan pemulihan pelacakan instan.
* **Tugas Utama:**
  1. Integrasi binding **ONNX Runtime** di Rust dengan dukungan kompilasi akselerator mobile (NNAPI/CoreML).
  2. Integrasi model AI estimasi 3D Face Mesh (468 vertices) dan ekspresi wajah blendshapes (52 parameters).
  3. Integrasi model AI pelacakan 21 sendi tangan dan pengenal gestur kontrol.
  4. Implementasi sistem **Neural Tracking Fallback** menggunakan model SuperPoint dan LightGlue untuk relokalisasi pose instan pasca kegagalan tracking visual.
* **Milestone 4:** Inferensi ML berjalan sinkron di bawah 16 ms per frame pada NPU target, berhasil mendeteksi ekspresi wajah, gestur tangan, serta mampu memulihkan posisi kamera secara instan saat terjadi tracking lost.

### Fase 5: Estimasi SV-HDR (NSRF), Fisika Rapier3D & Kolaborasi P2P
* **Tujuan:** Menghasilkan refleksi logam mengkilap virtual, interaksi fisik dinamis, dan kolaborasi spasial lokal.
* **Tugas Utama:**
  1. Integrasi model AI CNN ringan **Neural Specular Reflection Fields (NSRF)** untuk inferensi peta lingkungan SV-HDR dari kamera SDR.
  2. Integrasi engine fisika **Rapier3D** di sisi Rust dan konversi mesh spasial menjadi collider statis.
  3. Integrasi modul komunikasi P2P desentralisasi menggunakan **libp2p** di Rust.
  4. Sinkronisasi delta voxel hash keys dan delta pose spasial melintasi Wi-Fi Direct/Bluetooth LE lokal.
* **Milestone 5:** Objek virtual mengkilap memantulkan bayangan ruangan nyata secara spekular, bola virtual memantul secara elastis pada mes fisik, dan kolaborasi peta spasial P2P berjalan lancar tanpa internet.

### Fase 6: C++ FFI Bridge, Custom Shader SH & Optimasi Ponsel
* **Tujuan:** Menghubungkan pipeline pelacakan Rust dengan engine render grafis C++ dan optimasi kinerja akhir.
* **Tugas Utama:**
  1. Eksposur API FFI `extern "C"` untuk koordinat pose, warping matrix, 3DGS splats, face blendshapes, sendi tangan, dan collider fisika.
  2. Implementasi shader fragment Vulkan/Metal/GLSL untuk konvolusi irradiance Harmonik Sferis Orde 2 (27 floats) berbiaya konstan $O(1)$.
  3. Integrasi warping koreksi Late Latching pada rendering shader grafis sesaat sebelum buffer frame ditampilkan di layar.
  4. Profiling memori komprehensif (RAM $\le 120$ MB) dan multithreading CPU/NPU mobile di bawah 15% menggunakan `cargo-ndk`.
* **Milestone 6:** Mesin AR berjalan stabil pada target smartphone Android & iOS dengan rendering 60 FPS terkunci, latensi visual yang dirasakan $\approx 0$ ms, oklusi manusia sempurna, dan konsumsi daya baterai minimal.

---

## 3. Matriks Alokasi Risiko & Strategi Mitigasi

| Risiko Teknis | Dampak | Strategi Mitigasi |
| :--- | :--- | :--- |
| **Drift Akumulatif VIO Tinggi** | Tinggi | Integrasikan komparasi deskriptor visual relokalisasi secara paralel di sub-thread ML menggunakan modul Neural Tracking. |
| **Penyimpanan RAM 3DGS / TSDF Membengkak** | Sedang - Tinggi | Batasi radius jangkauan pemetaan Gaussian Splatting hanya di sekitar kerucut pandang kamera (*view frustum*); buang Gaussians yang tidak terlihat atau memiliki opasitas sangat rendah. |
| **FFI Memory Leak melintasi batas Rust/C++** | Tinggi | Terapkan pola *RAII* ketat di C++ dan gunakan wrapper smart pointer untuk otomatis memanggil `ar_engine_release()`. |
| **Beban Inferensi ML Mengakibatkan Frame Drops** | Tinggi | Pindahkan siklus eksekusi model ML (Face/Hand Tracking dan NSRF) pada thread frekuensi lebih rendah (30Hz) secara asinkron, sementara loop pelacakan sensor utama VIO tetap berjalan pada 60Hz/1000Hz. |
| **Koneksi P2P Terputus di Area Padat Sinyal** | Sedang | Terapkan penanganan rekoneksi otomatis (*auto-reconnect*) dan sinkronisasi delta keadaan spasial secara inkremental menggunakan antrean prioritas paket. |
| **Late Latching Mengakibatkan Artefak Robek (Tearing)** | Sedang | Terapkan sinkronisasi vertikal (*V-Sync*) penuh di sisi render engine dan batasi jangkauan warping koreksi sudut maksimal 5 derajat untuk mencegah distorsi perspektif berlebih. |
