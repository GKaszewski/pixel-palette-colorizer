use anyhow::Context;

pub trait PaletteSource {
    fn extension(&self) -> &str;
    fn read_bytes(&self) -> anyhow::Result<Vec<u8>>;
}

pub struct FilePaletteSource(pub std::path::PathBuf);

impl PaletteSource for FilePaletteSource {
    fn extension(&self) -> &str {
        self.0
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
    }

    fn read_bytes(&self) -> anyhow::Result<Vec<u8>> {
        std::fs::read(&self.0).context("Failed to read palette file")
    }
}

#[derive(Debug)]
pub struct Palette(Vec<[u8; 4]>);

fn parse_hex_text(bytes: &[u8]) -> anyhow::Result<Vec<[u8; 4]>> {
    let content = std::str::from_utf8(bytes).context("Palette text is not valid UTF-8")?;
    let mut colors = Vec::new();
    for line in content.lines() {
        let stripped = line.trim().trim_start_matches('#');
        if stripped.is_empty() {
            continue;
        }
        let expanded: String = if stripped.len() == 3 || stripped.len() == 4 {
            stripped.chars().flat_map(|c| [c, c]).collect()
        } else {
            stripped.to_string()
        };
        let hex = expanded.as_str();
        if hex.len() == 6 || hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).context("Invalid hex color")?;
            let g = u8::from_str_radix(&hex[2..4], 16).context("Invalid hex color")?;
            let b = u8::from_str_radix(&hex[4..6], 16).context("Invalid hex color")?;
            let a = if hex.len() == 8 {
                u8::from_str_radix(&hex[6..8], 16).context("Invalid hex color")?
            } else {
                255
            };
            colors.push([r, g, b, a]);
        } else {
            tracing::warn!("Skipping invalid hex color: {}", line.trim());
        }
    }
    Ok(colors)
}

fn parse_image_bytes(bytes: &[u8]) -> anyhow::Result<Vec<[u8; 4]>> {
    use image::GenericImageView;
    let img = image::load_from_memory(bytes).context("Failed to decode palette image")?;
    let mut seen = std::collections::HashSet::new();
    let mut colors = Vec::new();
    for (_, _, pixel) in img.pixels() {
        let key = [pixel[0], pixel[1], pixel[2], pixel[3]];
        if seen.insert(key) {
            colors.push(key);
        }
    }
    Ok(colors)
}

impl Palette {
    pub fn load(source: &dyn PaletteSource) -> anyhow::Result<Self> {
        let bytes = source.read_bytes()?;
        let colors = match source.extension() {
            "txt" | "hex" => parse_hex_text(&bytes)?,
            "png" | "jpg" | "jpeg" | "bmp" | "gif" => parse_image_bytes(&bytes)?,
            ext => anyhow::bail!("Unsupported palette file format: {ext}"),
        };
        anyhow::ensure!(!colors.is_empty(), "Palette contains no colors");
        Ok(Palette(colors))
    }

    pub fn colors(&self) -> &[[u8; 4]] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct InMemorySource {
        ext: &'static str,
        data: Vec<u8>,
    }

    impl PaletteSource for InMemorySource {
        fn extension(&self) -> &str { self.ext }
        fn read_bytes(&self) -> anyhow::Result<Vec<u8>> { Ok(self.data.clone()) }
    }

    fn src(ext: &'static str, data: &str) -> InMemorySource {
        InMemorySource { ext, data: data.as_bytes().to_vec() }
    }

    #[test]
    fn hex_parses_six_char_rgb() {
        let p = Palette::load(&src("hex", "#ff0000\n")).unwrap();
        assert_eq!(p.colors(), &[[255u8, 0, 0, 255]]);
    }

    #[test]
    fn hex_parses_eight_char_rgba() {
        let p = Palette::load(&src("hex", "ff000080\n")).unwrap();
        assert_eq!(p.colors(), &[[255u8, 0, 0, 128]]);
    }

    #[test]
    fn hex_skips_blank_lines() {
        let p = Palette::load(&src("hex", "\n#ff0000\n\n#00ff00\n\n")).unwrap();
        assert_eq!(p.colors().len(), 2);
    }

    #[test]
    fn hex_parses_six_char_rgb_without_prefix() {
        let p = Palette::load(&src("txt", "ff0000\n")).unwrap();
        assert_eq!(p.colors(), &[[255u8, 0, 0, 255]]);
    }

    #[test]
    fn hex_skips_wrong_length_lines() {
        // 5-char hex is invalid after shorthand expansion
        let p = Palette::load(&src("hex", "#fffff\n#ff0000\n")).unwrap();
        assert_eq!(p.colors().len(), 1);
    }

    #[test]
    fn hex_expands_three_char_shorthand() {
        let p = Palette::load(&src("hex", "#FFF\n")).unwrap();
        assert_eq!(p.colors(), &[[255u8, 255, 255, 255]]);
    }

    #[test]
    fn hex_expands_three_char_shorthand_lowercase() {
        let p = Palette::load(&src("hex", "#fff\n")).unwrap();
        assert_eq!(p.colors(), &[[255u8, 255, 255, 255]]);
    }

    #[test]
    fn hex_expands_four_char_shorthand_with_alpha() {
        // #F00A → FF0000AA → [255, 0, 0, 170]
        let p = Palette::load(&src("hex", "#F00A\n")).unwrap();
        assert_eq!(p.colors(), &[[255u8, 0, 0, 170]]);
    }

    #[test]
    fn hex_rejects_invalid_hex_digits() {
        let result = Palette::load(&src("hex", "gggggg\n"));
        assert!(result.is_err());
    }

    fn make_png_bytes(colors: &[[u8; 4]]) -> Vec<u8> {
        let mut img = image::RgbaImage::new(colors.len() as u32, 1);
        for (i, &c) in colors.iter().enumerate() {
            img.put_pixel(i as u32, 0, image::Rgba(c));
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn image_extracts_unique_colors() {
        let bytes = make_png_bytes(&[[255u8, 0, 0, 255], [0, 255, 0, 255], [255, 0, 0, 255]]);
        let source = InMemorySource { ext: "png", data: bytes };
        let p = Palette::load(&source).unwrap();
        assert_eq!(p.colors().len(), 2); // [255,0,0,255] deduped
    }

    #[test]
    fn image_jpeg_extension_also_works() {
        let bytes = make_png_bytes(&[[0u8, 0, 255, 255]]);
        let source = InMemorySource { ext: "jpg", data: bytes };
        assert!(Palette::load(&source).is_ok());
    }

    #[test]
    fn empty_text_palette_is_rejected() {
        assert!(Palette::load(&src("hex", "\n\n")).is_err());
    }

    #[test]
    fn unknown_extension_is_rejected() {
        let err = Palette::load(&src("csv", "#ff0000\n")).unwrap_err();
        assert!(err.to_string().contains("Unsupported"), "{err}");
    }
}
