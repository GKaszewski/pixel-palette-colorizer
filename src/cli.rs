use std::path::PathBuf;

use clap::Parser;
use rayon::prelude::*;
use tracing::info;

use crate::{
    color_space::{ColorSpace, ColorSpaceKind},
    palette::{FilePaletteSource, Palette},
    processor::{process_image, ProcessResult},
};

#[derive(Parser, Debug)]
#[command(author, version, about = "A pixel palette colorizer tool.")]
struct Cli {
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    #[arg(short, long)]
    palette: PathBuf,

    #[arg(short, long, default_value = "output")]
    out_dir: PathBuf,

    #[arg(long, default_value_t = false)]
    dry_run: bool,

    #[arg(
        short = 'e',
        long,
        default_value = "png,jpg",
        value_delimiter = ','
    )]
    extensions: Vec<String>,

    #[arg(short, long, default_value = "rgb")]
    color_space: ColorSpaceKind,
}

pub fn expand_inputs(inputs: &[PathBuf], extensions: &[String]) -> Vec<PathBuf> {
    let exts: Vec<String> = extensions.iter().map(|e| e.to_lowercase()).collect();
    let mut result = Vec::new();
    for path in inputs {
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| match e {
                    Ok(entry) => Some(entry),
                    Err(err) => {
                        tracing::warn!("Skipping unreadable entry: {}", err);
                        None
                    }
                })
            {
                let p = entry.path();
                if p.is_file() {
                    let matches = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| exts.contains(&e.to_lowercase()))
                        .unwrap_or(false);
                    if matches {
                        result.push(p.to_path_buf());
                    }
                }
            }
        } else {
            result.push(path.clone());
        }
    }
    result
}

pub trait Reporter: Send + Sync {
    fn on_complete(&self, outcome: &FileOutcome);
    fn summarize(&self, outcomes: &[FileOutcome]);
}

pub struct FileOutcome {
    pub path: PathBuf,
    pub result: anyhow::Result<ProcessResult>,
}

pub fn run_batch(
    inputs: &[PathBuf],
    out_dir: &std::path::Path,
    palette: &[[u8; 4]],
    space: &dyn ColorSpace,
    dry_run: bool,
    reporter: &dyn Reporter,
) -> Vec<FileOutcome> {
    inputs
        .par_iter()
        .map(|path| {
            let result = process_image(path, out_dir, palette, space, dry_run);
            let outcome = FileOutcome { path: path.clone(), result };
            reporter.on_complete(&outcome);
            outcome
        })
        .collect()
}

pub struct DefaultReporter {
    bar: indicatif::ProgressBar,
}

impl DefaultReporter {
    pub fn new(total: u64) -> Self {
        let bar = indicatif::ProgressBar::new(total);
        bar.set_style(
            indicatif::ProgressStyle::with_template("{pos}/{len} [{bar:40}] {msg}")
                .unwrap()
                .progress_chars("=> "),
        );
        Self { bar }
    }
}

impl Reporter for DefaultReporter {
    fn on_complete(&self, outcome: &FileOutcome) {
        self.bar.inc(1);
        match &outcome.result {
            Ok(r) => info!(
                "Processed {:?} ({} pixels changed)",
                outcome.path.file_name().unwrap_or_default(),
                r.pixels_changed
            ),
            Err(e) => tracing::error!(
                "Failed {:?}: {:#}",
                outcome.path.file_name().unwrap_or_default(),
                e
            ),
        }
    }

    fn summarize(&self, outcomes: &[FileOutcome]) {
        self.bar.finish_and_clear();
        let succeeded = outcomes.iter().filter(|o| o.result.is_ok()).count();
        let total_pixels: u64 = outcomes
            .iter()
            .filter_map(|o| o.result.as_ref().ok())
            .map(|r| r.pixels_changed)
            .sum();
        info!(
            "Done: {}/{} files succeeded, {} pixels remapped.",
            succeeded,
            outcomes.len(),
            total_pixels
        );
    }
}

