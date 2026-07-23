# Daftar Fitur Lengkap & Detail — Fizgravity AR Try-On

Fizgravity AR Engine menyediakan sistem pelacakan wajah (*face tracking*) berbasis kecerdasan buatan (ONNX Runtime) dan mesin rendering grafis (OpenGL ES 3.0) yang dioptimalkan untuk simulasi riasan kosmetik (*virtual makeup*) secara real-time pada perangkat seluler.

Berikut adalah daftar lengkap fitur kosmetik dan teknologi pendukung yang diimplementasikan di dalam sistem:

---

## 1. Fitur Riasan Utama (Core Cosmetics)

### 💄 A. Lipstick Simulator (Simulasi Lipstik)
*   **Fungsi**: Menerapkan riasan pewarna bibir yang presisi mengikuti bentuk bibir pengguna secara real-time.
*   **Mode Finishing**:
    *   **Matte**: Tampilan warna solid tanpa kilauan pantulan cahaya.
    *   **Glossy (Basah/Mengkilap)**: Menambahkan lapisan specular reflektif di atas warna dasar bibir untuk mensimulasikan kilau lip gloss.
*   **Parameter Kontrol**:
    *   `Red`, `Green`, `Blue` (Warna RGB dasar).
    *   `Opacity` (Tingkat kepekatan riasan, 0.0 - 1.0).
    *   `IsGlossy` (Flag Boolean untuk mengaktifkan efek pantulan cahaya).
*   **Detail Teknis**:
    *   Menggunakan indeks triangulasi Delaunay bibir atas (*Upper Lip*) dan bibir bawah (*Lower Lip*) yang diekstrak langsung dari Rust Core untuk merender mesh bibir secara presisi.

### 🌸 B. Blush-On Simulator (Simulasi Blush-On)
*   **Fungsi**: Menerapkan riasan perona pipi pada area tulang pipi secara halus.
*   **Parameter Kontrol**:
    *   `Red`, `Green`, `Blue` (Warna RGB dasar pipi).
    *   `Opacity` (Kepekatan perona pipi, 0.0 - 1.0).
*   **Detail Teknis**:
    *   Menerapkan interpolasi warna pipi yang menyebar (*soft-edge blending*) pada area vertex pipi luar kiri dan kanan agar riasan tidak terlihat kaku atau berbentuk lingkaran tajam.

### 🧴 C. Foundation & Concealer Simulator (Simulasi Foundation & Alas Bedak)
*   **Fungsi**: Meratakan warna kulit wajah, menyamarkan noda, dan memberikan efek tekstur kulit tertentu.
*   **Mode Finishing**:
    *   **Matte**: Tampilan riasan wajah bebas minyak dengan kecerahan merata.
    *   **Satin/Dewy**: Tampilan wajah semi-kilap yang sehat.
*   **Parameter Kontrol**:
    *   `Red`, `Green`, `Blue` (Warna shade kulit).
    *   `Opacity` (Ketebalan penutupan bedak, 0.0 - 1.0).
    *   `Finishing` (String filter matte/dewy).
*   **Detail Teknis**:
    *   **Pore Detail Preservation**: Menggunakan teknik *High-Pass filter* pada shader OpenGL untuk membaca tekstur pori-pori kulit asli dari kamera (`uCamTexture`), kemudian mencampurkannya kembali di atas foundation. Kulit wajah terlihat mulus tetapi detail pori-pori alami tetap terjaga (tidak terlihat seperti topeng plastik).
    *   **Hairline Blending**: Menggunakan fungsi `fizgravity_engine_calculate_hairline_blending` untuk menghitung gradasi opacity transparan pada dahi bagian atas (batas rambut) agar riasan menyatu mulus dengan rambut pengguna.

---

## 2. Fitur Grafis & Realisme Premium (Premium Shader Enhancements)

