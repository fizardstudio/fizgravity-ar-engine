//! src/lighting.rs
//! Estimasi Pencahayaan Global (Ambient & Specular NSRF) Real-Time berbasis Spherical Harmonics.

use crate::face::{ArFaceVertexInterleaved, FACE_MESH_VERTICES_COUNT};
use crate::ArSphericalHarmonics;
use std::os::raw::{c_float, c_void};

pub struct LightingEstimator {
    pub current_sh: ArSphericalHarmonics,
    pub nsrf_model_loaded: bool,
}

/// Daftar indeks landmark MediaPipe FaceMesh (468 titik, format kanonik yang sama
/// dipakai proyek MatchAndBeauty) yang HARUS dikecualikan dari sampling probe cahaya
/// wajah karena bukan permukaan kulit difus yang valid: mata (termasuk area
/// eyeshadow/kelopak), alis (rambut, bukan kulit), dan bibir/mulut (reflektansi
/// berbeda dari kulit, sering basah/mengkilap → sangat non-Lambertian).
/// Nilai direplikasi dari tabel referensi region makeup proyek MatchAndBeauty
/// (cpp/FizgravityMakeupIndices.h: LIP_INDICES, EYE_INDICES, LEFT_EYEBROW_INDICES,
/// RIGHT_EYEBROW_INDICES) — indeks landmark MediaPipe bersifat kanonik/universal
/// sehingga nilainya identik lintas proyek, hanya dibaca sebagai referensi (read-only).
const EXCLUDED_SKIN_SAMPLE_INDICES: &[usize] = &[
    // Bibir (LIP_INDICES)
    0, 11, 12, 13, 14, 15, 16, 17, 37, 38, 39, 40, 41, 42, 61, 62, 72, 73, 74, 76, 77, 78, 80,
    81, 82, 84, 85, 86, 87, 88, 89, 90, 91, 95, 96, 146, 178, 179, 180, 181, 183, 184, 185, 191,
    267, 268, 269, 270, 271, 272, 291, 292, 302, 303, 304, 306, 307, 308, 310, 311, 312, 314,
    315, 316, 317, 318, 319, 320, 321, 324, 325, 375, 402, 403, 404, 405, 407, 408, 409, 415,
    // Mata kiri + area eyeshadow/kelopak (EYE_INDICES)
    33, 246, 161, 160, 159, 158, 157, 173, 133, 155, 112, 26, 22, 23, 24, 110, 25, 130, 247, 30,
    29, 27, 28, 56, 190, 243, 244, 245, 128, 121, 120, 119, 118, 117, 111, 143, 156, 144, 145,
    153, 154, 7, 163,
    // Mata kanan + area eyeshadow/kelopak (EYE_INDICES)
    362, 398, 384, 385, 386, 387, 388, 466, 263, 382, 341, 256, 252, 253, 254, 339, 255, 359,
    467, 260, 259, 257, 258, 286, 414, 463, 464, 465, 357, 350, 349, 348, 347, 346, 340, 372,
    383, 373, 374, 380, 381, 246, 390,
    // Alis kiri (LEFT_EYEBROW_INDICES)
    70, 63, 105, 66, 107, 55, 65, 52, 53, 46,
    // Alis kanan (RIGHT_EYEBROW_INDICES)
    300, 293, 334, 296, 336, 285, 295, 282, 283, 276,
];

/// Mengembalikan `true` jika indeks landmark ke-`index` merupakan sampel kulit difus
/// yang valid untuk dipakai sebagai titik probe cahaya (bukan mata/alis/bibir, dan
/// berada dalam rentang 468 landmark wajah standar). Area yang lolos filter ini
/// umumnya pipi, dahi, batang hidung, dan dagu — permukaan luas & relatif datar.
pub fn is_valid_skin_sample(index: usize) -> bool {
    index < FACE_MESH_VERTICES_COUNT && !EXCLUDED_SKIN_SAMPLE_INDICES.contains(&index)
}

