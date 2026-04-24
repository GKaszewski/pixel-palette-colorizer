use pixel_palette_colorizer::run;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    run()
}
