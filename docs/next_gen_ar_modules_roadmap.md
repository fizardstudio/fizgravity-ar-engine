# Peta Jalan Pengembangan: Modul AR Tingkat Lanjut untuk Deteksi Wajah & Tubuh Sempurna

Dokumen ini berfungsi sebagai daftar periksa (*checklist*) fitur pengembangan untuk memantau kemajuan implementasi modul-modul **Fizgravity AR Engine** guna menyokong fitur-fitur premium pada aplikasi **GlowMatch**.

---

## 🛡️ Kategori 1: Akurasi & Stabilisasi (Menghilangkan Jitter & Distorsi)

- [x] **1. Modul Stabilisasi Adaptif One-Euro Filter (`src/stabilizer.rs`)**
  * **Prioritas**: **[PRIORITAS UTAMA - DIBUTUHKAN LANGSUNG UNTUK GLOWMATCH]**
  * **Dampak**: Mengeliminasi getaran halus (*jitter*) pada lipstik, eyeliner, dan blush-on saat wajah diam di depan kamera, sekaligus menghilangkan keterlambatan (*movement lag*) ketika wajah menoleh cepat.
  * **Solusi**: Filter low-pass adaptif yang mengubah frekuensi cutoff secara dinamis berdasarkan kecepatan gerakan.
  
- [ ] **2. Auto-Kalibrasi Intrinsik Kamera (`src/calibration.rs`)**
  * **Prioritas**: **[PRIORITAS MENENGAH - OPTIMASI LENS DISTORTION]**
  * **Dampak**: Menjamin proyeksi 3D mesh leher dan kosmetik tidak terlihat melar/distorsi pada kamera ponsel dengan lensa ultra-wide atau rasio layar non-standar.
  * **Solusi**: Modul kalibrasi online yang mengestimasi parameter kamera ($f_x, f_y, c_x, c_y$) secara dinamis berdasarkan rasio jarak pupil mata pengguna yang konstan secara antropometri.

---

## 👗 Kategori 2: Deteksi Badan Penuh & Interaksi Spasial (Full-Body AR)

- [ ] **3. Modul Pelacakan Sendi Tubuh 3D (`src/body_pose.rs`)**
  * **Prioritas**: **[PRIORITAS RENDAH - EKSPANSI MASA DEPAN]**
  * **Dampak**: Fitur jangka panjang jika GlowMatch memperluas layanan ke uji coba aksesoris tubuh penuh (pakaian, gaun, dll.).
  * **Solusi**: Mengintegrasikan 33 koordinat sendi tubuh spasial (MediaPipe Pose) dan memetakan koordinat tersebut ke dalam model bodi 3D (*rigged mesh* seperti model SMPL) menggunakan *Inverse Kinematics* (IK).
  
- [ ] **4. Pembangkit Kolider Tubuh Dinamis (`src/body_collider.rs`)**
  * **Prioritas**: **[PRIORITAS RENDAH - EKSPANSI MASA DEPAN]**
  * **Dampak**: Diperlukan untuk simulasi anting-anting/kalung virtual agar jatuh secara alami dan menabrak bahu pengguna.
  * **Solusi**: Mengekstrapolasi kapsul kolider 3D di sekitar leher, bahu, lengan, dan dada secara real-time berdasarkan koordinat sendi.

---

## 💄 Kategori 3: Spesialisasi Kecantikan & Riasan Premium (Beauty-Tech)

- [ ] **5. Triangulator Kosmetik Lokal Delaunay (`src/makeup_triangulator.rs`)**
  * **Prioritas**: **[PRIORITAS UTAMA - DIBUTUHKAN LANGSUNG UNTUK GLOWMATCH]**
  * **Dampak**: Memberikan presisi rendering 3D pada batas-batas sensitif kelopak mata dan bibir agar makeup tidak pecah atau bocor keluar dari garis bibir saat berbicara/membuka mulut.
  * **Solusi**: Modul yang melakukan Delaunay Triangulation lokal khusus untuk sub-area kosmetik (bibir atas/bawah terpisah, kelopak mata berkedip).

