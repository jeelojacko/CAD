use slint::Image;
use truck_cad_engine::TruckCadEngine;

pub struct TruckBackend {
    engine: TruckCadEngine,
}

impl TruckBackend {
    pub fn new(width: u32, height: u32) -> Self {
        let mut engine = TruckCadEngine::new(width, height);
        engine.add_unit_cube();
        Self { engine }
    }

    pub fn render(&mut self) -> Image {
        self.engine.render_to_image()
    }
}
