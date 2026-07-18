# Cetak Biru Riset: Hyper-Realistic AR Makeup Rendering Engine
## Formulasi Fisika, Pemodelan Pigmen, & Optimasi GLES3 (Metode ATMO)

Laporan riset ini menguraikan langkah-langkah penelitian teoritis, pemodelan matematis, dan strategi implementasi untuk meningkatkan **Fizgravity AR Engine** menjadi mesin perender riasan virtual tingkat dunia yang fotorealistis (*hyper-realistic*).

---

## 🎨 1. PBR Skin Shading & Light Transport (Subsurface Scattering - SSS)

### A. Pengamatan (Amati)
Kulit manusia tidak bersifat *Lambertian diffuse* sederhana. Cahaya menembus lapisan luar (*epidermis*), memantul di dalam lapisan pembuluh darah (*dermis*), dan keluar kembali sebagai warna kemerahan lembut. Tanpa pemodelan ini, foundation virtual membuat wajah terlihat datar, kering, dan seperti "topeng plastik/zombie".

### B. Peniruan (Tiru)
Mesin render game AAA menggunakan Screen-Space Subsurface Scattering (SSSSS) dengan filter gaussian blur bertingkat pada buffer kedalaman. Metode ini terlalu berat untuk GPU seluler (Android kelas menengah).

### C. Modifikasi (Modify)
Kita mengadopsi model **Wrap Lighting** (Half-Lambertian) yang dimodifikasi untuk mensimulasikan transmisi cahaya di area terminator (perbatasan bayangan):
$$I_{\text{diffuse}} = \text{saturate}\left( \frac{N \cdot L + w}{1.0 + w} \right)^p$$
Di mana $w \approx 0.5$ adalah parameter wrap, dan $p \approx 2.0$ adalah faktor pemulusan. Untuk memberikan efek rona darah dermis, kita menambahkan *color bleeding* kemerahan di daerah transisi bayangan:
$$\text{Bleed}_{\text{red}} = \text{saturate}(1.0 - (N \cdot L)) \cdot (1.0 - (N \cdot V)) \cdot \text{Color}_{\text{blood}}$$

### D. Optimalisasi (Optimalisasi)
Implementasikan Wrap-Lighting and Bleed-Red secara langsung di Fragment Shader GLES3 tanpa texture lookup tambahan. Ini memberikan ilusi kelembutan kulit (*skin softness*) dengan 0% memory bandwidth overhead.

---

## 🧪 2. Pemodelan Pigmen Fisik (Kubelka-Munk Theory)

### A. Pengamatan (Amati)
Pencampuran riasan virtual tradisional menggunakan interpolasi linear alpha:
$$\text{Color}_{\text{output}} = \text{mix}(\text{Color}_{\text{skin}}, \text{Color}_{\text{makeup}}, \alpha)$$
Namun, dalam fisika nyata, kosmetik (foundation/lipstik) terdiri dari pigmen mikro (titanium dioksida, besi oksida) yang menyerap (*Absorption, $K$*) dan menghamburkan (*Scattering, $S$*) cahaya secara non-linear berdasarkan ketebalan lapisan pigmentasi ($d$).

### B. Peniruan (Tiru)
Gunakan teori spektral **Kubelka-Munk** untuk menghitung reflektansi ($R$) campuran:
$$\frac{K_{\text{mix}}}{S_{\text{mix}}} = \frac{\sum c_i \cdot K_i}{\sum c_i \cdot S_i}$$
Di mana $c_i$ adalah konsentrasi pigmen ke-$i$.

### C. Modifikasi (Modify)
Untuk kalkulasi real-time di shader, kita menyederhanakan formula reflektansi tak terhingga (untuk lapisan tebal) menggunakan fungsi hampiran rasional:
$$R_{\infty} = 1 + \frac{K}{S} - \sqrt{ \left( \frac{K}{S} \right)^2 + 2\left(\frac{K}{S}\right) }$$
Kita memetakan nilai $K/S$ unik untuk foundation (sheer, medium, full coverage) dan mencampurkannya secara fisik dengan koefisien kulit asli pengguna.