- [ ] **6. PBR Shader & Dynamic Specular Lighting (Matte, Satin, Dewy)**
  * **Prioritas**: **[PRIORITAS UTAMA - DIBUTUHKAN LANGSUNG UNTUK GLOWMATCH]**
  * **Dampak**: Membuat foundation, highlighter, dan lipstik terlihat ultra-realistis dengan memantulkan cahaya ruangan asli (kilau bergerak secara dinamis mengikuti arah lampu saat kepala menoleh).
  * **Solusi**: Fragment shader berbasis *Physically Based Rendering* (PBR) di native C++ GLES3 (GGX specular model untuk kilau basah, parametrisasi roughness/metallicness).

- [ ] **7. Skin Normal Map Shader (Preservasi Pori & Tekstur Kulit)**
  * **Prioritas**: **[PRIORITAS UTAMA - DIBUTUHKAN LANGSUNG UNTUK GLOWMATCH]**
  * **Dampak**: Mencegah efek "wajah topeng plastik" saat pengguna menggunakan foundation tebal. Wajah tetap terlihat sebagai kulit manusia asli yang halus, sehat, dan berpori alami.
  * **Solusi**: Normal mapping ringan untuk mempertahankan pori-pori halus dan struktur kulit asli di bawah lapisan foundation virtual.

- [ ] **8. Analisis Tekstur & Kesehatan Kulit LBP (`src/texture_analyzer.rs`)**
  * **Prioritas**: **[PRIORITAS UTAMA - SCANNER & PRODUCT RECOMMENDATION]**
  * **Dampak**: Menjadi otak dari fitur analisis kesehatan kulit GlowMatch (mendeteksi kerutan, noda jerawat, kehalusan kulit) untuk memberikan rekomendasi produk makeup/skincare.
  * **Solusi**: Algoritma penganalisis tekstur kulit menggunakan filter *Local Binary Patterns* (LBP) dan operator Sobel.

- [ ] **9. Bayangan Lipatan Wajah (Ambient Occlusion & Shading)**
  * **Prioritas**: **[PRIORITAS MENENGAH - PENYEMPURNAAN KONTUR]**
  * **Dampak**: Memberikan efek kedalaman 3D pada kontur wajah (cuping hidung, bawah bibir, kelopak mata) sehingga riasan tidak terlihat flat.
  * **Solusi**: Simulasi bayangan gelap pada area lipatan wajah agar objek virtual tidak terlihat seperti tempelan stiker datar.

- [ ] **10. Penyelarasan Cahaya Lingkungan Dinamis (Ambient Relighting Engine)**
  * **Prioritas**: **[PRIORITAS UTAMA - DIBUTUHKAN LANGSUNG UNTUK GLOWMATCH]**
  * **Dampak**: Mengantisipasi warna lampu ruangan pengguna (hangat/dingin) agar warna lipstik dan foundation otomatis menyesuaikan secara organik, tidak terlihat menyala terang sendirian di ruangan redup.
  * **Solusi**: Analisis histogram citra background secara real-time untuk memperkirakan nilai suhu warna ($T_{ambient}$) dan intensitasnya ($I_{ambient}$).

- [ ] **11. Kilau Shimmer Fisik Berbasis Giroskop (IMU Specular Shimmer)**
  * **Prioritas**: **[PRIORITAS UTAMA - GLOWMATCH SHIMMER]**
  * **Dampak**: Efek highlighter/gliter kelap-kelip berkilau dinamis di tulang pipi dan kelopak mata yang bergerak responsif mengikuti arah orientasi kemiringan kepala pengguna nyata.
  * **Solusi**: Menggeser koordinat tekstur gliter (*specular noise map*) di fragment shader berdasarkan arah rotasi roll, pitch, dan yaw dari sensor giroskop (IMU) menggunakan noise Voronoi.

