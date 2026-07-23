# Panduan Instruksi: Agent Client App Integrator

Anda ditugaskan sebagai **Agent Client App Integrator** untuk menghubungkan AR Engine ke aplikasi **GlowMatch (Android C++ JNI & Flutter UI)**.

---

## 🎯 Fokus Wilayah Kerja
*   Direktori Utama: [`GlowMatch/android/app/src/main/cpp/`](file:///E:/APP%20PROJECT/GlowMatch/android/app/src/main/cpp/) (terutama `gl_mesh_engine.cpp`, `gl_renderer.cpp`).
*   Tanggung Jawab: Kamera JNI, penskalaan piksel bilinear, koordinasi VAO/VBO/EBO, dan fragment shader kecantikan (`MAKEUP_FS`).

---

## 🛡️ Aturan Kompilasi & Penulisan Kode
1.  **Gunakan Penskalaan Piksel Dinamis**:
    *   Selalu gunakan variabel global `g_rotated_width` dan `g_rotated_height` untuk melacak resolusi screen aktif. Jangan berasumsi resolusi statis $640 \times 480$ saat menggambar mesh di OpenGL.
2.  **Hanya Mengubah JNI & Renderer C++**:
    *   Abaikan semua file Rust di folder `New AR Engine/src/`. Tugas Anda murni selesai ketika renderer C++ berhasil menggambar riasan di atas kamera dengan rotasi tegak lurus dan Gradle berhasil di-build (`gradlew assembleDebug`).
