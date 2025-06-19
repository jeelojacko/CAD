use std::path::Path;

fn main() {
    // Ensure the required font is available during the build.
    if !Path::new("assets/DejaVuSans.ttf").exists() {
        panic!(
            "assets/DejaVuSans.ttf not found. Please add the font file to the assets directory."
        );
    }

    slint_build::compile("ui/main.slint").unwrap();
}
