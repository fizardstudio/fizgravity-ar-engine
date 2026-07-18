# Panduan Penyelarasan Indeks Kosmetik: Google ML Kit & MediaPipe
## Pemetaan Wajah Geometris untuk Riasan GlowMatch

Dokumen ini menjelaskan kompatibilitas antara **Google ML Kit Face Mesh** (Android) dan **Fizgravity AR Engine**, serta memberikan daftar indeks koordinat landmark wajah standar untuk memetakan berbagai jenis kosmetik secara presisi di shader OpenGL.

---

## 🤝 1. Kompatibilitas Google ML Kit & MediaPipe

**Google ML Kit Face Mesh menggunakan topologi landmark yang 100% identik dengan Google MediaPipe Face Mesh!**
Keduanya menghasilkan jaring wajah yang terdiri dari **468 koordinat 3D** dengan penomoran indeks vertex `0` hingga `467` yang sama secara antropometri. 

Oleh karena itu, seluruh indeks pelacakan wajah di Rust Core maupun C++ renderer kita secara otomatis **selaras sempurna** dengan output dari SDK Google ML Kit di sisi Android Kotlin.

---

## 💄 2. Daftar Indeks Landmark per Kategori Kosmetik

Untuk menggambar riasan di shader OpenGL, kita membagi koordinat vertex menjadi zona-zona kosmetik berikut:

### 💋 A. Lipstick (Bibir Atas & Bibir Bawah)
Untuk menghindari bug bibir "menempel" saat mulut terbuka, kita memisahkan bibir atas dan bawah menjadi jaring triangulasi Delaunay terpisah:

1.  **Bibir Atas (Outer Contour)**:
    `[61, 185, 40, 39, 37, 0, 267, 269, 270, 409, 291]` (Batas luar atas)
2.  **Bibir Atas (Inner Contour)**:
    `[78, 191, 80, 81, 82, 13, 312, 311, 310, 415, 308]` (Batas dalam atas)
3.  **Bibir Bawah (Outer Contour)**:
    `[61, 146, 91, 181, 84, 17, 314, 405, 321, 375, 291]` (Batas luar bawah)
4.  **Bibir Bawah (Inner Contour)**:
    `[78, 95, 88, 178, 87, 14, 317, 402, 318, 324, 308]` (Batas dalam bawah)

*   *Tips Shader*: Gunakan gradasi alpha lembut dari batas dalam ke luar bibir untuk efek *ombre lip* premium.

---

### 👁️ B. Eyeliner (Garis Tepi Kelopak Mata)
Eyeliner digambar sebagai garis tipis (*polyline*) mengikuti tepi bukaan mata:

1.  **Mata Kiri (Upper Lid)**: `[33, 161, 160, 159, 158, 157, 173, 133]`
2.  **Mata Kiri (Lower Lid)**: `[33, 7, 163, 144, 145, 153, 154, 155, 133]`
3.  **Mata Kanan (Upper Lid)**: `[263, 388, 387, 386, 385, 384, 398, 362]`
4.  **Mata Kanan (Lower Lid)**: `[263, 249, 390, 373, 374, 380, 381, 382, 362]`

---

### 🎨 C. Eye Shadow (Kelopak Mata Atas & Bawah)
Eye shadow menutupi kelopak mata atas dari garis mata hingga ke bawah alis:

1.  **Kelopak Mata Kiri (Atas)**: Dibatasi oleh mata kiri atas (`33..133`) dan alis kiri bawah (`70, 63, 105, 66, 107`).
2.  **Kelopak Mata Kanan (Atas)**: Dibatasi oleh mata kanan atas (`362..263`) dan alis kanan bawah (`300, 293, 334, 296, 336`).

*   *Tips Shader*: Gunakan blending multi-warna (warna transisi di bagian atas kelopak, warna gelap/aksen dekat garis mata) dengan interpolation linear pada sumbu $V$ koordinat UV lokal mata.

---

### 🌸 D. Blush On (Pipi Kiri & Pipi Kanan)
Blush on digambar melingkar dengan degradasi keburaman (*radial falloff*) di tulang pipi:

1.  **Pusat Pipi Kiri**: Landmark **`50`** (tulang pipi kiri) atau **`117`** (pusat pipi).
2.  **Pusat Pipi Kanan**: Landmark **`280`** (tulang pipi kanan) or **`347`** (pusat pipi).
3.  **Radius Penyebaran**:
    *   Sisi kiri mencakup vertex: `[117, 118, 119, 47, 142, 205, 50, 123]`.
    *   Sisi kanan mencakup vertex: `[346, 347, 348, 277, 371, 425, 280, 352]`.

*   *Tips Shader*: Hitung jarak Euclidean 2D piksel terhadap proyeksi layar landmark pusat (`50` / `280`). Aplikasikan fungsi smoothing radial untuk menghasilkan blush-on dengan pinggiran yang memudar sangat halus:
    $$\alpha_{\text{blush}} = \text{smoothstep}(\text{max\_radius}, \text{min\_radius}, \text{distance})$$

---

### 💆 E. Base / Foundation (Seluruh Wajah Terproteksi)
Foundation menutupi seluruh 468 vertex wajah, tetapi harus **memiliki lubang** di daerah sensitif agar tidak menutupi bola mata dan rongga mulut:

1.  **Area Oklusi Mata Kiri**: Atur alpha foundation ke `0.0` pada area indeks mata kiri (`33, 7, 163, 144, 145, 153, 154, 155, 133, 173, 157, 158, 159, 160, 161, 246`).
2.  **Area Oklusi Mata Kanan**: Atur alpha foundation ke `0.0` pada area indeks mata kanan (`362, 382, 381, 380, 374, 373, 390, 249, 263, 466, 388, 387, 386, 385, 384, 398`).
3.  **Area Oklusi Mulut (Bibir Dalam)**: Atur alpha ke `0.0` pada seluruh indeks bibir dalam (`78` s.d `308`).
4.  **Koreksi Batas Tepi Rambut**: Hubungkan dengan array alpha dari modul hairline blending kita (`fizgravity_engine_calculate_hairline_blending`) di dahi atas.
