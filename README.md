# Fizgravity AR Engine

Fizgravity AR Engine adalah kit pengembangan perangkat lunak (**Spatial Computing & Augmented Reality SDK**) lintas-platform berkinerja tinggi yang dikembangkan secara kolaboratif oleh **Fizard Studio** dan **Antigravity**. 

Mesin ini ditulis dalam bahasa **Rust** untuk menjamin keamanan memori (*thread-safe*), efisiensi baterai yang tinggi, dan performa tanpa *Garbage Collection*, serta diekspos melalui antarmuka FFI C-ABI standar ke **C++** untuk kemudahan integrasi dengan render engine modern (Unreal Engine, Unity, custom Vulkan/Metal/OpenGL app).

---

## 🚀 Fitur Utama & Keunggulan Teknologis

### 1. Pelacakan & Estimasi Pose
*   **MSCKF VIO:** Multi-State Constraint Kalman Filter yang melacak pergerakan kamera dengan kompleksitas waktu $O(N)$ linier terhadap jumlah fitur visual.
*   **Adaptive EKF (AEKF):** Kontrol kovarians dinamis terhadap kebisingan akselerasi inersia yang secara adaptif menyaring gangguan akibat getaran tangan fisik atau perubahan suhu perangkat.
*   **Late Latching (Inertial Motion Extrapolator):** Memprediksi pose kamera 16-33 ms ke depan menggunakan sensor IMU 1000Hz untuk melakukan warping visual di GPU sesaat sebelum frame ditampilkan di layar, memotong jeda visual hingga **~0 milidetik**.

### 2. Rekonstruksi Spasial & Oklusi
*   **Spatial Voxel Hashing (TSDF):** Memetakan lingkungan 3D padat secara real-time dengan alokasi memori tabel hash spasial yang sangat hemat RAM.
*   **Bilateral Guided Depth-Completion:** Menggabungkan data sensor kedalaman LiDAR renggang dengan garis kontur kamera RGB resolusi tinggi untuk menghasilkan batas oklusi sub-piksel yang sangat tajam dan rapi.
*   **Real-Time 3D Gaussian Splatting (3DGS):** Mengkloning lingkungan fisik menjadi representasi awan titik elipsoid fotorealistik yang menangkap transparansi dan pantulan cahaya nyata.

### 3. Kecerdasan Buatan (AI/ML)
*   **3D Face Mesh & Blendshapes (ONNX Runtime):** Melacak 468 titik geometri wajah dan 52 parameter gerakan ekspresi secara real-time (sangat ideal untuk aplikasi kosmetik *Virtual Makeup* realistis).
*   **3D Hand Tracking:** Melacak koordinat spasial 21 sendi jari tangan untuk kontrol interaksi tanpa sentuhan.
*   **Neural Tracking Recovery:** Peta referensi visual dicocokkan menggunakan model AI SuperPoint & LightGlue untuk relokalisasi posisi instan saat tracking konvensional terputus (*lost*).

### 4. Fisika & Kolaborasi Spasial
*   **Deterministic Physics (Rapier3D):** Simulasi gravitasi, rigid-bodies, dan kolisi bola/kotak virtual di atas mes spasial nyata.
*   **Decentralized P2P Map Sync (libp2p):** Sinkronisasi kunci voxel hash lokal antar-perangkat terdekat menggunakan protokol penemuan mDNS P2P tanpa memerlukan koneksi internet.

---

## 📁 Struktur Repositori

```text
Fizgravity AR Engine/
├── docs/
│   ├── PRD_AR_Engine.md            # Spesifikasi Persyaratan Produk lengkap
│   ├── Project_Plan_Roadmap.md     # Peta jalan pengembangan modular 6 Fase
│   └── comparison_report.md        # Laporan komparasi jujur dengan engine dunia
├── include/
│   └── ar_bridge.h                 # Header jembatan interoperabilitas C++ (C-ABI FFI)
├── src/
│   ├── face.rs                     # Modul AI Face tracking & blendshapes
│   ├── hand.rs                     # Modul AI Hand joints tracking
│   ├── imu.rs                      # Modul IMU Pre-integration & Jacobians bias
│   ├── lib.rs                      # Titik masuk ekspor FFI & tipe data FFI C-ABI
│   ├── math.rs                     # Grup Lie SO(3) & aljabar Lie so(3) manifold
│   ├── msckf.rs                    # Filter navigasi sliding window MSCKF EKF
│   ├── p2p.rs                      # Modul sinkronisasi peta kolaboratif libp2p
│   ├── physics.rs                  # Abstraksi solver fisika Rapier3D
│   ├── splatting.rs                # Modul representasi data 3D Gaussian Splats
│   └── tsdf.rs                     # Modul pemetaan volumetrik Voxel Hashing TSDF
└── Cargo.toml                      # Konfigurasi dependensi Cargo Rust
```

---

## 🛠️ Cara Membangun Proyek (Build Guide)

Pastikan toolchain Rust (`cargo` & `rustc`) telah terpasang di sistem Anda.

### 1. Menjalankan Tes Unit & Integrasi
Untuk memastikan semua fungsi matematika manifold dan pemetaan berjalan presisi:
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

        ar::Pose cameraPose;
        ar::SphericalHarmonics lighting;

        // Perbarui pelacakan & pencahayaan
        if (engine->update(timestamp, camera_pixels, imu_data, cameraPose, lighting)) {
            // Gunakan cameraPose untuk memperbarui kamera virtual 3D Anda
            // Gunakan lighting untuk mewarnai material objek virtual Anda
        }

        // Uji coba Face Mesh untuk kosmetik Makeup wajah virtual
        ar::FaceMesh faceMesh;
        if (engine->getFaceMesh(faceMesh)) {
            // Gambar 468 vertices wajah ke GPU vertex buffer
            // Render lipstik virtual pada koordinat indeks bibir faceMesh.vertices
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
