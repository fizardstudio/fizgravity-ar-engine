# Daftar Periksa Tugas (TODO List): Siklus 2 - Kompleksi Wajah (Foundation & Blush-On)

- [x] **Tugas 1: Adaptasi Radius Hairline Blending (Rust Core)**
  * File: `src/makeup_triangulator.rs`
  * Deskripsi: Sempurnakan fungsi `calculate_hairline_blending` agar radius pudar (fade radius) bisa beradaptasi secara dinamis terhadap ukuran wajah.
  
- [x] **Tugas 2: Deteksi Klasifikasi Undertone Kulit ITA° (Rust Core)**
  * File: `src/texture_analyzer.rs`
  * Deskripsi: Implementasikan algoritma penghitung sudut ITA (Individual Typology Angle) dari sampel RGB kulit untuk menentukan undertone (Cool, Neutral, Warm).
  
- [ ] **Tugas 3: High-Pass Blending Shader Foundation (Renderer C++)**
  * File: `gl_renderer.cpp`
  * Deskripsi: Tulis shader peredam warna foundation yang mempertahankan tekstur pori-pori kulit asli (High-Pass frequency preservation) agar bedak tidak terlihat seperti topeng plastik.
  
- [ ] **Tugas 4: Shader Blush-On & Koreksi Kelvin (Renderer C++)**
  * File: `gl_renderer.cpp`
  * Deskripsi: Implementasikan rendering pipi untuk Blush-On, dan pasang kalkulator suhu McCamy Kelvin agar warna blush-on otomatis beradaptasi dengan pencahayaan ruangan.
