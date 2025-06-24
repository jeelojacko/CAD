use slint::Image;
use truck_cad_engine::TruckCadEngine;
use truck_modeling::base::Point3;

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

    pub fn rotate(&mut self, dx: f64, dy: f64) {
        self.engine.rotate_camera(dx, dy);
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.engine.pan_camera(dx, dy);
    }

    pub fn zoom(&mut self, delta: f64) {
        self.engine.zoom_camera(delta);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.engine.resize(width, height);
    }

    pub fn add_point(&mut self, x: f64, y: f64, z: f64) -> usize {
        self.engine.add_point_marker(Point3::new(x, y, z))
    }

    pub fn update_point(&mut self, id: usize, x: f64, y: f64, z: f64) {
        self.engine.update_point_marker(id, Point3::new(x, y, z));
    }

    pub fn remove_point(&mut self, id: usize) {
        self.engine.remove_point_marker(id);
    }

    pub fn add_line(&mut self, a: [f64; 3], b: [f64; 3]) -> usize {
        self.engine
            .add_line(Point3::new(a[0], a[1], a[2]), Point3::new(b[0], b[1], b[2]))
    }

    pub fn update_line(&mut self, id: usize, a: [f64; 3], b: [f64; 3]) {
        self.engine.update_line(
            id,
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
        );
    }

    pub fn remove_line(&mut self, id: usize) {
        self.engine.remove_line(id);
    }

    pub fn add_surface(&mut self, vertices: &[Point3], triangles: &[[usize; 3]]) -> usize {
        self.engine.add_surface(vertices, triangles)
    }

    pub fn update_surface(
        &mut self,
        id: usize,
        vertices: &[Point3],
        triangles: &[[usize; 3]],
    ) {
        self.engine.update_surface(id, vertices, triangles);
    }

    pub fn remove_surface(&mut self, id: usize) {
        self.engine.remove_surface(id);
    }
}