### ✨ A. Korean Glass Skin Effect (Efek Kulit Kaca)
*   **Fungsi**: Menambahkan efek kilau khas riasan ala Korea Selatan yang membuat kulit wajah terlihat berair (*dewy*), sehat, dan memantulkan cahaya secara alami.
*   **Detail Teknis**:
    *   Menggunakan perhitungan **Fresnel Reflection** pada shader fragment OpenGL:
        $$\text{Fresnel} = (1.0 - \text{clamp}(\vec{N} \cdot \vec{V}, 0.0, 1.0))^5$$
    *   Kilauan pantulan specular akan meningkat secara otomatis pada sudut-sudut wajah yang melengkung menjauhi arah pandang kamera (seperti tulang pipi dan dahi samping).

### 👥 B. Topology-Guided Dynamic Ambient Occlusion (AO)
*   **Fungsi**: Menerapkan bayangan halus (*soft shadows*) pada lipatan-lipatan wajah (seperti sela hidung, bawah bibir, dan kelopak mata).
*   **Detail Teknis**:
    *   Rust Core secara dinamis menghitung jarak kedekatan antar-landmark wajah melalui fungsi `fizgravity_engine_calculate_dynamic_ao` untuk menghasilkan nilai AO per-vertex.
    *   Nilai AO disalurkan langsung ke GPU melalui VBO buffer (`mAOVBO`) untuk menggelapkan warna makeup di area lekukan dalam secara real-time, memberikan efek kedalaman 3D yang sangat realistis.

### 🕯️ C. Smart Relighting & Kelvin Temperature Solver
*   **Fungsi**: Menyesuaikan warna dan kecerahan makeup secara otomatis mengikuti pencahayaan lingkungan sekitar pengguna (misalnya, di bawah lampu warm/kuning atau lampu cool/neon).
*   **Detail Teknis**:
    *   Menerapkan **McCamy Formula** di Rust Core untuk memperkirakan temperatur warna cahaya dalam satuan Kelvin (CCT) dan intensitas lux sekitar langsung dari pixel kamera.
    *   Shader merotasi dan menyesuaikan saturasi serta suhu warna makeup menggunakan konversi Kelvin-ke-RGB secara real-time untuk memastikan riasan selalu terlihat menyatu natural dengan warna foto/video.

---

## 3. Fitur Sensor & Tracker AI (Inertial & Calibration Engines)

### 🔄 A. Gyro-Guided Specular Shimmer (Efek Kilau Giroskop)
*   **Fungsi**: Membuat riasan glossy (seperti lipstik basah) memantulkan kilatan cahaya yang dinamis dan bergeser mengikuti kemiringan ponsel pengguna saat digerakkan.
*   **Detail Teknis**:
    *   Menggunakan sensor giroskop ponsel yang disalurkan melalui FFI Rust `fizgravity_engine_calculate_glitter_shimmer_shift`.
    *   Menggunakan penyelesai **Leaky Integrator** untuk meluruhkan akumulasi rotasi sensor agar pantulan cahaya tidak melompat (*drift*) dan bergerak dengan sangat halus (*smooth specular shift*).

### 📐 B. Continuous Face-Width Auto-Calibration
*   **Fungsi**: Melakukan kalibrasi dinamis terhadap jarak wajah pengguna dari kamera seluler untuk memastikan skala riasan tidak membesar atau mengecil secara salah saat pengguna bergerak maju-mundur.
*   **Detail Teknis**:
    *   Rust Core membandingkan rasio jarak antara pipi kiri (`landmark 234`) dan pipi kanan (`landmark 454`) secara spasial 3D terhadap model proyeksi kamera untuk menghitung panjang fokus intrinsik secara instan.

### 🩺 C. Skin Health Analyzer (Analisis Kesehatan Kulit)
*   **Fungsi**: Melakukan pemindaian medis awal terhadap kualitas kulit wajah pengguna (seperti tingkat kerutan dan kekasaran kulit).
*   **Detail Teknis**:
    *   Fungsi Rust `fizgravity_engine_analyze_skin_health` menganalisis variasi intensitas kontras lokal (*local contrast variance*) pada area dahi dan pipi dari frame kamera RGB untuk menentukan skor kerutan (*wrinkles*) dan kekasaran kulit (*roughness*).
