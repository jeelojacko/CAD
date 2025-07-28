use slint::Image;
use truck_cad_engine::TruckCadEngine;
use truck_modeling::base::{InnerSpace, Matrix4, Point3, Rad, Transform, Vector3, Vector4};
use truck_modeling::topology::Solid;
use truck_rendimpl::PolygonState;

use crate::geometry::GeometryStore;
use rstar::{RTree, RTreeObject, AABB};
use survey_cad::alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
use survey_cad::geometry::Point;

/// Line definition used for batch additions.
type LineInput = ([f64; 3], [f64; 3], [f32; 4], f32);
/// Surface definition used for batch additions.
type SurfaceInput<'a> = (&'a [Point3], &'a [[usize; 3]]);

const GIZMO_SIZE: f64 = 1.0;

#[derive(Clone)]
pub enum HitObject {
    Point(usize),
    Line(usize),
    Surface(usize),
    Handle(usize),
    Breakline,
    Boundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone)]
enum HandleTarget {
    Point(usize),
    Line(usize),
    Surface(usize),
    AlignmentPi,
    VerticalVertex,
    Gizmo(GizmoMode, HitObject),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SpatialElement {
    Point(usize),
    Line(usize),
    Surface(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SpatialItem {
    bbox: AABB<[f64; 3]>,
    elem: SpatialElement,
}

impl RTreeObject for SpatialItem {
    type Envelope = AABB<[f64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.bbox
    }
}

pub struct TruckBackend {
    engine: TruckCadEngine,
    point_ids: Vec<Option<usize>>,
    line_ids: Vec<Option<usize>>,
    dimension_ids: Vec<Option<usize>>,
    surface_ids: Vec<Option<usize>>,
    geometry: GeometryStore,
    handles: Option<(HandleTarget, Vec<usize>)>,
    hover_surface: Option<usize>,
    hover_handle: Option<usize>,
    spatial_index: RTree<SpatialItem>,
    snap_point: Option<Point3>,
    gizmo_origin: Option<Point3>,
}

impl TruckBackend {
    pub fn new(width: u32, height: u32) -> Self {
        let mut engine = TruckCadEngine::new(width, height);
        engine.add_unit_cube();
        let mut backend = Self {
            engine,
            point_ids: Vec::new(),
            line_ids: Vec::new(),
            dimension_ids: Vec::new(),
            surface_ids: Vec::new(),
            geometry: GeometryStore::new(),
            handles: None,
            hover_surface: None,
            hover_handle: None,
            spatial_index: RTree::new(),
            snap_point: None,
            gizmo_origin: None,
        };
        backend.rebuild_index();
        backend
    }

    fn rebuild_index(&mut self) {
        self.spatial_index = RTree::new();
        for (i, p) in self.geometry.points.iter().enumerate() {
            self.spatial_index.insert(SpatialItem {
                bbox: AABB::from_corners([p.x, p.y, p.z], [p.x, p.y, p.z]),
                elem: SpatialElement::Point(i),
            });
        }
        for (i, (a, b, _, _)) in self.geometry.lines.iter().enumerate() {
            let min = [a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)];
            let max = [a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)];
            self.spatial_index.insert(SpatialItem {
                bbox: AABB::from_corners(min, max),
                elem: SpatialElement::Line(i),
            });
        }
        for (i, surf) in self.geometry.surfaces.iter().enumerate() {
            if let Some(first) = surf.vertices.first() {
                let mut min = [first.x, first.y, first.z];
                let mut max = min;
                for v in &surf.vertices {
                    if v.x < min[0] {
                        min[0] = v.x;
                    }
                    if v.y < min[1] {
                        min[1] = v.y;
                    }
                    if v.z < min[2] {
                        min[2] = v.z;
                    }
                    if v.x > max[0] {
                        max[0] = v.x;
                    }
                    if v.y > max[1] {
                        max[1] = v.y;
                    }
                    if v.z > max[2] {
                        max[2] = v.z;
                    }
                }
                self.spatial_index.insert(SpatialItem {
                    bbox: AABB::from_corners(min, max),
                    elem: SpatialElement::Surface(i),
                });
            }
        }
    }

    pub fn render(&mut self) -> Image {
        self.engine.render_to_image()
    }

    pub fn set_lod(&mut self, enabled: bool, distance: f64) {
        if enabled {
            self.engine.enable_lod(distance);
        } else {
            self.engine.disable_lod();
        }
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
        let idx = self.geometry.add_point(Point3::new(x, y, z));
        self.rebuild_index();
        idx
    }

    pub fn update_point(&mut self, idx: usize, x: f64, y: f64, z: f64) {
        if let Some(Some(id)) = self.point_ids.get(idx) {
            self.engine.update_point_marker(*id, Point3::new(x, y, z));
        }
        self.geometry.update_point(idx, Point3::new(x, y, z));
        self.rebuild_index();
    }

    pub fn remove_point(&mut self, idx: usize) {
        if idx < self.point_ids.len() {
            if let Some(id) = self.point_ids.remove(idx) {
                self.engine.remove_point_marker(id);
            }
            self.geometry.remove_point(idx);
        }
        self.rebuild_index();
    }

    /// Add multiple points in a single operation.
    pub fn add_points(&mut self, points: &[Point3]) {
        for p in points {
            let id = self.engine.add_point_marker(*p);
            self.point_ids.push(Some(id));
            self.geometry.add_point(*p);
        }
        self.rebuild_index();
    }

    pub fn add_line(&mut self, a: [f64; 3], b: [f64; 3], color: [f32; 4], weight: f32) -> usize {
        let col = Vector4::new(
            color[0] as f64,
            color[1] as f64,
            color[2] as f64,
            color[3] as f64,
        );
        let id = self.engine.add_line(
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
            col,
            weight,
        );
        self.line_ids.push(Some(id));
        let idx = self.geometry.add_line(
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
            col,
            weight,
        );
        self.rebuild_index();
        idx
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
                Vector4::new(
                    color[0] as f64,
                    color[1] as f64,
                    color[2] as f64,
                    color[3] as f64,
                ),
                weight,
            );
        }
        self.geometry.update_line(
            idx,
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
            Vector4::new(
                color[0] as f64,
                color[1] as f64,
                color[2] as f64,
                color[3] as f64,
            ),
            weight,
        );
        self.rebuild_index();
    }

    pub fn remove_line(&mut self, idx: usize) {
        if idx < self.line_ids.len() {
            if let Some(id) = self.line_ids.remove(idx) {
                self.engine.remove_line(id);
            }
            self.geometry.remove_line(idx);
        }
        self.rebuild_index();
    }

    /// Add multiple lines at once.
    pub fn add_lines(&mut self, lines: &[LineInput]) {
        for (a, b, color, weight) in lines {
            let col = Vector4::new(
                color[0] as f64,
                color[1] as f64,
                color[2] as f64,
                color[3] as f64,
            );
            let id = self.engine.add_line(
                Point3::new(a[0], a[1], a[2]),
                Point3::new(b[0], b[1], b[2]),
                col,
                *weight,
            );
            self.line_ids.push(Some(id));
            self.geometry.add_line(
                Point3::new(a[0], a[1], a[2]),
                Point3::new(b[0], b[1], b[2]),
                col,
                *weight,
            );
        }
        self.rebuild_index();
    }

    /// Add a dimension represented as a simple line between two points.
    pub fn add_dimension(
        &mut self,
        a: [f64; 3],
        b: [f64; 3],
        color: [f32; 4],
        weight: f32,
    ) -> usize {
        let id = self.engine.add_line(
            Point3::new(a[0], a[1], a[2]),
            Point3::new(b[0], b[1], b[2]),
            Vector4::new(
                color[0] as f64,
                color[1] as f64,
                color[2] as f64,
                color[3] as f64,
            ),
            weight,
        );
        self.dimension_ids.push(Some(id));
        self.geometry
            .add_dimension(Point3::new(a[0], a[1], a[2]), Point3::new(b[0], b[1], b[2]))
    }

    /// Remove an existing dimension.
    pub fn remove_dimension(&mut self, idx: usize) {
        if idx < self.dimension_ids.len() {
            if let Some(id) = self.dimension_ids.remove(idx) {
                self.engine.remove_line(id);
            }
            self.geometry.remove_dimension(idx);
        }
    }

    pub fn add_surface(&mut self, vertices: &[Point3], triangles: &[[usize; 3]]) -> usize {
        let id = self.engine.add_surface(vertices, triangles);
        self.surface_ids.push(Some(id));
        let idx = self.geometry.add_surface(vertices, triangles);
        self.rebuild_index();
        idx
    }

    /// Add many surfaces using the engine's batching API.
    pub fn add_surfaces(&mut self, surfs: &[SurfaceInput<'_>]) {
        let mut engine_meshes = Vec::new();
        for (v, t) in surfs {
            self.surface_ids.push(None);
            self.geometry.add_surface(v, t);
            engine_meshes.push((v.to_vec(), t.to_vec()));
        }
        self.engine
            .add_batched_mesh(&engine_meshes, &PolygonState::default());
        self.rebuild_index();
    }

    pub fn add_solid(&mut self, solid: Solid) {
        self.geometry.add_solid(solid.clone());
        self.engine.add_solid(solid);
    }

    #[allow(dead_code)]
    pub fn update_surface(&mut self, idx: usize, vertices: &[Point3], triangles: &[[usize; 3]]) {
        if let Some(Some(id)) = self.surface_ids.get(idx) {
            self.engine.update_surface(*id, vertices, triangles);
        }
        self.geometry.update_surface(idx, vertices, triangles);
        self.rebuild_index();
    }

    pub fn remove_surface(&mut self, idx: usize) {
        if idx < self.surface_ids.len() {
            if let Some(id) = self.surface_ids.remove(idx) {
                self.engine.remove_surface(id);
            }
            self.geometry.remove_surface(idx);
        }
        self.rebuild_index();
    }

    pub fn add_vertex(&mut self, surface: usize, p: Point3) -> Option<usize> {
        let res = self.engine.add_surface_vertex(surface, p);
        if let Some(idx) = res {
            if let Some(g_idx) = self.geometry.add_vertex(surface, p) {
                debug_assert_eq!(idx, g_idx);
            }
        }
        self.rebuild_index();
        res
    }

    pub fn move_vertex(&mut self, surface: usize, idx: usize, p: Point3) {
        self.engine.move_surface_vertex(surface, idx, p);
        self.geometry.move_vertex(surface, idx, p);
        self.rebuild_index();
    }

    pub fn delete_vertex(&mut self, surface: usize, idx: usize) {
        self.engine.delete_surface_vertex(surface, idx);
        self.geometry.delete_vertex(surface, idx);
        self.rebuild_index();
    }

    pub fn add_triangle(&mut self, surface: usize, tri: [usize; 3]) {
        self.engine.add_surface_triangle(surface, tri);
        self.geometry.add_triangle(surface, tri);
        self.rebuild_index();
    }

    pub fn delete_triangle(&mut self, surface: usize, tri_idx: usize) {
        self.engine.delete_surface_triangle(surface, tri_idx);
        self.geometry.delete_triangle(surface, tri_idx);
        self.rebuild_index();
    }

    pub fn add_breakline(&mut self, surface: usize, a: usize, b: usize) {
        self.geometry.add_breakline(surface, a, b);
    }

    pub fn remove_breakline(&mut self, surface: usize, a: usize, b: usize) {
        self.geometry.remove_breakline(surface, a, b);
    }

    pub fn set_boundary(&mut self, surface: usize, boundary: Vec<usize>) {
        self.geometry.set_boundary(surface, boundary);
    }

    pub fn clear_boundary(&mut self, surface: usize) {
        self.geometry.clear_boundary(surface);
    }

    pub fn clear(&mut self) {
        self.engine.clear_scene();
        self.point_ids.clear();
        self.line_ids.clear();
        self.dimension_ids.clear();
        self.surface_ids.clear();
        self.geometry.clear();
        if let Some((_, handles)) = self.handles.take() {
            for id in handles {
                self.engine.remove_point_marker(id);
            }
        }
        self.rebuild_index();
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
        if let Some(surf) = self.geometry.surfaces.get(idx) {
            let mut ids = Vec::new();
            for v in &surf.vertices {
                ids.push(self.engine.add_point_marker(*v));
            }
            self.handles = Some((HandleTarget::Surface(idx), ids));
        }
    }

    /// Show handle for a single point.
    pub fn show_point_handles(&mut self, idx: usize) {
        self.hide_handles();
        if idx < self.geometry.points.len() {
            let p = self.geometry.points[idx];
            let id = self.engine.add_point_marker(p);
            self.handles = Some((HandleTarget::Point(idx), vec![id]));
        }
    }

    /// Show handles for a line's endpoints.
    pub fn show_line_handles(&mut self, idx: usize) {
        self.hide_handles();
        if idx < self.geometry.lines.len() {
            let (a, b, _, _) = self.geometry.lines[idx];
            let ids = vec![
                self.engine.add_point_marker(a),
                self.engine.add_point_marker(b),
            ];
            self.handles = Some((HandleTarget::Line(idx), ids));
        }
    }

    /// Show handles for horizontal alignment PI points.
    pub fn show_alignment_pi_handles(&mut self, align: &Alignment) {
        use survey_cad::alignment::HorizontalElement;
        self.hide_handles();
        let mut ids = Vec::new();
        if let Some(first) = align.horizontal.elements.first() {
            let p = match first {
                HorizontalElement::Tangent { start, .. } => *start,
                HorizontalElement::Curve { arc } => Point::new(
                    arc.center.x + arc.radius * arc.start_angle.cos(),
                    arc.center.y + arc.radius * arc.start_angle.sin(),
                ),
                HorizontalElement::Spiral { spiral } => spiral.start_point(),
            };
            ids.push(self.engine.add_point_marker(Point3::new(p.x, p.y, 0.0)));
        }
        for elem in &align.horizontal.elements {
            let p = match elem {
                HorizontalElement::Tangent { end, .. } => *end,
                HorizontalElement::Curve { arc } => Point::new(
                    arc.center.x + arc.radius * arc.end_angle.cos(),
                    arc.center.y + arc.radius * arc.end_angle.sin(),
                ),
                HorizontalElement::Spiral { spiral } => spiral.end_point(),
            };
            ids.push(self.engine.add_point_marker(Point3::new(p.x, p.y, 0.0)));
        }
        self.handles = Some((HandleTarget::AlignmentPi, ids));
    }

    /// Show handles for vertical profile vertices projected into 3D.
    pub fn show_vertical_handles(&mut self, align: &Alignment) {
        use survey_cad::alignment::VerticalElement;
        self.hide_handles();
        let mut ids = Vec::new();
        let va = &align.vertical;
        if let Some(first) = va.elements.first() {
            let (s, z) = match first {
                VerticalElement::Grade {
                    start_station,
                    start_elev,
                    ..
                } => (*start_station, *start_elev),
                VerticalElement::Parabola {
                    start_station,
                    start_elev,
                    ..
                } => (*start_station, *start_elev),
            };
            if let Some(p) = align.horizontal.point_at(s) {
                ids.push(self.engine.add_point_marker(Point3::new(p.x, p.y, z)));
            }
        }
        for e in &va.elements {
            let (s, z) = match e {
                VerticalElement::Grade {
                    end_station,
                    end_elev,
                    ..
                } => (*end_station, *end_elev),
                VerticalElement::Parabola {
                    start_station,
                    end_station,
                    start_elev,
                    start_grade,
                    end_grade,
                } => {
                    let l = end_station - start_station;
                    let dz = start_grade * l + 0.5 * (end_grade - start_grade) * l;
                    (*end_station, start_elev + dz)
                }
            };
            if let Some(p) = align.horizontal.point_at(s) {
                ids.push(self.engine.add_point_marker(Point3::new(p.x, p.y, z)));
            }
        }
        self.handles = Some((HandleTarget::VerticalVertex, ids));
    }

    /// Show transformation gizmo for the selected object.
    pub fn show_gizmo(&mut self, mode: GizmoMode, target: HitObject) {
        self.hide_handles();
        let origin = self.object_center(&target);
        let ids = [
            self.engine
                .add_point_marker(origin + Vector3::unit_x() * GIZMO_SIZE),
            self.engine
                .add_point_marker(origin + Vector3::unit_y() * GIZMO_SIZE),
            self.engine
                .add_point_marker(origin + Vector3::unit_z() * GIZMO_SIZE),
        ]
        .to_vec();
        self.gizmo_origin = Some(origin);
        self.handles = Some((HandleTarget::Gizmo(mode, target), ids));
    }

    /// Remove all editing handles.
    pub fn hide_handles(&mut self) {
        if let Some((_, handles)) = self.handles.take() {
            for id in handles {
                self.engine.remove_point_marker(id);
            }
        }
        self.gizmo_origin = None;
    }

    /// Move a handle and the underlying vertex.
    #[allow(dead_code)]
    pub fn move_handle(&mut self, handle_idx: usize, new_pos: Point3) {
        let target = match self.handles.clone() {
            Some((t, _)) => t,
            None => return,
        };
        if let Some((_, ref mut handles)) = self.handles {
            if let Some(id) = handles.get(handle_idx) {
                self.engine.update_point_marker(*id, new_pos);
            }
        }
        match target {
            HandleTarget::Surface(idx) => self.move_vertex(idx, handle_idx, new_pos),
            HandleTarget::Point(idx) => {
                if handle_idx == 0 {
                    self.update_point(idx, new_pos.x, new_pos.y, new_pos.z);
                }
            }
            HandleTarget::Line(idx) => {
                if let Some(line) = self.geometry.lines.get_mut(idx) {
                    if handle_idx == 0 {
                        line.0 = new_pos;
                    } else if handle_idx == 1 {
                        line.1 = new_pos;
                    }
                    let (p0, p1, col, weight) = *line;
                    self.update_line(
                        idx,
                        [p0.x, p0.y, p0.z],
                        [p1.x, p1.y, p1.z],
                        [col.x as f32, col.y as f32, col.z as f32, col.w as f32],
                        weight,
                    );
                }
            }
            HandleTarget::AlignmentPi | HandleTarget::VerticalVertex => {}
            HandleTarget::Gizmo(mode, obj) => {
                if let Some(origin) = self.gizmo_origin {
                    let axis = match handle_idx {
                        0 => Vector3::unit_x(),
                        1 => Vector3::unit_y(),
                        _ => Vector3::unit_z(),
                    };
                    match mode {
                        GizmoMode::Translate => {
                            let start = origin + axis * GIZMO_SIZE;
                            let delta = (new_pos - start).dot(axis) * axis;
                            self.translate_object(&obj, delta);
                            self.gizmo_origin = Some(origin + delta);
                        }
                        GizmoMode::Rotate => {
                            let start_vec = (axis * GIZMO_SIZE).normalize();
                            let new_vec = (new_pos - origin).normalize();
                            let angle = start_vec.angle(new_vec);
                            let sign = if start_vec.cross(new_vec).dot(axis) < 0.0 {
                                -1.0
                            } else {
                                1.0
                            };
                            self.rotate_object(&obj, origin, axis, angle.0 * sign);
                            self.gizmo_origin = Some(self.object_center(&obj));
                        }
                        GizmoMode::Scale => {
                            let start_len = GIZMO_SIZE;
                            let new_len = (new_pos - origin).dot(axis);
                            let factor = if start_len.abs() < f64::EPSILON {
                                1.0
                            } else {
                                new_len / start_len
                            };
                            self.scale_object(&obj, origin, factor);
                            self.gizmo_origin = Some(self.object_center(&obj));
                        }
                    }
                    if let Some((_, ref mut handles)) = self.handles {
                        if let Some(o) = self.gizmo_origin {
                            for (i, hid) in handles.iter().enumerate() {
                                let ax = match i {
                                    0 => Vector3::unit_x(),
                                    1 => Vector3::unit_y(),
                                    _ => Vector3::unit_z(),
                                };
                                let p = o + ax * GIZMO_SIZE;
                                self.engine.update_point_marker(*hid, p);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Move a horizontal alignment PI handle.
    pub fn move_alignment_pi_handle(
        &mut self,
        alignment: &mut Alignment,
        handle_idx: usize,
        pos: Point3,
    ) {
        use survey_cad::alignment::HorizontalElement;
        if let Some((HandleTarget::AlignmentPi, ref handles)) = self.handles {
            if let Some(id) = handles.get(handle_idx) {
                self.engine.update_point_marker(*id, pos);
            }
        }
        let p = Point::new(pos.x, pos.y);
        if handle_idx == 0 {
            if let Some(HorizontalElement::Tangent { start, .. }) =
                alignment.horizontal.elements.get_mut(0)
            {
                *start = p;
            }
        }
        if handle_idx > 0 {
            if let Some(HorizontalElement::Tangent { end, .. }) =
                alignment.horizontal.elements.get_mut(handle_idx - 1)
            {
                *end = p;
            }
        }
        if let Some(HorizontalElement::Tangent { start, .. }) =
            alignment.horizontal.elements.get_mut(handle_idx)
        {
            *start = p;
        }
    }

    /// Move a vertical profile handle.
    pub fn move_vertical_handle(
        &mut self,
        hal: &HorizontalAlignment,
        valign: &mut VerticalAlignment,
        idx: usize,
        pos: Point3,
    ) {
        use survey_cad::alignment::VerticalElement;
        if let Some((HandleTarget::VerticalVertex, ref handles)) = self.handles {
            if let Some(id) = handles.get(idx) {
                self.engine.update_point_marker(*id, pos);
            }
        }
        let station = Self::nearest_station(hal, Point::new(pos.x, pos.y));
        let elev = pos.z;
        if idx == 0 {
            match &mut valign.elements[0] {
                VerticalElement::Grade {
                    start_station,
                    start_elev,
                    ..
                } => {
                    *start_station = station;
                    *start_elev = elev;
                }
                VerticalElement::Parabola {
                    start_station,
                    start_elev,
                    ..
                } => {
                    *start_station = station;
                    *start_elev = elev;
                }
            }
        }
        if idx > 0 {
            if let Some(prev) = valign.elements.get_mut(idx - 1) {
                match prev {
                    VerticalElement::Grade {
                        end_station,
                        end_elev,
                        ..
                    } => {
                        *end_station = station;
                        *end_elev = elev;
                    }
                    VerticalElement::Parabola { end_station, .. } => {
                        *end_station = station;
                    }
                }
            }
        }
        if let Some(cur) = valign.elements.get_mut(idx) {
            match cur {
                VerticalElement::Grade {
                    start_station,
                    start_elev,
                    ..
                } => {
                    *start_station = station;
                    *start_elev = elev;
                }
                VerticalElement::Parabola {
                    start_station,
                    start_elev,
                    ..
                } => {
                    *start_station = station;
                    *start_elev = elev;
                }
            }
        }
    }

    /// Highlight or un-highlight a handle.
    pub fn highlight_handle(&mut self, handle_idx: usize, on: bool) {
        if let Some((_, ref handles)) = self.handles {
            if handle_idx < handles.len() {
                let id = handles[handle_idx];
                let color = if on {
                    Vector4::new(1.0, 0.0, 0.0, 1.0)
                } else {
                    Vector4::new(1.0, 1.0, 1.0, 1.0)
                };
                self.engine.set_point_marker_color(id, color);
            }
        }
    }

    fn translate_object(&mut self, obj: &HitObject, delta: Vector3) {
        match *obj {
            HitObject::Point(i) => {
                if let Some(p) = self.geometry.points.get(i).cloned() {
                    self.update_point(i, p.x + delta.x, p.y + delta.y, p.z + delta.z);
                }
            }
            HitObject::Line(i) => {
                if let Some((a, b, col, weight)) = self.geometry.lines.get(i).cloned() {
                    self.update_line(
                        i,
                        [a.x + delta.x, a.y + delta.y, a.z + delta.z],
                        [b.x + delta.x, b.y + delta.y, b.z + delta.z],
                        [col.x as f32, col.y as f32, col.z as f32, col.w as f32],
                        weight,
                    );
                }
            }
            HitObject::Surface(i) => {
                if let Some(s) = self.geometry.surfaces.get(i).cloned() {
                    for (vi, v) in s.vertices.iter().enumerate() {
                        self.move_vertex(i, vi, *v + delta);
                    }
                }
            }
            _ => {}
        }
    }

    fn rotate_object(&mut self, obj: &HitObject, origin: Point3, axis: Vector3, angle: f64) {
        let rot = Matrix4::from_axis_angle(axis, Rad(angle));
        match *obj {
            HitObject::Point(i) => {
                if let Some(p) = self.geometry.points.get(i).cloned() {
                    let v = rot.transform_vector(p - origin);
                    let np = origin + v;
                    self.update_point(i, np.x, np.y, np.z);
                }
            }
            HitObject::Line(i) => {
                if let Some((a, b, col, weight)) = self.geometry.lines.get(i).cloned() {
                    let na = origin + rot.transform_vector(a - origin);
                    let nb = origin + rot.transform_vector(b - origin);
                    self.update_line(
                        i,
                        [na.x, na.y, na.z],
                        [nb.x, nb.y, nb.z],
                        [col.x as f32, col.y as f32, col.z as f32, col.w as f32],
                        weight,
                    );
                }
            }
            HitObject::Surface(i) => {
                if let Some(s) = self.geometry.surfaces.get(i).cloned() {
                    for (vi, v) in s.vertices.iter().enumerate() {
                        let nv = origin + rot.transform_vector(*v - origin);
                        self.move_vertex(i, vi, nv);
                    }
                }
            }
            _ => {}
        }
    }

    fn scale_object(&mut self, obj: &HitObject, origin: Point3, factor: f64) {
        match *obj {
            HitObject::Point(i) => {
                if let Some(p) = self.geometry.points.get(i).cloned() {
                    let v = origin + (p - origin) * factor;
                    self.update_point(i, v.x, v.y, v.z);
                }
            }
            HitObject::Line(i) => {
                if let Some((a, b, col, weight)) = self.geometry.lines.get(i).cloned() {
                    let na = origin + (a - origin) * factor;
                    let nb = origin + (b - origin) * factor;
                    self.update_line(
                        i,
                        [na.x, na.y, na.z],
                        [nb.x, nb.y, nb.z],
                        [col.x as f32, col.y as f32, col.z as f32, col.w as f32],
                        weight,
                    );
                }
            }
            HitObject::Surface(i) => {
                if let Some(s) = self.geometry.surfaces.get(i).cloned() {
                    for (vi, v) in s.vertices.iter().enumerate() {
                        let nv = origin + (*v - origin) * factor;
                        self.move_vertex(i, vi, nv);
                    }
                }
            }
            _ => {}
        }
    }

    /// Get the world position of a handle.
    pub fn handle_position(&self, handle_idx: usize) -> Option<Point3> {
        if let Some((_, handles)) = self.handles.as_ref() {
            if let Some(&id) = handles.as_slice().get(handle_idx) {
                self.engine.point_marker_position(id)
            } else {
                None
            }
        } else {
            None
        }
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

    fn nearest_station(hal: &HorizontalAlignment, p: Point) -> f64 {
        use survey_cad::alignment::HorizontalElement;
        let mut sta = 0.0;
        let mut best = 0.0;
        let mut best_d2 = f64::INFINITY;
        for elem in &hal.elements {
            match elem {
                HorizontalElement::Tangent { start, end } => {
                    let dx = end.x - start.x;
                    let dy = end.y - start.y;
                    let len2 = dx * dx + dy * dy;
                    if len2 > 0.0 {
                        let t = ((p.x - start.x) * dx + (p.y - start.y) * dy) / len2;
                        let t = t.clamp(0.0, 1.0);
                        let px = start.x + dx * t;
                        let py = start.y + dy * t;
                        let d2 = (p.x - px).powi(2) + (p.y - py).powi(2);
                        if d2 < best_d2 {
                            best_d2 = d2;
                            best = sta + (len2.sqrt() * t);
                        }
                        sta += len2.sqrt();
                    }
                }
                HorizontalElement::Curve { arc } => {
                    sta += arc.length();
                }
                HorizontalElement::Spiral { spiral } => {
                    sta += spiral.length;
                }
            }
        }
        best
    }

    /// Collect geometry for snapping.
    pub fn snap_scene(&self) -> crate::snap::Scene3D {
        let lines = self
            .geometry
            .lines
            .iter()
            .map(|(a, b, _, _)| (*a, *b))
            .collect();
        let mut surface_vertices = Vec::new();
        for s in &self.geometry.surfaces {
            surface_vertices.extend_from_slice(&s.vertices);
        }
        let mut solid_vertices = Vec::new();
        for verts in &self.geometry.solids {
            solid_vertices.extend_from_slice(verts);
        }
        crate::snap::Scene3D {
            points: self.geometry.points.clone(),
            lines,
            surface_vertices,
            solid_vertices,
        }
    }

    fn object_center(&self, obj: &HitObject) -> Point3 {
        match *obj {
            HitObject::Point(i) => self
                .geometry
                .points
                .get(i)
                .cloned()
                .unwrap_or(Point3::new(0.0, 0.0, 0.0)),
            HitObject::Line(i) => {
                if let Some((a, b, _, _)) = self.geometry.lines.get(i) {
                    let v = Vector3::new(a.x + b.x, a.y + b.y, a.z + b.z) * 0.5;
                    Point3::new(v.x, v.y, v.z)
                } else {
                    Point3::new(0.0, 0.0, 0.0)
                }
            }
            HitObject::Surface(i) => {
                if let Some(s) = self.geometry.surfaces.get(i) {
                    let mut sum = Vector3::new(0.0, 0.0, 0.0);
                    let len = s.vertices.len() as f64;
                    if len > 0.0 {
                        for v in &s.vertices {
                            sum += Vector3::new(v.x, v.y, v.z);
                        }
                        Point3::new(sum.x / len, sum.y / len, sum.z / len)
                    } else {
                        Point3::new(0.0, 0.0, 0.0)
                    }
                } else {
                    Point3::new(0.0, 0.0, 0.0)
                }
            }
            _ => Point3::new(0.0, 0.0, 0.0),
        }
    }

    pub fn resolve_snap_3d(
        &mut self,
        target: Point3,
        tol: f64,
        opts: crate::snap::SnapOptions,
    ) -> Option<Point3> {
        let scene = self.snap_scene();
        let res = crate::snap::resolve_snap_3d(target, &scene, tol, opts);
        self.snap_point = res;
        res
    }

    pub fn snap_point(&self) -> Option<Point3> {
        self.snap_point
    }

    pub fn clear_snap_point(&mut self) {
        self.snap_point = None;
    }

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

        let ray = self.engine.screen_ray(x, y);
        let cam = self.engine.camera();
        let start = ray.origin() + ray.direction() * cam.near_clip;
        let end = ray.origin() + ray.direction() * cam.far_clip;
        let expand = 0.5;
        let min = [
            start.x.min(end.x) - expand,
            start.y.min(end.y) - expand,
            start.z.min(end.z) - expand,
        ];
        let max = [
            start.x.max(end.x) + expand,
            start.y.max(end.y) + expand,
            start.z.max(end.z) + expand,
        ];
        let env = AABB::from_corners(min, max);
        let candidates: Vec<_> = self
            .spatial_index
            .locate_in_envelope_intersecting(&env)
            .cloned()
            .collect();

        use std::collections::HashSet;
        let mut cand_points = HashSet::new();
        let mut cand_lines = HashSet::new();
        let mut cand_surfaces = HashSet::new();
        for c in candidates {
            match c.elem {
                SpatialElement::Point(i) => {
                    cand_points.insert(i);
                }
                SpatialElement::Line(i) => {
                    cand_lines.insert(i);
                }
                SpatialElement::Surface(i) => {
                    cand_surfaces.insert(i);
                }
            }
        }

        for i in cand_points {
            if let Some(&p) = self.geometry.points.as_slice().get(i) {
                if let Some((sx, sy, z)) = self.engine.project_point(p) {
                    let d2 = (sx - x).powi(2) + (sy - y).powi(2);
                    if d2 < 64.0 && z < best_z {
                        best_z = z;
                        result = Some(HitObject::Point(i));
                    }
                }
            }
        }

        for i in cand_lines {
            if let Some(&(a, b, _, _)) = self.geometry.lines.as_slice().get(i) {
                if let (Some((ax, ay, az)), Some((bx, by, bz))) =
                    (self.engine.project_point(a), self.engine.project_point(b))
                {
                    let t = ((x - ax) * (bx - ax) + (y - ay) * (by - ay))
                        / ((bx - ax).powi(2) + (by - ay).powi(2));
                    if (0.0..=1.0).contains(&t) {
                        let lx = ax + t * (bx - ax);
                        let ly = ay + t * (by - ay);
                        let lz = az + t * (bz - az);
                        let d2 = (x - lx).powi(2) + (y - ly).powi(2);
                        if d2 < 36.0 && lz < best_z {
                            best_z = lz;
                            result = Some(HitObject::Line(i));
                        }
                    }
                }
            }
        }

        for si in cand_surfaces {
            if let Some(surf) = self.geometry.surfaces.get(si) {
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
                                let _ = (si, bi);
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
                                result = Some(HitObject::Surface(si));
                            }
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
