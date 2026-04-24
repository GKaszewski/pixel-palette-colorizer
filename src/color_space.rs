use palette::{Hsl, IntoColor, Lab, Oklab, Srgb};

pub trait ColorSpace: Send + Sync {
    fn to_cartesian(&self, p: &[u8; 4]) -> [f64; 3];
}

pub struct RgbSpace;
impl ColorSpace for RgbSpace {
    fn to_cartesian(&self, p: &[u8; 4]) -> [f64; 3] {
        [p[0] as f64, p[1] as f64, p[2] as f64]
    }
}

pub struct LabSpace;
impl ColorSpace for LabSpace {
    fn to_cartesian(&self, p: &[u8; 4]) -> [f64; 3] {
        let lab: Lab = Srgb::new(p[0], p[1], p[2])
            .into_format::<f32>()
            .into_color();
        [lab.l as f64, lab.a as f64, lab.b as f64]
    }
}

pub struct HslSpace;
impl ColorSpace for HslSpace {
    fn to_cartesian(&self, p: &[u8; 4]) -> [f64; 3] {
        let hsl: Hsl = Srgb::new(p[0], p[1], p[2])
            .into_format::<f32>()
            .into_color();
        let h = hsl.hue.into_radians() as f64;
        let s = hsl.saturation as f64;
        let l = hsl.lightness as f64;
        [s * h.cos(), s * h.sin(), l]
    }
}

pub struct OklabSpace;
impl ColorSpace for OklabSpace {
    fn to_cartesian(&self, p: &[u8; 4]) -> [f64; 3] {
        let oklab: Oklab = Srgb::new(p[0], p[1], p[2])
            .into_format::<f32>()
            .into_color();
        [oklab.l as f64, oklab.a as f64, oklab.b as f64]
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum ColorSpaceKind {
    Rgb,
    Hsl,
    Lab,
    Oklab,
}

impl ColorSpaceKind {
    pub fn into_space(self) -> Box<dyn ColorSpace> {
        match self {
            Self::Rgb => Box::new(RgbSpace),
            Self::Hsl => Box::new(HslSpace),
            Self::Lab => Box::new(LabSpace),
            Self::Oklab => Box::new(OklabSpace),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn euclidean(a: [f64; 3], b: [f64; 3]) -> f64 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    #[test]
    fn lab_distance_positive_for_different_colors() {
        let space = LabSpace;
        let white = [255u8, 255, 255, 255];
        let black = [0u8, 0, 0, 255];
        assert!(euclidean(space.to_cartesian(&white), space.to_cartesian(&black)) > 0.0);
        assert!(euclidean(space.to_cartesian(&white), space.to_cartesian(&white)) < 1e-10);
    }

    #[test]
    fn hsl_cylindrical_distance_works() {
        let space = HslSpace;
        let red = [255u8, 0, 0, 255];
        assert!(euclidean(space.to_cartesian(&red), space.to_cartesian(&red)) < 1e-10);
        let blue = [0u8, 0, 255, 255];
        assert!(euclidean(space.to_cartesian(&red), space.to_cartesian(&blue)) > 0.0);
        let grey = [128u8, 128, 128, 255];
        assert!(euclidean(space.to_cartesian(&grey), space.to_cartesian(&grey)) < 1e-10);
    }

    #[test]
    fn oklab_distance_positive_for_different_colors() {
        let space = OklabSpace;
        let white = [255u8, 255, 255, 255];
        let black = [0u8, 0, 0, 255];
        assert!(euclidean(space.to_cartesian(&white), space.to_cartesian(&black)) > 0.0);
        assert!(euclidean(space.to_cartesian(&white), space.to_cartesian(&white)) < 1e-10);
    }

    #[test]
    fn all_variants_produce_working_spaces() {
        let variants = [
            ColorSpaceKind::Rgb,
            ColorSpaceKind::Hsl,
            ColorSpaceKind::Lab,
            ColorSpaceKind::Oklab,
        ];
        let white = [255u8, 255, 255, 255];
        let black = [0u8, 0, 0, 255];
        for variant in variants {
            let space = variant.into_space();
            assert!(euclidean(space.to_cartesian(&white), space.to_cartesian(&black)) > 0.0);
            assert!(euclidean(space.to_cartesian(&white), space.to_cartesian(&white)) < 1e-10);
        }
    }
}
