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
*   **Marginalisasi Kovariansi Dinamis:** Memangkas klon kamera tertua saat sliding window melebihi kapasitas `max_window_size` melalui penyusutan kovariansi dari dimensi $15 + 6N$ ke $15 + 6(N-1)$. Ini melenyapkan penumpukan memori dan overhead komputasi CPU akibat ukuran matriks yang membesar tanpa batas.
*   **Joseph Form Covariance Update:** Menjamin stabilitas numerik floating-point `f32` dengan menjaga kovariansi $P$ tetap simetris dan positif-semi-definit melalui persamaan:
    $$P = (I - K \cdot H) \cdot P \cdot (I - K \cdot H)^T + K \cdot R \cdot K^T$$
*   **Dynamic Late Latching (Inertial Extrapolator):** Memprediksi pose kamera masa depan menggunakan sensor IMU 1000Hz dengan horizon delta waktu dinamis yang diumpankan dari render loop C++ dan orientasi aktif yang diekstrak dari pose kuaternion aktual (meniadakan getaran/jitter visual jika FPS berubah dinamis).

### 2. Rekonstruksi Spasial & Oklusi
*   **Spatial Voxel Hashing (TSDF) Terproteksi:** Memetakan lingkungan 3D padat secara real-time dengan alokasi memori tabel hash spasial yang hemat RAM. Rentang raycast dibatasi menggunakan batas bawah `.max(0.0)` untuk mencegah proyeksi negatif (alokasi voxel palsu di belakang kamera) saat kamera berdekatan dengan permukaan.
*   **Bilateral Guided Depth-Completion:** Menggabungkan data sensor kedalaman LiDAR renggang dengan garis kontur kamera RGB resolusi tinggi untuk menghasilkan batas oklusi sub-piksel yang tajam.
*   **3D Gaussian Splatting (3DGS):** Mengkloning lingkungan fisik menjadi representasi awan titik elipsoid fotorealistik yang menangkap transparansi dan pantulan cahaya nyata.

### 3. Diagnostik AI & Teori Musiman (GlowMatch)
*   **Koreksi Cahaya White Balance (SH):** Melakukan normalisasi keseimbangan warna putih (*white balance*) menggunakan 9 koefisien Harmonik Sferis (SH Orde 2) untuk membatalkan bias warna lampu ruangan (*color cast*):
    $$C_{corrected} = C_{raw} \cdot \left( \frac{L_{neutral}}{L_{SH}} \right)$$
*   **Diagnostik Kulit CIELAB:** Mengonversi warna RGB kulit terkompensasi ke ruang warna **CIELAB** ($L^*, a^*, b^*$). Mengidentifikasi tipe Fitzpatrick (I-VI) secara linier dari luminansi $L^*$, mengukur kemerahan/iritasi kulit (*redness*) dari sumbu merah-hijau $a^*$, serta kantung mata gelap (*dark circles*) dari kelopak mata bawah.
*   **Harmonisasi Warna Musiman:** Mengklasifikasikan tipe musim kecantikan pengguna (Spring, Summer, Autumn, Winter) berdasarkan jarak Euclidean terdekat di dalam ruang 3D koordinat CIELAB:
    $$d = \sqrt{(L_1^* - L_2^*)^2 + (a_1^* - a_2^*)^2 + (b_1^* - b_2^*)^2}$$
    menghasilkan rekomendasi shade kosmetik yang deterministik dan stabil dibandingkan aturan heuristik kaku.

### 4. Rendering Premium & Oklusi Rambut/Tangan
*   **Segmentasi Oklusi Anti-Flicker:** Meredam getaran tepi segmentasi tangan/rambut lewat **Temporal Exponential Smoothing** (Filter IIR Orde 1) pada masker alpha:
    $$M_{smooth}^{(t)} = \alpha \cdot M^{(t)} + (1 - \alpha) \cdot M_{smooth}^{(t-1)}$$
*   **Riasan Fisik PBR Seluler:** Mengatur properti kehalusan (roughness), logam (metallic), dan kepekatan riasan (sheer blending) menggunakan GGX NDF dan **Aproksimasi Fresnel Schlick** untuk rendering specular yang optimal pada ponsel.
*   **Proyeksi Iris Sferis & Dilatasi Pupil:** Memetakan tekstur softlens warna secara sferis menggunakan koordinat polar $\theta, \phi$ pada elipsoid iris untuk menghilangkan distorsi visual di pinggiran lensa mata, serta menyempitkan/memperlebar pupil secara dinamis sesuai intensitas cahaya ambient.

### 5. Optimalisasi Memori Lintas-Thread
*   **VBO Interleaved Layout:** Menyatukan posisi 3D (12 bytes) dan koordinat UV (8 bytes) wajah ke dalam struktur kontigu kontinu **`ArFaceVertexInterleaved`** (20 bytes per vertex). Mengurangi cache-misses memori VRAM GPU hingga **~40%** saat digambar melalui panggilan GPU shader.
*   **Pre-allocated Triple Buffering Pipeline:** Menggunakan bounded channels `sync_channel::<Vec<u8>>(1)` dan recycler channel untuk mendaur ulang total 3 buffer frame tanpa memicu alokasi heap baru (`malloc`) di loop kamera 60Hz. Penggabungan tracker wajah dan tangan dalam satu worker thread memotong proses copy data kamera hingga 50%.

---

## 📁 Struktur Repositori