- [x] **12. Klasifikasi Rona Kulit Spektrofotometri (ITA° Skin Undertone Classifier) (`src/skin_analyzer.rs`)**
  * **Prioritas**: **[PRIORITAS UTAMA - SCANNER PERSONALISASI]**
  * **Dampak**: Mengukur tingkat pigmentasi kulit dan rona bawah kulit secara ilmiah dan objektif untuk mencocokkan shade foundation produk kosmetik dengan akurasi klinis.
  * **Solusi**: Segmentasi area kulit dahi/pipi, konversi warna piksel ke ruang warna CIE L\*a\*b\*, dan kalkulasi rumus *Individual Typology Angle* (ITA°):
    $$\text{ITA}^\circ = \arctan\left(\frac{L^* - 50}{b^*}\right) \times \frac{180}{\pi}$$

- [ ] **13. Masking Batas Halus Tepi Rambut (Hairline Soft-Blending Mask)**
  * **Prioritas**: **[PRIORITAS UTAMA - DIBUTUHKAN LANGSUNG UNTUK GLOWMATCH]**
  * **Dampak**: Menghilangkan batas pemotongan kasar foundation di area dahi dekat garis rambut (menyatu secara organik dengan folikel rambut asli).
  * **Solusi**: Penggunaan *Alpha Distance Field* (ADF) dan noise Perlin di shader untuk membuat gradasi transparansi non-linear yang memudar halus saat mendekati garis hairline.

- [ ] **14. Sigmoid Slider Wipe Divider (Transisi Geser Premium)**
  * **Prioritas**: **[PRIORITAS MENENGAH - DESAIN PREMIUM]**
  * **Dampak**: Efek visual pembanding "sebelum/sesudah" pada slider pembagi yang memiliki transisi tepi lembut (*feathering*), menghilangkan pemotongan garis kasar yang kaku.
  * **Solusi**: Blending alpha riasan menggunakan fungsi sigmoid halus di shader di koordinat X slider:
    $$\alpha_{\text{makeup}} = \frac{1}{1 + e^{-k(x - X_{\text{slider}})}}$$

---

## 📦 Kategori 4: Fitur Dasar & Infrastruktur Core yang Sudah Selesai (`src/`)

- [x] **15. Oklusi Temporal Segmentasi (`src/segmentation.rs`)** (Mencegah flicker oklusi tangan/rambut lewat filter IIR)
- [x] **16. Penganalisis Kulit CIELAB & Fitzpatrick (`src/skin_analyzer.rs`)** (Kombinasi white balance ambient SH dan diagnostic Fitzpatrick)
- [x] **17. Harmonisasi Warna Musiman CIELAB (`src/color_harmonizer.rs`)** (Pencari shade musim kecantikan berbasis jarak Euclidean terdekat)
- [x] **18. Pengaturan Riasan Fisik PBR (`src/pbr_makeup.rs`)** (Penyedia helper parameter roughness, metallic, dan Schlick Fresnel)
- [x] **19. Pelacakan Iris Sferis & Dilatasi Pupil (`src/eye_contacts.rs`)** (Warping sferis softlens warna dan dilatasi pupil dinamis)
- [x] **20. Ekstrapolator Leher Virtual 3D (`src/face.rs` & `src/lib.rs`)** (Drape foundation leher mengikuti rotasi kepala 3D rahang bawah)
- [x] **21. Penyelarasan Frame Percepatan EKF MSCKF (`src/msckf.rs`)** (Geometri VIO akselerasi lokal $\mathbf{a}_{local} = \Delta R^T \mathbf{a}_{b_i}$)
- [x] **22. Marginalisasi Kovariansi Dinamis EKF MSCKF (`src/msckf.rs`)** (Penyusutan kovariansi sliding window dari $15+6N$ ke $15+6(N-1)$)
- [x] **23. Late Latching Motion Extrapolator dengan Kuaternion (`src/extrapolator.rs`)** (Prediksi pose visual gyroscope/IMU)
- [x] **24. Estimator Ambient SH Cahaya Kamera Perspektif (`src/lighting.rs`)** (Proyeksi 9 koefisien SH Orde 2)
- [x] **25. Proteksi Raycast TSDF Voxel Hashing (`src/tsdf.rs`)** (Mencegah alokasi voxel negatif belakang kamera)
- [x] **26. Interleaved Face Mesh VBO Layout (`src/face.rs`)** (Layout 20 bytes per vertex kontigu untuk reduksi cache misses GPU)
