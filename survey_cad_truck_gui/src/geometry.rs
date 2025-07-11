use truck_modeling::base::{Point3, Vector4};
use truck_modeling::topology::Solid;
use truck_meshalgo::prelude::*;
use truck_meshalgo::tessellation::MeshableShape;

#[derive(Clone)]
pub struct SurfaceData {
    pub vertices: Vec<Point3>,
    pub triangles: Vec<[usize; 3]>,
    pub breaklines: Vec<(usize, usize)>,
    pub boundary: Option<Vec<usize>>,
}

impl SurfaceData {
    pub fn new(vertices: &[Point3], triangles: &[[usize; 3]]) -> Self {
        Self {
            vertices: vertices.to_vec(),
            triangles: triangles.to_vec(),
            breaklines: Vec::new(),
            boundary: None,
        }
    }
}

pub struct GeometryStore {
    pub points: Vec<Point3>,
    pub lines: Vec<(Point3, Point3, Vector4, f32)>,
    pub dimensions: Vec<(Point3, Point3)>,
    pub surfaces: Vec<SurfaceData>,
    pub solids: Vec<Vec<Point3>>, // triangulated positions for snapping
}

impl Default for GeometryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl GeometryStore {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            lines: Vec::new(),
            dimensions: Vec::new(),
            surfaces: Vec::new(),
            solids: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.lines.clear();
        self.dimensions.clear();
        self.surfaces.clear();
        self.solids.clear();
    }

    pub fn add_point(&mut self, p: Point3) -> usize {
        self.points.push(p);
        self.points.len() - 1
    }

    pub fn update_point(&mut self, idx: usize, p: Point3) {
        if let Some(pt) = self.points.get_mut(idx) {
            *pt = p;
        }
    }

    pub fn remove_point(&mut self, idx: usize) {
        if idx < self.points.len() {
            self.points.remove(idx);
        }
    }

    pub fn add_line(&mut self, a: Point3, b: Point3, color: Vector4, weight: f32) -> usize {
        self.lines.push((a, b, color, weight));
        self.lines.len() - 1
    }

    pub fn update_line(&mut self, idx: usize, a: Point3, b: Point3, color: Vector4, weight: f32) {
        if let Some(line) = self.lines.get_mut(idx) {
            *line = (a, b, color, weight);
        }
    }

    pub fn remove_line(&mut self, idx: usize) {
        if idx < self.lines.len() {
            self.lines.remove(idx);
        }
    }

    pub fn add_dimension(&mut self, a: Point3, b: Point3) -> usize {
        self.dimensions.push((a, b));
        self.dimensions.len() - 1
    }

    pub fn remove_dimension(&mut self, idx: usize) {
        if idx < self.dimensions.len() {
            self.dimensions.remove(idx);
        }
    }

    pub fn add_surface(&mut self, vertices: &[Point3], triangles: &[[usize; 3]]) -> usize {
        self.surfaces.push(SurfaceData::new(vertices, triangles));
        self.surfaces.len() - 1
    }

    pub fn update_surface(&mut self, idx: usize, vertices: &[Point3], triangles: &[[usize; 3]]) {
        if let Some(s) = self.surfaces.get_mut(idx) {
            s.vertices = vertices.to_vec();
            s.triangles = triangles.to_vec();
            s.breaklines.clear();
            s.boundary = None;
        }
    }

    pub fn remove_surface(&mut self, idx: usize) {
        if idx < self.surfaces.len() {
            self.surfaces.remove(idx);
        }
    }

    pub fn add_solid(&mut self, solid: Solid) {
        let mesh = solid.triangulation(0.01).to_polygon();
        self.solids.push(mesh.positions().clone());
    }

    pub fn add_vertex(&mut self, surface: usize, p: Point3) -> Option<usize> {
        if let Some(s) = self.surfaces.get_mut(surface) {
            s.vertices.push(p);
            Some(s.vertices.len() - 1)
        } else {
            None
        }
    }

    pub fn move_vertex(&mut self, surface: usize, idx: usize, p: Point3) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            if idx < s.vertices.len() {
                s.vertices[idx] = p;
            }
        }
    }

    pub fn delete_vertex(&mut self, surface: usize, idx: usize) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            if idx < s.vertices.len() {
                s.vertices.remove(idx);
                s.triangles.retain(|t| !t.contains(&idx));
                for tri in &mut s.triangles {
                    for v in tri.iter_mut() {
                        if *v > idx {
                            *v -= 1;
                        }
                    }
                }
            }
        }
    }

    pub fn add_triangle(&mut self, surface: usize, tri: [usize; 3]) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            s.triangles.push(tri);
        }
    }

    pub fn delete_triangle(&mut self, surface: usize, tri_idx: usize) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            if tri_idx < s.triangles.len() {
                s.triangles.remove(tri_idx);
            }
        }
    }

    pub fn add_breakline(&mut self, surface: usize, a: usize, b: usize) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            if a < s.vertices.len()
                && b < s.vertices.len()
                && !s.breaklines.iter().any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
            {
                s.breaklines.push((a, b));
            }
        }
    }

    pub fn remove_breakline(&mut self, surface: usize, a: usize, b: usize) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            if let Some(pos) = s
                .breaklines
                .iter()
                .position(|&(x, y)| (x == a && y == b) || (x == b && y == a))
            {
                s.breaklines.remove(pos);
            }
        }
    }

    pub fn set_boundary(&mut self, surface: usize, boundary: Vec<usize>) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            if boundary.iter().all(|&i| i < s.vertices.len()) && boundary.len() >= 3 {
                s.boundary = Some(boundary);
            }
        }
    }

    pub fn clear_boundary(&mut self, surface: usize) {
        if let Some(s) = self.surfaces.get_mut(surface) {
            s.boundary = None;
        }
    }
}

