use palette::{Hsl, IntoColor, Lab, Oklab, Srgb};

pub trait ColorSpace: Send + Sync {
    fn distance(&self, p1: &[u8; 4], p2: &[u8; 4]) -> f64;
}

pub struct RgbSpace;
impl ColorSpace for RgbSpace {
    fn distance(&self, p1: &[u8; 4], p2: &[u8; 4]) -> f64 {
        let dr = (p1[0] as f64) - (p2[0] as f64);
        let dg = (p1[1] as f64) - (p2[1] as f64);
        let db = (p1[2] as f64) - (p2[2] as f64);
        (dr * dr + dg * dg + db * db).sqrt()
    }
}

pub struct LabSpace;
impl ColorSpace for LabSpace {
    fn distance(&self, p1: &[u8; 4], p2: &[u8; 4]) -> f64 {
        let to_lab = |p: &[u8; 4]| -> Lab {
            Srgb::new(p[0], p[1], p[2]).into_format::<f32>().into_color()
        };
        let l1 = to_lab(p1);
        let l2 = to_lab(p2);
        let dl = (l1.l - l2.l) as f64;
        let da = (l1.a - l2.a) as f64;
        let db = (l1.b - l2.b) as f64;
        (dl * dl + da * da + db * db).sqrt()
    }
}

pub struct HslSpace;
impl ColorSpace for HslSpace {
    fn distance(&self, p1: &[u8; 4], p2: &[u8; 4]) -> f64 {
        let to_cart = |p: &[u8; 4]| -> (f64, f64, f64) {
            let hsl: Hsl = Srgb::new(p[0], p[1], p[2]).into_format::<f32>().into_color();
            let h = hsl.hue.into_radians() as f64;
            let s = hsl.saturation as f64;
            let l = hsl.lightness as f64;
            (s * h.cos(), s * h.sin(), l)
        };
        let (x1, y1, z1) = to_cart(p1);
        let (x2, y2, z2) = to_cart(p2);
        let dx = x1 - x2;
        let dy = y1 - y2;
        let dz = z1 - z2;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

pub struct OklabSpace;
impl ColorSpace for OklabSpace {
    fn distance(&self, p1: &[u8; 4], p2: &[u8; 4]) -> f64 {
        let to_oklab = |p: &[u8; 4]| -> Oklab {
            Srgb::new(p[0], p[1], p[2]).into_format::<f32>().into_color()
        };
        let o1 = to_oklab(p1);
        let o2 = to_oklab(p2);
        let dl = (o1.l - o2.l) as f64;
        let da = (o1.a - o2.a) as f64;
        let db = (o1.b - o2.b) as f64;
        (dl * dl + da * da + db * db).sqrt()
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
            Self::Rgb   => Box::new(RgbSpace),
            Self::Hsl   => Box::new(HslSpace),
            Self::Lab   => Box::new(LabSpace),
            Self::Oklab => Box::new(OklabSpace),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_distance_positive_for_different_colors() {
        let space = LabSpace;
        let white = [255u8, 255, 255, 255];
        let black = [0u8, 0, 0, 255];
        assert!(space.distance(&white, &black) > 0.0);
        assert!(space.distance(&white, &white) < 1e-10);
    }

    #[test]
    fn hsl_cylindrical_distance_works() {
        let space = HslSpace;
        let red = [255u8, 0, 0, 255];
        assert!(space.distance(&red, &red) < 1e-10);
        let blue = [0u8, 0, 255, 255];
        assert!(space.distance(&red, &blue) > 0.0);
        let grey = [128u8, 128, 128, 255];
        assert!(space.distance(&grey, &grey) < 1e-10);
    }

    #[test]
    fn oklab_distance_positive_for_different_colors() {
        let space = OklabSpace;
        let white = [255u8, 255, 255, 255];
        let black = [0u8, 0, 0, 255];
        assert!(space.distance(&white, &black) > 0.0);
        assert!(space.distance(&white, &white) < 1e-10);
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
            assert!(space.distance(&white, &black) > 0.0);
            assert!(space.distance(&white, &white) < 1e-10);
        }
    }
}
