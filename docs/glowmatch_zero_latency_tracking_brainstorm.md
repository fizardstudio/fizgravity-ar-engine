# Brainstorming: Solusi Lacak Tanpa Latensi (Zero-Lag AR Tracking)
## Rekayasa Sinkronisasi Frame-Perfect untuk GlowMatch

Tantangan terbesar dalam AR kosmetik premium adalah efek riasan yang terlihat "melayang" atau tertinggal di belakang wajah ketika pengguna menggelengkan kepala secara cepat. Hal ini disebabkan oleh **Latensi Jalur Kamera & AI** ($\sim 60-80\text{ms}$ total tumpukan delay dari penangkapan sensor kamera, inferensi neural network, hingga antrean render GPU).

Berikut adalah rancangan metode termutakhir (*state-of-the-art*) untuk mencapai **Zero-Lag Tracking**:

---

## ⚡ 1. Metode Utama: Inertial Late Latching & Quaternion Extrapolator

Metode ini telah diimplementasikan sebagian di core engine kita (`src/extrapolator.rs`) dan merupakan metode paling sempurna karena memanfaatkan kecepatan sensor inersia (giroskop/IMU) yang berjalan sangat cepat pada frekuensi **$1000\text{Hz}$** (dibandingkan kamera yang hanya $30\text{Hz}$ atau $60\text{Hz}$).

### Cara Kerja Matematis:
1.  Ketika frame kamera ditangkap pada waktu $T_{\text{capture}}$, AI membutuhkan waktu $\Delta t_{\text{infer}}$ ($\approx 30\text{ms}$) untuk mendeteksi landmark wajah.
2.  Selama $30\text{ms}$ tersebut, kepala pengguna sudah bergerak ke posisi baru.
3.  Kita mengekstraksi data kecepatan sudut giroskop $\boldsymbol{\omega} = (\omega_x, \omega_y, \omega_z)$ yang terintegrasi selama selisih waktu tersebut.
4.  Kita menghitung kuaternion rotasi prediksi $\Delta q$:
    $$\Delta q = \left[ \cos\left(\frac{\|\boldsymbol{\omega}\| \cdot \Delta t}{2}\right), \frac{\boldsymbol{\omega}}{\|\boldsymbol{\omega}\|} \sin\left(\frac{\|\boldsymbol{\omega}\| \cdot \Delta t}{2}\right) \right]$$
5.  Kita memutar seluruh koordinat vertex wajah virtual menggunakan $\Delta q$ sesaat sebelum rendering dilakukan di GPU (Late Latching). Riasan akan terproyeksi tepat pada posisi wajah nyata saat itu juga.

---

## 👁️ 2. Metode Pendukung: GPU-Accelerated KLT Optical Flow

Jika sensor IMU perangkat Android kelas bawah tidak stabil atau mengalami kalibrasi buruk, kita menggunakan **Optical Flow**.

### Cara Kerja:
*   Model ML Kit/MediaPipe hanya berjalan pada 30 FPS untuk menghemat baterai.
*   Di antara dua frame AI tersebut (misal pada frame ke-2 di mana hasil AI belum keluar), GPU menjalankan algoritma pelacakan fitur ringan **Kanade-Lucas-Tomasi (KLT)** secara langsung pada citra kamera yang baru masuk.
*   KLT melacak pergeseran piksel (*optical flow vectors*) dari 15 titik anchor utama wajah (ujung mata, ujung bibir, ujung hidung).
*   Kita mengekstrapolasi posisi 468 vertex sisa mengikuti tren pergeseran vektor KLT:
    $$P_{\text{vertex}}^{(t)} = P_{\text{vertex}}^{(t-1)} + \mathbf{v}_{\text{flow}} \cdot dt$$
*   Ini menghasilkan gerakan transisi yang super halus dan responsif setara **60 FPS/120 FPS** meskipun model AI aslinya hanya berjalan pada 30 FPS.

---

## 📸 3. Kompensasi Rolling-Shutter Kamera (Rolling-Shutter Correction)

Hampir semua ponsel menggunakan sensor kamera CMOS tipe *rolling-shutter* (sensor memindai gambar baris-per-baris dari atas ke bawah, bukan sekaligus). 

Ketika HP bergerak cepat, dahi pengguna tertangkap pada milidetik ke-0, sedangkan dagu baru tertangkap pada milidetik ke-16. Hal ini menyebabkan wajah terlihat melar (*skewed/sheared*) di gambar mentah, sehingga mesh riasan terlihat tidak pas menempel di wajah.

### Solusi Matematis:
Kita menerapkan deformasi geser (*shear deformation*) terbalik pada vertex shader berdasarkan kecepatan sudut giroskop $\boldsymbol{\omega}$ dan indeks baris vertex ($y$):
$$x_{\text{corrected}} = x_{\text{projected}} + \omega_y \cdot t_{\text{scan}} \cdot \left(\frac{y}{H_{\text{screen}}}\right)$$
$$y_{\text{corrected}} = y_{\text{projected}} - \omega_x \cdot t_{\text{scan}} \cdot \left(\frac{y}{H_{\text{screen}}}\right)$$
Di mana $t_{\text{scan}}$ adalah waktu total pemindaian sensor ($\approx 16\text{ms}$). Ini akan meluruskan kembali bentuk wajah yang terdistorsi rolling-shutter sebelum digambar.