### D. Optimalisasi (Optimalisasi)
Simpan koefisien $K/S$ pigmen kosmetik dalam array seragam (*uniform array*) kecil di shader untuk melakukan blending pigmen spektral secara instan pada GPU.

---

## ✨ 3. Dual-Lobe Clear Coat BRDF (Efek Lip Gloss Basah)

### A. Pengamatan (Amati)
Lip gloss atau bibir basah memiliki dua lapisan reflektansi yang jelas:
1.  Lapisan dasar (*base lobe*): Berwarna merah pigmen, bersifat kasar (*roughness* $\approx 0.6$).
2.  Lapisan pernis bening atas (*clear coat lobe*): Bersifat sangat mengkilap (*roughness* $\approx 0.05$) dan memantulkan refleksi tajam.

### B. Peniruan (Tiru)
Model Cook-Torrance standar hanya menyediakan satu lobe specular, yang membuat lip gloss terlihat seperti logam krom yang kasar.

### C. Modifikasi (Modify)
Terapkan **Dual-Lobe Specular BRDF** di shader:
$$f_r(q) = (1 - F_{\text{coat}}) \cdot f_{\text{base}} + F_{\text{coat}} \cdot f_{\text{coat}}$$
Di mana $F_{\text{coat}}$ adalah Fresnel Schlick dari lapisan pernis atas bening ($IOR \approx 1.5$):
$$F_{\text{coat}} = F_0 + (1.0 - F_0) \cdot (1.0 - (N \cdot V))^5$$

### D. Optimalisasi (Optimalisasi)
Batasi pemantulan lobe atas (*coat specular*) menggunakan masker *specular noise map* untuk mensimulasikan ketidakrataan cairan gloss pada permukaan bibir yang bergerak.

---

## 🌀 4. Specular Glitter & Micro-flaking (Eyeshadow & Highlighter)

### A. Pengamatan (Amati)
Eyeshadow premium mengandung serpihan mika berkilau yang memantulkan kilau kelap-kelip intens secara acak hanya pada sudut pandang tertentu terhadap cahaya.

### B. Peniruan (Tiru)
Snapchat menggunakan static noise map, tetapi kilauannya tidak berkedip secara alami saat kamera/HP digeser.

### C. Modifikasi (Modify)
Gunakan **3D Voronoi Cell Noise** dengan sumbu spasial lokal:
$$\text{Glitter}_{\text{normal}} = \text{normalize}(N + \text{VoronoiNormal}(\text{UV} \cdot \text{scale} + \text{IMU}_{\text{shift}}) \cdot \text{intensity})$$
Vektor normal mikro ini diumpankan ke NDF GGX. Jika sudut setengah vektor ($H = \text{normalize}(L+V)$) sejajar secara acak dengan normal mikro sel Voronoi tertentu, pixel tersebut akan berkilau tajam secara matematis.

### D. Optimalisasi (Optimalisasi)
Pre-compute tekstur Voronoi 2D yang dibungkus (*tiled noise texture*) untuk menghindari kalkulasi noise prosedural yang mahal di loop fragment shader.

---

## 📐 5. Reflektansi Anisotropik (Satin & Velvet Lips)

### A. Pengamatan (Amati)
Riasan satin atau velvet memantulkan cahaya secara memanjang sepanjang kerutan alami bibir atau arah goresan kuas eyeshadow.

### B. Peniruan (Tiru)
Gunakan model **Kajiya-Kay** atau **GGX Anisotropic BRDF** yang memisahkan roughness sumbu tangen ($\alpha_x$) dan bitangen ($\alpha_y$).

### C. Modifikasi (Modify)
Petakan arah tangen ($T$) mengikuti garis radial mulut pada koordinat UV bibir kita. Hitung distribusi anisotropik:
$$D_{\text{aniso}}(H) = \frac{1}{\pi \alpha_x \alpha_y \left( \frac{(H \cdot T)^2}{\alpha_x^2} + \frac{(H \cdot B)^2}{\alpha_y^2} + (H \cdot N)^2 \right)^2}$$

### D. Optimalisasi (Optimalisasi)
Gunakan tangen koordinat UV statis wajah untuk menghindari kalkulasi matriks basis ortonormal (TBN) dinamis pada vertex shader.
