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

struct ColorSpaceEntry {
    name: &'static str,
    build: fn() -> Box<dyn ColorSpace>,
}

static REGISTRY: &[ColorSpaceEntry] = &[
    ColorSpaceEntry { name: "rgb",   build: || Box::new(RgbSpace)   },
    ColorSpaceEntry { name: "hsl",   build: || Box::new(HslSpace)   },
    ColorSpaceEntry { name: "lab",   build: || Box::new(LabSpace)   },
    ColorSpaceEntry { name: "oklab", build: || Box::new(OklabSpace) },
];

pub fn available_names() -> impl Iterator<Item = &'static str> {
    REGISTRY.iter().map(|e| e.name)
}

pub fn from_name(name: &str) -> anyhow::Result<Box<dyn ColorSpace>> {
    REGISTRY
        .iter()
        .find(|e| e.name == name)
        .map(|e| (e.build)())
        .ok_or_else(|| anyhow::anyhow!("Unknown color space: {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_rgb_returns_working_space() {
        let space = from_name("rgb").unwrap();
        let white = [255u8, 255, 255, 255];
        let black = [0u8, 0, 0, 255];
        assert!(space.distance(&white, &black) > 0.0);
        assert_eq!(space.distance(&white, &white), 0.0);
    }

    #[test]
    fn from_name_unknown_is_err() {
        assert!(from_name("xyz").is_err());
    }

    #[test]
    fn registry_is_coherent() {
        let names: Vec<_> = available_names().collect();
        assert_eq!(names.len(), 4, "expected 4 registered color spaces");
        for name in &names {
            assert!(from_name(name).is_ok(), "from_name failed for {name}");
        }
    }

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
}
