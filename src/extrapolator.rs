//! src/extrapolator.rs
//! Motion Extrapolator Prediktif untuk Eliminasi Lag Visual (Late Latching).

use nalgebra::{Vector3, Rotation3};
use crate::math::exp_map;

pub struct MotionExtrapolator {
    /// Latensi sistem target (misal 0.016 detik untuk target 60FPS)
    pub prediction_horizon: f32, 
}

impl MotionExtrapolator {
    pub fn new(horizon_seconds: f32) -> Self {
        Self { prediction_horizon: horizon_seconds }
    }

    /// Memprediksi pose kamera masa depan berdasarkan data inersia instan dari IMU dan horizon delta waktu dinamis.
    pub fn extrapolate_pose(
        &self,
        dt: f32, // Horizon prediksi dinamis disuplai oleh render loop
        current_r: &Rotation3<f32>,
        current_p: &Vector3<f32>,
        current_v: &Vector3<f32>,
        gyro_reading: &Vector3<f32>,
        acc_reading: &Vector3<f32>,
        bg: &Vector3<f32>,
        ba: &Vector3<f32>,
    ) -> (Rotation3<f32>, Vector3<f32>) {
        let gravity = Vector3::new(0.0, 0.0, -9.81);

        // Koreksi bias IMU instan
        let w_corrected = gyro_reading - bg;
        let a_corrected = acc_reading - ba;

        // 1. Ekstrapolasi Rotasi: R_pred = R_current * Exp(w * dt)
        let delta_rot = exp_map(&(w_corrected * dt));
        let r_pred = current_r * delta_rot;

        // 2. Ekstrapolasi Posisi: p_pred = p_current + v_current * dt + 0.5 * (R * a + g) * dt^2
        let acc_global = current_r * a_corrected;
        let p_pred = current_p + current_v * dt + 0.5 * (acc_global + gravity) * (dt * dt);

        (r_pred, p_pred)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extrapolation_static() {
        let extrapolator = MotionExtrapolator::new(0.016);
        let r = Rotation3::identity();
        let p = Vector3::new(0.0, 0.0, 0.0);
        let v = Vector3::new(0.0, 0.0, 0.0);
        
        // Simulasikan IMU dalam keadaan diam (hanya ada gravitasi berlawanan arah akselerometer)
        let gyro = Vector3::new(0.0, 0.0, 0.0);
        let acc = Vector3::new(0.0, 0.0, 9.81); // Mengimbangi gaya gravitasi
        let bg = Vector3::new(0.0, 0.0, 0.0);
        let ba = Vector3::new(0.0, 0.0, 0.0);
        
        let (r_pred, p_pred) = extrapolator.extrapolate_pose(0.016, &r, &p, &v, &gyro, &acc, &bg, &ba);
        
        // Karena sistem setimbang dinamis (acc_global + gravity = 0), posisi dan rotasi prediksi harus tetap diam
        assert!((r_pred.matrix()[(0,0)] - 1.0).abs() < 1e-5);
        assert!(p_pred.norm() < 1e-5);
    }
}
