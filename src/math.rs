//! Modul matematika spasial untuk grup Lie SO(3) dan aljabar Lie so(3).
//! Menyediakan implementasi peta eksponensial (Exponential Map), peta logaritma (Logarithm Map),
//! matriks skew-symmetric, dan integrasi manifold rotasi 3D.

use nalgebra::{Matrix3, Rotation3, Vector3, UnitQuaternion};

/// Konversi vektor 3D ke matriks miring-simetris (skew-symmetric matrix) [w]_x.
/// Peta dari aljabar Lie so(3) ke matriks 3x3.
/// 
/// ```text
///          [  0  -z   y ]
/// [w]_x =  [  z   0  -x ]
///          [ -y   x   0 ]
/// ```
pub fn skew_symmetric(w: &Vector3<f32>) -> Matrix3<f32> {
    Matrix3::new(
        0.0,  -w.z,   w.y,
        w.z,   0.0,  -w.x,
       -w.y,   w.x,   0.0,
    )
}

/// Peta Eksponensial (Exponential Map) dari aljabar Lie so(3) ke grup Lie SO(3).
/// Menggunakan Formula Rotasi Rodrigues untuk akurasi numerik yang tinggi.
/// 
/// R = Exp(w) = I + (sin(||w||)/||w||) * [w]_x + ((1 - cos(||w||))/||w||^2) * [w]_x^2
pub fn exp_map(w: &Vector3<f32>) -> Rotation3<f32> {
    let theta = w.norm();
    
    // Penanganan batas numerik (taylor expansion) jika nilai rotasi sangat kecil (hampir 0)
    // untuk mencegah pembagian dengan nol.
    if theta < 1e-5 {
        let skew = skew_symmetric(w);
        let res = Matrix3::identity() + skew + skew * skew * 0.5;
        return Rotation3::from_matrix_unchecked(res);
    }
    
    let u = w / theta; // Sumbu rotasi unit
    let skew = skew_symmetric(&u);
    let res = Matrix3::identity() + skew * theta.sin() + (skew * skew) * (1.0 - theta.cos());
    Rotation3::from_matrix_unchecked(res)
}

/// Peta Logaritma (Logarithm Map) dari grup Lie SO(3) kembali ke aljabar Lie so(3).
/// Mengekstrak sumbu-sudut rotasi 3D dari matriks rotasi.
/// 
/// w = Log(R)
pub fn log_map(r: &Rotation3<f32>) -> Vector3<f32> {
    let matrix = r.matrix();
    let trace = matrix.trace();
    
    // theta = acos((trace - 1) / 2)
    let cos_theta = (trace - 1.0) * 0.5;
    
    // Batasi nilai cos_theta agar tidak melebihi rentang domain [-1.0, 1.0] akibat galat floating-point
    let cos_theta = cos_theta.clamp(-1.0, 1.0);
    let theta = cos_theta.acos();
    
    if theta < 1e-5 {
        // Taylor expansion jika sudut rotasi sangat dekat dengan 0
        return Vector3::new(
            0.5 * (matrix[(2, 1)] - matrix[(1, 2)]),
            0.5 * (matrix[(0, 2)] - matrix[(2, 0)]),
            0.5 * (matrix[(1, 0)] - matrix[(0, 1)]),
        );
    }
    
    let scale = theta / (2.0 * theta.sin());
    Vector3::new(
        (matrix[(2, 1)] - matrix[(1, 2)]) * scale,
        (matrix[(0, 2)] - matrix[(2, 0)]) * scale,
        (matrix[(1, 0)] - matrix[(0, 1)]) * scale,
    )
}

/// Mengintegrasikan orientasi rotasi saat ini dengan kecepatan sudut baru (gyroscope)
/// melintasi interval delta waktu (dt) pada manifold SO(3).
/// 
/// R_new = R_current * Exp(w * dt)
pub fn integrate_rotation(r: &Rotation3<f32>, w: &Vector3<f32>, dt: f32) -> Rotation3<f32> {
    let delta_rot = exp_map(&(w * dt));
    r * delta_rot
}

/// Interpolasi Linier Sferis (Slerp) antara dua Kuaternion Orientasi.
/// Berguna untuk menghaluskan perpindahan rotasi kamera AR antar bingkai.
pub fn slerp(q1: &UnitQuaternion<f32>, q2: &UnitQuaternion<f32>, t: f32) -> UnitQuaternion<f32> {
    q1.slerp(q2, t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;

    #[test]
    fn test_skew_symmetric() {
        let w = Vector3::new(1.0, 2.0, 3.0);
        let m = skew_symmetric(&w);
        
        assert_eq!(m[(0, 0)], 0.0);
        assert_eq!(m[(0, 1)], -3.0);
        assert_eq!(m[(0, 2)], 2.0);
        assert_eq!(m[(1, 0)], 3.0);
        assert_eq!(m[(1, 1)], 0.0);
        assert_eq!(m[(1, 2)], -1.0);
        assert_eq!(m[(2, 0)], -2.0);
        assert_eq!(m[(2, 1)], 1.0);
        assert_eq!(m[(2, 2)], 0.0);
    }

    #[test]
    fn test_exp_log_identity() {
        // Uji identitas peta eksponensial dan logaritma: Log(Exp(w)) == w
        let w = Vector3::new(0.1, -0.2, 0.15);
        let r = exp_map(&w);
        let w_recovered = log_map(&r);
        
        assert!((w.x - w_recovered.x).abs() < 1e-5);
        assert!((w.y - w_recovered.y).abs() < 1e-5);
        assert!((w.z - w_recovered.z).abs() < 1e-5);
    }

    #[test]
    fn test_small_rotation_taylor() {
        // Uji stabilitas numerik untuk rotasi yang sangat kecil
        let w = Vector3::new(1e-7, 2e-7, -1e-7);
        let r = exp_map(&w);
        let w_recovered = log_map(&r);
        
        assert!((w.x - w_recovered.x).abs() < 1e-10);
        assert!((w.y - w_recovered.y).abs() < 1e-10);
        assert!((w.z - w_recovered.z).abs() < 1e-10);
    }
}
