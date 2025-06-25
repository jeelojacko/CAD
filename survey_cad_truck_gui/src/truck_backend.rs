use slint::Image;
use truck_cad_engine::TruckCadEngine;
use truck_modeling::base::Point3;

pub struct TruckBackend {
    engine: TruckCadEngine,
    point_ids: Vec<Option<usize>>,
    line_ids: Vec<Option<usize>>,
    surface_ids: Vec<Option<usize>>,
}

impl TruckBackend {
    pub fn new(width: u32, height: u32) -> Self {
        let mut engine = TruckCadEngine::new(width, height);
        engine.add_unit_cube();
        Self {
            engine,
            point_ids: Vec::new(),
            line_ids: Vec::new(),
            surface_ids: Vec::new(),
        }
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
        let id = self.engine.add_point_marker(Point3::new(x, y, z));
        self.point_ids.push(Some(id));
        self.point_ids.len() - 1
    }

    pub fn update_point(&mut self, idx: usize, x: f64, y: f64, z: f64) {
        if let Some(Some(id)) = self.point_ids.get(idx) {
            self.engine.update_point_marker(*id, Point3::new(x, y, z));
        }
    }

    pub fn remove_point(&mut self, idx: usize) {
        if idx < self.point_ids.len() {
            if let Some(id) = self.point_ids.remove(idx) {
                self.engine.remove_point_marker(id);
            }
        }
    }

    pub fn add_line(&mut self, a: [f64; 3], b: [f64; 3]) -> usize {
        let id = self
            .engine
            .add_line(Point3::new(a[0], a[1], a[2]), Point3::new(b[0], b[1], b[2]));
        self.line_ids.push(Some(id));
        self.line_ids.len() - 1
    }

    #[allow(dead_code)]
    pub fn update_line(&mut self, idx: usize, a: [f64; 3], b: [f64; 3]) {
        if let Some(Some(id)) = self.line_ids.get(idx) {
            self.engine.update_line(
                *id,
                Point3::new(a[0], a[1], a[2]),
                Point3::new(b[0], b[1], b[2]),
            );
        }
    }

    pub fn remove_line(&mut self, idx: usize) {
        if idx < self.line_ids.len() {
            if let Some(id) = self.line_ids.remove(idx) {
                self.engine.remove_line(id);
            }
        }
    }

    pub fn add_surface(&mut self, vertices: &[Point3], triangles: &[[usize; 3]]) -> usize {
        let id = self.engine.add_surface(vertices, triangles);
        self.surface_ids.push(Some(id));
        self.surface_ids.len() - 1
    }

    #[allow(dead_code)]
    pub fn update_surface(&mut self, idx: usize, vertices: &[Point3], triangles: &[[usize; 3]]) {
        if let Some(Some(id)) = self.surface_ids.get(idx) {
            self.engine.update_surface(*id, vertices, triangles);
        }
    }

    pub fn remove_surface(&mut self, idx: usize) {
        if idx < self.surface_ids.len() {
            if let Some(id) = self.surface_ids.remove(idx) {
                self.engine.remove_surface(id);
            }
        }
    }

    pub fn add_vertex(&mut self, surface: usize, p: Point3) -> Option<usize> {
        self.engine.add_surface_vertex(surface, p)
    }

    pub fn move_vertex(&mut self, surface: usize, idx: usize, p: Point3) {
        self.engine.move_surface_vertex(surface, idx, p);
    }

    pub fn delete_vertex(&mut self, surface: usize, idx: usize) {
        self.engine.delete_surface_vertex(surface, idx);
    }

    pub fn add_triangle(&mut self, surface: usize, tri: [usize; 3]) {
        self.engine.add_surface_triangle(surface, tri);
    }

    pub fn delete_triangle(&mut self, surface: usize, tri_idx: usize) {
        self.engine.delete_surface_triangle(surface, tri_idx);
    }

    pub fn clear(&mut self) {
        for _ in 0..self.point_ids.len() {
            self.remove_point(0);
        }
        for _ in 0..self.line_ids.len() {
            self.remove_line(0);
        }
        for _ in 0..self.surface_ids.len() {
            self.remove_surface(0);
        }
    }
}
