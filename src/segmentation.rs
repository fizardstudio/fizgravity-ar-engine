//! Modul Segmentasi dan Oklusi Wajah/Tangan menggunakan IIR Temporal Filter.

pub struct SegmentationTracker {
    pub is_loaded: bool,
    pub mask_width: usize,
    pub mask_height: usize,
    pub current_mask: Vec<f32>, // Masker smoothed berbasis tingkat keyakinan [0.0, 1.0]
}

impl SegmentationTracker {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            is_loaded: true,
            mask_width: width,
            mask_height: height,
            current_mask: vec![0.0; width * height],
        }
    }

    /// Memperbarui masker segmentasi dengan temporal IIR smoothing filter:
    /// M_smooth = alpha * M_new + (1 - alpha) * M_smooth_old
    pub fn update_mask(&mut self, new_mask: &[u8], alpha: f32) -> Result<(), &'static str> {
        if new_mask.len() != self.current_mask.len() {
            return Err("Ukuran data input masker tidak cocok.");
        }

        let alpha_clamped = alpha.clamp(0.0, 1.0);

        for (i, &new_val) in new_mask.iter().enumerate() {
            let target_val = (new_val as f32) / 255.0;
            let old_val = self.current_mask[i];
            self.current_mask[i] = alpha_clamped * target_val + (1.0 - alpha_clamped) * old_val;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segmentation_smoothing() {
        let mut tracker = SegmentationTracker::new(4, 4);
        assert_eq!(tracker.current_mask[0], 0.0);

        // Frame 1: masker diset penuh (255) dengan alpha = 0.5
        let input_mask = vec![255; 16];
        tracker.update_mask(&input_mask, 0.5).unwrap();

        // Nilai harus naik setengahnya (0.5)
        assert!((tracker.current_mask[0] - 0.5).abs() < 1e-4);

        // Frame 2: masker diset penuh (255) dengan alpha = 0.5
        tracker.update_mask(&input_mask, 0.5).unwrap();

        // Nilai harus naik lagi (0.5 * 1.0 + 0.5 * 0.5 = 0.75)
        assert!((tracker.current_mask[0] - 0.75).abs() < 1e-4);
    }
}
