use slint::Image;
use truck_cad_engine::TruckCadEngine;
use truck_modeling::base::{Point3, Vector4};
use truck_modeling::topology::Solid;

pub enum HitObject {
    Point,
    Line,
    Surface(usize),
    Handle(usize),
    Breakline,
    Boundary,
}

struct SurfaceData {
    vertices: Vec<Point3>,
    triangles: Vec<[usize; 3]>,
    breaklines: Vec<(usize, usize)>,
    boundary: Option<Vec<usize>>,
}

pub struct TruckBackend {
    engine: TruckCadEngine,
    point_ids: Vec<Option<usize>>,
    line_ids: Vec<Option<usize>>,
    dimension_ids: Vec<Option<usize>>,
    surface_ids: Vec<Option<usize>>,
    points: Vec<Point3>,
    lines: Vec<(Point3, Point3, Vector4, f32)>,
    dimensions: Vec<(Point3, Point3)>,
    surfaces: Vec<SurfaceData>,
    handles: Option<(usize, Vec<usize>)>,
    hover_surface: Option<usize>,
    hover_handle: Option<usize>,
}

impl TruckBackend {
    pub fn new(width: u32, height: u32) -> Self {
        let mut engine = TruckCadEngine::new(width, height);
        engine.add_unit_cube();
        Self {
            engine,
            point_ids: Vec::new(),
            line_ids: Vec::new(),
            dimension_ids: Vec::new(),
            surface_ids: Vec::new(),
            points: Vec::new(),
            lines: Vec::new(),
            dimensions: Vec::new(),
            surfaces: Vec::new(),
            handles: None,
            hover_surface: None,
            hover_handle: None,
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
        self.points.push(Point3::new(x, y, z));
        self.point_ids.len() - 1
    }

    pub fn update_point(&mut self, idx: usize, x: f64, y: f64, z: f64) {
        if let Some(Some(id)) = self.point_ids.get(idx) {
            self.engine.update_point_marker(*id, Point3::new(x, y, z));
        }
        if let Some(p) = self.points.get_mut(idx) {
            *p = Point3::new(x, y, z);
        }
    }

    pub fn remove_point(&mut self, idx: usize) {
        if idx < self.point_ids.len() {
            if let Some(id) = self.point_ids.remove(idx) {
                self.engine.remove_point_marker(id);
            }
            if idx < self.points.len() {
                self.points.remove(idx);
            }
        }
    }

    pub fn add_line(
        &mut self,
        a: [f64; 3],
        b: [f64; 3],
        color: [f32; 4],
        weight: f32,
    ) -> usize {
        let col = Vector4::new(color[0] as f64, color[1] as f64, color[2] as f64, color[3] as f64);
        let id = self.engine.add_line(
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
            col,
            weight,
        );
        self.line_ids.push(Some(id));
        self.lines.push((
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
            col,
            weight,
        ));
        self.line_ids.len() - 1
    }

    #[allow(dead_code)]
    pub fn update_line(
        &mut self,
        idx: usize,
        a: [f64; 3],
        b: [f64; 3],
        color: [f32; 4],
        weight: f32,
    ) {
        if let Some(Some(id)) = self.line_ids.get(idx) {
            self.engine.update_line(
                *id,
                Point3::new(a[0], a[1], a[2]),
                Point3::new(b[0], b[1], b[2]),
                Vector4::new(color[0] as f64, color[1] as f64, color[2] as f64, color[3] as f64),
                weight,
            );
        }
        if let Some(line) = self.lines.get_mut(idx) {
            *line = (
                Point3::new(a[0], a[1], a[2]),
                Point3::new(b[0], b[1], b[2]),
                Vector4::new(color[0] as f64, color[1] as f64, color[2] as f64, color[3] as f64),
                weight,
            );
        }
    }

    pub fn remove_line(&mut self, idx: usize) {
        if idx < self.line_ids.len() {
            if let Some(id) = self.line_ids.remove(idx) {
                self.engine.remove_line(id);
            }
            if idx < self.lines.len() {
                self.lines.remove(idx);
            }
        }
    }

    /// Add a dimension represented as a simple line between two points.
    pub fn add_dimension(&mut self, a: [f64; 3], b: [f64; 3], color: [f32; 4], weight: f32) -> usize {
        let id = self.engine.add_line(
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
            Vector4::new(color[0] as f64, color[1] as f64, color[2] as f64, color[3] as f64),
            weight,
        );
        self.dimension_ids.push(Some(id));
        self.dimensions.push((
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
        ));
        self.dimension_ids.len() - 1
    }

    /// Remove an existing dimension.
    pub fn remove_dimension(&mut self, idx: usize) {
        if idx < self.dimension_ids.len() {
            if let Some(id) = self.dimension_ids.remove(idx) {
                self.engine.remove_line(id);
            }
            if idx < self.dimensions.len() {
                self.dimensions.remove(idx);
            }
        }
    }

    pub fn add_surface(&mut self, vertices: &[Point3], triangles: &[[usize; 3]]) -> usize {
        let id = self.engine.add_surface(vertices, triangles);
        self.surface_ids.push(Some(id));
        self.surfaces.push(SurfaceData {
            vertices: vertices.to_vec(),
            triangles: triangles.to_vec(),
            breaklines: Vec::new(),
            boundary: None,
        });
        self.surface_ids.len() - 1
    }

    pub fn add_solid(&mut self, solid: Solid) {
        self.engine.add_solid(solid);
    }

    #[allow(dead_code)]
    pub fn update_surface(&mut self, idx: usize, vertices: &[Point3], triangles: &[[usize; 3]]) {
        if let Some(Some(id)) = self.surface_ids.get(idx) {
            self.engine.update_surface(*id, vertices, triangles);
        }
        if let Some(surf) = self.surfaces.get_mut(idx) {
            surf.vertices = vertices.to_vec();
            surf.triangles = triangles.to_vec();
            surf.breaklines.clear();
            surf.boundary = None;
        }
    }

    pub fn remove_surface(&mut self, idx: usize) {
        if idx < self.surface_ids.len() {
            if let Some(id) = self.surface_ids.remove(idx) {
                self.engine.remove_surface(id);
            }
            if idx < self.surfaces.len() {
                self.surfaces.remove(idx);
            }
        }
    }

    pub fn add_vertex(&mut self, surface: usize, p: Point3) -> Option<usize> {
        let res = self.engine.add_surface_vertex(surface, p);
        if let (Some(idx), Some(surf)) = (res, self.surfaces.get_mut(surface)) {
            surf.vertices.push(p);
        }
        res
    }

    pub fn move_vertex(&mut self, surface: usize, idx: usize, p: Point3) {
        self.engine.move_surface_vertex(surface, idx, p);
        if let Some(surf) = self.surfaces.get_mut(surface) {
            if idx < surf.vertices.len() {
                surf.vertices[idx] = p;
            }
        }
    }

    pub fn delete_vertex(&mut self, surface: usize, idx: usize) {
        self.engine.delete_surface_vertex(surface, idx);
        if let Some(surf) = self.surfaces.get_mut(surface) {
            if idx < surf.vertices.len() {
                surf.vertices.remove(idx);
                surf.triangles.retain(|t| !t.contains(&idx));
                for tri in &mut surf.triangles {
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
        self.engine.add_surface_triangle(surface, tri);
        if let Some(surf) = self.surfaces.get_mut(surface) {
            surf.triangles.push(tri);
        }
    }

    pub fn delete_triangle(&mut self, surface: usize, tri_idx: usize) {
        self.engine.delete_surface_triangle(surface, tri_idx);
        if let Some(surf) = self.surfaces.get_mut(surface) {
            if tri_idx < surf.triangles.len() {
                surf.triangles.remove(tri_idx);
            }
        }
    }

    pub fn add_breakline(&mut self, surface: usize, a: usize, b: usize) {
        if let Some(surf) = self.surfaces.get_mut(surface) {
            if a < surf.vertices.len()
                && b < surf.vertices.len()
                && !surf
                    .breaklines
                    .iter()
                    .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
            {
                surf.breaklines.push((a, b));
            }
        }
    }

    pub fn remove_breakline(&mut self, surface: usize, a: usize, b: usize) {
        if let Some(surf) = self.surfaces.get_mut(surface) {
            if let Some(pos) = surf
                .breaklines
                .iter()
                .position(|&(x, y)| (x == a && y == b) || (x == b && y == a))
            {
                surf.breaklines.remove(pos);
            }
        }
    }

    pub fn set_boundary(&mut self, surface: usize, boundary: Vec<usize>) {
        if let Some(surf) = self.surfaces.get_mut(surface) {
            if boundary.iter().all(|&i| i < surf.vertices.len()) && boundary.len() >= 3 {
                surf.boundary = Some(boundary);
            }
        }
    }

    pub fn clear_boundary(&mut self, surface: usize) {
        if let Some(surf) = self.surfaces.get_mut(surface) {
            surf.boundary = None;
        }
    }

    pub fn clear(&mut self) {
        for _ in 0..self.point_ids.len() {
            self.remove_point(0);
        }
        for _ in 0..self.line_ids.len() {
            self.remove_line(0);
        }
        for _ in 0..self.dimension_ids.len() {
            self.remove_dimension(0);
        }
        for _ in 0..self.surface_ids.len() {
            self.remove_surface(0);
        }
        self.points.clear();
        self.lines.clear();
        self.dimensions.clear();
        self.surfaces.clear();
        if let Some((_, handles)) = self.handles.take() {
            for id in handles {
                self.engine.remove_point_marker(id);
            }
        }
    }

    /// Highlight or un-highlight a surface.
    pub fn highlight_surface(&mut self, idx: usize, on: bool) {
        let color = if on {
            Vector4::new(1.0, 1.0, 0.0, 1.0)
        } else {
            Vector4::new(1.0, 1.0, 1.0, 1.0)
        };
        self.engine.set_surface_color(idx, color);
    }

    /// Show editing handles for the given surface.
    pub fn show_surface_handles(&mut self, idx: usize) {
        self.hide_handles();
        if let Some(surf) = self.surfaces.get(idx) {
            let mut ids = Vec::new();
            for v in &surf.vertices {
                ids.push(self.engine.add_point_marker(*v));
            }
            self.handles = Some((idx, ids));
        }
    }

    /// Remove all editing handles.
    pub fn hide_handles(&mut self) {
        if let Some((_, handles)) = self.handles.take() {
            for id in handles {
                self.engine.remove_point_marker(id);
            }
        }
    }

    /// Move a handle and the underlying vertex.
    #[allow(dead_code)]
    pub fn move_handle(&mut self, handle_idx: usize, new_pos: Point3) {
        if let Some((surf_idx, ref mut handles)) = self.handles {
            if let Some(id) = handles.get(handle_idx).copied() {
                self.engine.update_point_marker(id, new_pos);
                self.move_vertex(surf_idx, handle_idx, new_pos);
            }
        }
    }

    /// Highlight or un-highlight a handle.
    pub fn highlight_handle(&mut self, handle_idx: usize, on: bool) {
        if let Some((_, ref handles)) = self.handles {
            if let Some(id) = handles.get(handle_idx).copied() {
                let color = if on {
                    Vector4::new(1.0, 0.0, 0.0, 1.0)
                } else {
                    Vector4::new(1.0, 1.0, 1.0, 1.0)
                };
                self.engine.set_point_marker_color(id, color);
            }
        }
    }

    /// Get the world position of a handle.
    pub fn handle_position(&self, handle_idx: usize) -> Option<Point3> {
        self.handles.as_ref().and_then(|(_, handles)| {
            handles
                .get(handle_idx)
                .copied()
                .and_then(|id| self.engine.point_marker_position(id))
        })
    }

    /// Convert screen coordinates to a point on the plane z.
    pub fn screen_to_plane(&self, x: f64, y: f64, z: f64) -> Point3 {
        let ray = self.engine.screen_ray(x, y);
        let dir = ray.direction();
        let orig = ray.origin();
        let t = if dir.z.abs() < f64::EPSILON {
            0.0
        } else {
            (z - orig.z) / dir.z
        };
        orig + dir * t
    }

    /// Hit test screen coordinates against existing objects.
    pub fn hit_test(&mut self, x: f64, y: f64) -> Option<HitObject> {
        let mut result = None;
        let mut best_z = f64::INFINITY;

        if let Some((_, handles)) = &self.handles {
            for (i, hid) in handles.iter().enumerate() {
                if let Some(p) = self.engine.point_marker_position(*hid) {
                    if let Some((sx, sy, z)) = self.engine.project_point(p) {
                        let d2 = (sx - x).powi(2) + (sy - y).powi(2);
                        if d2 < 64.0 && z < best_z {
                            best_z = z;
                            result = Some(HitObject::Handle(i));
                        }
                    }
                }
            }
            if result.is_some() {
                return result;
            }
        }

        for (i, p) in self.points.iter().enumerate() {
            if let Some((sx, sy, z)) = self.engine.project_point(*p) {
                let d2 = (sx - x).powi(2) + (sy - y).powi(2);
                if d2 < 64.0 && z < best_z {
                    best_z = z;
                    result = Some(HitObject::Point);
                }
            }
        }

        for (i, (a, b, _, _)) in self.lines.iter().enumerate() {
            if let (Some((ax, ay, az)), Some((bx, by, bz))) = (
                self.engine.project_point(*a),
                self.engine.project_point(*b),
            ) {
                let t = ((x - ax) * (bx - ax) + (y - ay) * (by - ay))
                    / ((bx - ax).powi(2) + (by - ay).powi(2));
                if (0.0..=1.0).contains(&t) {
                    let lx = ax + t * (bx - ax);
                    let ly = ay + t * (by - ay);
                    let lz = az + t * (bz - az);
                    let d2 = (x - lx).powi(2) + (y - ly).powi(2);
                    if d2 < 36.0 && lz < best_z {
                        best_z = lz;
                        result = Some(HitObject::Line);
                    }
                }
            }
        }

        for (si, surf) in self.surfaces.iter().enumerate() {
            for (bi, &(i1, i2)) in surf.breaklines.iter().enumerate() {
                if let (Some((ax, ay, az)), Some((bx, by, bz))) = (
                    self.engine.project_point(surf.vertices[i1]),
                    self.engine.project_point(surf.vertices[i2]),
                ) {
                    let t = ((x - ax) * (bx - ax) + (y - ay) * (by - ay))
                        / ((bx - ax).powi(2) + (by - ay).powi(2));
                    if (0.0..=1.0).contains(&t) {
                        let lx = ax + t * (bx - ax);
                        let ly = ay + t * (by - ay);
                        let lz = az + t * (bz - az);
                        let d2 = (x - lx).powi(2) + (y - ly).powi(2);
                        if d2 < 36.0 && lz < best_z {
                            best_z = lz;
                            let _ = (si, bi); // indices currently unused
                            result = Some(HitObject::Breakline);
                        }
                    }
                }
            }
            if let Some(bound) = &surf.boundary {
                for (bi, window) in bound.windows(2).enumerate() {
                    let i1 = window[0];
                    let i2 = window[1];
                    if let (Some((ax, ay, az)), Some((bx, by, bz))) = (
                        self.engine.project_point(surf.vertices[i1]),
                        self.engine.project_point(surf.vertices[i2]),
                    ) {
                        let t = ((x - ax) * (bx - ax) + (y - ay) * (by - ay))
                            / ((bx - ax).powi(2) + (by - ay).powi(2));
                        if (0.0..=1.0).contains(&t) {
                            let lx = ax + t * (bx - ax);
                            let ly = ay + t * (by - ay);
                            let lz = az + t * (bz - az);
                            let d2 = (x - lx).powi(2) + (y - ly).powi(2);
                            if d2 < 36.0 && lz < best_z {
                                best_z = lz;
                                let _ = (si, bi);
                                result = Some(HitObject::Boundary);
                            }
                        }
                    }
                }
                // close edge from last to first
                if bound.len() > 1 {
                    let i1 = bound[bound.len() - 1];
                    let i2 = bound[0];
                    if let (Some((ax, ay, az)), Some((bx, by, bz))) = (
                        self.engine.project_point(surf.vertices[i1]),
                        self.engine.project_point(surf.vertices[i2]),
                    ) {
                        let t = ((x - ax) * (bx - ax) + (y - ay) * (by - ay))
                            / ((bx - ax).powi(2) + (by - ay).powi(2));
                        if (0.0..=1.0).contains(&t) {
                            let lx = ax + t * (bx - ax);
                            let ly = ay + t * (by - ay);
                            let lz = az + t * (bz - az);
                            let d2 = (x - lx).powi(2) + (y - ly).powi(2);
                            if d2 < 36.0 && lz < best_z {
                                best_z = lz;
                                let _ = (si, bound.len() - 1);
                                result = Some(HitObject::Boundary);
                            }
                        }
                    }
                }
            }
        }

        for (i, surf) in self.surfaces.iter().enumerate() {
            for tri in &surf.triangles {
                let p0 = surf.vertices[tri[0]];
                let p1 = surf.vertices[tri[1]];
                let p2 = surf.vertices[tri[2]];
                if let (Some(a), Some(b), Some(c)) = (
                    self.engine.project_point(p0),
                    self.engine.project_point(p1),
                    self.engine.project_point(p2),
                ) {
                    let denom = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
                    if denom.abs() < f64::EPSILON {
                        continue;
                    }
                    let w1 = ((b.1 - c.1) * (x - c.0) + (c.0 - b.0) * (y - c.1)) / denom;
                    let w2 = ((c.1 - a.1) * (x - c.0) + (a.0 - c.0) * (y - c.1)) / denom;
                    let w3 = 1.0 - w1 - w2;
                    if w1 >= 0.0 && w2 >= 0.0 && w3 >= 0.0 {
                        let z = w1 * a.2 + w2 * b.2 + w3 * c.2;
                        if z < best_z {
                            best_z = z;
                            result = Some(HitObject::Surface(i));
                        }
                    }
                }
            }
        }

        match result {
            Some(HitObject::Handle(i)) => {
                if self.hover_handle != Some(i) {
                    if let Some(prev) = self.hover_handle.take() {
                        self.highlight_handle(prev, false);
                    }
                    self.highlight_handle(i, true);
                    self.hover_handle = Some(i);
                }
                if let Some(prev) = self.hover_surface.take() {
                    self.highlight_surface(prev, false);
                }
            }
            Some(HitObject::Surface(i)) => {
                if self.hover_surface != Some(i) {
                    if let Some(prev) = self.hover_surface.take() {
                        self.highlight_surface(prev, false);
                    }
                    self.highlight_surface(i, true);
                    self.hover_surface = Some(i);
                }
                if let Some(prev) = self.hover_handle.take() {
                    self.highlight_handle(prev, false);
                }
            }
            _ => {
                if let Some(prev) = self.hover_surface.take() {
                    self.highlight_surface(prev, false);
                }
                if let Some(prev) = self.hover_handle.take() {
                    self.highlight_handle(prev, false);
                }
            }
        }

        result
    }
}
