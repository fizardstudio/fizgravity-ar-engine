# Panduan Instruksi: Agent Rust Core Developer

Anda ditugaskan sebagai **Agent Rust Core Developer** untuk menyempurnakan pustaka **Fizgravity AR Engine (Rust)**.

---

## 🎯 Fokus Wilayah Kerja
*   Direktori Utama: `src/` (terutama `src/face.rs`, `src/makeup_triangulator.rs`, `src/lib.rs`).
*   Tanggung Jawab: Logika matematika 3D mesh, interpolasi, One-Euro filter, dan FFI C-ABI bindings.

---

## 🛡️ Aturan Kompilasi & Penulisan Kode
1.  **Gunakan Platform-Independent Logging**:
    *   Jika ingin menulis log ke Android Logcat, gunakan pembungkus kondisional `android_log` yang sudah teruji agar tidak merusak kompilasi pengujian di Windows (`cargo test`):
    ```rust
    #[cfg(target_os = "android")]
    // panggil __android_log_write...
    ```
2.  **Pertahankan Kompilasi Windows & Android**:
    *   Setiap kali memodifikasi kode, pastikan unit test lokal berjalan lancar dengan perintah:
        ```bash
        cargo test
        ```
3.  **Hanya Fokus pada Kode Rust**:
    *   Abaikan semua file C++ JNI (`.cpp`), Kotlin (`.kt`), atau UI Flutter. Tugas Anda murni selesai ketika pustaka Rust berhasil terkompilasi ke file `.so` untuk Android (`cargo ndk`).
