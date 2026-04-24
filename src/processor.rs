use std::path::Path;

use anyhow::Context;

use crate::color_space::ColorSpace;

pub struct ProcessResult {
    pub pixels_changed: u64,
}

pub struct RemapStats {
    pub pixels_changed: u64,
}

// Palette alpha is ignored; each pixel's original alpha is preserved.
pub fn remap_pixels(
    img: &mut image::RgbaImage,
    palette: &[[u8; 4]],
    color_space: &dyn ColorSpace,
) -> RemapStats {
    debug_assert!(!palette.is_empty(), "remap_pixels called with empty palette");
    let mut pixels_changed: u64 = 0;
    for pixel in img.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        // Premul is exact for RGB; non-linear spaces (Lab, Oklab, HSL) treat premul'd bytes as straight sRGB.
        let alpha = pixel[3];
        let premul = [
            (pixel[0] as u16 * alpha as u16 / 255) as u8,
            (pixel[1] as u16 * alpha as u16 / 255) as u8,
            (pixel[2] as u16 * alpha as u16 / 255) as u8,
            alpha,
        ];
        let mut min_distance = f64::MAX;
        let mut best_match: [u8; 4] = pixel.0;
        for palette_color in palette {
            let dist = color_space.distance(&premul, palette_color);
            if dist < min_distance {
                min_distance = dist;
                best_match = *palette_color;
            }
        }
        best_match[3] = pixel[3];
        if pixel.0 != best_match {
            pixels_changed += 1;
        }
        pixel.0 = best_match;
    }
    RemapStats { pixels_changed }
}

pub fn process_image(
    input_path: &Path,
    out_dir: &Path,
    palette: &[[u8; 4]],
    color_space: &dyn ColorSpace,
    dry_run: bool,
) -> anyhow::Result<ProcessResult> {
    let img = image::open(input_path)
        .with_context(|| format!("Failed to open input image {:?}", input_path))?;

    let mut out_img = img.to_rgba8();
    let stats = remap_pixels(&mut out_img, palette, color_space);

    let file_name = input_path.file_name().context("Invalid input file name")?;
    let out_path = out_dir.join(file_name);

    if !dry_run {
        std::fs::create_dir_all(out_dir)?;
        out_img
            .save(&out_path)
            .with_context(|| format!("Failed to save output image to {:?}", out_path))?;
    }

    Ok(ProcessResult {
        pixels_changed: stats.pixels_changed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_space::RgbSpace;

    #[test]
    fn transparent_pixels_are_not_remapped() {
        let mut img = image::RgbaImage::from_pixel(1, 1, image::Rgba([100u8, 150, 200, 0]));
        let palette = vec![[255u8, 0, 0, 255]];
        let stats = remap_pixels(&mut img, &palette, &RgbSpace);
        assert_eq!(stats.pixels_changed, 0);
        assert_eq!(img.get_pixel(0, 0).0, [100, 150, 200, 0]);
    }

    #[test]
    fn nearest_color_replaces_pixel() {
        let mut img = image::RgbaImage::from_pixel(1, 1, image::Rgba([100u8, 150, 200, 255]));
        let palette = vec![[255u8, 0, 0, 255], [0u8, 0, 255, 255]];
        let stats = remap_pixels(&mut img, &palette, &RgbSpace);
        assert_eq!(stats.pixels_changed, 1);
        assert_eq!(img.get_pixel(0, 0).0, [0, 0, 255, 255]);
    }

    #[test]
    fn alpha_is_preserved_after_remap() {
        let mut img = image::RgbaImage::from_pixel(1, 1, image::Rgba([200u8, 200, 200, 128]));
        let palette = vec![[0u8, 0, 0, 255]];
        let stats = remap_pixels(&mut img, &palette, &RgbSpace);
        assert_eq!(img.get_pixel(0, 0).0[3], 128);
        assert_eq!(stats.pixels_changed, 1);
    }

    #[test]
    fn identical_pixel_is_not_counted_as_changed() {
        let mut img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255u8, 0, 0, 255]));
        let palette = vec![[255u8, 0, 0, 255]];
        let stats = remap_pixels(&mut img, &palette, &RgbSpace);
        assert_eq!(stats.pixels_changed, 0);
    }

    #[test]
    fn semi_transparent_pixel_uses_premultiplied_distance() {
        // [255, 0, 0, 128] premultiplied → [128, 0, 0]
        // palette: bright-red [255,0,0] vs dark-red [128,0,0]
        // premul distance to dark-red = 0, so dark-red wins; original alpha preserved
        let mut img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255u8, 0, 0, 128]));
        let palette = vec![[255u8, 0, 0, 255], [128u8, 0, 0, 255]];
        remap_pixels(&mut img, &palette, &RgbSpace);
        assert_eq!(img.get_pixel(0, 0).0, [128, 0, 0, 128]);
    }

    #[test]
    fn process_image_dry_run_does_not_write() {
        let tmp = std::env::temp_dir().join("ppc_proc_test_input.png");
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([100u8, 150, 200, 255]));
        img.save(&tmp).unwrap();

        let out_dir = std::env::temp_dir().join("ppc_proc_test_dryrun_out");
        let _ = std::fs::remove_dir_all(&out_dir);

        let palette = vec![[255u8, 0, 0, 255]];
        let result = process_image(&tmp, &out_dir, &palette, &RgbSpace, true).unwrap();

        assert_eq!(result.pixels_changed, 1);
        assert!(!out_dir.exists(), "dry_run must not create output directory");

        std::fs::remove_file(&tmp).ok();
    }
}
