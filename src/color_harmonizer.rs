//! Modul Harmonisasi Warna Musiman (Seasonal Color Analysis) menggunakan Jarak Euclidean CIELAB.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BeautySeason {
    Spring,
    Summer,
    Autumn,
    Winter,
}

pub struct SeasonPrototype {
    pub season: BeautySeason,
    pub l_target: f32,
    pub a_target: f32,
    pub b_target: f32,
}

pub struct ColorHarmonizer {
    pub prototypes: [SeasonPrototype; 4],
}

impl ColorHarmonizer {
    pub fn new() -> Self {
        Self {
            prototypes: [
                // Spring: Cerah, Hangat, Terang (L* tinggi, a* positif, b* tinggi positif)
                SeasonPrototype { season: BeautySeason::Spring, l_target: 75.0, a_target: 15.0, b_target: 25.0 },
                // Summer: Muted, Dingin, Terang (L* tinggi, a* rendah, b* rendah/negatif)
                SeasonPrototype { season: BeautySeason::Summer, l_target: 70.0, a_target: 5.0, b_target: -5.0 },
                // Autumn: Muted, Hangat, Gelap (L* rendah, a* sedang positif, b* sedang positif)
                SeasonPrototype { season: BeautySeason::Autumn, l_target: 45.0, a_target: 12.0, b_target: 18.0 },
                // Winter: Cerah, Dingin, Gelap (L* rendah, a* tinggi, b* negatif)
                SeasonPrototype { season: BeautySeason::Winter, l_target: 40.0, a_target: 20.0, b_target: -10.0 },
            ],
        }
    }

    /// Mengklasifikasikan Beauty Season pengguna berdasarkan jarak Euclidean warna kulit terkompensasi.
    pub fn classify_season(&self, l: f32, a: f32, b: f32) -> BeautySeason {
        let mut min_distance = f32::MAX;
        let mut best_season = BeautySeason::Winter;

        for proto in &self.prototypes {
            // Hitung Jarak Euclidean 3D di ruang CIELAB
            let dl = l - proto.l_target;
            let da = a - proto.a_target;
            let db = b - proto.b_target;
            let dist = (dl*dl + da*da + db*db).sqrt();

            if dist < min_distance {
                min_distance = dist;
                best_season = proto.season;
            }
        }

        best_season
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_spring() {
        let harmonizer = ColorHarmonizer::new();
        // Input warna warm, light, bright (dekati target Spring: L*=75, a*=15, b*=25)
        let season = harmonizer.classify_season(76.0, 14.0, 24.0);
        assert_eq!(season, BeautySeason::Spring);
    }

    #[test]
    fn test_classify_winter() {
        let harmonizer = ColorHarmonizer::new();
        // Input warna cool, dark, bright (dekati target Winter: L*=40, a*=20, b*=-10)
        let season = harmonizer.classify_season(38.0, 22.0, -8.0);
        assert_eq!(season, BeautySeason::Winter);
    }
}