pub fn run() -> anyhow::Result<()> {
    info!("Running pixel palette colorizer...");

    let cli = Cli::parse();

    let space = cli.color_space.into_space();
    info!("Color space: {:?}", cli.color_space);

    info!("Loading palette from {:?}", cli.palette);
    let palette = Palette::load(&FilePaletteSource(cli.palette.clone()))?;
    info!("Loaded {} colors.", palette.len());

    let inputs = expand_inputs(&cli.inputs, &cli.extensions);
    info!("Processing {} files...", inputs.len());
    let reporter = DefaultReporter::new(inputs.len() as u64);
    let outcomes = run_batch(&inputs, &cli.out_dir, palette.colors(), &*space, cli.dry_run, &reporter);
    reporter.summarize(&outcomes);

    let failed = outcomes.iter().filter(|o| o.result.is_err()).count();
    if failed > 0 {
        anyhow::bail!("{}/{} files failed", failed, outcomes.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_space::ColorSpaceKind;

    struct NoopReporter;
    impl Reporter for NoopReporter {
        fn on_complete(&self, _: &FileOutcome) {}
        fn summarize(&self, _: &[FileOutcome]) {}
    }

    #[test]
    fn run_batch_collects_all_errors_without_aborting() {
        let space = ColorSpaceKind::Rgb.into_space();
        let palette = vec![[255u8, 0, 0, 255]];
        let inputs = vec![
            std::path::PathBuf::from("/nonexistent/a.png"),
            std::path::PathBuf::from("/nonexistent/b.png"),
        ];
        let outcomes = run_batch(
            &inputs,
            std::path::Path::new("/tmp"),
            &palette,
            &*space,
            false,
            &NoopReporter,
        );
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.result.is_err()));
    }

    #[test]
    fn run_batch_returns_one_outcome_per_input() {
        let space = ColorSpaceKind::Rgb.into_space();
        let palette = vec![[0u8, 0, 0, 255]];
        let inputs: Vec<std::path::PathBuf> = (0..5)
            .map(|i| std::path::PathBuf::from(format!("/nonexistent/{i}.png")))
            .collect();
        let outcomes = run_batch(
            &inputs,
            std::path::Path::new("/tmp"),
            &palette,
            &*space,
            false,
            &NoopReporter,
        );
        assert_eq!(outcomes.len(), 5);
    }

    #[test]
    fn expand_inputs_passes_through_explicit_file() {
        let path = PathBuf::from("/nonexistent/file.png");
        let result = expand_inputs(&[path.clone()], &["png".to_string()]);
        assert_eq!(result, vec![path]);
    }

    #[test]
    fn expand_inputs_passes_through_nonexistent_path() {
        let path = PathBuf::from("/nonexistent/missing.xyz");
        let result = expand_inputs(&[path.clone()], &["png".to_string()]);
        assert_eq!(result, vec![path]);
    }

    #[test]
    fn expand_inputs_recurses_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.png"), b"").unwrap();
        std::fs::write(dir.path().join("top.png"), b"").unwrap();

        let result = expand_inputs(&[dir.path().to_path_buf()], &["png".to_string()]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn expand_inputs_filters_by_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.png"), b"").unwrap();
        std::fs::write(dir.path().join("skip.txt"), b"").unwrap();

        let result = expand_inputs(&[dir.path().to_path_buf()], &["png".to_string()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name().unwrap(), "keep.png");
    }

    #[test]
    fn expand_inputs_extension_match_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("image.PNG"), b"").unwrap();

        let result = expand_inputs(&[dir.path().to_path_buf()], &["png".to_string()]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn expand_inputs_mixed_file_and_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.png"), b"").unwrap();
        let explicit = PathBuf::from("/nonexistent/b.png");

        let mut result = expand_inputs(
            &[dir.path().to_path_buf(), explicit.clone()],
            &["png".to_string()],
        );
        result.sort();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&explicit));
    }
}
