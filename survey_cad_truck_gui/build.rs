use std::path::Path;

fn main() {
    // Propagate PyO3's build configuration so the Python interpreter links
    // correctly when this binary is built.
    pyo3_build_config::use_pyo3_cfgs();
    // Allow overriding the bundled font at build time via the SURVEY_CAD_FONT
    // environment variable. Otherwise fall back to the default font shipped in
    // the assets directory.
    let font_path = std::env::var("SURVEY_CAD_FONT")
        .unwrap_or_else(|_| "assets/DejaVuSans.ttf".to_string());
    let font_path = Path::new(&font_path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(&font_path).to_path_buf());
    if !font_path.exists() {
        panic!(
            "{} not found. Please provide a valid font file via SURVEY_CAD_FONT or place DejaVuSans.ttf in assets/",
            font_path.display()
        );
    }

    // Pass the chosen font path to the compiled crate so it can be embedded.
    println!("cargo:rustc-env=DEFAULT_FONT_PATH={}", font_path.display());

    slint_build::compile("ui/main.slint").unwrap();
}