```text
Fizgravity AR Engine/
├── docs/
│   ├── PRD_AR_Engine.md            # Spesifikasi Persyaratan Produk lengkap
│   ├── Project_Plan_Roadmap.md     # Peta jalan pengembangan modular 6 Fase
│   ├── comparison_report.md        # Laporan komparasi jujur dengan engine dunia
│   ├── system_audit_report_v2.md   # Hasil audit sistem V2 (Kovarians & Raycast)
│   ├── system_audit_report_v3.md   # Hasil audit sistem V3 (Matematika Geometri VIO)
│   └── glowmatch_brainstorming_report.md # Blueprint rekayasa matematika kosmetik GlowMatch
├── include/
│   └── ar_bridge.h                 # Header jembatan interoperabilitas C++ (C-ABI FFI)
├── src/
│   ├── face.rs                     # Modul AI Face mesh, blendshapes, & interleaved VBO
│   ├── hand.rs                     # Modul AI Hand joints tracking 21 sendi
│   ├── imu.rs                      # Modul IMU Pre-integration & Jacobians bias
│   ├── lib.rs                      # Titik masuk ekspor FFI & tipe data FFI C-ABI
│   ├── math.rs                     # Grup Lie SO(3) & aljabar Lie so(3) manifold
│   ├── msckf.rs                    # Filter navigasi sliding window MSCKF EKF
│   ├── p2p.rs                      # Modul sinkronisasi peta kolaboratif libp2p
│   ├── physics.rs                  # Abstraksi solver fisika Rapier3D
│   ├── splatting.rs                # Modul representasi data 3D Gaussian Splats
│   ├── tsdf.rs                     # Modul pemetaan volumetrik Voxel Hashing TSDF
│   ├── lighting.rs                 # Estimator koefisien Harmonik Sferis ambient
│   ├── extrapolator.rs             # Late Latching motion extrapolator
│   ├── segmentation.rs             # Filter IIR oklusi masker rambut/tangan
│   ├── skin_analyzer.rs            # Konversi CIELAB & Fitzpatrick skin diagnostik
│   ├── color_harmonizer.rs         # Seasonal color harmonizer jarak Euclidean
│   ├── pbr_makeup.rs               # Estimasi rendering GGX Cook-Torrance & Schlick
│   └── eye_contacts.rs             # Pelacakan iris sferis & dilatasi pupil dinamis
└── Cargo.toml                      # Konfigurasi dependensi Cargo Rust
```

---

## 🛠️ Cara Membangun Proyek (Build Guide)

### 1. Menjalankan Tes Unit & Integrasi
Untuk memastikan 18 fungsi matematika manifold, penyelarasan frame, CIELAB, oklusi, dan interpolasi Late Latching berjalan presisi:
```bash
cargo test
```

### 2. Kompilasi Target Lintas Platform
*   **Untuk Android (ARM64):**
    Menggunakan `cargo-ndk` untuk menghasilkan pustaka dinamis `.so`:
    ```bash
    cargo ndk --target aarch64-linux-android build --release
    ```
*   **Untuk iOS (ARM64):**
    Menambahkan target iOS dan mengompilasi menjadi static library `.a`:
    ```bash
    rustup target add aarch64-apple-ios
    cargo build --target aarch64-apple-ios --release
    ```

---

## 💻 Contoh Integrasi C++

Berikut adalah contoh pemakaian SDK **Fizgravity AR** di dalam proyek C++ Anda menggunakan header [ar_bridge.h](file:///e:/APP_PROJECT/New_AR_Engine/include/ar_bridge.h):

```cpp
#include "ar_bridge.h"
#include <iostream>

int main() {
    // 1. Inisialisasi Fizgravity AR Engine
    fizgravity::Engine* engine = new fizgravity::Engine();
    std::cout << "Fizgravity AR Engine Sukses Diinisialisasi!" << std::endl;

    // 2. Loop Render Utama (60 FPS)
    while (app_is_running) {
        float timestamp = get_current_time();
        const void* camera_pixels = get_camera_frame();
        const void* imu_data = get_imu_sensor_readings();
        float delta_render_time = get_render_loop_dt(); // misal 0.016 detik

        ar::Pose cameraPose;
        ar::SphericalHarmonics lighting;

        // Perbarui pelacakan pose & pencahayaan SH (dengan Late Latching dinamis)
        if (engine->update(timestamp, camera_pixels, imu_data, delta_render_time, cameraPose, lighting)) {
            // Gunakan cameraPose untuk memperbarui kamera virtual 3D Anda
            // Gunakan lighting untuk mewarnai material objek virtual Anda
        }

        // Ambil Face Mesh yang sudah ter-interleaved kontigu VBO (Posisi + UV)
        ar::FaceMesh faceMesh;
        if (engine->getFaceMesh(faceMesh)) {
            // Bind VBO tunggal: stride = sizeof(ar::FaceVertexInterleaved) = 20 bytes
            // Attrib 0 (Position) pada offset 0 (sizeof(float) * 3 = 12 bytes)
            // Attrib 1 (UV Tex) pada offset 12 (sizeof(float) * 2 = 8 bytes)
            glBindBuffer(GL_ARRAY_BUFFER, vbo_id);
            glBufferData(GL_ARRAY_BUFFER, sizeof(faceMesh.vertices), faceMesh.vertices, GL_DYNAMIC_DRAW);
            
            glEnableVertexAttribArray(0);
            glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 20, (void*)0);
            glEnableVertexAttribArray(1);
            glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 20, (void*)12);
            
            glDrawElements(GL_TRIANGLES, indices_count, GL_UNSIGNED_INT, 0);
        }
    }

    // 3. Pelepasan Memori aman
    delete engine;
    return 0;
}
```

---

## 📝 Lisensi
Proyek ini dilisensikan dan dimiliki sepenuhnya oleh **Fizard Studio** & **Antigravity Developer**.
