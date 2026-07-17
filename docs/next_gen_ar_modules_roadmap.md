# Peta Jalan Pengembangan: Modul AR Tingkat Lanjut untuk Deteksi Wajah & Tubuh Sempurna

Dokumen ini merumuskan modul-modul tambahan yang direkomendasikan untuk diimplementasikan ke dalam **Fizgravity AR Engine** guna mencapai akurasi pelacakan spasial yang sempurna dan mendukung deteksi seluruh tubuh (*full-body tracking*) secara fotorealistik pada aplikasi **GlowMatch**.

---

## 🛡️ Kategori 1: Akurasi & Stabilisasi (Menghilangkan Jitter & Distorsi)

Agar pelacakan tidak bergetar sama sekali (*zero jitter*) tanpa memperkenalkan jeda gerakan (*zero lag*), kita membutuhkan:

### 1. Modul Stabilisasi Adaptif One-Euro Filter (`src/stabilizer.rs`)
*   **Masalah**: Koordinat wajah/tubuh dari ML Kit sering bergetar halus (*high-frequency jitter*) saat pengguna diam, namun filter rata-rata biasa membuat gerakan terasa lambat (*lag*).
*   **Solusi**: Terapkan **One-Euro Filter**, sebuah filter low-pass adaptif yang mengubah frekuensi cutoff secara dinamis berdasarkan kecepatan gerakan:
    *   *Kepala Diam*: Redaman ditingkatkan maksimal untuk menghilangkan getaran (lipstik terlihat kokoh).
    *   *Kepala Bergerak Cepat*: Redaman diturunkan ke nol agar koordinat langsung mengikuti gerakan wajah instan tanpa jeda fase (*phase lag*).

### 2. Auto-Kalibrasi Intrinsik Kamera (`src/calibration.rs`)
*   **Masalah**: Ponsel Android memiliki sudut pandang lensa (*Field of View*) yang berbeda-beda. Estimasi fokal statis membuat proyeksi 3D tampak melar di kamera *ultra-wide*.
*   **Solusi**: Modul kalibrasi online yang mengestimasi parameter kamera ($f_x, f_y, c_x, c_y$) secara dinamis berdasarkan rasio jarak pupil mata pengguna yang konstan secara antropometri.

---

## 👗 Kategori 2: Deteksi Badan Penuh & Interaksi Spasial (Full-Body AR)

Untuk mendukung fitur uji coba pakaian virtual (*Virtual Try-On*), perhiasan (anting, kalung), dan pelacakan postur tubuh:

### 3. Modul Pelacakan Sendi Tubuh 3D (`src/body_pose.rs`)
*   **Solusi**: Mengintegrasikan 33 koordinat sendi tubuh spasial (MediaPipe Pose) dan memetakan koordinat tersebut ke dalam model bodi 3D (*rigged mesh* seperti model SMPL) menggunakan **Inverse Kinematics (IK)**. Ini memungkinkan pakaian virtual (baju, celana) berdeformasi mengikuti lekuk tubuh nyata.

### 4. Pembangkit Kolider Tubuh Dinamis (`src/body_collider.rs`)
*   **Solusi**: Mengekstrapolasi kapsul kolider 3D di sekitar leher, bahu, lengan, dan dada secara real-time berdasarkan koordinat sendi. Ini memastikan kalung, anting-anting, atau tali pakaian virtual jatuh secara gravitasi dan menabrak (*collision*) pundak fisik secara realistis menggunakan solver fisika `physics.rs`.

---

## 💄 Kategori 3: Spesialisasi Kecantikan & Riasan Premium (Beauty-Tech)

Untuk meningkatkan detail dan kualitas rendering riasan kosmetik pada wajah:

### 5. Triangulator Kosmetik Lokal Delaunay (`src/makeup_triangulator.rs`)
*   **Masalah**: Menggambar riasan dengan triangle fan global pada seluruh wajah tidak memberikan presisi pada batas-batas sensitif (seperti ujung bibir atau kelopak mata).
*   **Solusi**: Modul yang melakukan **Delaunay Triangulation** lokal khusus untuk sub-area kosmetik:
    *   *Bibir*: Triangulasi bibir atas dan bawah secara terpisah dengan parameter *border feathering* (gradasi tepi yang halus).
    *   *Mata*: Triangulasi kelopak mata (*eyeshadow*) yang mengikuti lipatan mata dinamis saat berkedip.

### 6. Analisis Tekstur & Kesehatan Kulit LBP (`src/texture_analyzer.rs`)
*   **Solusi**: Mengembangkan algoritma penganalisis tekstur kulit menggunakan filter **Local Binary Patterns (LBP)** dan operator Sobel pada citra wajah untuk mendeteksi:
    *   Kerutan wajah (Wrinkle depth & count).
    *   Tingkat kehalusan/kekasaran kulit (Skin roughness index).
    *   Deteksi noda jerawat atau flek hitam secara spasial.

### 7. Bayangan Lipatan Wajah (Ambient Occlusion)
*   **Solusi**: Modul pencahayaan yang mensimulasikan bayangan gelap pada area lipatan wajah (cuping hidung, bawah bibir, dan kelopak mata) agar objek virtual tidak terlihat seperti "tempelan stiker" datar.
