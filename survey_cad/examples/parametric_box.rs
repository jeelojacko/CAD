fn main() {
    #[cfg(feature = "pmetra")]
    {
        survey_cad::pmetra::render_box(bevy::prelude::Vec3::splat(1.0));
    }
    #[cfg(not(feature = "pmetra"))]
    {
        eprintln!("This example requires the `pmetra` feature.");
    }
}