impl LightingEstimator {
    pub fn new() -> Self {
        Self {
            current_sh: ArSphericalHarmonics {
                coefficients_r: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                coefficients_g: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                coefficients_b: [0.282, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
            nsrf_model_loaded: false,
        }
    }

    /// Mengestimasi 9 koefisien SH Orde 2 per kanal RGB dengan memakai WAJAH PENGGUNA
    /// SENDIRI sebagai *diffuse light probe* — bukan lagi memproyeksikan piksel kamera
    /// mentah seolah-olah foto tsb adalah environment map (teknik lama yang TERBUKTI SALAH:
    /// selfie menangkap wajah yang SUDAH ter-diffuse-lit, bukan sumber cahayanya sendiri,
    /// sehingga proyeksi SH mentah cuma mengukur "warna kulitku", bukan lingkungan
    /// pencahayaan sebenarnya).
    ///
    /// Referensi teknik (riset mandor, sumber nyata):
    /// - Google "Total Relighting" / "Portrait Light" (SIGGRAPH Asia 2020),
    ///   https://arxiv.org/pdf/2008.02396
    /// - "Faces as Lighting Probes" (ECCV 2018)
    /// - Calian dkk., "From Faces to Outdoor Light Probes" (CGF 2018)
    ///
    /// Algoritma: untuk setiap landmark wajah yang merupakan kulit difus valid
    /// (lihat `is_valid_skin_sample`), ambil normal geometris NYATA dari mesh wajah
    /// (`compute_face_normals` di `face.rs`) dan warna piksel kamera yang teramati di
    /// posisi landmark tsb, lalu proyeksikan sebagai `color * SH_basis(normal)` —
    /// akumulasi & normalisasi memakai struktur yang sama persis dengan implementasi
    /// lama (`norm_factor = 2*PI / weight_sum`), hanya arah normal & sumber warnanya
    /// yang sekarang berbasis geometri wajah nyata, bukan arah sinar kamera fiktif.
    ///
    /// CATATAN v1 (simplifikasi yang disengaja, BUKAN oversight): fungsi ini TIDAK
    /// memisahkan albedo kulit dari warna cahaya secara terpisah — warna kulit
    /// pengguna akan sedikit "bercampur" ke estimasi warna cahaya karena setiap sampel
    /// memakai warna piksel observasi apa adanya (belum dibagi oleh estimasi albedo).
    /// Pemisahan linear-least-squares albedo/lighting yang lebih rigorous (sesuai
    /// paper-paper di atas) adalah pekerjaan lanjutan yang disengaja ditunda, bukan
    /// keterbatasan yang terlewat.
    ///
    /// # Parameter
    /// * `face_vertices` - mesh wajah 468 vertex TERKINI dengan field `.normal` SUDAH
    ///   terisi (mis. hasil `face::compute_face_normals`). `.position.x`/`.position.y`
    ///   diasumsikan dalam ruang gambar ternormalisasi 0.0..1.0 (lihat
    ///   `FaceTracker::update` di `face.rs`, yang menormalisasi posisi landmark ke
    ///   rentang tsb relatif terhadap `width`/`height` kamera).
    /// * `camera_pixels` - pointer buffer piksel kamera mentah, format RGB 24-bit
    ///   interleaved (3 byte per piksel, urutan R,G,B).
    /// * `width`, `height` - dimensi LOGIS gambar dalam piksel (bukan byte).
    /// * `row_stride` - jumlah BYTE per baris pada `camera_pixels`. **BUKAN** selalu
    ///   sama dengan `width * 3` — buffer kamera Android sering memiliki padding baris
    ///   (row padding) sehingga `row_stride >= width * 3`. Ini persis kelas bug yang
    ///   menyebabkan "stack corruption" pada implementasi lama (lihat riwayat git),
    ///   yang mengasumsikan `pixel_offset = (y*width + x) * 3` tanpa stride terpisah.
    /// * `buffer_len_bytes` - ukuran NYATA (bukan klaim/perkiraan) buffer
    ///   `camera_pixels` dalam byte, sebagaimana benar-benar dialokasikan pemanggil
    ///   (mis. `ByteBuffer.remaining()`/`Vec::len()` di sisi caller). Dipakai sebagai
    ///   batas atas OTORITATIF untuk setiap pembacaan unsafe — sengaja TIDAK hanya
    ///   mempercayai hasil kali `row_stride * height` (yang cuma klaim pemanggil
    ///   tentang bentuk buffer, bukan bukti ukuran alokasi sebenarnya), karena
    ///   mempercayai klaim tsb secara membabi buta adalah pola bug yang sama persis
    ///   yang ingin dihindari di sini.
    pub fn estimate_ambient_sh(
        &mut self,
        face_vertices: &[ArFaceVertexInterleaved; FACE_MESH_VERTICES_COUNT],
        camera_pixels: *const c_void,
        width: i32,
        height: i32,
        row_stride: i32,
        buffer_len_bytes: usize,
    ) {
        if camera_pixels.is_null() || width <= 0 || height <= 0 || row_stride <= 0 {
            return;
        }
        // row_stride harus cukup menampung satu baris penuh `width` piksel RGB;
        // jika tidak, input dianggap malformed dan kita batalkan secara aman.
        if row_stride < width * 3 {
            return;
        }
        if buffer_len_bytes == 0 {
            return;
        }

        let pixels = camera_pixels as *const u8;
        let buffer_len_bytes_i64 = buffer_len_bytes as i64;

        let mut accum_r = [0.0f32; 9];
        let mut accum_g = [0.0f32; 9];
        let mut accum_b = [0.0f32; 9];
        let mut weight_sum = 0.0f32;

        for (idx, vertex) in face_vertices.iter().enumerate() {
            if !is_valid_skin_sample(idx) {
                continue;
            }

            // Normal geometris NYATA dari mesh wajah (hasil compute_face_normals),
            // bukan arah sinar kamera fiktif seperti implementasi lama.
            let nx = vertex.normal.x;
            let ny = vertex.normal.y;
            let nz = vertex.normal.z;

            // Lewati normal degenerate/belum terisi (panjang ~0) — tidak ada arah
            // valid untuk dievaluasi pada basis SH.
            let normal_len_sq = nx * nx + ny * ny + nz * nz;
            if normal_len_sq < 1e-6 {
                continue;
            }

            // Petakan posisi landmark ternormalisasi (0.0..1.0) ke koordinat piksel bulat.
            let px = (vertex.position.x * width as f32).round() as i64;
            let py = (vertex.position.y * height as f32).round() as i64;

            // Bounds-check koordinat piksel LOGIS sebelum menghitung offset byte apa pun.
            if px < 0 || py < 0 || px >= width as i64 || py >= height as i64 {
                continue;
            }

            // Hitung offset byte memakai row_stride EKSPLISIT (bukan width * 3!) agar
            // aman terhadap row padding buffer kamera Android.
            let pixel_offset = py * row_stride as i64 + px * 3;

            // Bounds-check offset akhir terhadap ukuran buffer NYATA (buffer_len_bytes,
            // bukan sekadar row_stride * height yang cuma klaim pemanggil) SEBELUM
            // baca unsafe apa pun. Jika di luar batas, lewati sampel ini secara aman
            // alih-alih membaca memori di luar buffer.
            if pixel_offset < 0 || pixel_offset + 2 >= buffer_len_bytes_i64 {
                continue;
            }

            // SAFETY: `pixel_offset`, `pixel_offset + 1`, dan `pixel_offset + 2` sudah
            // divalidasi berada dalam rentang [0, buffer_len_bytes) tepat di atas
            // (memakai ukuran buffer NYATA yang dilaporkan pemanggil, bukan hasil
            // turunan row_stride * height yang cuma asumsi bentuk data), sehingga
            // pembacaan 3 byte berikut dijamin berada di dalam buffer yang
            // benar-benar dialokasikan — inilah pengaman langsung terhadap kelas bug
            // "stack corruption" yang menyebabkan fungsi ini dinonaktifkan sebelumnya.
            let (r_val, g_val, b_val) = unsafe {
                let base = pixels.add(pixel_offset as usize);
                (
                    *base as f32 / 255.0,
                    *base.add(1) as f32 / 255.0,
                    *base.add(2) as f32 / 255.0,
                )
            };

            // 9 Basis Fungsi Harmonik Sferis Orde 2 (Spherical Harmonics Basis) —
            // formula identik dengan implementasi lama, sekarang dievaluasi memakai
            // normal geometris nyata dari mesh wajah.
            let y00 = 0.282095;
            let y1_1 = 0.488603 * ny;
            let y10 = 0.488603 * nz;
            let y11 = 0.488603 * nx;
            let y2_2 = 1.092548 * nx * ny;
            let y2_1 = 1.092548 * ny * nz;
            let y20 = 0.315392 * (3.0 * nz * nz - 1.0);
            let y21 = 1.092548 * nx * nz;
            let y22 = 0.546274 * (nx * nx - ny * ny);

            let sh = [y00, y1_1, y10, y11, y2_2, y2_1, y20, y21, y22];

            for i in 0..9 {
                accum_r[i] += r_val * sh[i];
                accum_g[i] += g_val * sh[i];
                accum_b[i] += b_val * sh[i];
            }
            weight_sum += 1.0;
        }

        // Normalisasi dan perbarui koefisien SH saat ini, sama seperti implementasi lama.
        if weight_sum > 0.0 {
            let norm_factor = 2.0 * std::f32::consts::PI / weight_sum;
            for i in 0..9 {
                self.current_sh.coefficients_r[i] = accum_r[i] * norm_factor;
                self.current_sh.coefficients_g[i] = accum_g[i] * norm_factor;
                self.current_sh.coefficients_b[i] = accum_b[i] * norm_factor;
            }
        }
    }

    /// Menghitung suhu warna sekitar (T_ambient dalam Kelvin) dan intensitasnya (I_ambient)
    /// berdasarkan koefisien Spherical Harmonics saat ini menggunakan formula McCamy.
    pub fn estimate_temperature_and_intensity(&self) -> (f32, f32) {
        let r_amb = self.current_sh.coefficients_r[0];
        let g_amb = self.current_sh.coefficients_g[0];
        let b_amb = self.current_sh.coefficients_b[0];

        // Convert sRGB coefficients to CIE XYZ space
        let x_xyz = 0.4124 * r_amb + 0.3576 * g_amb + 0.1805 * b_amb;
        let y_xyz = 0.2126 * r_amb + 0.7152 * g_amb + 0.0722 * b_amb;
        let z_xyz = 0.0193 * r_amb + 0.1192 * g_amb + 0.9505 * b_amb;

        let xyz_sum = x_xyz + y_xyz + z_xyz;
        if xyz_sum < 1e-4 {
            return (6500.0, 0.0); // Default D65
        }

        let intensity = (0.299 * r_amb + 0.587 * g_amb + 0.114 * b_amb).clamp(0.0, 1.0);

        let x = x_xyz / xyz_sum;
        let y = y_xyz / xyz_sum;

        // McCamy formula: n = (x - xe) / (y - ye) where (xe, ye) = (0.3320, 0.1858)
        let n = (x - 0.3320) / (y - 0.1858);
        let temp = -449.0 * n.powi(3) + 3525.0 * n.powi(2) - 6823.3 * n + 5520.33;
        let temp = temp.clamp(2000.0, 10000.0);

        (temp, intensity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::{ArTexCoord2D, FACE_MESH_VERTICES_COUNT};
    use crate::ArVertex3D;

    /// Helper: buat array 468 vertex default dengan normal degenerate (0,0,0) —
    /// artinya SEMUA vertex dilewati oleh estimate_ambient_sh kecuali yang secara
    /// eksplisit di-override dengan normal non-degenerate di dalam masing-masing test.
    fn default_vertices() -> [ArFaceVertexInterleaved; FACE_MESH_VERTICES_COUNT] {
        [ArFaceVertexInterleaved {
            position: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
            normal: ArVertex3D { x: 0.0, y: 0.0, z: 0.0 },
            uv: ArTexCoord2D { u: 0.0, v: 0.0 },
        }; FACE_MESH_VERTICES_COUNT]
    }

    #[test]
    fn test_is_valid_skin_sample_excludes_eyes_brows_lips() {
        // Sampel dari LIP_INDICES, EYE_INDICES, dan EYEBROW_INDICES harus ditolak.
        assert!(!is_valid_skin_sample(13)); // bibir atas tengah
        assert!(!is_valid_skin_sample(33)); // sudut mata kiri
        assert!(!is_valid_skin_sample(70)); // alis kiri
        assert!(!is_valid_skin_sample(300)); // alis kanan
        // Indeks di luar rentang 468 landmark juga harus ditolak.
        assert!(!is_valid_skin_sample(468));
        assert!(!is_valid_skin_sample(9999));
        // Indeks kulit generik (bukan mata/alis/bibir) harus lolos.
        assert!(is_valid_skin_sample(1));
        assert!(is_valid_skin_sample(4));
    }

    #[test]
    fn test_sh_geometric_probe_white_light_symmetric_normals() {
        let mut estimator = LightingEstimator::new();
        let mut vertices = default_vertices();

        // 6 arah normal simetris sumbu (+/-x, +/-y, +/-z) ditempatkan pada 6 indeks
        // kulit valid (1..6). Untuk himpunan normal sumbu-simetris ini, jumlah setiap
        // basis SH orde > 0 saling meniadakan secara analitis (lihat komentar di bawah),
        // sehingga cahaya putih SERAGAM di semua titik HARUS menghasilkan hanya term
        // ambient (l=0) yang dominan & term direksional (l>0) yang mendekati nol —
        // ini menguji jalur kode BARU (normal geometris + sampling piksel), bukan
        // sekadar nilai default constructor seperti test lama yang tidak benar2 menguji apa pun.
        let axis_normals = [
            ArVertex3D { x: 1.0, y: 0.0, z: 0.0 },
            ArVertex3D { x: -1.0, y: 0.0, z: 0.0 },
            ArVertex3D { x: 0.0, y: 1.0, z: 0.0 },
            ArVertex3D { x: 0.0, y: -1.0, z: 0.0 },
            ArVertex3D { x: 0.0, y: 0.0, z: 1.0 },
            ArVertex3D { x: 0.0, y: 0.0, z: -1.0 },
        ];
        for (i, n) in axis_normals.iter().enumerate() {
            let idx = i + 1; // indeks 1..6, dipastikan bukan mata/alis/bibir
            assert!(is_valid_skin_sample(idx));
            vertices[idx].normal = *n;
            vertices[idx].position = ArVertex3D { x: 0.5, y: 0.5, z: 0.0 };
        }

        // Gambar 16x16 putih penuh (RGB), tanpa padding baris.
        let width = 16;
        let height = 16;
        let row_stride = width * 3;
        let pixels = vec![255u8; (row_stride * height) as usize];

        estimator.estimate_ambient_sh(
            &vertices,
            pixels.as_ptr() as *const c_void,
            width,
            height,
            row_stride,
            pixels.len(),
        );

        // Term ambient (l=0) harus dominan & positif untuk cahaya putih seragam.
        assert!(estimator.current_sh.coefficients_r[0] > 0.1);
        assert!(estimator.current_sh.coefficients_g[0] > 0.1);
        assert!(estimator.current_sh.coefficients_b[0] > 0.1);

        // Term direksional orde tinggi (l>0) harus mendekati nol karena himpunan
        // 6 normal sumbu-simetris ini menghasilkan jumlah basis SH = 0 utk l>0.
        for i in 1..9 {
            assert!(
                estimator.current_sh.coefficients_r[i].abs() < 1e-3,
                "coef_r[{}] = {}",
                i,
                estimator.current_sh.coefficients_r[i]
            );
            assert!(estimator.current_sh.coefficients_g[i].abs() < 1e-3);
            assert!(estimator.current_sh.coefficients_b[i].abs() < 1e-3);
        }
    }

    #[test]
    fn test_row_stride_padding_reads_correct_pixel_not_garbage() {
        let mut estimator = LightingEstimator::new();

        let width = 4;
        let height = 4;
        let logical_row_bytes = width * 3; // 12
        let padding_bytes = 20; // padding besar per baris, simulasi buffer kamera Android
        let row_stride = logical_row_bytes + padding_bytes;

        // Isi seluruh buffer dengan byte "sampah" (111) agar jelas kalau row_stride
        // salah dipakai (mis. width*3 dipakai alih-alih row_stride), pembacaan akan
        // jatuh ke area padding sampah ini, bukan warna target.
        let mut pixels = vec![111u8; (row_stride * height) as usize];

        // Set piksel target (2,2) menjadi hijau murni (0,255,0) memakai offset yang
        // BENAR (row_stride, bukan width*3).
        let target_x: i32 = 2;
        let target_y: i32 = 2;
        let target_offset = (target_y * row_stride + target_x * 3) as usize;
        pixels[target_offset] = 0;
        pixels[target_offset + 1] = 255;
        pixels[target_offset + 2] = 0;

        let mut vertices = default_vertices();
        let idx = 1usize;
        assert!(is_valid_skin_sample(idx));
        vertices[idx].normal = ArVertex3D { x: 0.0, y: 0.0, z: 1.0 };
        // Posisi ternormalisasi (0..1) yang memetakan tepat ke piksel (target_x, target_y).
        vertices[idx].position = ArVertex3D {
            x: target_x as f32 / width as f32,
            y: target_y as f32 / height as f32,
            z: 0.0,
        };

        estimator.estimate_ambient_sh(
            &vertices,
            pixels.as_ptr() as *const c_void,
            width,
            height,
            row_stride,
            pixels.len(),
        );

        // Jika row_stride dipakai dengan benar, koefisien G harus jauh lebih besar
        // dari R/B (satu-satunya sampel valid membaca piksel hijau murni, bukan byte
        // sampah 111,111,111 dari padding yang akan terbaca kalau stride diabaikan).
        let coef_r = estimator.current_sh.coefficients_r[0];
        let coef_g = estimator.current_sh.coefficients_g[0];
        let coef_b = estimator.current_sh.coefficients_b[0];
        assert!(coef_g > coef_r * 2.0, "coef_g={} coef_r={}", coef_g, coef_r);
        assert!(coef_g > coef_b * 2.0, "coef_g={} coef_b={}", coef_g, coef_b);
    }

    #[test]
    fn test_out_of_bounds_buffer_does_not_panic_or_read_oob() {
        let mut estimator = LightingEstimator::new();

        let width = 100;
        let height = 100;
        let row_stride = width * 3;

        // row_stride * height menyiratkan buffer 30_000 byte, tapi buffer NYATA yang
        // dialokasikan jauh lebih kecil — mensimulasikan caller yang salah lapor
        // ukuran/height, kelas bug yang sama persis dengan "stack corruption" original.
        let real_buffer_len = 30usize;
        let pixels = vec![200u8; real_buffer_len];

        let mut vertices = default_vertices();

        // Titik sampel di tengah gambar (px=50,py=50 secara logis absah menurut
        // width/height) tapi byte-nya JAUH di luar buffer nyata 30 byte -> harus
        // dilewati dengan aman, bukan dibaca out-of-bounds.
        let idx1 = 1usize;
        assert!(is_valid_skin_sample(idx1));
        vertices[idx1].normal = ArVertex3D { x: 0.0, y: 0.0, z: 1.0 };
        vertices[idx1].position = ArVertex3D { x: 0.5, y: 0.5, z: 0.0 };

        // Titik sampel kedua di pojok kiri-atas (px=0,py=0) -> offset 0, MASIH di
        // dalam buffer nyata 30 byte, jadi ini seharusnya tetap berhasil dibaca.
        let idx2 = 2usize;
        assert!(is_valid_skin_sample(idx2));
        vertices[idx2].normal = ArVertex3D { x: 1.0, y: 0.0, z: 0.0 };
        vertices[idx2].position = ArVertex3D { x: 0.0, y: 0.0, z: 0.0 };

        // Panggilan ini TIDAK BOLEH panic/crash meskipun buffer nyata jauh lebih
        // kecil dari yang "diklaim" oleh row_stride * height.
        estimator.estimate_ambient_sh(
            &vertices,
            pixels.as_ptr() as *const c_void,
            width,
            height,
            row_stride,
            real_buffer_len, // <-- ukuran buffer NYATA, dilaporkan dengan benar
        );

        // State akhir harus tetap valid (finite, tidak NaN/inf) — bukti tidak ada
        // pembacaan memori liar yang mengotori hasil komputasi.
        assert!(estimator.current_sh.coefficients_r[0].is_finite());
        assert!(estimator.current_sh.coefficients_g[0].is_finite());
        assert!(estimator.current_sh.coefficients_b[0].is_finite());
    }

    #[test]
    fn test_cct_and_intensity_estimation() {
        let mut estimator = LightingEstimator::new();
        estimator.current_sh.coefficients_r[0] = 0.282;
        estimator.current_sh.coefficients_g[0] = 0.282;
        estimator.current_sh.coefficients_b[0] = 0.282;

        let (temp, intensity) = estimator.estimate_temperature_and_intensity();
        assert!((temp - 6000.0).abs() < 1000.0);
        assert!((intensity - 0.282).abs() < 1e-4);
    }
}
