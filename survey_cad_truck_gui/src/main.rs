#![allow(unused_variables)]

use i_slint_common::sharedfontdb;
use slint::platform::PointerEventButton;
use slint::{Image, Model, SharedString, VecModel, PhysicalSize};
use std::io::Write;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use survey_cad::alignment::{Alignment, VerticalAlignment, VerticalElement};
use survey_cad::corridor;
use survey_cad::crs::list_known_crs;
use survey_cad::dtm::Tin;
use survey_cad::geometry::point::PointStyle;
use survey_cad::geometry::{
    Arc, Line, LineAnnotation, LineStyle, LineType, Point, PointSymbol, Polyline,
    convex_hull, Point3 as ScPoint3, LinearDimension,
};
use survey_cad::layers::{Layer, LayerManager as ScLayerManager};
use survey_cad::io::project::{read_project_json, write_project_json, Project, GridSettings};
use survey_cad::point_database::PointDatabase;
use survey_cad::styles::{
    format_dms, LineLabelPosition, LineLabelStyle, LineWeight, PointLabelStyle,
    TextStyle as ScTextStyle, HatchPattern,
};
use survey_cad::subassembly;
use survey_cad::superelevation::SuperelevationPoint;
mod snap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use truck_modeling::base::Point3;
use truck_modeling::base::Vector3;
use truck_modeling::builder;
use truck_modeling::topology::{Solid, Wire};
use truck_modeling::base::InnerSpace;

mod truck_backend;
use truck_backend::{TruckBackend, HitObject};
mod persistence;
use persistence::{load_layers, load_styles, save_layers, save_styles, StyleSettings};

use once_cell::sync::Lazy;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use rusttype::{point, Font, Scale};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform, FillRule};

slint::include_modules!();

// Load font from the crate's `assets` directory. The binary font file is not
// committed to the repository; place `DejaVuSans.ttf` inside the `assets`
// folder next to this crate's `Cargo.toml`.
static FONT_DATA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/DejaVuSans.ttf"
));
static FONT: Lazy<Font<'static>> = Lazy::new(|| Font::try_from_bytes(FONT_DATA).unwrap());

struct WorkspaceRenderData<'a> {
    points: &'a [Point],
    lines: &'a [(Point, Point)],
    polygons: &'a [Vec<Point>],
    polylines: &'a [Polyline],
    arcs: &'a [Arc],
    dimensions: &'a [LinearDimension],
    surfaces: &'a [Tin],
    alignments: &'a [Alignment],
}

#[derive(Default, Clone)]
struct Vec2 {
    x: f32,
    y: f32,
}

#[derive(Default, Clone)]
struct DragSelect {
    start: (f32, f32),
    end: (f32, f32),
    active: bool,
}

#[derive(Default, Clone)]
struct CursorFeedback {
    pos: (f32, f32),
    frame: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct SnapPrefs {
    snap_to_grid: bool,
    snap_to_entities: bool,
    snap_points: bool,
    snap_endpoints: bool,
    snap_midpoints: bool,
    snap_intersections: bool,
    snap_nearest: bool,
    snap_tolerance: f32,
}

impl Default for SnapPrefs {
    fn default() -> Self {
        Self {
            snap_to_grid: true,
            snap_to_entities: true,
            snap_points: true,
            snap_endpoints: true,
            snap_midpoints: true,
            snap_intersections: true,
            snap_nearest: true,
            snap_tolerance: 5.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    window_width: u32,
    window_height: u32,
    last_open_dir: Option<String>,
    snap: SnapPrefs,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_width: 800,
            window_height: 600,
            last_open_dir: None,
            snap: SnapPrefs::default(),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("survey_cad_truck_gui").join("config.json"))
}

fn load_config() -> Config {
    if let Some(path) = config_path() {
        if let Ok(data) = fs::read_to_string(path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Config::default()
        }
    } else {
        Config::default()
    }
}

fn save_config(cfg: &Config) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(cfg) {
            let _ = fs::write(path, json);
        }
    }
}


#[derive(Default, Clone, PartialEq)]
enum DrawingMode {
    #[default]
    None,
    Line {
        start: Option<Point>,
    },
    Polygon {
        vertices: Vec<Point>,
    },
    /// Center, start and end order
    ArcCenter {
        center: Option<Point>,
        radius: Option<f64>,
        start_angle: Option<f64>,
    },
    /// Three point arc
    ArcThreePoint {
        p1: Option<Point>,
        p2: Option<Point>,
    },
    /// Start, end, then radius via third click
    ArcStartEndRadius {
        start: Option<Point>,
        end: Option<Point>,
        radius: Option<f64>,
    },
    Dimension { start: Option<Point> },
}

#[derive(Clone)]
enum Command {
    RemovePoint { index: usize, point: Point },
    AddPoint { index: usize, point: Point },
    RemoveLine { index: usize, line: (Point, Point) },
    AddLine { index: usize, line: (Point, Point) },
    RemoveDimension { index: usize, dim: LinearDimension },
    AddDimension { index: usize, dim: LinearDimension },
    TinDeleteVertex { surface: usize, index: usize, point: Point3 },
    TinAddVertex { surface: usize, index: usize, point: Point3 },
}

struct CommandStack {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

#[derive(Default)]
struct MacroRecorder {
    file: Option<std::fs::File>,
}

#[derive(Default)]
struct MacroPlaying(bool);

fn record_macro(rec: &mut MacroRecorder, line: &str) {
    if let Some(file) = &mut rec.file {
        let _ = writeln!(file, "{line}");
    }
}

impl CommandStack {
    fn new() -> Self {
        Self { undo: Vec::new(), redo: Vec::new() }
    }

    fn push(&mut self, cmd: Command) {
        self.undo.push(cmd);
        self.redo.clear();
    }

    fn undo(&mut self, ctx: &Context) {
        if let Some(cmd) = self.undo.pop() {
            let inverse = apply_command(&cmd, ctx);
            self.redo.push(inverse);
        }
    }

    fn redo(&mut self, ctx: &Context) {
        if let Some(cmd) = self.redo.pop() {
            let inverse = apply_command(&cmd, ctx);
            self.undo.push(inverse);
        }
    }
}

struct Context<'a> {
    points: &'a Rc<RefCell<PointDatabase>>,
    point_styles: &'a Rc<RefCell<Vec<usize>>>,
    lines: &'a Rc<RefCell<Vec<(Point, Point)>>>,
    line_styles: &'a Rc<RefCell<Vec<usize>>>,
    dimensions: &'a Rc<RefCell<Vec<LinearDimension>>>,
    backend: &'a Rc<RefCell<TruckBackend>>,
}

fn apply_command(cmd: &Command, ctx: &Context) -> Command {
    match cmd {
        Command::RemovePoint { index, point } => {
            ctx.points.borrow_mut().remove(*index);
            ctx.point_styles.borrow_mut().remove(*index);
            ctx.backend.borrow_mut().remove_point(*index);
            Command::AddPoint { index: *index, point: *point }
        }
        Command::AddPoint { index, point } => {
            ctx.points.borrow_mut().insert(*index, *point);
            ctx.point_styles.borrow_mut().insert(*index, 0);
            ctx.backend
                .borrow_mut()
                .add_point(point.x, point.y, 0.0);
            Command::RemovePoint { index: *index, point: *point }
        }
        Command::RemoveLine { index, line } => {
            ctx.lines.borrow_mut().remove(*index);
            ctx.line_styles.borrow_mut().remove(*index);
            ctx.backend.borrow_mut().remove_line(*index);
            Command::AddLine { index: *index, line: *line }
        }
        Command::AddLine { index, line } => {
            ctx.lines.borrow_mut().insert(*index, *line);
            ctx.line_styles.borrow_mut().insert(*index, 0);
            ctx.backend.borrow_mut().add_line(
                [line.0.x, line.0.y, 0.0],
                [line.1.x, line.1.y, 0.0],
                [1.0, 1.0, 1.0, 1.0],
                1.0,
            );
            Command::RemoveLine { index: *index, line: *line }
        }
        Command::RemoveDimension { index, dim } => {
            ctx.backend.borrow_mut().remove_dimension(*index);
            ctx.dimensions.borrow_mut().remove(*index);
            Command::AddDimension {
                index: *index,
                dim: dim.clone(),
            }
        }
        Command::AddDimension { index, dim } => {
            ctx.dimensions.borrow_mut().insert(*index, dim.clone());
            ctx.backend
                .borrow_mut()
                .add_dimension(
                    [dim.start.x, dim.start.y, 0.0],
                    [dim.end.x, dim.end.y, 0.0],
                    [1.0, 1.0, 1.0, 1.0],
                    1.0,
                );
            Command::RemoveDimension {
                index: *index,
                dim: dim.clone(),
            }
        }
        Command::TinDeleteVertex { surface, index, point } => {
            ctx.backend.borrow_mut().delete_vertex(*surface, *index);
            Command::TinAddVertex {
                surface: *surface,
                index: *index,
                point: *point,
            }
        }
        Command::TinAddVertex { surface, index, point } => {
            ctx.backend
                .borrow_mut()
                .add_vertex(*surface, *point);
            Command::TinDeleteVertex {
                surface: *surface,
                index: *index,
                point: *point,
            }
        }
    }
}

struct RenderState<'a> {
    offset: &'a Rc<RefCell<Vec2>>,
    zoom: &'a Rc<RefCell<f32>>,
    selected: &'a Rc<RefCell<Vec<usize>>>,
    selected_lines: &'a Rc<RefCell<Vec<(Point, Point)>>>,
    selected_polygons: &'a Rc<RefCell<Vec<usize>>>,
    selected_polylines: &'a Rc<RefCell<Vec<usize>>>,
    selected_arcs: &'a Rc<RefCell<Vec<usize>>>,
    selected_dimensions: &'a Rc<RefCell<Vec<usize>>>,
    drag: &'a Rc<RefCell<DragSelect>>,
    cursor_feedback: &'a Rc<RefCell<Option<CursorFeedback>>>,
    snap_target: &'a Rc<RefCell<Option<Point>>>,
}

struct RenderStyles<'a> {
    point_styles: &'a [PointStyle],
    style_indices: &'a Rc<RefCell<Vec<usize>>>,
    line_styles: &'a [LineStyle],
    line_style_indices: &'a Rc<RefCell<Vec<usize>>>,
    polygon_styles: &'a [survey_cad::styles::PolygonStyle],
    polygon_style_indices: &'a Rc<RefCell<Vec<usize>>>,
    show_labels: bool,
    label_style: &'a LineLabelStyle,
    point_label_style: &'a PointLabelStyle,
    show_point_numbers: bool,
}

fn draw_text(pixmap: &mut Pixmap, text: &str, font: &Font, x: f32, y: f32, color: Color, size: f32) {
    let scale = Scale::uniform(size);
    let v_metrics = font.v_metrics(scale);
    let mut cursor = x;
    for ch in text.chars() {
        let glyph = font
            .glyph(ch)
            .scaled(scale)
            .positioned(point(cursor, y + v_metrics.ascent));
        if let Some(bb) = glyph.pixel_bounding_box() {
            glyph.draw(|gx, gy, gv| {
                let px = gx as i32 + bb.min.x;
                let py = gy as i32 + bb.min.y;
                if px >= 0
                    && py >= 0
                    && (px as u32) < pixmap.width()
                    && (py as u32) < pixmap.height()
                {
                    let idx = (py as u32 * pixmap.width() + px as u32) as usize;
                    pixmap.pixels_mut()[idx] = tiny_skia::ColorU8::from_rgba(
                        (color.red() * 255.0) as u8,
                        (color.green() * 255.0) as u8,
                        (color.blue() * 255.0) as u8,
                        (gv * 255.0) as u8,
                    )
                    .premultiply();
                }
            });
        }
        cursor += glyph.unpositioned().h_metrics().advance_width;
    }
}

fn screen_to_workspace(
    x: f32,
    y: f32,
    offset: &Rc<RefCell<Vec2>>,
    zoom: &Rc<RefCell<f32>>,
    width: f32,
    height: f32,
) -> Point {
    let origin_x = width / 2.0;
    let origin_y = height / 2.0;
    let z = *zoom.borrow();
    let off = offset.borrow();
    let wx = (x - origin_x) / z - off.x;
    let wy = -((y - origin_y) / z) - off.y;
    Point::new(wx as f64, wy as f64)
}

fn arc_from_three_points(p1: Point, p2: Point, p3: Point) -> Option<Arc> {
    let a = p2.x - p1.x;
    let b = p2.y - p1.y;
    let c = p3.x - p1.x;
    let d = p3.y - p1.y;
    let e = a * (p1.x + p2.x) + b * (p1.y + p2.y);
    let f = c * (p1.x + p3.x) + d * (p1.y + p3.y);
    let g = 2.0 * (a * (p3.y - p2.y) - b * (p3.x - p2.x));
    if g.abs() < f64::EPSILON {
        return None;
    }
    let cx = (d * e - b * f) / g;
    let cy = (a * f - c * e) / g;
    let center = Point::new(cx, cy);
    let r = ((center.x - p1.x).powi(2) + (center.y - p1.y).powi(2)).sqrt();
    let sa = (p1.y - cy).atan2(p1.x - cx);
    let mut ma = (p2.y - cy).atan2(p2.x - cx);
    let mut ea = (p3.y - cy).atan2(p3.x - cx);
    let cross = (p2.x - p1.x) * (p3.y - p2.y) - (p2.y - p1.y) * (p3.x - p2.x);
    if cross >= 0.0 {
        while ma < sa {
            ma += 2.0 * std::f64::consts::PI;
        }
        while ea < ma {
            ea += 2.0 * std::f64::consts::PI;
        }
    } else {
        while ma > sa {
            ma -= 2.0 * std::f64::consts::PI;
        }
        while ea > ma {
            ea -= 2.0 * std::f64::consts::PI;
        }
    }
    Some(Arc::new(center, r, sa, ea))
}

fn arc_from_start_end_radius(start: Point, end: Point, r: f64, orient: Point) -> Option<Arc> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let chord = (dx * dx + dy * dy).sqrt();
    if r <= chord / 2.0 {
        return None;
    }
    let mid = Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
    let len = (dx * dx + dy * dy).sqrt();
    if len.abs() < f64::EPSILON {
        return None;
    }
    let perp = (-dy / len, dx / len);
    let h = (r * r - (chord / 2.0).powi(2)).sqrt();
    let sign = ((orient.x - start.x) * dy - (orient.y - start.y) * dx).signum();
    let cx = mid.x + perp.0 * h * sign;
    let cy = mid.y + perp.1 * h * sign;
    let center = Point::new(cx, cy);
    let sa = (start.y - cy).atan2(start.x - cx);
    let ea = (end.y - cy).atan2(end.x - cx);
    Some(Arc::new(center, r, sa, ea))
}

fn spawn_point(
    points: &Rc<RefCell<PointDatabase>>,
    styles: &Rc<RefCell<Vec<usize>>>,
    backend: &Rc<RefCell<TruckBackend>>,
    p: Point,
) {
    points.borrow_mut().push(p);
    styles.borrow_mut().push(0);
    backend.borrow_mut().add_point(p.x, p.y, 0.0);
}

fn spawn_line(
    points: &Rc<RefCell<PointDatabase>>,
    lines: &Rc<RefCell<Vec<(Point, Point)>>>,
    point_styles: &Rc<RefCell<Vec<usize>>>,
    line_styles: &Rc<RefCell<Vec<usize>>>,
    backend: &Rc<RefCell<TruckBackend>>,
    a: Point,
    b: Point,
) {
    spawn_point(points, point_styles, backend, a);
    spawn_point(points, point_styles, backend, b);
    lines.borrow_mut().push((a, b));
    line_styles.borrow_mut().push(0);
    backend
        .borrow_mut()
        .add_line([a.x, a.y, 0.0], [b.x, b.y, 0.0], [1.0, 1.0, 1.0, 1.0], 1.0);
}

fn polyline_to_solid(pl: &Polyline, vector: Vector3) -> Option<Solid> {
    if pl.vertices.len() < 3 {
        return None;
    }
    let verts: Vec<_> = pl
        .vertices
        .iter()
        .map(|p| builder::vertex(Point3::new(p.x, p.y, 0.0)))
        .collect();
    let mut edges = Vec::new();
    for i in 0..verts.len() {
        edges.push(builder::line(&verts[i], &verts[(i + 1) % verts.len()]));
    }
    let wire = Wire::from_iter(edges);
    let face = builder::try_attach_plane(&[wire]).ok()?;
    let solid: Solid = builder::tsweep(&face, vector);
    Some(solid)
}

fn render_workspace(
    data: &WorkspaceRenderData,
    state: &RenderState,
    styles: &RenderStyles,
    drawing: &DrawingMode,
    grid: &GridSettings,
    width: u32,
    height: u32,
) -> Image {
    if width == 0 || height == 0 {
        // When the window has not been laid out yet slint reports a size of
        // zero. Returning an empty image avoids panicking until a real size is
        // available.
        return Image::default();
    }
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(Color::from_rgba8(32, 32, 32, 255));
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(
        grid.color[0],
        grid.color[1],
        grid.color[2],
        255,
    ));
    paint.anti_alias = true;
    let grid_stroke = Stroke {
        width: 1.0,
        ..Stroke::default()
    };
    let origin_x = width as f32 / 2.0;
    let origin_y = height as f32 / 2.0;
    let zoom_val = *state.zoom.borrow();
    let off = state.offset.borrow();
    let off_x = off.x;
    let off_y = off.y;
    drop(off);
    let tx = |x: f32| (x + off_x) * zoom_val + origin_x;
    let ty = |y: f32| origin_y - (y + off_y) * zoom_val;
    let step = grid.spacing * zoom_val;
    let mut x = origin_x;
    if grid.visible {
        while x < width as f32 {
            let mut pb = PathBuilder::new();
            pb.move_to(x, 0.0);
            pb.line_to(x, height as f32);
            if let Some(p) = pb.finish() {
                pixmap.stroke_path(&p, &paint, &grid_stroke, Transform::identity(), None);
            }
            x += step;
        }
        x = origin_x - step;
        while x >= 0.0 {
            let mut pb = PathBuilder::new();
            pb.move_to(x, 0.0);
            pb.line_to(x, height as f32);
            if let Some(p) = pb.finish() {
                pixmap.stroke_path(&p, &paint, &grid_stroke, Transform::identity(), None);
            }
            x -= step;
        }
        let mut y = origin_y;
        while y < height as f32 {
            let mut pb = PathBuilder::new();
            pb.move_to(0.0, y);
            pb.line_to(width as f32, y);
            if let Some(p) = pb.finish() {
                pixmap.stroke_path(&p, &paint, &grid_stroke, Transform::identity(), None);
            }
            y += step;
        }
        y = origin_y - step;
        while y >= 0.0 {
            let mut pb = PathBuilder::new();
            pb.move_to(0.0, y);
            pb.line_to(width as f32, y);
            if let Some(p) = pb.finish() {
                pixmap.stroke_path(&p, &paint, &grid_stroke, Transform::identity(), None);
            }
            y -= step;
        }
    }
    paint.set_color(Color::from_rgba8(90, 90, 90, 255));
    let mut pb = PathBuilder::new();
    pb.move_to(origin_x, 0.0);
    pb.line_to(origin_x, height as f32);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &grid_stroke, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, origin_y);
    pb.line_to(width as f32, origin_y);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &grid_stroke, Transform::identity(), None);
    }

    paint.set_color(Color::from_rgba8(255, 0, 0, 255));
    for (i, (s, e)) in data.lines.iter().enumerate() {
        let selected = state
            .selected_lines
            .borrow()
            .iter()
            .any(|(ls, le)| (*ls == *s && *le == *e) || (*ls == *e && *le == *s));
        let style_idx = styles
            .line_style_indices
            .borrow()
            .get(i)
            .copied()
            .unwrap_or(0);
        let mut style = styles
            .line_styles
            .get(style_idx)
            .copied()
            .unwrap_or_default();
        if selected {
            style.color = [255, 255, 0];
        }
        paint.set_color(Color::from_rgba8(
            style.color[0],
            style.color[1],
            style.color[2],
            255,
        ));
        let mut stroke = Stroke {
            width: style.weight.0,
            ..Stroke::default()
        };
        use tiny_skia::StrokeDash;
        match style.line_type {
            LineType::Dashed => {
                stroke.dash = StrokeDash::new(vec![10.0, 10.0], 0.0);
            }
            LineType::Dotted => {
                stroke.dash = StrokeDash::new(vec![2.0, 6.0], 0.0);
            }
            _ => {}
        }
        let mut pb = PathBuilder::new();
        pb.move_to(tx(s.x as f32), ty(s.y as f32));
        pb.line_to(tx(e.x as f32), ty(e.y as f32));
        if let Some(path) = pb.finish() {
            let stroke = Stroke {
                width: 1.0,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }

        if styles.show_labels {
            let line = Line::new(*s, *e);
            let ann = LineAnnotation::from_line(&line);
            let mut angle = 90.0 - ann.azimuth.to_degrees();
            if angle < 0.0 {
                angle += 360.0;
            }
            let text = format!("{:.2} m\n{}", ann.distance, format_dms(angle));
            let mid = line.midpoint();
            let dx = e.x - s.x;
            let dy = e.y - s.y;
            let len = (dx * dx + dy * dy).sqrt();
            let (ox, oy) = if len > 0.0 {
                let nx = dx / len;
                let ny = dy / len;
                match styles.label_style.position {
                    LineLabelPosition::Above => (-ny as f32, nx as f32),
                    LineLabelPosition::Below => (ny as f32, -nx as f32),
                    LineLabelPosition::Center => (0.0, 0.0),
                }
            } else {
                (0.0, 0.0)
            };
            draw_text(
                &mut pixmap,
                &text,
                &FONT,
                tx(mid.x as f32 + ox * 10.0),
                ty(mid.y as f32 + oy * 10.0),
                Color::from_rgba8(
                    styles.label_style.color[0],
                    styles.label_style.color[1],
                    styles.label_style.color[2],
                    255,
                ),
                styles.label_style.text_style.height as f32,
            );
        }
    }

    for (i, poly) in data.polygons.iter().enumerate() {
        if poly.len() < 2 {
            continue;
        }
        let mut pb = PathBuilder::new();
        let first = poly.first().unwrap();
        pb.move_to(tx(first.x as f32), ty(first.y as f32));
        for p in &poly[1..] {
            pb.line_to(tx(p.x as f32), ty(p.y as f32));
        }
        pb.close();
        if let Some(path) = pb.finish() {
            let style_idx = styles
                .polygon_style_indices
                .borrow()
                .get(i)
                .copied()
                .unwrap_or(0);
            let pstyle = styles
                .polygon_styles
                .get(style_idx)
                .copied()
                .unwrap_or_default();

            paint.set_color(Color::from_rgba8(
                pstyle.fill_color[0],
                pstyle.fill_color[1],
                pstyle.fill_color[2],
                255,
            ));
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

            if pstyle.hatch_pattern != HatchPattern::None {
                paint.set_color(Color::from_rgba8(
                    pstyle.hatch_color[0],
                    pstyle.hatch_color[1],
                    pstyle.hatch_color[2],
                    255,
                ));
                let stroke = Stroke { width: 1.0, ..Stroke::default() };
                {
                    let bb = path.bounds();
                    let step = 10.0;
                    if matches!(pstyle.hatch_pattern, HatchPattern::Cross | HatchPattern::Grid) {
                        let mut x = bb.left();
                        while x <= bb.right() {
                            let mut pb = PathBuilder::new();
                            pb.move_to(x, bb.top());
                            pb.line_to(x, bb.bottom());
                            if let Some(p) = pb.finish() {
                                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
                            }
                            x += step;
                        }
                        let mut y = bb.top();
                        while y <= bb.bottom() {
                            let mut pb = PathBuilder::new();
                            pb.move_to(bb.left(), y);
                            pb.line_to(bb.right(), y);
                            if let Some(p) = pb.finish() {
                                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
                            }
                            y += step;
                        }
                    }
                    if pstyle.hatch_pattern == HatchPattern::ForwardDiagonal {
                        let mut x = bb.left() - bb.height();
                        while x <= bb.right() {
                            let mut pb = PathBuilder::new();
                            pb.move_to(x, bb.bottom());
                            pb.line_to(x + bb.height(), bb.top());
                            if let Some(p) = pb.finish() {
                                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
                            }
                            x += step;
                        }
                    } else if pstyle.hatch_pattern == HatchPattern::BackwardDiagonal {
                        let mut x = bb.left();
                        while x <= bb.right() + bb.height() {
                            let mut pb = PathBuilder::new();
                            pb.move_to(x, bb.top());
                            pb.line_to(x - bb.height(), bb.bottom());
                            if let Some(p) = pb.finish() {
                                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
                            }
                            x += step;
                        }
                    }
                }
            }

            let selected = state.selected_polygons.borrow().contains(&i);
            if selected {
                paint.set_color(Color::from_rgba8(255, 255, 0, 255));
            } else {
                paint.set_color(Color::from_rgba8(255, 0, 0, 255));
            }
            let stroke = Stroke { width: 1.0, ..Stroke::default() };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    for (i, pl) in data.polylines.iter().enumerate() {
        if pl.vertices.len() < 2 {
            continue;
        }
        let mut pb = PathBuilder::new();
        let first = &pl.vertices[0];
        pb.move_to(tx(first.x as f32), ty(first.y as f32));
        for p in &pl.vertices[1..] {
            pb.line_to(tx(p.x as f32), ty(p.y as f32));
        }
        if let Some(path) = pb.finish() {
            let selected = state.selected_polylines.borrow().contains(&i);
            if selected {
                paint.set_color(Color::from_rgba8(255, 255, 0, 255));
            }
            let stroke = Stroke { width: 1.0, ..Stroke::default() };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            if selected {
                paint.set_color(Color::from_rgba8(255, 0, 0, 255));
            }
        }
    }

    for (i, arc) in data.arcs.iter().enumerate() {
        let steps = 32;
        let mut pb = PathBuilder::new();
        for i in 0..=steps {
            let t = arc.start_angle + (arc.end_angle - arc.start_angle) * (i as f64 / steps as f64);
            let x = arc.center.x + arc.radius * t.cos();
            let y = arc.center.y + arc.radius * t.sin();
            let px = tx(x as f32);
            let py = ty(y as f32);
            if i == 0 {
                pb.move_to(px, py);
            } else {
                pb.line_to(px, py);
            }
        }
        if let Some(path) = pb.finish() {
            let selected = state.selected_arcs.borrow().contains(&i);
            if selected {
                paint.set_color(Color::from_rgba8(255, 255, 0, 255));
            }
            let stroke = Stroke { width: 1.0, ..Stroke::default() };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            if selected {
                paint.set_color(Color::from_rgba8(255, 0, 0, 255));
            }
        }
    }

    paint.set_color(Color::from_rgba8(200, 200, 0, 255));
    for (i, dim) in data.dimensions.iter().enumerate() {
        let selected = state.selected_dimensions.borrow().contains(&i);
        if selected {
            paint.set_color(Color::from_rgba8(255, 255, 0, 255));
        } else {
            paint.set_color(Color::from_rgba8(200, 200, 0, 255));
        }
        let mut pb = PathBuilder::new();
        pb.move_to(tx(dim.start.x as f32), ty(dim.start.y as f32));
        pb.line_to(tx(dim.end.x as f32), ty(dim.end.y as f32));
        if let Some(path) = pb.finish() {
            let stroke = Stroke { width: 1.0, ..Stroke::default() };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
        let line = Line::new(dim.start, dim.end);
        let mid = line.midpoint();
        let text = if let Some(t) = &dim.text { t.clone() } else { format!("{:.2}", line.length()) };
        draw_text(
            &mut pixmap,
            &text,
            &FONT,
            tx(mid.x as f32),
            ty(mid.y as f32 - 10.0),
            Color::from_rgba8(255, 255, 255, 255),
            12.0,
        );
    }

    paint.set_color(Color::from_rgba8(128, 128, 128, 255));
    for tin in data.surfaces {
        for tri in &tin.triangles {
            let a = tin.vertices[tri[0]];
            let b = tin.vertices[tri[1]];
            let c = tin.vertices[tri[2]];
            let mut pb = PathBuilder::new();
            pb.move_to(tx(a.x as f32), ty(a.y as f32));
            pb.line_to(tx(b.x as f32), ty(b.y as f32));
            pb.line_to(tx(c.x as f32), ty(c.y as f32));
            pb.close();
            if let Some(path) = pb.finish() {
                let stroke = Stroke {
                    width: 1.0,
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    paint.set_color(Color::from_rgba8(0, 200, 255, 255));
    for al in data.alignments {
        for elem in &al.horizontal.elements {
            match elem {
                survey_cad::alignment::HorizontalElement::Tangent { start, end } => {
                    let mut pb = PathBuilder::new();
                    pb.move_to(tx(start.x as f32), ty(start.y as f32));
                    pb.line_to(tx(end.x as f32), ty(end.y as f32));
                    if let Some(path) = pb.finish() {
                        let stroke = Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
                survey_cad::alignment::HorizontalElement::Curve { arc } => {
                    let steps = 32;
                    let mut pb = PathBuilder::new();
                    for i in 0..=steps {
                        let t = arc.start_angle
                            + (arc.end_angle - arc.start_angle) * (i as f64 / steps as f64);
                        let x = arc.center.x + arc.radius * t.cos();
                        let y = arc.center.y + arc.radius * t.sin();
                        let px = tx(x as f32);
                        let py = ty(y as f32);
                        if i == 0 {
                            pb.move_to(px, py);
                        } else {
                            pb.line_to(px, py);
                        }
                    }
                    if let Some(path) = pb.finish() {
                        let stroke = Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
                survey_cad::alignment::HorizontalElement::Spiral { spiral } => {
                    let mut pb = PathBuilder::new();
                    let sp = spiral.start_point();
                    let ep = spiral.end_point();
                    pb.move_to(tx(sp.x as f32), ty(sp.y as f32));
                    pb.line_to(tx(ep.x as f32), ty(ep.y as f32));
                    if let Some(path) = pb.finish() {
                        let stroke = Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
            }
        }
    }

    for (idx, p) in data.points.iter().enumerate() {
        let style_idx = styles.style_indices.borrow().get(idx).copied().unwrap_or(0);
        let style = styles
            .point_styles
            .get(style_idx)
            .copied()
            .unwrap_or(PointStyle::new(PointSymbol::Circle, [0, 255, 0], 3.0));
        let selected = state.selected.borrow().contains(&idx);
        let color = if selected { [255, 255, 0] } else { style.color };
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], 255));
        match style.symbol {
            PointSymbol::Circle => {
                if let Some(c) =
                    PathBuilder::from_circle(tx(p.x as f32), ty(p.y as f32), style.size)
                {
                    pixmap.fill_path(
                        &c,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
            PointSymbol::Square => {
                let half = style.size;
                let mut pb = PathBuilder::new();
                pb.move_to(tx(p.x as f32 - half), ty(p.y as f32 - half));
                pb.line_to(tx(p.x as f32 + half), ty(p.y as f32 - half));
                pb.line_to(tx(p.x as f32 + half), ty(p.y as f32 + half));
                pb.line_to(tx(p.x as f32 - half), ty(p.y as f32 + half));
                pb.close();
                if let Some(path) = pb.finish() {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
            PointSymbol::Cross => {
                let half = style.size;
                let mut pb = PathBuilder::new();
                pb.move_to(tx(p.x as f32 - half), ty(p.y as f32 - half));
                pb.line_to(tx(p.x as f32 + half), ty(p.y as f32 + half));
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        },
                        Transform::identity(),
                        None,
                    );
                }
                let mut pb = PathBuilder::new();
                pb.move_to(tx(p.x as f32 - half), ty(p.y as f32 + half));
                pb.line_to(tx(p.x as f32 + half), ty(p.y as f32 - half));
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        },
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        if styles.show_point_numbers {
            draw_text(
                &mut pixmap,
                &(idx + 1).to_string(),
                &FONT,
                tx(p.x as f32 + styles.point_label_style.offset[0]),
                ty(p.y as f32 + styles.point_label_style.offset[1]),
                Color::from_rgba8(
                    styles.point_label_style.color[0],
                    styles.point_label_style.color[1],
                    styles.point_label_style.color[2],
                    255,
                ),
                styles.point_label_style.text_style.height as f32,
            );
        }
    }

    if let Some(cf) = state.cursor_feedback.borrow().as_ref() {
        let wp = screen_to_workspace(
            cf.pos.0,
            cf.pos.1,
            state.offset,
            state.zoom,
            width as f32,
            height as f32,
        );
        paint.set_color(Color::from_rgba8(255, 255, 0, 255));
        match drawing {
            DrawingMode::Line { start: Some(s) } => {
                let mut pb = PathBuilder::new();
                pb.move_to(tx(s.x as f32), ty(s.y as f32));
                pb.line_to(tx(wp.x as f32), ty(wp.y as f32));
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        },
                        Transform::identity(),
                        None,
                    );
                }
            }
            DrawingMode::Dimension { start: Some(s) } => {
                let mut pb = PathBuilder::new();
                pb.move_to(tx(s.x as f32), ty(s.y as f32));
                pb.line_to(tx(wp.x as f32), ty(wp.y as f32));
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke { width: 1.0, ..Stroke::default() },
                        Transform::identity(),
                        None,
                    );
                }
            }
            DrawingMode::Polygon { vertices } if !vertices.is_empty() => {
                let mut pb = PathBuilder::new();
                let first = vertices.first().unwrap();
                pb.move_to(tx(first.x as f32), ty(first.y as f32));
                for p in &vertices[1..] {
                    pb.line_to(tx(p.x as f32), ty(p.y as f32));
                }
                pb.line_to(tx(wp.x as f32), ty(wp.y as f32));
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        },
                        Transform::identity(),
                        None,
                    );
                }
            }
            DrawingMode::ArcCenter {
                center: Some(c),
                radius: Some(r),
                start_angle: Some(sa),
            } => {
                let ea = (wp.y - c.y).atan2(wp.x - c.x);
                let mut pb = PathBuilder::new();
                for i in 0..=32 {
                    let t = sa + (ea - sa) * (i as f64 / 32.0);
                    let x = c.x + r * t.cos();
                    let y = c.y + r * t.sin();
                    if i == 0 {
                        pb.move_to(tx(x as f32), ty(y as f32));
                    } else {
                        pb.line_to(tx(x as f32), ty(y as f32));
                    }
                }
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        },
                        Transform::identity(),
                        None,
                    );
                }
            }
            DrawingMode::ArcThreePoint {
                p1: Some(p1),
                p2: None,
            } => {
                let mut pb = PathBuilder::new();
                pb.move_to(tx(p1.x as f32), ty(p1.y as f32));
                pb.line_to(tx(wp.x as f32), ty(wp.y as f32));
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        },
                        Transform::identity(),
                        None,
                    );
                }
            }
            DrawingMode::ArcThreePoint {
                p1: Some(p1),
                p2: Some(p2),
            } => {
                if let Some(arc) = arc_from_three_points(*p1, *p2, wp) {
                    let mut pb = PathBuilder::new();
                    for i in 0..=32 {
                        let t =
                            arc.start_angle + (arc.end_angle - arc.start_angle) * (i as f64 / 32.0);
                        let x = arc.center.x + arc.radius * t.cos();
                        let y = arc.center.y + arc.radius * t.sin();
                        if i == 0 {
                            pb.move_to(tx(x as f32), ty(y as f32));
                        } else {
                            pb.line_to(tx(x as f32), ty(y as f32));
                        }
                    }
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(
                            &path,
                            &paint,
                            &Stroke {
                                width: 1.0,
                                ..Stroke::default()
                            },
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
            DrawingMode::ArcStartEndRadius {
                start: Some(s),
                end: None,
                ..
            } => {
                let mut pb = PathBuilder::new();
                pb.move_to(tx(s.x as f32), ty(s.y as f32));
                pb.line_to(tx(wp.x as f32), ty(wp.y as f32));
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke {
                            width: 1.0,
                            ..Stroke::default()
                        },
                        Transform::identity(),
                        None,
                    );
                }
            }
            DrawingMode::ArcStartEndRadius {
                start: Some(s),
                end: Some(e),
                radius: None,
            } => {
                let r = ((wp.x - s.x).powi(2) + (wp.y - s.y).powi(2)).sqrt();
                if let Some(arc) = arc_from_start_end_radius(*s, *e, r, wp) {
                    let mut pb = PathBuilder::new();
                    for i in 0..=32 {
                        let t =
                            arc.start_angle + (arc.end_angle - arc.start_angle) * (i as f64 / 32.0);
                        let x = arc.center.x + arc.radius * t.cos();
                        let y = arc.center.y + arc.radius * t.sin();
                        if i == 0 {
                            pb.move_to(tx(x as f32), ty(y as f32));
                        } else {
                            pb.line_to(tx(x as f32), ty(y as f32));
                        }
                    }
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(
                            &path,
                            &paint,
                            &Stroke {
                                width: 1.0,
                                ..Stroke::default()
                            },
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(sp) = state.snap_target.borrow().as_ref() {
        paint.set_color(Color::from_rgba8(255, 0, 0, 255));
        let r = 3.0;
        let (sx, sy) = (tx(sp.x as f32), ty(sp.y as f32));
        let mut pb = PathBuilder::new();
        pb.move_to(sx - r, sy);
        pb.line_to(sx + r, sy);
        pb.move_to(sx, sy - r);
        pb.line_to(sx, sy + r);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(
                &path,
                &paint,
                &Stroke { width: 1.0, ..Stroke::default() },
                Transform::identity(),
                None,
            );
        }
    }

    if state.drag.borrow().active {
        let ds = state.drag.borrow();
        let x1 = ds.start.0.min(ds.end.0);
        let y1 = ds.start.1.min(ds.end.1);
        let x2 = ds.start.0.max(ds.end.0);
        let y2 = ds.start.1.max(ds.end.1);
        paint.set_color(Color::from_rgba8(255, 255, 255, 128));
        let rect_stroke = Stroke {
            width: 1.0,
            ..Stroke::default()
        };
        let mut pb = PathBuilder::new();
        pb.move_to(x1, y1);
        pb.line_to(x2, y1);
        pb.line_to(x2, y2);
        pb.line_to(x1, y2);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &rect_stroke, Transform::identity(), None);
        }
    }

    if let Some(cf) = state.cursor_feedback.borrow().as_ref() {
        let t = (cf.frame % 30) as f32 / 30.0;
        paint.set_color(Color::from_rgba8(
            (255.0 * t) as u8,
            (255.0 * (1.0 - t)) as u8,
            0,
            255,
        ));
        let mut pb = PathBuilder::new();
        pb.move_to(cf.pos.0 - 5.0, cf.pos.1);
        pb.line_to(cf.pos.0 + 5.0, cf.pos.1);
        pb.move_to(cf.pos.0, cf.pos.1 - 5.0);
        pb.line_to(cf.pos.0, cf.pos.1 + 5.0);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(
                &path,
                &paint,
                &Stroke {
                    width: 1.0,
                    ..Stroke::default()
                },
                Transform::identity(),
                None,
            );
        }
    }
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        pixmap.data(),
        width,
        height,
    );
    Image::from_rgba8_premultiplied(buffer)
}

fn render_cross_section(section: &corridor::CrossSection, width: u32, height: u32) -> Image {
    if width == 0 || height == 0 {
        return Image::default();
    }
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(Color::from_rgba8(32, 32, 32, 255));
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(0, 255, 0, 255));
    paint.anti_alias = true;

    if section.points.len() >= 2 {
        let first = section.points.first().unwrap();
        let last = section.points.last().unwrap();
        let dx = last.x - first.x;
        let dy = last.y - first.y;
        let len = (dx * dx + dy * dy).sqrt();
        let dir = if len.abs() < f64::EPSILON {
            (1.0, 0.0)
        } else {
            (dx / len, dy / len)
        };
        let center = section.points[section.points.len() / 2];
        let mut pts = Vec::new();
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for p in &section.points {
            let off = ((p.x - center.x) * dir.0 + (p.y - center.y) * dir.1) as f32;
            let elev = (p.z - center.z) as f32;
            pts.push((off, elev));
            min_x = min_x.min(off);
            max_x = max_x.max(off);
            min_y = min_y.min(elev);
            max_y = max_y.max(elev);
        }
        if (max_x - min_x).abs() < f32::EPSILON {
            max_x += 1.0;
        }
        if (max_y - min_y).abs() < f32::EPSILON {
            max_y += 1.0;
        }
        let scale =
            ((width as f32 * 0.8) / (max_x - min_x)).min((height as f32 * 0.8) / (max_y - min_y));
        let ox = width as f32 / 2.0 - scale * (min_x + max_x) / 2.0;
        let oy = height as f32 / 2.0 + scale * (min_y + max_y) / 2.0;
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            let px = ox + *x * scale;
            let py = oy - *y * scale;
            if i == 0 {
                pb.move_to(px, py);
            } else {
                pb.line_to(px, py);
            }
        }
        if let Some(path) = pb.finish() {
            let stroke = Stroke {
                width: 2.0,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        pixmap.data(),
        width,
        height,
    );
    Image::from_rgba8_premultiplied(buffer)
}

struct SectionParams {
    dir: (f64, f64),
    center: ScPoint3,
    scale: f32,
    ox: f32,
    oy: f32,
}

fn calc_section_params(section: &corridor::CrossSection, width: f32, height: f32) -> Option<SectionParams> {
    if section.points.len() < 2 {
        return None;
    }
    let first = section.points.first().unwrap();
    let last = section.points.last().unwrap();
    let dx = last.x - first.x;
    let dy = last.y - first.y;
    let len = (dx * dx + dy * dy).sqrt();
    let dir = if len.abs() < f64::EPSILON { (1.0, 0.0) } else { (dx / len, dy / len) };
    let center = section.points[section.points.len() / 2];
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for p in &section.points {
        let off = ((p.x - center.x) * dir.0 + (p.y - center.y) * dir.1) as f32;
        let elev = (p.z - center.z) as f32;
        min_x = min_x.min(off);
        max_x = max_x.max(off);
        min_y = min_y.min(elev);
        max_y = max_y.max(elev);
    }
    if (max_x - min_x).abs() < f32::EPSILON {
        max_x += 1.0;
    }
    if (max_y - min_y).abs() < f32::EPSILON {
        max_y += 1.0;
    }
    let scale = ((width * 0.8) / (max_x - min_x)).min((height * 0.8) / (max_y - min_y));
    let ox = width / 2.0 - scale * (min_x + max_x) / 2.0;
    let oy = height / 2.0 + scale * (min_y + max_y) / 2.0;
    Some(SectionParams { dir, center, scale, ox, oy })
}

fn screen_to_world(section: &corridor::CrossSection, x: f32, y: f32, width: f32, height: f32) -> Option<ScPoint3> {
    let params = calc_section_params(section, width, height)?;
    let off = (x - params.ox) / params.scale;
    let elev = (params.oy - y) / params.scale;
    Some(ScPoint3::new(
        params.center.x + off as f64 * params.dir.0,
        params.center.y + off as f64 * params.dir.1,
        params.center.z + elev as f64,
    ))
}

fn nearest_point(section: &corridor::CrossSection, x: f32, y: f32, width: f32, height: f32) -> Option<usize> {
    let params = calc_section_params(section, width, height)?;
    let mut best = None;
    let mut best_dist = f32::MAX;
    for (i, p) in section.points.iter().enumerate() {
        let off = ((p.x - params.center.x) * params.dir.0 + (p.y - params.center.y) * params.dir.1) as f32;
        let elev = (p.z - params.center.z) as f32;
        let sx = params.ox + off * params.scale;
        let sy = params.oy - elev * params.scale;
        let dx = sx - x;
        let dy = sy - y;
        let dist = dx * dx + dy * dy;
        if dist < best_dist {
            best_dist = dist;
            best = Some(i);
        }
    }
    if best_dist.sqrt() <= 10.0 { best } else { None }
}

fn grade_at(profile: &VerticalAlignment, station: f64) -> Option<f64> {
    for elem in &profile.elements {
        match *elem {
            VerticalElement::Grade { start_station, end_station, start_elev, end_elev } => {
                if station >= start_station && station <= end_station {
                    if (end_station - start_station).abs() < f64::EPSILON {
                        return Some(0.0);
                    }
                    return Some((end_elev - start_elev) / (end_station - start_station));
                }
            }
            VerticalElement::Parabola { start_station, end_station, start_grade, end_grade, .. } => {
                if station >= start_station && station <= end_station {
                    let t = (station - start_station) / (end_station - start_station);
                    return Some(start_grade + (end_grade - start_grade) * t);
                }
            }
        }
    }
    None
}

fn read_line_csv(path: &str, dst_epsg: u32) -> std::io::Result<(Point, Point)> {
    let pts = survey_cad::io::read_points_csv(path, Some(4326), Some(dst_epsg))?;
    if pts.len() != 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected exactly two points",
        ));
    }
    Ok((pts[0], pts[1]))
}

fn read_points_list(path: &str, dst_epsg: u32) -> std::io::Result<Vec<Point>> {
    survey_cad::io::read_points_csv(path, Some(4326), Some(dst_epsg))
}

fn read_arc_csv(path: &str) -> std::io::Result<Arc> {
    let lines = survey_cad::io::read_lines(path)?;
    if lines.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "empty file",
        ));
    }
    let parts: Vec<&str> = lines[0].split(',').collect();
    if parts.len() != 5 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected cx,cy,radius,start,end",
        ));
    }
    let cx: f64 = parts[0]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let cy: f64 = parts[1]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let r: f64 = parts[2]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let sa: f64 = parts[3]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let ea: f64 = parts[4]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Arc::new(Point::new(cx, cy), r, sa, ea))
}

fn refresh_workspace(
    app: &MainWindow,
    render_image: &dyn Fn() -> Image,
    backend_render: &Rc<RefCell<TruckBackend>>,
) {
    if app.get_workspace_mode() == 0 {
        app.set_workspace_image(render_image());
    } else {
        let image = backend_render.borrow_mut().render();
        app.set_workspace_texture(image);
    }
    app.window().request_redraw();
}

#[allow(clippy::too_many_arguments)]
fn show_inspector_for_point(
    idx: usize,
    app: &MainWindow,
    layer_names: &Rc<RefCell<Vec<String>>>,
    style_names: &[SharedString],
    layers: &Rc<RefCell<Vec<usize>>>,
    styles: &Rc<RefCell<Vec<usize>>>,
    metadata: &Rc<RefCell<Vec<String>>>,
    inspector: &Rc<RefCell<Option<slint::Weak<EntityInspector>>>>,
    render_image: Rc<dyn Fn() -> Image>,
    backend: &Rc<RefCell<TruckBackend>>,
) {
    while layers.borrow().len() <= idx { layers.borrow_mut().push(0); }
    while styles.borrow().len() <= idx { styles.borrow_mut().push(0); }
    while metadata.borrow().len() <= idx { metadata.borrow_mut().push(String::new()); }

    let layer_model = Rc::new(VecModel::from(
        layer_names
            .borrow()
            .iter()
            .cloned()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ));
    let style_model = Rc::new(VecModel::from(style_names.to_vec()));

    let dlg = if let Some(w) = inspector.borrow().as_ref().and_then(|w| w.upgrade()) {
        w
    } else {
        let d = EntityInspector::new().unwrap();
        *inspector.borrow_mut() = Some(d.as_weak());
        d
    };

    dlg.set_layers_model(layer_model.into());
    dlg.set_styles_model(style_model.into());
    dlg.set_entity_type(SharedString::from("Point"));
    dlg.set_layer_index(layers.borrow()[idx] as i32);
    dlg.set_style_index(styles.borrow()[idx] as i32);
    dlg.set_metadata(SharedString::from(metadata.borrow()[idx].clone()));

    {
        let layers = layers.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        dlg.on_layer_changed(move |val| {
            if let Some(l) = layers.borrow_mut().get_mut(idx) { *l = val as usize; }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let styles_ref = styles.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        dlg.on_style_changed(move |val| {
            if let Some(s) = styles_ref.borrow_mut().get_mut(idx) { *s = val as usize; }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let meta_ref = metadata.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        dlg.on_metadata_changed(move |text| {
            if let Some(m) = meta_ref.borrow_mut().get_mut(idx) { *m = text.to_string(); }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    dlg.show().unwrap();
}

#[allow(clippy::too_many_arguments)]
fn show_inspector_for_polygon(
    idx: usize,
    app: &MainWindow,
    layer_names: &Rc<RefCell<Vec<String>>>,
    hatch_names: &[SharedString],
    layers: &Rc<RefCell<Vec<usize>>>,
    hatches: &Rc<RefCell<Vec<usize>>>,
    inspector: &Rc<RefCell<Option<slint::Weak<EntityInspector>>>>,
    render_image: Rc<dyn Fn() -> Image>,
    backend: &Rc<RefCell<TruckBackend>>,
) {
    while layers.borrow().len() <= idx { layers.borrow_mut().push(0); }
    while hatches.borrow().len() <= idx { hatches.borrow_mut().push(0); }

    let layer_model = Rc::new(VecModel::from(
        layer_names
            .borrow()
            .iter()
            .cloned()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ));
    let hatch_model = Rc::new(VecModel::from(hatch_names.to_vec()));

    let dlg = if let Some(w) = inspector.borrow().as_ref().and_then(|w| w.upgrade()) {
        w
    } else {
        let d = EntityInspector::new().unwrap();
        *inspector.borrow_mut() = Some(d.as_weak());
        d
    };

    dlg.set_layers_model(layer_model.into());
    dlg.set_styles_model(Rc::new(VecModel::from(Vec::<SharedString>::new())).into());
    dlg.set_hatch_model(hatch_model.into());
    dlg.set_entity_type(SharedString::from("Polygon"));
    dlg.set_layer_index(layers.borrow()[idx] as i32);
    dlg.set_hatch_index(hatches.borrow()[idx] as i32);
    dlg.set_metadata(SharedString::from(""));

    {
        let layers = layers.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        dlg.on_layer_changed(move |val| {
            if let Some(l) = layers.borrow_mut().get_mut(idx) { *l = val as usize; }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    {
        let h_ref = hatches.clone();
        let app_weak = app.as_weak();
        let backend = backend.clone();
        let render_image = render_image.clone();
        dlg.on_hatch_changed(move |val| {
            if let Some(h) = h_ref.borrow_mut().get_mut(idx) { *h = val as usize; }
            if let Some(a) = app_weak.upgrade() {
                refresh_workspace(&a, &*render_image, &backend);
            }
        });
    }

    dlg.show().unwrap();
}

fn main() -> Result<(), slint::PlatformError> {
    let cfg = load_config();
    let config = Rc::new(RefCell::new(cfg));
    let backend = Rc::new(RefCell::new(TruckBackend::new(
        config.borrow().window_width,
        config.borrow().window_height,
    )));
    // Always populate the font database with the system fonts first so that the
    // embedded font can complement, rather than replace, them. This ensures
    // that built-in controls can resolve their default fonts while we still
    // provide our bundled DejaVuSans.
    sharedfontdb::FONT_DB.with_borrow_mut(|db| db.make_mut().load_system_fonts());
    sharedfontdb::register_font_from_memory(FONT_DATA).expect("failed to register embedded font");
    let app = MainWindow::new()?;
    app.window().set_size(PhysicalSize::new(
        config.borrow().window_width,
        config.borrow().window_height,
    ));

    let snap_prefs = Rc::new(RefCell::new(config.borrow().snap.clone()));
    {
        let p = snap_prefs.borrow();
        app.set_snap_to_grid(p.snap_to_grid);
        app.set_snap_to_entities(p.snap_to_entities);
        app.set_snap_endpoints(p.snap_endpoints);
        app.set_snap_points(p.snap_points);
        app.set_snap_intersections(p.snap_intersections);
        app.set_snap_midpoints(p.snap_midpoints);
        app.set_snap_nearest(p.snap_nearest);
        app.set_snap_tolerance(p.snap_tolerance);
    }
    let last_folder = Rc::new(RefCell::new(config.borrow().last_open_dir.clone()));
    let window_size = Rc::new(RefCell::new(app.window().size()));

    // example data so the 2D workspace has something to draw
    let example_line = Line::new(Point::new(0.0, 0.0), Point::new(50.0, 50.0));
    let point_db = Rc::new(RefCell::new(PointDatabase::new()));
    let lines = Rc::new(RefCell::new(vec![(example_line.start, example_line.end)]));
    let polygons = Rc::new(RefCell::new(Vec::<Vec<Point>>::new()));
    let polylines = Rc::new(RefCell::new(Vec::<Polyline>::new()));
    let arcs = Rc::new(RefCell::new(Vec::<Arc>::new()));
    let dimensions = Rc::new(RefCell::new(Vec::<LinearDimension>::new()));
    let surfaces = Rc::new(RefCell::new(Vec::<Tin>::new()));
    let surface_units = Rc::new(RefCell::new(Vec::<String>::new()));
    let surface_styles = Rc::new(RefCell::new(Vec::<String>::new()));
    let surface_descriptions = Rc::new(RefCell::new(Vec::<String>::new()));
    let alignments = Rc::new(RefCell::new(Vec::<Alignment>::new()));
    let superelevation = Rc::new(RefCell::new(Vec::<SuperelevationPoint>::new()));
    let layers = Rc::new(RefCell::new(ScLayerManager::new()));
    let layer_names = Rc::new(RefCell::new(Vec::<String>::new()));
    if let Some(saved) = load_layers(Path::new("layers.json")) {
        *layers.borrow_mut() = saved;
        layer_names
            .borrow_mut()
            .extend(layers.borrow().iter().map(|l| l.name.clone()));
    } else {
        let mut mgr = layers.borrow_mut();
        let default = Layer::new("DEFAULT");
        mgr.add_layer(default);
        layer_names.borrow_mut().push("DEFAULT".to_string());
    }

    let zoom = Rc::new(RefCell::new(1.0_f32));
    let offset = Rc::new(RefCell::new(Vec2::default()));
    let grid_settings = Rc::new(RefCell::new(GridSettings::default()));
    let workspace_crs = Rc::new(RefCell::new(4326u32));
    let pan_2d_flag = Rc::new(RefCell::new(false));
    let last_pos_2d = Rc::new(RefCell::new((0.0_f64, 0.0_f64)));
    let rotate_flag = Rc::new(RefCell::new(false));
    let pan_flag = Rc::new(RefCell::new(false));
    let last_pos = Rc::new(RefCell::new((0.0_f64, 0.0_f64)));
    let selected_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_lines = Rc::new(RefCell::new(Vec::<(Point, Point)>::new()));
    let selected_polygons = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_polylines = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_arcs = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_dimensions = Rc::new(RefCell::new(Vec::<usize>::new()));
    let drag_select = Rc::new(RefCell::new(DragSelect::default()));
    let cursor_feedback = Rc::new(RefCell::new(None));
    let snap_target = Rc::new(RefCell::new(None::<Point>));
    let drawing_mode = Rc::new(RefCell::new(DrawingMode::None));
    let last_click = Rc::new(RefCell::new(None));
    let selected_surface = Rc::new(RefCell::new(None::<usize>));
    let click_pos_3d = Rc::new(RefCell::new(None::<(f64, f64)>));
    let active_handle = Rc::new(RefCell::new(None::<usize>));
    let current_line: Rc<RefCell<Option<Polyline>>> = Rc::new(RefCell::new(None));
    let point_style_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let point_layers = Rc::new(RefCell::new(Vec::<usize>::new()));
    let line_layers = Rc::new(RefCell::new(Vec::<usize>::new()));
    let polygon_layers = Rc::new(RefCell::new(Vec::<usize>::new()));
    let point_metadata = Rc::new(RefCell::new(Vec::<String>::new()));
    let line_metadata = Rc::new(RefCell::new(Vec::<String>::new()));
    let inspector_window: Rc<RefCell<Option<slint::Weak<EntityInspector>>>> = Rc::new(RefCell::new(None));
    let style_settings = load_styles(Path::new("styles.json")).unwrap_or_else(|| StyleSettings {
        point_styles: survey_cad::styles::default_point_styles(),
        line_styles: survey_cad::styles::default_line_styles(),
        polygon_styles: survey_cad::styles::default_polygon_styles(),
    });
    let point_styles = style_settings.point_styles.clone();
    let point_style_names: Vec<SharedString> = point_styles
        .iter()
        .map(|(n, _)| SharedString::from(n.clone()))
        .collect();
    let point_style_values: Vec<PointStyle> = point_styles.iter().map(|(_, s)| *s).collect();

    let line_styles = style_settings.line_styles.clone();
    let line_style_indices = Rc::new(RefCell::new(vec![0; line_styles.len()]));

    let polygon_styles = style_settings.polygon_styles.clone();
    let polygon_style_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let polygon_style_names: Vec<SharedString> = polygon_styles
        .iter()
        .map(|(n, _)| SharedString::from(n.clone()))
        .collect();
    let polygon_style_values: Vec<survey_cad::styles::PolygonStyle> =
        polygon_styles.iter().map(|(_, s)| *s).collect();
    let command_stack = Rc::new(RefCell::new(CommandStack::new()));
    let macro_recorder = Rc::new(RefCell::new(MacroRecorder::default()));
    let macro_playing = Rc::new(RefCell::new(MacroPlaying::default()));
    let line_type_names = Rc::new(VecModel::from(vec![
        SharedString::from("Solid"),
        SharedString::from("Dashed"),
        SharedString::from("Dotted"),
    ]));
    let line_label_styles = survey_cad::styles::default_line_label_styles();
    let point_label_styles = survey_cad::styles::default_point_label_styles();
    let point_label_style = Rc::new(RefCell::new(point_label_styles[0].1.clone()));
    let line_style_names: Rc<Vec<SharedString>> = Rc::new(
        line_styles
            .iter()
            .map(|(n, _)| SharedString::from(n.clone()))
            .collect(),
    );
    let open_line_style_managers: Rc<RefCell<Vec<slint::Weak<LineStyleManager>>>> =
        Rc::new(RefCell::new(Vec::new()));
    let refresh_line_style_dialogs: Rc<dyn Fn()> = {
        let dialogs = open_line_style_managers.clone();
        let style_names = line_style_names.clone();
        let lines = lines.clone();
        let indices = line_style_indices.clone();
        Rc::new(move || {
            let needed = style_names.len();
            {
                let mut idx = indices.borrow_mut();
                if idx.len() < needed {
                    idx.resize(needed, 0);
                }
            }
            let style_model = Rc::new(VecModel::from((*style_names).clone()));
            let current_indices = indices.borrow().clone();
            let current_lines = lines.borrow().clone();
            let rows = current_indices
                .iter()
                .enumerate()
                .map(|(i, s_idx)| {
                    if let Some((s, e)) = current_lines.get(i) {
                        LineRow {
                            start: SharedString::from(format!(
                                "{sx:.2},{sy:.2}",
                                sx = s.x,
                                sy = s.y
                            )),
                            end: SharedString::from(format!(
                                "{ex:.2},{ey:.2}",
                                ex = e.x,
                                ey = e.y
                            )),
                            style_index: *s_idx as i32,
                        }
                    } else {
                        LineRow {
                            start: SharedString::from(""),
                            end: SharedString::from(""),
                            style_index: *s_idx as i32,
                        }
                    }
                })
                .collect::<Vec<_>>();
            let line_model = Rc::new(VecModel::from(rows));
            dialogs.borrow_mut().retain(|d| {
                if let Some(dlg) = d.upgrade() {
                    dlg.set_styles_model(style_model.clone().into());
                    dlg.set_lines_model(line_model.clone().into());
                    true
                } else {
                    false
                }
            });
        })
    };
    let line_style_values: Vec<LineStyle> = line_styles.iter().map(|(_, s)| *s).collect();

    let render_image = {
        let app_weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let surface_units_ref = surface_units.clone();
        let surface_styles_ref = surface_styles.clone();
        let surface_descriptions_ref = surface_descriptions.clone();
        let alignments = alignments.clone();
        let zoom = zoom.clone();
        let offset = offset.clone();
        let selected_indices = selected_indices.clone();
        let drag_select = drag_select.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let dimensions = dimensions.clone();
        let selected_dimensions = selected_dimensions.clone();
        let style_indices = point_style_indices.clone();
        let point_styles = point_style_values.clone();
        let line_styles_vals = line_style_values.clone();
        let line_style_indices = line_style_indices.clone();
        let polygon_style_indices = polygon_style_indices.clone();
        let cursor_feedback = cursor_feedback.clone();
        let snap_target = snap_target.clone();
        let drawing_mode = drawing_mode.clone();
        let label_style = line_label_styles[0].1.clone();
        let point_label_style = point_label_style.clone();
        let grid_settings_ref = grid_settings.clone();
        move || {
            let size = app_weak.upgrade().map(|a| a.window().size()).unwrap();
            let show_numbers = app_weak
                .upgrade()
                .map(|a| a.get_show_point_numbers())
                .unwrap_or(true);
            render_workspace(
                &WorkspaceRenderData {
                    points: &point_db.borrow(),
                    lines: &lines.borrow(),
                    polygons: &polygons.borrow(),
                    polylines: &polylines.borrow(),
                    arcs: &arcs.borrow(),
                    dimensions: &dimensions.borrow(),
                    surfaces: &surfaces.borrow(),
                    alignments: &alignments.borrow(),
                },
                &RenderState {
                    offset: &offset,
                    zoom: &zoom,
                    selected: &selected_indices,
                    selected_lines: &selected_lines,
                    selected_polygons: &selected_polygons,
                    selected_polylines: &selected_polylines,
                    selected_arcs: &selected_arcs,
                    selected_dimensions: &selected_dimensions,
                    drag: &drag_select,
                    cursor_feedback: &cursor_feedback,
                    snap_target: &snap_target,
                },
                &RenderStyles {
                    point_styles: &point_styles,
                    style_indices: &style_indices,
                    line_styles: &line_styles_vals,
                    line_style_indices: &line_style_indices,
                    polygon_styles: &polygon_style_values,
                    polygon_style_indices: &polygon_style_indices,
                    show_labels: true,
                    label_style: &label_style,
                    point_label_style: &point_label_style.borrow(),
                    show_point_numbers: show_numbers,
                },
                &drawing_mode.borrow(),
                &grid_settings_ref.borrow(),
                size.width,
                size.height,
            )
        }
    };

    // basic CRS list as before
    let crs_entries = list_known_crs();
    let crs_entries_rc = Rc::new(crs_entries);
    let crs_model = Rc::new(VecModel::from(
        crs_entries_rc
            .iter()
            .map(|e| SharedString::from(format!("{} - {}", e.code, e.name)))
            .collect::<Vec<_>>(),
    ));
    let default_idx = crs_entries_rc
        .iter()
        .position(|e| e.code == format!("EPSG:{}", *workspace_crs.borrow()))
        .unwrap_or(0);
    app.set_crs_list(crs_model.into());
    app.set_crs_index(default_idx as i32);
    app.set_workspace_mode(0); // start with 2D mode
    app.set_show_point_numbers(true);

    // show length of example line in the status bar so Line import is used
    app.set_status(SharedString::from(format!(
        "Example line length: {:.1}",
        example_line.length()
    )));

    // prepare initial 2D workspace image and schedule continuous redraws
    app.set_workspace_image(render_image());
    app.window().request_redraw();

    {
        use slint::{Timer, TimerMode};
        use std::rc::Rc;

        let weak = app.as_weak();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let command_stack = command_stack.clone();
        let timer = Rc::new(Timer::default());
        let timer_handle = timer.clone();

        // Perform an initial refresh immediately
        if let Some(app) = weak.upgrade() {
            refresh_workspace(&app, &render_image, &backend_render);
        }

        timer.start(
            TimerMode::Repeated,
            core::time::Duration::from_millis(16),
            move || {
                if let Some(app) = weak.upgrade() {
                    refresh_workspace(&app, &render_image, &backend_render);
                } else {
                    timer_handle.stop();
                }
            },
        );

        use slint::CloseRequestResponse;
        let timer_handle = timer.clone();
        let cfg = config.clone();
        let snap = snap_prefs.clone();
        let win = window_size.clone();
        let last_dir = last_folder.clone();
        app.window().on_close_requested(move || {
            timer_handle.stop();
            {
                let mut c = cfg.borrow_mut();
                c.window_width = win.borrow().width;
                c.window_height = win.borrow().height;
                c.last_open_dir = last_dir.borrow().clone();
                c.snap = snap.borrow().clone();
                save_config(&c);
            }
            CloseRequestResponse::HideWindow
        });
    }

    {
        let recorder = macro_recorder.clone();
        app.on_macro_record(move || {
            if recorder.borrow().file.is_some() {
                recorder.borrow_mut().file = None;
            } else if let Some(path) = rfd::FileDialog::new().add_filter("Text", &["txt"]).save_file() {
                if let Ok(f) = std::fs::File::create(&path) {
                    recorder.borrow_mut().file = Some(f);
                }
            }
        });
    }

    {
        let recorder = macro_recorder.clone();
        let playing = macro_playing.clone();
        let point_db = point_db.clone();
        let point_styles = point_style_indices.clone();
        let lines_ref = lines.clone();
        let line_styles = line_style_indices.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        let weak = app.as_weak();
        app.on_macro_play(move || {
            if let Some(path) = rfd::FileDialog::new().add_filter("Text", &["txt"]).pick_file() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    playing.borrow_mut().0 = true;
                    for line in content.lines() {
                        let parts = shell_words::split(line).unwrap_or_default();
                        if parts.is_empty() {
                            continue;
                        }
                        match parts[0].as_str() {
                            "point" if parts.len() >= 3 => {
                                if let (Ok(x), Ok(y)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
                                    spawn_point(&point_db, &point_styles, &backend_render, Point::new(x, y));
                                }
                            }
                            "line" if parts.len() >= 5 => {
                                if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                                    parts[1].parse::<f64>(),
                                    parts[2].parse::<f64>(),
                                    parts[3].parse::<f64>(),
                                    parts[4].parse::<f64>(),
                                ) {
                                    spawn_line(
                                        &point_db,
                                        &lines_ref,
                                        &point_styles,
                                        &line_styles,
                                        &backend_render,
                                        Point::new(x1, y1),
                                        Point::new(x2, y2),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    playing.borrow_mut().0 = false;
                    recorder.borrow_mut().file = None;
                    if let Some(app) = weak.upgrade() {
                        if app.get_workspace_mode() == 0 {
                            app.set_workspace_image(render_image());
                            app.window().request_redraw();
                        }
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let surfaces_ref = surfaces.clone();
        app.on_run_python_script(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Python", &["py"])
                .pick_file()
            {
                match std::fs::read_to_string(&path) {
                    Ok(code) => {
                        let result = Python::with_gil(|py| {
                            let module = PyModule::new_bound(py, "survey_cad_python")?;
                            survey_cad_python::init(py, &module)?;

                            let pts: Vec<Py<survey_cad_python::Point>> = point_db
                                .borrow()
                                .iter()
                                .map(|p| Py::new(py, survey_cad_python::Point::new(p.x, p.y)))
                                .collect::<PyResult<_>>()?;

                            let lines_py: Vec<(Py<survey_cad_python::Point>, Py<survey_cad_python::Point>)> =
                                lines_ref
                                    .borrow()
                                    .iter()
                                    .map(|(a, b)| {
                                        Ok((
                                            Py::new(py, survey_cad_python::Point::new(a.x, a.y))?,
                                            Py::new(py, survey_cad_python::Point::new(b.x, b.y))?,
                                        ))
                                    })
                                    .collect::<PyResult<_>>()?;

                            let surfs: Vec<Py<PyAny>> = surfaces_ref
                                .borrow()
                                .iter()
                                .map(|s| {
                                    let dict = PyDict::new_bound(py);
                                    let verts: Vec<(f64, f64, f64)> = s
                                        .vertices
                                        .iter()
                                        .map(|v| (v.x, v.y, v.z))
                                        .collect();
                                    let tris: Vec<(usize, usize, usize)> = s
                                        .triangles
                                        .iter()
                                        .map(|t| (t[0], t[1], t[2]))
                                        .collect();
                                    dict.set_item("vertices", verts)?;
                                    dict.set_item("triangles", tris)?;
                                    Ok(dict.into())
                                })
                                .collect::<PyResult<_>>()?;

                            let globals = PyDict::new_bound(py);
                            globals.set_item("survey_cad_python", module)?;
                            globals.set_item("points", pts)?;
                            globals.set_item("lines", lines_py)?;
                            globals.set_item("surfaces", surfs)?;

                            py.run_bound(&code, Some(&globals), None)
                        });

                        match result {
                            Ok(_) => {
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from("Python script finished"));
                                }
                            }
                            Err(e) => {
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from(format!(
                                        "Python error: {e}"
                                    )));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to read: {e}")));
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let surfaces = surfaces.clone();
        let selected_indices = selected_indices.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_create_surface_from_selection(move || {
            let sc_pts: Vec<ScPoint3> = selected_indices
                .borrow()
                .iter()
                .filter_map(|&i| point_db.borrow().get(i).copied())
                .map(|p| ScPoint3::new(p.x, p.y, 0.0))
                .collect();
            if sc_pts.len() >= 3 {
                let tin = survey_cad::dtm::Tin::from_points(sc_pts.clone());
                let verts: Vec<Point3> = tin
                    .vertices
                    .iter()
                    .map(|p| Point3::new(p.x, p.y, p.z))
                    .collect();
                backend_render
                    .borrow_mut()
                    .add_surface(&verts, &tin.triangles);
                surfaces.borrow_mut().push(tin);
                selected_indices.borrow_mut().clear();
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from(format!(
                        "Total surfaces: {}",
                        surfaces.borrow().len()
                    )));
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                    } else {
                        let image = backend_render.borrow_mut().render();
                        app.set_workspace_texture(image);
                    }
                    app.window().request_redraw();
                }
            } else if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Need at least 3 points"));
            }
        });
    }

    {
        let drawing_mode = drawing_mode.clone();
        app.on_draw_line_mode(move || {
            *drawing_mode.borrow_mut() = DrawingMode::Line { start: None };
        });
    }

    {
        let drawing_mode = drawing_mode.clone();
        app.on_draw_polygon_mode(move || {
            *drawing_mode.borrow_mut() = DrawingMode::Polygon {
                vertices: Vec::new(),
            };
        });
    }

    {
        let drawing_mode = drawing_mode.clone();
        app.on_draw_arc_mode(move || {
            let dlg = ArcModeDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let dm = drawing_mode.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_center_start_end(move || {
                    *dm.borrow_mut() = DrawingMode::ArcCenter {
                        center: None,
                        radius: None,
                        start_angle: None,
                    };
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let dm = drawing_mode.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_three_point(move || {
                    *dm.borrow_mut() = DrawingMode::ArcThreePoint { p1: None, p2: None };
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let dm = drawing_mode.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_start_end_radius(move || {
                    *dm.borrow_mut() = DrawingMode::ArcStartEndRadius {
                        start: None,
                        end: None,
                        radius: None,
                    };
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let drawing_mode = drawing_mode.clone();
        app.on_draw_dimension_mode(move || {
            *drawing_mode.borrow_mut() = DrawingMode::Dimension { start: None };
        });
    }

    {
        let weak = app.as_weak();
        let layer_names = layer_names.clone();
        let point_style_names = point_style_names.clone();
        let point_layers = point_layers.clone();
        let point_style_indices = point_style_indices.clone();
        let point_metadata = point_metadata.clone();
        let inspector_ref = inspector_window.clone();
        let selected_indices = selected_indices.clone();
        let selected_polygons = selected_polygons.clone();
        let polygon_style_names = polygon_style_names.clone();
        let polygon_layers = polygon_layers.clone();
        let polygon_style_indices = polygon_style_indices.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_inspector(move || {
            if let Some(app) = weak.upgrade() {
                if let Some(idx) = selected_indices.borrow().first().copied() {
                    show_inspector_for_point(
                        idx,
                        &app,
                        &layer_names,
                        &point_style_names,
                        &point_layers,
                        &point_style_indices,
                        &point_metadata,
                        &inspector_ref,
                        Rc::new(render_image.clone()),
                        &backend_render,
                    );
                } else if let Some(idx) = selected_polygons.borrow().first().copied() {
                    show_inspector_for_polygon(
                        idx,
                        &app,
                        &layer_names,
                        &polygon_style_names,
                        &polygon_layers,
                        &polygon_style_indices,
                        &inspector_ref,
                        Rc::new(render_image.clone()),
                        &backend_render,
                    );
                }
            }
        });
    }

    let weak = app.as_weak();

    {
        let command_stack = command_stack.clone();
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        let lines = lines.clone();
        let line_style_indices = line_style_indices.clone();
        let backend = backend.clone();
        let render_image = render_image.clone();
        let dimensions = dimensions.clone();
        let weak = app.as_weak();
        app.on_undo(move || {
            let ctx = Context {
                points: &point_db,
                point_styles: &point_style_indices,
                lines: &lines,
                line_styles: &line_style_indices,
                dimensions: &dimensions,
                backend: &backend,
            };
            command_stack.borrow_mut().undo(&ctx);
            if let Some(app) = weak.upgrade() {
                refresh_workspace(&app, &render_image, &backend);
            }
        });
    }

    {
        let command_stack = command_stack.clone();
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        let lines = lines.clone();
        let line_style_indices = line_style_indices.clone();
        let backend = backend.clone();
        let render_image = render_image.clone();
        let dimensions = dimensions.clone();
        let weak = app.as_weak();
        app.on_redo(move || {
            let ctx = Context {
                points: &point_db,
                point_styles: &point_style_indices,
                lines: &lines,
                line_styles: &line_style_indices,
                dimensions: &dimensions,
                backend: &backend,
            };
            command_stack.borrow_mut().redo(&ctx);
            if let Some(app) = weak.upgrade() {
                refresh_workspace(&app, &render_image, &backend);
            }
        });
    }

    {
        let weak = app.as_weak();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        app.on_zoom_in(move || {
            *zoom.borrow_mut() *= 1.2;
            if let Some(app) = weak.upgrade() {
                app.set_zoom_level(*zoom.borrow());
                if app.get_workspace_mode() == 0 {
                    app.set_workspace_image(render_image());
                    app.window().request_redraw();
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        app.on_zoom_out(move || {
            *zoom.borrow_mut() /= 1.2;
            if let Some(app) = weak.upgrade() {
                app.set_zoom_level(*zoom.borrow());
                if app.get_workspace_mode() == 0 {
                    app.set_workspace_image(render_image());
                    app.window().request_redraw();
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let zoom = zoom.clone();
        app.on_view_changed(move |mode| {
            if let Some(app) = weak.upgrade() {
                app.set_workspace_mode(mode);
                app.set_zoom_level(*zoom.borrow());
                if mode == 0 {
                    app.set_workspace_image(render_image());
                } else {
                    let image = backend_render.borrow_mut().render();
                    app.set_workspace_texture(image);
                }
                app.window().request_redraw();
            }
        });
    }

    {
        let weak = app.as_weak();
        let render_image = render_image.clone();
        app.on_point_numbers_changed(move |_| {
            if let Some(app) = weak.upgrade() {
                if app.get_workspace_mode() == 0 {
                    app.set_workspace_image(render_image());
                    app.window().request_redraw();
                }
            }
        });
    }

    {
        let workspace_crs = workspace_crs.clone();
        let crs_entries_rc = crs_entries_rc.clone();
        app.on_crs_changed(move |idx| {
            if let Some(entry) = crs_entries_rc.get(idx as usize) {
                if let Some(code) = entry.code.split(':').nth(1) {
                    if let Ok(epsg) = code.parse::<u32>() {
                        *workspace_crs.borrow_mut() = epsg;
                    }
                }
            }
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_grid_changed(move |val| {
            prefs.borrow_mut().snap_to_grid = val;
            cfg.borrow_mut().snap.snap_to_grid = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_objects_changed(move |val| {
            prefs.borrow_mut().snap_to_entities = val;
            cfg.borrow_mut().snap.snap_to_entities = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_endpoints_changed(move |val| {
            prefs.borrow_mut().snap_endpoints = val;
            cfg.borrow_mut().snap.snap_endpoints = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_intersections_changed(move |val| {
            prefs.borrow_mut().snap_intersections = val;
            cfg.borrow_mut().snap.snap_intersections = val;
            save_config(&cfg.borrow());
        });
    }



    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_points_changed(move |val| {
            prefs.borrow_mut().snap_points = val;
            cfg.borrow_mut().snap.snap_points = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let drawing_mode = drawing_mode.clone();
        let polygons = polygons.clone();
        let render_image = render_image.clone();
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let line_style_indices = line_style_indices.clone();
        let point_style_indices = point_style_indices.clone();
        let backend = backend.clone();
        let command_stack = command_stack.clone();
        let dimensions = dimensions.clone();
        app.on_key_pressed(move |key| {
            if key.as_str() == "\u{001a}" {
                let ctx = Context {
                    points: &point_db,
                    point_styles: &point_style_indices,
                    lines: &lines,
                    line_styles: &line_style_indices,
                    dimensions: &dimensions,
                    backend: &backend,
                };
                command_stack.borrow_mut().undo(&ctx);
                if let Some(app) = weak.upgrade() {
                    refresh_workspace(&app, &render_image, &backend);
                }
            } else if key.as_str() == "\u{0019}" {
                let ctx = Context {
                    points: &point_db,
                    point_styles: &point_style_indices,
                    lines: &lines,
                    line_styles: &line_style_indices,
                    dimensions: &dimensions,
                    backend: &backend,
                };
                command_stack.borrow_mut().redo(&ctx);
                if let Some(app) = weak.upgrade() {
                    refresh_workspace(&app, &render_image, &backend);
                }
            } else if key.as_str() == "\u{001b}" {
                *drawing_mode.borrow_mut() = DrawingMode::None;
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            } else if key.as_str() == "\u{000a}" {
                let mut dm = drawing_mode.borrow_mut();
                if let DrawingMode::Polygon { vertices } = &mut *dm {
                    if vertices.len() > 2 {
                        vertices.push(vertices[0]);
                        polygons.borrow_mut().push(vertices.clone());
                        *dm = DrawingMode::None;
                    }
                }
                drop(dm);
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            }
        });
    }

    // camera interaction callbacks for the 3D workspace
    {
        let rotate_flag = rotate_flag.clone();
        let last_pos = last_pos.clone();
        let click_pos = click_pos_3d.clone();
        let backend = backend.clone();
        let weak = app.as_weak();
        let active_handle_ref = active_handle.clone();
        app.on_workspace_left_pressed(move |x, y| {
            *last_pos.borrow_mut() = (x as f64, y as f64);
            if let Some(HitObject::Handle(i)) = backend.borrow().hit_test(x as f64, y as f64) {
                *rotate_flag.borrow_mut() = false;
                *active_handle_ref.borrow_mut() = Some(i);
                backend.borrow_mut().highlight_handle(i, true);
                if let Some(app) = weak.upgrade() {
                    let image = backend.borrow_mut().render();
                    app.set_workspace_texture(image);
                    app.window().request_redraw();
                }
            } else {
                *rotate_flag.borrow_mut() = true;
                *active_handle_ref.borrow_mut() = None;
                *click_pos.borrow_mut() = Some((x as f64, y as f64));
            }
        });
    }

    {
        let pan_flag = pan_flag.clone();
        let last_pos = last_pos.clone();
        app.on_workspace_right_pressed(move |x, y| {
            *pan_flag.borrow_mut() = true;
            *last_pos.borrow_mut() = (x as f64, y as f64);
        });
    }

    {
        let pan_2d_flag = pan_2d_flag.clone();
        let drag_select = drag_select.clone();
        let last_pos_2d = last_pos_2d.clone();
        let drawing_mode = drawing_mode.clone();
        let offset = offset.clone();
        let zoom = zoom.clone();
        let lines_ref = lines.clone();
        let polygons_ref = polygons.clone();
        let polylines = polylines.clone();
        let point_db = point_db.clone();
        let arcs_ref = arcs.clone();
        let dimensions = dimensions.clone();
        let last_click = last_click.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let command_stack = command_stack.clone();
        let weak = app.as_weak();
        let macro_playing = macro_playing.clone();
        let macro_recorder = macro_recorder.clone();
        let snap_target = snap_target.clone();
        app.on_workspace_pointer_pressed(move |x, y, ev| {
            if *drawing_mode.borrow() != DrawingMode::None {
                if ev.button == PointerEventButton::Left {
                    if let Some(app) = weak.upgrade() {
                        let size = app.window().size();
                        let mut p = screen_to_workspace(
                            x,
                            y,
                            &offset,
                            &zoom,
                            size.width as f32,
                            size.height as f32,
                        );
                        let zoom_factor = *zoom.borrow();
                        if app.get_snap_to_entities() {
                            let scene = snap::Scene {
                                points: &point_db.borrow(),
                                lines: &lines_ref.borrow(),
                                polygons: &polygons_ref.borrow(),
                                polylines: &polylines.borrow(),
                                arcs: &arcs_ref.borrow(),
                            };
                            let opts = snap::SnapOptions {
                                snap_points: app.get_snap_points(),
                                snap_endpoints: app.get_snap_endpoints(),
                                snap_midpoints: app.get_snap_midpoints(),
                                snap_intersections: app.get_snap_intersections(),
                                snap_nearest: app.get_snap_nearest(),
                            };
                            if let Some(sp) = snap::resolve_snap(
                                p,
                                &scene,
                                app.get_snap_tolerance() as f64 / (zoom_factor as f64),
                                opts,
                            ) {
                                *snap_target.borrow_mut() = Some(sp);
                                p = sp;
                            } else {
                                *snap_target.borrow_mut() = None;
                            }
                        } else {
                            *snap_target.borrow_mut() = None;
                        }
                        if app.get_snap_to_grid() {
                            p.x = p.x.round();
                            p.y = p.y.round();
                        }
                        let mut mode = drawing_mode.borrow_mut();
                        match &mut *mode {
                            DrawingMode::Line { start } => {
                                if start.is_none() {
                                    *start = Some(p);
                                } else if let Some(s) = start.take() {
                                lines_ref.borrow_mut().push((s, p));
                                backend_render
                                    .borrow_mut()
                                    .add_line(
                                        [s.x, s.y, 0.0],
                                        [p.x, p.y, 0.0],
                                        [1.0, 1.0, 1.0, 1.0],
                                        1.0,
                                    );
                                if !macro_playing.borrow().0 {
                                    let sx = s.x;
                                    let sy = s.y;
                                    let px = p.x;
                                    let py = p.y;
                                    record_macro(
                                        &mut macro_recorder.borrow_mut(),
                                        &format!("line {sx} {sy} {px} {py}"),
                                    );
                                }
                                *mode = DrawingMode::None;
                                } else {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(
                                            "No start point, line cancelled",
                                        ));
                                    }
                                    *mode = DrawingMode::None;
                                    return;
                                }
                            }
                            DrawingMode::Dimension { start } => {
                                if start.is_none() {
                                    *start = Some(p);
                                } else if let Some(s) = start.take() {
                                    dimensions.borrow_mut().push(LinearDimension::new(s, p));
                                    backend_render
                                        .borrow_mut()
                                        .add_dimension(
                                            [s.x, s.y, 0.0],
                                            [p.x, p.y, 0.0],
                                            [1.0, 1.0, 1.0, 1.0],
                                            1.0,
                                        );
                                    command_stack.borrow_mut().push(Command::RemoveDimension {
                                        index: dimensions.borrow().len() - 1,
                                        dim: LinearDimension::new(s, p),
                                    });
                                    *mode = DrawingMode::None;
                                }
                            }
                            DrawingMode::Polygon { vertices } => {
                                let now = Instant::now();
                                let double = last_click
                                    .borrow()
                                    .map(|t| now.duration_since(t).as_millis() < 500)
                                    .unwrap_or(false);
                                *last_click.borrow_mut() = Some(now);
                                vertices.push(p);
                                if double && vertices.len() > 2 {
                                    vertices.push(vertices[0]);
                                    polygons_ref.borrow_mut().push(vertices.clone());
                                    *mode = DrawingMode::None;
                                }
                            }
                            DrawingMode::ArcCenter {
                                center,
                                radius,
                                start_angle,
                            } => {
                                if center.is_none() {
                                    *center = Some(p);
                                } else if radius.is_none() {
                                    if let Some(c) = *center {
                                        *radius = Some(
                                            ((p.x - c.x).powi(2) + (p.y - c.y).powi(2)).sqrt(),
                                        );
                                    }
                                } else if start_angle.is_none() {
                                    if let Some(c) = *center {
                                        *start_angle = Some((p.y - c.y).atan2(p.x - c.x));
                                    }
                                } else if let (Some(c), Some(r), Some(sa)) =
                                    (*center, *radius, *start_angle)
                                {
                                    let ea = (p.y - c.y).atan2(p.x - c.x);
                                    let arc = Arc::new(c, r, sa, ea);
                                    arcs_ref.borrow_mut().push(arc);
                                    *mode = DrawingMode::None;
                                }
                            }
                            DrawingMode::ArcThreePoint { p1, p2 } => {
                                if p1.is_none() {
                                    *p1 = Some(p);
                                } else if p2.is_none() {
                                    *p2 = Some(p);
                                } else if let (Some(a), Some(b)) = (*p1, *p2) {
                                    if let Some(arc) = arc_from_three_points(a, b, p) {
                                        arcs_ref.borrow_mut().push(arc);
                                    }
                                    *mode = DrawingMode::None;
                                }
                            }
                            DrawingMode::ArcStartEndRadius { start, end, radius } => {
                                if start.is_none() {
                                    *start = Some(p);
                                } else if end.is_none() {
                                    *end = Some(p);
                                } else if radius.is_none() {
                                    if let (Some(s), Some(e)) = (*start, *end) {
                                        let r = ((p.x - s.x).powi(2) + (p.y - s.y).powi(2)).sqrt();
                                        if let Some(arc) = arc_from_start_end_radius(s, e, r, p) {
                                            arcs_ref.borrow_mut().push(arc);
                                        }
                                        *mode = DrawingMode::None;
                                    }
                                }
                            }
                            _ => {}
                        }
                        drop(mode);
                        if app.get_workspace_mode() == 0 {
                            app.set_workspace_image(render_image());
                            app.window().request_redraw();
                        }
                        if let Some(app) = weak.upgrade() {
                            refresh_workspace(&app, &render_image, &backend_render);
                        }
                    }
                }
            } else if ev.button == PointerEventButton::Middle {
                *pan_2d_flag.borrow_mut() = true;
            } else if ev.button == PointerEventButton::Left {
                let mut ds = drag_select.borrow_mut();
                ds.start = (x, y);
                ds.end = ds.start;
                ds.active = true;
                *last_pos_2d.borrow_mut() = (x as f64, y as f64);
            }
        });
    }

    {
        let rotate_flag = rotate_flag.clone();
        let pan_flag = pan_flag.clone();
        let pan_2d_flag = pan_2d_flag.clone();
        let drag_select = drag_select.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let polygons_ref = polygons.clone();
        let polylines = polylines.clone();
        let arcs_ref = arcs.clone();
        let offset = offset.clone();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        let cursor_feedback = cursor_feedback.clone();
        let weak = app.as_weak();
        let click_pos = click_pos_3d.clone();
        let selected_surface_ref = selected_surface.clone();
        let backend_inner = backend.clone();
        let dimensions = dimensions.clone();
        let selected_dimensions = selected_dimensions.clone();
        let active_handle_ref = active_handle.clone();
        app.on_workspace_pointer_released(move || {
            *rotate_flag.borrow_mut() = false;
            *pan_flag.borrow_mut() = false;
            *pan_2d_flag.borrow_mut() = false;
            *cursor_feedback.borrow_mut() = None;

            if let Some(i) = active_handle_ref.borrow_mut().take() {
                backend_inner.borrow_mut().highlight_handle(i, false);
                if let Some(app) = weak.upgrade() {
                    let image = backend_inner.borrow_mut().render();
                    app.set_workspace_texture(image);
                    app.window().request_redraw();
                }
            }

            let mut update = false;
            {
                let mut ds = drag_select.borrow_mut();
                if ds.active {
                    if let Some(app) = weak.upgrade() {
                        let size = app.window().size();
                        let p1 = screen_to_workspace(
                            ds.start.0,
                            ds.start.1,
                            &offset,
                            &zoom,
                            size.width as f32,
                            size.height as f32,
                        );
                        let p2 = screen_to_workspace(
                            ds.end.0,
                            ds.end.1,
                            &offset,
                            &zoom,
                            size.width as f32,
                            size.height as f32,
                        );
                        let min_x = p1.x.min(p2.x);
                        let max_x = p1.x.max(p2.x);
                        let min_y = p1.y.min(p2.y);
                        let max_y = p1.y.max(p2.y);
                        selected_indices.borrow_mut().clear();
                        selected_lines.borrow_mut().clear();
                        selected_polygons.borrow_mut().clear();
                        selected_polylines.borrow_mut().clear();
                        selected_arcs.borrow_mut().clear();
                        selected_dimensions.borrow_mut().clear();
                        for (i, pt) in point_db.borrow().iter().enumerate() {
                            if pt.x >= min_x && pt.x <= max_x && pt.y >= min_y && pt.y <= max_y {
                                selected_indices.borrow_mut().push(i);
                            }
                        }
                        for (s, e) in lines_ref.borrow().iter() {
                            if (s.x >= min_x && s.x <= max_x && s.y >= min_y && s.y <= max_y)
                                && (e.x >= min_x && e.x <= max_x && e.y >= min_y && e.y <= max_y)
                            {
                                selected_lines.borrow_mut().push((*s, *e));
                            }
                        }
                        for (i, poly) in polygons_ref.borrow().iter().enumerate() {
                            if poly.iter().all(|p| p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y) {
                                selected_polygons.borrow_mut().push(i);
                            }
                        }
                        for (i, pl) in polylines.borrow().iter().enumerate() {
                            if pl.vertices.iter().all(|p| p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y) {
                                selected_polylines.borrow_mut().push(i);
                            }
                        }
                        for (i, arc) in arcs_ref.borrow().iter().enumerate() {
                            let min_ax = arc.center.x - arc.radius;
                            let max_ax = arc.center.x + arc.radius;
                            let min_ay = arc.center.y - arc.radius;
                            let max_ay = arc.center.y + arc.radius;
                            if min_ax >= min_x && max_ax <= max_x && min_ay >= min_y && max_ay <= max_y {
                                selected_arcs.borrow_mut().push(i);
                            }
                        }
                        for (i, dim) in dimensions.borrow().iter().enumerate() {
                            let min_dx = dim.start.x.min(dim.end.x);
                            let max_dx = dim.start.x.max(dim.end.x);
                            let min_dy = dim.start.y.min(dim.end.y);
                            let max_dy = dim.start.y.max(dim.end.y);
                            if min_dx >= min_x && max_dx <= max_x && min_dy >= min_y && max_dy <= max_y {
                                selected_dimensions.borrow_mut().push(i);
                            }
                        }
                        ds.active = false;
                        update = true;
                    }
                }
            }

            if update {
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            } else if let Some(start) = click_pos.borrow_mut().take() {
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 1 {
                        if let Some(hit) = backend_inner.borrow().hit_test(start.0, start.1) {
                            match hit {
                                HitObject::Surface(i) => {
                                    if let Some(prev) = selected_surface_ref.replace(Some(i)) {
                                        backend_inner.borrow_mut().highlight_surface(prev, false);
                                    }
                                    backend_inner.borrow_mut().highlight_surface(i, true);
                                    backend_inner.borrow_mut().show_surface_handles(i);
                                }
                                _ => {
                                    if let Some(prev) = selected_surface_ref.take() {
                                        backend_inner.borrow_mut().highlight_surface(prev, false);
                                        backend_inner.borrow_mut().hide_handles();
                                    }
                                }
                            }
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
                    }
                }
            }
        });
    }

    {
        let backend = backend.clone();
        let rotate_flag = rotate_flag.clone();
        let pan_flag = pan_flag.clone();
        let last_pos = last_pos.clone();
        let click_pos = click_pos_3d.clone();
        let pan_2d_flag = pan_2d_flag.clone();
        let last_pos_2d = last_pos_2d.clone();
        let offset = offset.clone();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        let drag_select = drag_select.clone();
        let cursor_feedback = cursor_feedback.clone();
        let drawing_mode = drawing_mode.clone();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let polygons_ref = polygons.clone();
        let polylines = polylines.clone();
        let arcs_ref = arcs.clone();
        let current_line = current_line.clone();
        let snap_target = snap_target.clone();
        let weak = app.as_weak();
        let active_handle_ref = active_handle.clone();
        let backend_move = backend.clone();
        app.on_workspace_mouse_moved(move |x, y| {
            let mut last = last_pos.borrow_mut();
            let dx = x as f64 - last.0;
            let dy = y as f64 - last.1;
            *last = (x as f64, y as f64);
            if let Some(i) = *active_handle_ref.borrow() {
                if let Some(pos) = backend_move.borrow().handle_position(i) {
                    let new_p = backend_move
                        .borrow()
                        .screen_to_plane(x as f64, y as f64, pos.z);
                    backend_move.borrow_mut().move_handle(i, new_p);
                    if let Some(app) = weak.upgrade() {
                        let image = backend_move.borrow_mut().render();
                        app.set_workspace_texture(image);
                        app.window().request_redraw();
                    }
                    return;
                }
            }
            if *rotate_flag.borrow() {
                if let Some(start) = *click_pos.borrow() {
                    if (x as f64 - start.0).abs() > 3.0 || (y as f64 - start.1).abs() > 3.0 {
                        *click_pos.borrow_mut() = None;
                    }
                }
                backend.borrow_mut().rotate(dx, dy);
                if let Some(app) = weak.upgrade() {
                    app.window().request_redraw();
                }
            } else if *pan_flag.borrow() {
                backend.borrow_mut().pan(dx, dy);
                if let Some(app) = weak.upgrade() {
                    app.window().request_redraw();
                }
            }

            let mut last2 = last_pos_2d.borrow_mut();
            let dx2 = x - last2.0 as f32;
            let dy2 = y - last2.1 as f32;
            *last2 = (x as f64, y as f64);
            if *pan_2d_flag.borrow() {
                let z = *zoom.borrow();
                offset.borrow_mut().x += dx2 / z;
                offset.borrow_mut().y += -dy2 / z;
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            }

            if drag_select.borrow().active {
                drag_select.borrow_mut().end = (x, y);
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            }

            if matches!(*drawing_mode.borrow(), DrawingMode::Line { .. }) {
                if let Some(app) = weak.upgrade() {
                    let size = app.window().size();
                    let mut p = screen_to_workspace(
                        x,
                        y,
                        &offset,
                        &zoom,
                        size.width as f32,
                        size.height as f32,
                    );
                    let zoom_factor = *zoom.borrow();
                    if app.get_snap_to_entities() {
                        let scene = snap::Scene {
                            points: &point_db.borrow(),
                            lines: &lines_ref.borrow(),
                            polygons: &polygons_ref.borrow(),
                            polylines: &polylines.borrow(),
                            arcs: &arcs_ref.borrow(),
                        };
                        let opts = snap::SnapOptions {
                            snap_points: app.get_snap_points(),
                            snap_endpoints: app.get_snap_endpoints(),
                            snap_midpoints: app.get_snap_midpoints(),
                            snap_intersections: app.get_snap_intersections(),
                            snap_nearest: app.get_snap_nearest(),
                        };
                        if let Some(sp) = snap::resolve_snap(
                            p,
                            &scene,
                            app.get_snap_tolerance() as f64 / (zoom_factor as f64),
                            opts,
                        ) {
                            *snap_target.borrow_mut() = Some(sp);
                            p = sp;
                        } else {
                            *snap_target.borrow_mut() = None;
                        }
                    } else {
                        *snap_target.borrow_mut() = None;
                    }
                    if app.get_snap_to_grid() {
                        p.x = p.x.round();
                        p.y = p.y.round();
                    }
                    if let Some(cl) = current_line.borrow_mut().as_mut() {
                        if let Some(last) = cl.vertices.last_mut() {
                            *last = p;
                        }
                    }
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            }

            *cursor_feedback.borrow_mut() = Some(CursorFeedback {
                pos: (x, y),
                frame: 0,
            });
        });
    }

    {
        let backend = backend.clone();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        let weak = app.as_weak();
        app.on_workspace_scrolled(move |_dx, dy| {
            if let Some(app) = weak.upgrade() {
                if app.get_workspace_mode() == 1 {
                    backend.borrow_mut().zoom(dy as f64);
                    app.window().request_redraw();
                } else {
                    let new_zoom = {
                        let mut z = zoom.borrow_mut();
                        if dy < 0.0 {
                            *z *= 1.1;
                        } else {
                            *z /= 1.1;
                        }
                        *z = (*z).clamp(0.1, 100.0);
                        *z
                    };
                    app.set_zoom_level(new_zoom);
                    app.set_workspace_image(render_image());
                    app.window().request_redraw();
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let surface_units_np = surface_units.clone();
        let surface_styles_np = surface_styles.clone();
        let surface_descriptions_np = surface_descriptions.clone();
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
        let dimensions = dimensions.clone();
        let selected_dimensions = selected_dimensions.clone();
        let workspace_crs = workspace_crs.clone();
        let crs_entries_rc = crs_entries_rc.clone();
        app.on_new_project(move || {
            point_db.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            dimensions.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            surface_units_np.borrow_mut().clear();
            surface_styles_np.borrow_mut().clear();
            surface_descriptions_np.borrow_mut().clear();
            alignments.borrow_mut().clear();
            selected_indices.borrow_mut().clear();
            selected_lines.borrow_mut().clear();
            selected_polygons.borrow_mut().clear();
            selected_polylines.borrow_mut().clear();
            selected_arcs.borrow_mut().clear();
            selected_dimensions.borrow_mut().clear();
            backend_render.borrow_mut().clear();
            refresh_line_style_dialogs();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("New project created"));
                *workspace_crs.borrow_mut() = 4326;
                if let Some(idx) = crs_entries_rc
                    .iter()
                    .position(|e| e.code == "EPSG:4326")
                {
                    app.set_crs_index(idx as i32);
                }
                refresh_workspace(&app, &render_image, &backend_render);
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let surface_units_ref = surface_units.clone();
        let surface_styles_ref = surface_styles.clone();
        let surface_descriptions_ref = surface_descriptions.clone();
        let layers_ref = layers.clone();
        let layer_names_ref = layer_names.clone();
        let line_style_indices = line_style_indices.clone();
        let point_style_indices = point_style_indices.clone();
        let polygon_style_indices = polygon_style_indices.clone();
        let grid_settings = grid_settings.clone();
        let point_label_style = point_label_style.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let dimensions = dimensions.clone();
        let last_dir = last_folder.clone();
        let config_rc = config.clone();
        let workspace_crs = workspace_crs.clone();
        let crs_entries_rc = crs_entries_rc.clone();
        let alignments = alignments.clone();
        app.on_open_project(move || {
            let mut dialog = rfd::FileDialog::new();
            if let Some(dir) = last_dir.borrow().as_ref() {
                dialog = dialog.set_directory(dir);
            }
            if let Some(path) = dialog.add_filter("Project", &["json"]).pick_file() {
                *last_dir.borrow_mut() = path.parent().map(|p| p.to_string_lossy().to_string());
                config_rc.borrow_mut().last_open_dir = last_dir.borrow().clone();
                save_config(&config_rc.borrow());
                if let Some(p) = path.to_str() {
                    match read_project_json(p) {
                        Ok(proj) => {
                            *workspace_crs.borrow_mut() = proj.crs_epsg;
                            if let Some(idx) = crs_entries_rc
                                .iter()
                                .position(|e| e.code == format!("EPSG:{}", proj.crs_epsg))
                            {
                                if let Some(app) = weak.upgrade() {
                                    app.set_crs_index(idx as i32);
                                }
                            }
                            point_db.borrow_mut().clear();
                            point_db.borrow_mut().extend_from_slice(&proj.points);
                            lines.borrow_mut().clear();
                            lines.borrow_mut().extend(proj.lines.iter().map(|l| (l.start, l.end)));
                            polygons.borrow_mut().clear();
                            polygons.borrow_mut().extend(proj.polygons.clone());
                            polylines.borrow_mut().clear();
                            polylines.borrow_mut().extend(proj.polylines.clone());
                            arcs.borrow_mut().clear();
                            arcs.borrow_mut().extend(proj.arcs.clone());
                            dimensions.borrow_mut().clear();
                            dimensions.borrow_mut().extend(proj.dimensions.clone());
                            surfaces.borrow_mut().clear();
                            surfaces.borrow_mut().extend(proj.surfaces.clone());
                            surface_units_ref.borrow_mut().clear();
                            surface_units_ref.borrow_mut().extend(proj.surface_units.clone());
                            surface_styles_ref.borrow_mut().clear();
                            surface_styles_ref.borrow_mut().extend(proj.surface_styles.clone());
                            surface_descriptions_ref.borrow_mut().clear();
                            surface_descriptions_ref
                                .borrow_mut()
                                .extend(proj.surface_descriptions.clone());
                            alignments.borrow_mut().clear();
                            alignments.borrow_mut().extend(proj.alignments.clone());
                            *line_style_indices.borrow_mut() = proj.line_style_indices.clone();
                            *point_style_indices.borrow_mut() = proj.point_style_indices.clone();
                            *polygon_style_indices.borrow_mut() = proj.polygon_style_indices.clone();
                            *grid_settings.borrow_mut() = proj.grid.clone();
                            {
                                let mut pls = point_label_style.borrow_mut();
                                pls.text_style.font = proj.point_label_font.clone();
                                pls.offset = proj.point_label_offset;
                            }

                            let mut mgr = ScLayerManager::new();
                            layer_names_ref.borrow_mut().clear();
                            for l in proj.layers {
                                layer_names_ref.borrow_mut().push(l.name.clone());
                                mgr.add_layer(l);
                            }
                            *layers_ref.borrow_mut() = mgr;

                            backend_render.borrow_mut().clear();
                            for pt in point_db.borrow().iter() {
                                backend_render.borrow_mut().add_point(pt.x, pt.y, 0.0);
                            }
                            for tin in surfaces.borrow().iter() {
                                let verts: Vec<Point3> = tin
                                    .vertices
                                    .iter()
                                    .map(|p| Point3::new(p.x, p.y, p.z))
                                    .collect();
                                backend_render.borrow_mut().add_surface(&verts, &tin.triangles);
                            }
                            for dim in dimensions.borrow().iter() {
                                backend_render.borrow_mut().add_dimension(
                                    [dim.start.x, dim.start.y, 0.0],
                                    [dim.end.x, dim.end.y, 0.0],
                                    [1.0, 1.0, 1.0, 1.0],
                                    1.0,
                                );
                            }

                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Project loaded"));
                                refresh_workspace(&app, &render_image, &backend_render);
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("Failed to open: {e}")));
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let layers_ref = layers.clone();
        let line_style_indices = line_style_indices.clone();
        let point_style_indices = point_style_indices.clone();
        let polygon_style_indices = polygon_style_indices.clone();
        let grid_settings = grid_settings.clone();
        let point_label_style = point_label_style.clone();
        let point_styles = point_styles.clone();
        let line_styles = line_styles.clone();
        let dimensions = dimensions.clone();
        let last_dir = last_folder.clone();
        let config_rc = config.clone();
        let workspace_crs = workspace_crs.clone();
        let surface_units_ref = surface_units.clone();
        let surface_styles_ref = surface_styles.clone();
        let surface_descriptions_ref = surface_descriptions.clone();
        let alignments_save = alignments.clone();
        app.on_save_project(move || {
            let mut dialog = rfd::FileDialog::new();
            if let Some(dir) = last_dir.borrow().as_ref() {
                dialog = dialog.set_directory(dir);
            }
            if let Some(path) = dialog.add_filter("Project", &["json"]).save_file() {
                *last_dir.borrow_mut() = path.parent().map(|p| p.to_string_lossy().to_string());
                config_rc.borrow_mut().last_open_dir = last_dir.borrow().clone();
                save_config(&config_rc.borrow());
                if let Some(p) = path.to_str() {
                    let proj = Project {
                        points: point_db.borrow().points().to_vec(),
                        lines: lines.borrow().iter().map(|l| Line::new(l.0, l.1)).collect(),
                        polygons: polygons.borrow().clone(),
                        polylines: polylines.borrow().clone(),
                        arcs: arcs.borrow().clone(),
                        dimensions: dimensions.borrow().clone(),
                        alignments: alignments_save.borrow().clone(),
                        surfaces: surfaces.borrow().clone(),
                        surface_units: surface_units_ref.borrow().clone(),
                        surface_styles: surface_styles_ref.borrow().clone(),
                        surface_descriptions: surface_descriptions_ref.borrow().clone(),
                        layers: layers_ref.borrow().iter().cloned().collect(),
                        point_style_indices: point_style_indices.borrow().clone(),
                        line_style_indices: line_style_indices.borrow().clone(),
                        polygon_style_indices: polygon_style_indices.borrow().clone(),
                        grid: grid_settings.borrow().clone(),
                        crs_epsg: *workspace_crs.borrow(),
                        point_label_font: point_label_style.borrow().text_style.font.clone(),
                        point_label_offset: point_label_style.borrow().offset,
                    };
                    let base = Path::new(p);
                    let _ = save_layers(&base.with_extension("layers.json"), &layers_ref.borrow());
                    let style_settings = StyleSettings {
                        point_styles: point_styles.clone(),
                        line_styles: line_styles.clone(),
                        polygon_styles: polygon_styles.clone(),
                    };
                    let _ = save_styles(&base.with_extension("styles.json"), &style_settings);

                    if let Err(e) = write_project_json(p, &proj) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to save: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Saved"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let lines = lines.clone();
        let render_image = render_image.clone();
        let line_style_indices = line_style_indices.clone();
        let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
        let backend_render = backend.clone();
        let command_stack_outer = command_stack.clone();
        let macro_playing_outer = macro_playing.clone();
        let macro_recorder_outer = macro_recorder.clone();
        let workspace_crs_line = workspace_crs.clone();
        app.on_add_line(move || {
            let macro_playing = macro_playing_outer.clone();
            let macro_recorder = macro_recorder_outer.clone();
            let line_style_indices = line_style_indices.clone();
            let dlg = AddLineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let lines = lines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let workspace_crs = workspace_crs_line.clone();
                let line_style_indices = line_style_indices.clone();
                let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
                let backend_render = backend_render.clone();
                let command_stack = command_stack_outer.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("DWG", &["dwg"])
                        .add_filter("DGN", &["dgn"])
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_line_csv(p, *workspace_crs.borrow()) {
                                Ok(l) => {
                                    lines.borrow_mut().push(l);
                                    let (s, e) = l;
                                    backend_render
                                        .borrow_mut()
                                        .add_line(
                                            [s.x, s.y, 0.0],
                                            [e.x, e.y, 0.0],
                                            [1.0, 1.0, 1.0, 1.0],
                                            1.0,
                                        );
                                    command_stack.borrow_mut().push(Command::RemoveLine {
                                        index: lines.borrow().len() - 1,
                                        line: (s, e),
                                    });
                                    let count = lines.borrow().len();
                                    let mut idx = line_style_indices.borrow_mut();
                                    if idx.len() < count {
                                        idx.resize(count, 0);
                                    }
                                    refresh_line_style_dialogs();
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total lines: {}",
                                            lines.borrow().len()
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            app.set_workspace_image(render_image());
                                            app.window().request_redraw();
                                        }
                                        refresh_workspace(&app, &render_image, &backend_render);
                                        refresh_workspace(&app, &render_image, &backend_render);
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {e}"
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let lines = lines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let line_style_indices = line_style_indices.clone();
                let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
                let backend_render = backend_render.clone();
                let command_stack_outer = command_stack_outer.clone();
                let macro_playing = macro_playing.clone();
                let macro_recorder = macro_recorder.clone();
                dlg.on_manual(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let kd = LineKeyInDialog::new().unwrap();
                    let kd_weak = kd.as_weak();
                    let kd_weak2 = kd.as_weak();
                    {
                let lines = lines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let line_style_indices = line_style_indices.clone();
                let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
                let backend_render = backend_render.clone();
                let command_stack = command_stack_outer.clone();
                let macro_playing = macro_playing.clone();
                let macro_recorder = macro_recorder.clone();
                kd.on_accept(move || {
                    if let Some(dlg) = kd_weak2.upgrade() {
                        if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                            dlg.get_x1().parse::<f64>(),
                            dlg.get_y1().parse::<f64>(),
                                    dlg.get_x2().parse::<f64>(),
                                    dlg.get_y2().parse::<f64>(),
                                ) {
                                    lines
                                        .borrow_mut()
                                        .push((Point::new(x1, y1), Point::new(x2, y2)));
                                    backend_render
                                        .borrow_mut()
                                        .add_line(
                                            [x1, y1, 0.0],
                                            [x2, y2, 0.0],
                                            [1.0, 1.0, 1.0, 1.0],
                                            1.0,
                                        );
                                    if !macro_playing.borrow().0 {
                                        record_macro(
                                            &mut macro_recorder.borrow_mut(),
                                            &format!("line {x1} {y1} {x2} {y2}"),
                                        );
                                    }
                                    command_stack.borrow_mut().push(Command::RemoveLine {
                                        index: lines.borrow().len() - 1,
                                        line: (Point::new(x1, y1), Point::new(x2, y2)),
                                    });
                                    let count = lines.borrow().len();
                                    let mut idx = line_style_indices.borrow_mut();
                                    if idx.len() < count {
                                        idx.resize(count, 0);
                                    }
                                    refresh_line_style_dialogs();
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total lines: {}",
                                            lines.borrow().len()
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            app.set_workspace_image(render_image());
                                            app.window().request_redraw();
                                        }
                                        refresh_workspace(&app, &render_image, &backend_render);
                                    }
                                }
                            }
                            if let Some(k) = kd_weak.upgrade() {
                                let _ = k.hide();
                            }
                        });
                    }
                    {
                        let kd_weak = kd.as_weak();
                        kd.on_cancel(move || {
                            if let Some(k) = kd_weak.upgrade() {
                                let _ = k.hide();
                            }
                        });
                    }
                    kd.show().unwrap();
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        let point_style_indices = point_style_indices.clone();
        let backend_render = backend.clone();
        let command_stack_outer = command_stack.clone();
        let macro_playing_outer = macro_playing.clone();
        let macro_recorder_outer = macro_recorder.clone();
        let workspace_crs_point = workspace_crs.clone();
        app.on_add_point(move || {
            let macro_playing = macro_playing_outer.clone();
            let macro_recorder = macro_recorder_outer.clone();
            let dlg = AddPointDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let point_db = point_db.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let point_style_indices = point_style_indices.clone();
                let workspace_crs = workspace_crs_point.clone();
                let backend_render = backend_render.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("DWG", &["dwg"])
                        .add_filter("DGN", &["dgn"])
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match survey_cad::io::read_points_csv(p, Some(4326), Some(*workspace_crs.borrow())) {
                                Ok(pts) => {
                                    let len = {
                                        let mut db = point_db.borrow_mut();
                                        db.clear();
                                        db.extend(pts);
                                        point_style_indices.borrow_mut().clear();
                                        point_style_indices
                                            .borrow_mut()
                                            .extend(std::iter::repeat_n(0, db.len()));
                                        backend_render.borrow_mut().clear();
                                        for p in db.iter() {
                                            backend_render.borrow_mut().add_point(p.x, p.y, 0.0);
                                        }
                                        db.len()
                                    };
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Loaded {len} points"
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            app.set_workspace_image(render_image());
                                            app.window().request_redraw();
                                        }
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {e}"
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let point_db = point_db.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let point_style_indices = point_style_indices.clone();
                let backend_render = backend_render.clone();
                let cs_inner = command_stack_outer.clone();
                let macro_playing = macro_playing.clone();
                let macro_recorder = macro_recorder.clone();
                dlg.on_manual_keyin(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let key_dlg = KeyInDialog::new().unwrap();
                    let key_weak = key_dlg.as_weak();
                    let key_weak2 = key_dlg.as_weak();
                    {
                        let point_db = point_db.clone();
                        let render_image = render_image.clone();
                        let weak = weak.clone();
                        let psi = point_style_indices.clone();
                let backend_render = backend_render.clone();
                let command_stack = cs_inner.clone();
                let macro_playing = macro_playing.clone();
                let macro_recorder = macro_recorder.clone();
                        key_dlg.on_accept(move || {
                            if let Some(dlg) = key_weak2.upgrade() {
                                if let (Ok(x), Ok(y)) = (
                                    dlg.get_x_value().parse::<f64>(),
                                    dlg.get_y_value().parse::<f64>(),
                                ) {
                                    point_db.borrow_mut().push(Point::new(x, y));
                                    psi.borrow_mut().push(0);
                                    backend_render.borrow_mut().add_point(x, y, 0.0);
                                    if !macro_playing.borrow().0 {
                                        record_macro(
                                            &mut macro_recorder.borrow_mut(),
                                            &format!("point {x} {y}"),
                                        );
                                    }
                                    command_stack.borrow_mut().push(Command::RemovePoint {
                                        index: point_db.borrow().len() - 1,
                                        point: Point::new(x, y),
                                    });
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total points: {}",
                                            point_db.borrow().len()
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            app.set_workspace_image(render_image());
                                            app.window().request_redraw();
                                        }
                                    }
                                }
                            }
                            if let Some(k) = key_weak.upgrade() {
                                let _ = k.hide();
                            }
                        });
                    }
                    {
                        let key_weak = key_dlg.as_weak();
                        key_dlg.on_cancel(move || {
                            if let Some(k) = key_weak.upgrade() {
                                let _ = k.hide();
                            }
                        });
                    }
                    key_dlg.show().unwrap();
                });
            }
            {
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual_click(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    if let Some(app) = weak.upgrade() {
                        app.set_workspace_click_mode(true);
                    }
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let polygons = polygons.clone();
        let render_image = render_image.clone();
        let workspace_crs_polygon = workspace_crs.clone();
        app.on_add_polygon(move || {
            let dlg = AddPolygonDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let polygons = polygons.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let workspace_crs = workspace_crs_polygon.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("DWG", &["dwg"])
                        .add_filter("DGN", &["dgn"])
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_points_list(p, *workspace_crs.borrow()) {
                                Ok(pts) => {
                                    if pts.len() >= 3 {
                                        polygons.borrow_mut().push(pts);
                                        if let Some(app) = weak.upgrade() {
                                            app.set_status(SharedString::from(format!(
                                                "Total polygons: {}",
                                                polygons.borrow().len()
                                            )));
                                            if app.get_workspace_mode() == 0 {
                                                app.set_workspace_image(render_image());
                                                app.window().request_redraw();
                                            }
                                        }
                                    } else if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(
                                            "Need at least 3 points",
                                        ));
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {e}"
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let polygons = polygons.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let pd = PointsDialog::new().unwrap();
                    let model = Rc::new(VecModel::<SharedString>::from(Vec::<SharedString>::new()));
                    pd.set_points_model(model.clone().into());
                    let pts = Rc::new(RefCell::new(Vec::<Point>::new()));
                    {
                        let model = model.clone();
                        let pd_weak2 = pd.as_weak();
                        let pts = pts.clone();
                        pd.on_add_point(move || {
                            if let Some(d) = pd_weak2.upgrade() {
                                if let (Ok(x), Ok(y)) = (
                                    d.get_x_value().parse::<f64>(),
                                    d.get_y_value().parse::<f64>(),
                                ) {
                                    pts.borrow_mut().push(Point::new(x, y));
                                    model.push(SharedString::from(format!("{x:.3},{y:.3}")));
                                }
                            }
                        });
                    }
                    {
                        let polygons = polygons.clone();
                        let render_image = render_image.clone();
                        let weak = weak.clone();
                        let pd_weak2 = pd.as_weak();
                        let pts = pts.clone();
                        pd.on_accept(move || {
                            if pts.borrow().len() >= 3 {
                                polygons.borrow_mut().push(pts.borrow().clone());
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from(format!(
                                        "Total polygons: {}",
                                        polygons.borrow().len()
                                    )));
                                    if app.get_workspace_mode() == 0 {
                                        app.set_workspace_image(render_image());
                                        app.window().request_redraw();
                                    }
                                }
                            }
                            if let Some(p) = pd_weak2.upgrade() {
                                let _ = p.hide();
                            }
                        });
                    }
                    {
                        let pd_weak2 = pd.as_weak();
                        pd.on_cancel(move || {
                            if let Some(p) = pd_weak2.upgrade() {
                                let _ = p.hide();
                            }
                        });
                    }
                    pd.show().unwrap();
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let polylines = polylines.clone();
        let render_image = render_image.clone();
        let workspace_crs_polyline = workspace_crs.clone();
        app.on_add_polyline(move || {
            let dlg = AddPolylineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let polylines = polylines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let workspace_crs = workspace_crs_polyline.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_points_list(p, *workspace_crs.borrow()) {
                                Ok(pts) => {
                                    if pts.len() >= 2 {
                                        polylines.borrow_mut().push(Polyline::new(pts));
                                        if let Some(app) = weak.upgrade() {
                                            app.set_status(SharedString::from(format!(
                                                "Total polylines: {}",
                                                polylines.borrow().len()
                                            )));
                                            if app.get_workspace_mode() == 0 {
                                                app.set_workspace_image(render_image());
                                                app.window().request_redraw();
                                            }
                                        }
                                    } else if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(
                                            "Need at least 2 points",
                                        ));
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {e}"
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let polylines = polylines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let pd = PointsDialog::new().unwrap();
                    let model = Rc::new(VecModel::<SharedString>::from(Vec::<SharedString>::new()));
                    pd.set_points_model(model.clone().into());
                    let pts = Rc::new(RefCell::new(Vec::<Point>::new()));
                    {
                        let model = model.clone();
                        let pd_weak2 = pd.as_weak();
                        let pts = pts.clone();
                        pd.on_add_point(move || {
                            if let Some(d) = pd_weak2.upgrade() {
                                if let (Ok(x), Ok(y)) = (
                                    d.get_x_value().parse::<f64>(),
                                    d.get_y_value().parse::<f64>(),
                                ) {
                                    pts.borrow_mut().push(Point::new(x, y));
                                    model.push(SharedString::from(format!("{x:.3},{y:.3}")));
                                }
                            }
                        });
                    }
                    {
                        let polylines = polylines.clone();
                        let render_image = render_image.clone();
                        let weak = weak.clone();
                        let pd_weak2 = pd.as_weak();
                        let pts = pts.clone();
                        pd.on_accept(move || {
                            if pts.borrow().len() >= 2 {
                                polylines
                                    .borrow_mut()
                                    .push(Polyline::new(pts.borrow().clone()));
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from(format!(
                                        "Total polylines: {}",
                                        polylines.borrow().len()
                                    )));
                                    if app.get_workspace_mode() == 0 {
                                        app.set_workspace_image(render_image());
                                        app.window().request_redraw();
                                    }
                                }
                            }
                            if let Some(p) = pd_weak2.upgrade() {
                                let _ = p.hide();
                            }
                        });
                    }
                    {
                        let pd_weak2 = pd.as_weak();
                        pd.on_cancel(move || {
                            if let Some(p) = pd_weak2.upgrade() {
                                let _ = p.hide();
                            }
                        });
                    }
                    pd.show().unwrap();
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let arcs = arcs.clone();
        let render_image = render_image.clone();
        app.on_add_arc(move || {
            let dlg = AddArcDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let arcs = arcs.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_arc_csv(p) {
                                Ok(a) => {
                                    arcs.borrow_mut().push(a);
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total arcs: {}",
                                            arcs.borrow().len()
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            app.set_workspace_image(render_image());
                                            app.window().request_redraw();
                                        }
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {e}"
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let arcs = arcs.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let ad = ArcKeyInDialog::new().unwrap();
                    let ad_weak = ad.as_weak();
                    let ad_weak2 = ad.as_weak();
                    {
                        let arcs = arcs.clone();
                        let render_image = render_image.clone();
                        let weak = weak.clone();
                        ad.on_accept(move || {
                            if let Some(dlg) = ad_weak2.upgrade() {
                                if let (Ok(cx), Ok(cy), Ok(r), Ok(sa), Ok(ea)) = (
                                    dlg.get_cx().parse::<f64>(),
                                    dlg.get_cy().parse::<f64>(),
                                    dlg.get_radius().parse::<f64>(),
                                    dlg.get_start_angle().parse::<f64>(),
                                    dlg.get_end_angle().parse::<f64>(),
                                ) {
                                    arcs.borrow_mut()
                                        .push(Arc::new(Point::new(cx, cy), r, sa, ea));
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total arcs: {}",
                                            arcs.borrow().len()
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            app.set_workspace_image(render_image());
                                            app.window().request_redraw();
                                        }
                                    }
                                }
                            }
                            if let Some(a) = ad_weak.upgrade() {
                                let _ = a.hide();
                            }
                        });
                    }
                    {
                        let ad_weak = ad.as_weak();
                        ad.on_cancel(move || {
                            if let Some(a) = ad_weak.upgrade() {
                                let _ = a.hide();
                            }
                        });
                    }
                    ad.show().unwrap();
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let render_image = render_image.clone();
        app.on_create_polygon_from_selection(move || {
            let mut pts: Vec<Point> = selected_indices
                .borrow()
                .iter()
                .filter_map(|&i| point_db.borrow().get(i).copied())
                .collect();
            for (s, e) in selected_lines.borrow().iter() {
                pts.push(*s);
                pts.push(*e);
            }
            let hull = convex_hull(&pts);
            if hull.len() >= 3 {
                polygons.borrow_mut().push(hull);
                selected_indices.borrow_mut().clear();
                selected_lines.borrow_mut().clear();
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from(format!(
                        "Total polygons: {}",
                        polygons.borrow().len()
                    )));
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            } else if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Need at least 3 vertices"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_station_distance(move || {
            let dlg = StationDistanceDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let res = (|| {
                        let x1 = d.get_x1().parse::<f64>().ok()?;
                        let y1 = d.get_y1().parse::<f64>().ok()?;
                        let x2 = d.get_x2().parse::<f64>().ok()?;
                        let y2 = d.get_y2().parse::<f64>().ok()?;
                        Some(survey_cad::surveying::station_distance(
                            &survey_cad::surveying::Station::new("A", Point::new(x1, y1)),
                            &survey_cad::surveying::Station::new("B", Point::new(x2, y2)),
                        ))
                    })();
                    if let Some(app) = weak2.upgrade() {
                        if let Some(dist) = res {
                            app.set_status(SharedString::from(format!("Distance: {dist:.3}")));
                        } else {
                            app.set_status(SharedString::from("Invalid input"));
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let workspace_crs = workspace_crs.clone();
        app.on_traverse_area(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DWG", &["dwg"])
                .add_filter("DGN", &["dgn"])
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                if let (Some(p), Some(app)) = (path.to_str(), weak.upgrade()) {
                    match survey_cad::io::read_points_csv(p, Some(4326), Some(*workspace_crs.borrow())) {
                        Ok(pts) => {
                            let trav = survey_cad::surveying::Traverse::new(pts);
                            app.set_status(SharedString::from(format!("Area: {:.3}", trav.area())));
                        }
                        Err(e) => {
                            app.set_status(SharedString::from(format!("Failed: {e}")));
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_level_elevation_tool(move || {
            let dlg = LevelElevationDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let res = (|| {
                        let start = d.get_start_elev().parse::<f64>().ok()?;
                        let bs = d.get_backsight().parse::<f64>().ok()?;
                        let fs = d.get_foresight().parse::<f64>().ok()?;
                        Some(survey_cad::surveying::level_elevation(start, bs, fs))
                    })();
                    if let Some(app) = weak2.upgrade() {
                        if let Some(elev) = res {
                            app.set_status(SharedString::from(format!("Elevation: {elev:.3}")));
                        } else {
                            app.set_status(SharedString::from("Invalid input"));
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let surfaces_clone = surfaces.clone();
        let alignments_clone = alignments.clone();
        app.on_corridor_volume(move || {
            let dlg = CorridorVolumeDialog::new().unwrap();
            dlg.set_width_value("10".into());
            dlg.set_interval_value("10".into());
            dlg.set_offset_step_value("1".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let surfs = surfaces_clone.clone();
            let aligns = alignments_clone.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let res = (|| {
                        let width = d.get_width_value().parse::<f64>().ok()?;
                        let interval = d.get_interval_value().parse::<f64>().ok()?;
                        let step = d.get_offset_step_value().parse::<f64>().ok()?;
                        let surfs = surfs.borrow();
                        let aligns = aligns.borrow();
                        if surfs.len() < 2 || aligns.is_empty() {
                            return None;
                        }
                        let design = &surfs[0];
                        let ground = &surfs[1];
                        let al = &aligns[0];
                        Some(survey_cad::corridor::corridor_volume(
                            design, ground, al, width, interval, step,
                        ))
                    })();
                    if let Some(app) = weak2.upgrade() {
                        if let Some(vol) = res {
                            app.set_status(SharedString::from(format!("Volume: {vol:.3}")));
                        } else {
                            app.set_status(SharedString::from("Invalid input or missing data"));
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        app.on_move_entity(move || {
            let dlg = MoveEntityDialog::new().unwrap();
            dlg.set_dx_value("0".into());
            dlg.set_dy_value("0".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let point_db = point_db.clone();
            let lines = lines.clone();
            let polygons = polygons.clone();
            let polylines = polylines.clone();
            let arcs = arcs.clone();
            let selected_indices = selected_indices.clone();
            let selected_lines = selected_lines.clone();
            let selected_polygons = selected_polygons.clone();
            let selected_polylines = selected_polylines.clone();
            let selected_arcs = selected_arcs.clone();
            let backend_inner = backend_render.clone();
            let render_image = render_image.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let dx = d.get_dx_value().parse::<f64>().unwrap_or(0.0);
                    let dy = d.get_dy_value().parse::<f64>().unwrap_or(0.0);
                    for &idx in selected_indices.borrow().iter() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx) {
                            p.x += dx;
                            p.y += dy;
                            backend_inner.borrow_mut().update_point(idx, p.x, p.y, 0.0);
                        }
                    }
                    for (i, line) in lines.borrow_mut().iter_mut().enumerate() {
                        if selected_lines
                            .borrow()
                            .iter()
                            .any(|(s, e)| (*s == line.0 && *e == line.1) || (*s == line.1 && *e == line.0))
                        {
                            line.0.x += dx;
                            line.0.y += dy;
                            line.1.x += dx;
                            line.1.y += dy;
                            backend_inner.borrow_mut().update_line(
                                i,
                                [line.0.x, line.0.y, 0.0],
                                [line.1.x, line.1.y, 0.0],
                                [1.0, 1.0, 1.0, 1.0],
                                1.0,
                            );
                        }
                    }
                    for &idx in selected_polygons.borrow().iter() {
                        if let Some(poly) = polygons.borrow_mut().get_mut(idx) {
                            for v in poly.iter_mut() {
                                v.x += dx;
                                v.y += dy;
                            }
                        }
                    }
                    for &idx in selected_polylines.borrow().iter() {
                        if let Some(pl) = polylines.borrow_mut().get_mut(idx) {
                            for v in pl.vertices.iter_mut() {
                                v.x += dx;
                                v.y += dy;
                            }
                        }
                    }
                    for &idx in selected_arcs.borrow().iter() {
                        if let Some(a) = arcs.borrow_mut().get_mut(idx) {
                            a.center.x += dx;
                            a.center.y += dy;
                        }
                    }
                    if let Some(app) = weak2.upgrade() {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        app.on_rotate_entity(move || {
            let dlg = RotateEntityDialog::new().unwrap();
            dlg.set_angle_value("0".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let point_db = point_db.clone();
            let lines = lines.clone();
            let polygons = polygons.clone();
            let polylines = polylines.clone();
            let arcs = arcs.clone();
            let selected_indices = selected_indices.clone();
            let selected_lines = selected_lines.clone();
            let selected_polygons = selected_polygons.clone();
            let selected_polylines = selected_polylines.clone();
            let selected_arcs = selected_arcs.clone();
            let backend_inner = backend_render.clone();
            let render_image = render_image.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let ang = d.get_angle_value().parse::<f64>().unwrap_or(0.0).to_radians();
                    let cos_a = ang.cos();
                    let sin_a = ang.sin();
                    for &idx in selected_indices.borrow().iter() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx) {
                            let x = p.x * cos_a - p.y * sin_a;
                            let y = p.x * sin_a + p.y * cos_a;
                            p.x = x;
                            p.y = y;
                            backend_inner.borrow_mut().update_point(idx, p.x, p.y, 0.0);
                        }
                    }
                    for (i, line) in lines.borrow_mut().iter_mut().enumerate() {
                        if selected_lines
                            .borrow()
                            .iter()
                            .any(|(s, e)| (*s == line.0 && *e == line.1) || (*s == line.1 && *e == line.0))
                        {
                            for pt in [&mut line.0, &mut line.1] {
                                let x = pt.x * cos_a - pt.y * sin_a;
                                let y = pt.x * sin_a + pt.y * cos_a;
                                pt.x = x;
                                pt.y = y;
                            }
                            backend_inner.borrow_mut().update_line(
                                i,
                                [line.0.x, line.0.y, 0.0],
                                [line.1.x, line.1.y, 0.0],
                                [1.0, 1.0, 1.0, 1.0],
                                1.0,
                            );
                        }
                    }
                    for &idx in selected_polygons.borrow().iter() {
                        if let Some(poly) = polygons.borrow_mut().get_mut(idx) {
                            for v in poly.iter_mut() {
                                let x = v.x * cos_a - v.y * sin_a;
                                let y = v.x * sin_a + v.y * cos_a;
                                v.x = x;
                                v.y = y;
                            }
                        }
                    }
                    for &idx in selected_polylines.borrow().iter() {
                        if let Some(pl) = polylines.borrow_mut().get_mut(idx) {
                            for v in pl.vertices.iter_mut() {
                                let x = v.x * cos_a - v.y * sin_a;
                                let y = v.x * sin_a + v.y * cos_a;
                                v.x = x;
                                v.y = y;
                            }
                        }
                    }
                    for &idx in selected_arcs.borrow().iter() {
                        if let Some(a) = arcs.borrow_mut().get_mut(idx) {
                            let cx = a.center.x * cos_a - a.center.y * sin_a;
                            let cy = a.center.x * sin_a + a.center.y * cos_a;
                            a.center.x = cx;
                            a.center.y = cy;
                            a.start_angle += ang;
                            a.end_angle += ang;
                        }
                    }
                    if let Some(app) = weak2.upgrade() {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let polylines_ref = polylines.clone();
        let selected_polylines_ref = selected_polylines.clone();
        let backend_ref = backend.clone();
        app.on_extrude_polyline(move || {
            let dlg = ExtrudePolylineDialog::new().unwrap();
            dlg.set_distance_value("1".into());
            dlg.set_dx_value("0".into());
            dlg.set_dy_value("0".into());
            dlg.set_dz_value("1".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let polylines_inner = polylines_ref.clone();
            let selected_pl = selected_polylines_ref.clone();
            let backend_inner = backend_ref.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let dist = d.get_distance_value().parse::<f64>().unwrap_or(0.0);
                    let dx = d.get_dx_value().parse::<f64>().unwrap_or(0.0);
                    let dy = d.get_dy_value().parse::<f64>().unwrap_or(0.0);
                    let dz = d.get_dz_value().parse::<f64>().unwrap_or(1.0);
                    let mut dir = Vector3::new(dx, dy, dz);
                    if dir.magnitude2() < f64::EPSILON {
                        dir = Vector3::unit_z();
                    } else {
                        dir = dir.normalize();
                    }
                    let vec = dir * dist;
                    for &idx in selected_pl.borrow().iter() {
                        if let Some(pl) = polylines_inner.borrow().get(idx) {
                            if let Some(sol) = polyline_to_solid(pl, vec) {
                                backend_inner.borrow_mut().add_solid(sol);
                            }
                        }
                    }
                    if let Some(app) = weak2.upgrade() {
                        if app.get_workspace_mode() == 1 {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                        }
                        app.window().request_redraw();
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let alignments = alignments.clone();
        let backend = backend.clone();
        app.on_design_cross_sections(move || {
            let dlg = DesignSectionDialog::new().unwrap();
            dlg.set_start_station("0".into());
            dlg.set_end_station("100".into());
            dlg.set_interval("10".into());
            dlg.set_lane_width("3.5".into());
            dlg.set_lane_slope("-0.02".into());
            dlg.set_shoulder_width("1.0".into());
            dlg.set_shoulder_slope("-0.04".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let aligns = alignments.clone();
            let backend_inner = backend.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let res = (|| {
                        let start = d.get_start_station().parse::<f64>().ok()?;
                        let end = d.get_end_station().parse::<f64>().ok()?;
                        let interval = d.get_interval().parse::<f64>().ok()?;
                        let lane_w = d.get_lane_width().parse::<f64>().ok()?;
                        let lane_s = d.get_lane_slope().parse::<f64>().ok()?;
                        let sh_w = d.get_shoulder_width().parse::<f64>().ok()?;
                        let sh_s = d.get_shoulder_slope().parse::<f64>().ok()?;
                        let aligns = aligns.borrow();
                        if aligns.is_empty() {
                            return None;
                        }
                        let al = &aligns[0];
                        let lane = subassembly::lane(lane_w, lane_s);
                        let shoulder = subassembly::shoulder(sh_w, sh_s);
                        let sections = subassembly::symmetric_section(&[lane, shoulder]);
                        let mut cs =
                            corridor::extract_design_cross_sections(al, &sections, None, interval);
                        cs.retain(|c| c.station >= start && c.station <= end);
                        for section in cs {
                            for pair in section.points.windows(2) {
                                backend_inner.borrow_mut().add_line(
                                    [pair[0].x, pair[0].y, pair[0].z],
                                    [pair[1].x, pair[1].y, pair[1].z],
                                    [1.0, 1.0, 1.0, 1.0],
                                    1.0,
                                );
                            }
                        }
                        Some(())
                    })();
                    if let Some(app) = weak2.upgrade() {
                        if app.get_workspace_mode() == 1 {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
                        if res.is_some() {
                            app.set_status(SharedString::from("Sections generated"));
                        } else {
                            app.set_status(SharedString::from(
                                "Invalid input or missing alignment",
                            ));
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let backend = backend.clone();
        app.on_view_cross_sections(move || {
            let surfs = surfaces.borrow();
            let aligns = alignments.borrow();
            if surfs.is_empty() || aligns.is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("Need surface and alignment"));
                }
                return;
            }
            let al = aligns[0].clone();
            let sections = corridor::extract_cross_sections(&surfs[0], &al, 10.0, 10.0, 1.0);
            if sections.is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("No cross sections"));
                }
                return;
            }
            let viewer = CrossSectionViewer::new().unwrap();
            let current = Rc::new(RefCell::new(0usize));
            viewer.set_station_label(SharedString::from(format!(
                "Station: {:.2}",
                sections[0].station
            )));
            let elev = al.vertical.elevation_at(sections[0].station).unwrap_or(0.0);
            let grade = grade_at(&al.vertical, sections[0].station).unwrap_or(0.0);
            viewer.set_elevation_label(SharedString::from(format!("Elev: {elev:.2}")));
            viewer.set_slope_label(SharedString::from(format!("Slope: {grade:.4}")));
            viewer.set_section_image(render_cross_section(&sections[0], 600, 300));
            let viewer_weak = viewer.as_weak();
            let secs = Rc::new(RefCell::new(sections));
            let drag_index = Rc::new(RefCell::new(None::<usize>));
            {
                let current = current.clone();
                let secs = secs.clone();
                let viewer_weak = viewer_weak.clone();
                let al = al.clone();
                viewer.on_prev(move || {
                    if *current.borrow() > 0 {
                        *current.borrow_mut() -= 1;
                        let i = *current.borrow();
                        if let Some(v) = viewer_weak.upgrade() {
                            let secs_b = secs.borrow();
                            v.set_station_label(SharedString::from(format!(
                                "Station: {:.2}",
                                secs_b[i].station
                            )));
                            let elev = al.vertical.elevation_at(secs_b[i].station).unwrap_or(0.0);
                            let grade = grade_at(&al.vertical, secs_b[i].station).unwrap_or(0.0);
                            v.set_elevation_label(SharedString::from(format!("Elev: {elev:.2}")));
                            v.set_slope_label(SharedString::from(format!("Slope: {grade:.4}")));
                            v.set_section_image(render_cross_section(&secs_b[i], 600, 300));
                        }
                    }
                });
            }
            {
                let current = current.clone();
                let secs = secs.clone();
                let viewer_weak = viewer_weak.clone();
                let al = al.clone();
                viewer.on_next(move || {
                    if *current.borrow() + 1 < secs.borrow().len() {
                        *current.borrow_mut() += 1;
                        let i = *current.borrow();
                        if let Some(v) = viewer_weak.upgrade() {
                            let secs_b = secs.borrow();
                            v.set_station_label(SharedString::from(format!(
                                "Station: {:.2}",
                                secs_b[i].station
                            )));
                            let elev = al.vertical.elevation_at(secs_b[i].station).unwrap_or(0.0);
                            let grade = grade_at(&al.vertical, secs_b[i].station).unwrap_or(0.0);
                            v.set_elevation_label(SharedString::from(format!("Elev: {elev:.2}")));
                            v.set_slope_label(SharedString::from(format!("Slope: {grade:.4}")));
                            v.set_section_image(render_cross_section(&secs_b[i], 600, 300));
                        }
                    }
                });
            }
            {
                let secs_p = secs.clone();
                let current_p = current.clone();
                let drag_p = drag_index.clone();
                viewer.on_pointer_pressed(move |x, y| {
                    let secs_b = secs_p.borrow();
                    if let Some(idx) = nearest_point(&secs_b[*current_p.borrow()], x, y, 600.0, 300.0) {
                        *drag_p.borrow_mut() = Some(idx);
                    }
                });

                let secs_m = secs.clone();
                let current_m = current.clone();
                let drag_m = drag_index.clone();
                let viewer_weak_m = viewer_weak.clone();
                viewer.on_pointer_moved(move |x, y| {
                    if let Some(idx) = *drag_m.borrow() {
                        if let Some(p) = screen_to_world(&secs_m.borrow()[*current_m.borrow()], x, y, 600.0, 300.0) {
                            secs_m.borrow_mut()[*current_m.borrow()].points[idx] = p;
                            if let Some(v) = viewer_weak_m.upgrade() {
                                v.set_section_image(render_cross_section(&secs_m.borrow()[*current_m.borrow()], 600, 300));
                            }
                        }
                    }
                });

                let secs_r = secs.clone();
                let surfaces_r = surfaces.clone();
                let backend_r = backend.clone();
                let drag_r = drag_index.clone();
                viewer.on_pointer_released(move || {
                    if drag_r.borrow().is_some() {
                        *drag_r.borrow_mut() = None;
                        let tin = corridor::surface_from_cross_sections(&secs_r.borrow());
                        let verts: Vec<Point3> = tin.vertices.iter().map(|p| Point3::new(p.x, p.y, p.z)).collect();
                        if surfaces_r.borrow().is_empty() {
                            backend_r.borrow_mut().add_surface(&verts, &tin.triangles);
                            surfaces_r.borrow_mut().push(tin);
                        } else {
                            backend_r.borrow_mut().update_surface(0, &verts, &tin.triangles);
                            surfaces_r.borrow_mut()[0] = tin;
                        }
                    }
                });
            }
            viewer.show().unwrap();
        });
    }

    {
        let backend = backend.clone();
        let weak = app.as_weak();
        let cs_outer = command_stack.clone();
        app.on_tin_add_vertex(move || {
            let dlg = TinVertexDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let command_stack = cs_outer.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(x), Ok(y), Ok(z)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_x_val().parse::<f64>(),
                        d.get_y_val().parse::<f64>(),
                        d.get_z_val().parse::<f64>(),
                    ) {
                        if let Some(idx) = backend_inner
                            .borrow_mut()
                            .add_vertex(surf, Point3::new(x, y, z))
                        {
                            command_stack.borrow_mut().push(Command::TinDeleteVertex {
                                surface: surf,
                                index: idx,
                                point: Point3::new(x, y, z),
                            });
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let backend = backend.clone();
        let weak = app.as_weak();
        app.on_tin_move_vertex(move || {
            let dlg = TinVertexDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(idx), Ok(x), Ok(y), Ok(z)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_vertex_index().parse::<usize>(),
                        d.get_x_val().parse::<f64>(),
                        d.get_y_val().parse::<f64>(),
                        d.get_z_val().parse::<f64>(),
                    ) {
                        backend_inner
                            .borrow_mut()
                            .move_vertex(surf, idx, Point3::new(x, y, z));
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let backend = backend.clone();
        let weak = app.as_weak();
        app.on_tin_delete_vertex(move || {
            let dlg = TinVertexDialog::new().unwrap();
            dlg.set_x_val("0".into());
            dlg.set_y_val("0".into());
            dlg.set_z_val("0".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(idx)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_vertex_index().parse::<usize>(),
                    ) {
                        backend_inner.borrow_mut().delete_vertex(surf, idx);
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let backend = backend.clone();
        let weak = app.as_weak();
        app.on_tin_add_triangle(move || {
            let dlg = TinTriangleDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(a), Ok(b), Ok(c)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_v1().parse::<usize>(),
                        d.get_v2().parse::<usize>(),
                        d.get_v3().parse::<usize>(),
                    ) {
                        backend_inner.borrow_mut().add_triangle(surf, [a, b, c]);
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let backend = backend.clone();
        let weak = app.as_weak();
        app.on_tin_delete_triangle(move || {
            let dlg = TinTriangleDialog::new().unwrap();
            dlg.set_v1("0".into());
            dlg.set_v2("0".into());
            dlg.set_v3("0".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(idx)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_tri_index().parse::<usize>(),
                    ) {
                        backend_inner.borrow_mut().delete_triangle(surf, idx);
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
                    }
                    let _ = d.hide();
                }
            });
            let dlg_weak2 = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak2.upgrade() {
                    let _ = d.hide();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let workspace_crs = workspace_crs.clone();
        app.on_import_geojson(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("GeoJSON", &["geojson", "json"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    let dst = *workspace_crs.borrow();
                    match survey_cad::io::read_points_geojson(p, Some(4326), Some(dst)) {
                        Ok(pts) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts);
                                backend_render.borrow_mut().clear();
                                for pt in db.iter() {
                                    backend_render.borrow_mut().add_point(pt.x, pt.y, 0.0);
                                }
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_kml(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("KML", &["kml", "kmz"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "kml")]
                    match survey_cad::io::kml::read_points_kml(p) {
                        Ok(pts) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts);
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                    #[cfg(not(feature = "kml"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("KML support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_dxf(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DXF", &["dxf"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::read_dxf(p) {
                        Ok(ents) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(ents.into_iter().filter_map(|e| match e {
                                    survey_cad::io::DxfEntity::Point { point, .. } => Some(point),
                                    _ => None,
                                }));
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_dwg(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DWG", &["dwg"])
                .add_filter("DGN", &["dgn"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::read_dwg(p) {
                        Ok(ents) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(ents.into_iter().filter_map(|e| match e {
                                    survey_cad::io::DxfEntity::Point { point, .. } => Some(point),
                                    _ => None,
                                }));
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            let msg = if e.to_string().contains("dwg2dxf") {
                                "dwg2dxf tool not found".to_string()
                            } else {
                                format!("Failed to import: {e}")
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(msg));
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let workspace_crs = workspace_crs.clone();
        app.on_import_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    match survey_cad::io::shp::read_points_shp(p) {
                        Ok((mut pts, _)) => {
                            let dst = *workspace_crs.borrow();
                            let src = survey_cad::crs::Crs::from_epsg(4326);
                            let dst_crs = survey_cad::crs::Crs::from_epsg(dst);
                            for p in &mut pts {
                                if let Some((x, y)) = src.transform_point(&dst_crs, p.x, p.y) {
                                    p.x = x;
                                    p.y = y;
                                }
                            }
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts);
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                    #[cfg(not(feature = "shapefile"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("SHP support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let lines = lines.clone();
        let polylines_ref = polylines.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_polylines_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    match survey_cad::io::shp::read_polylines_shp(p) {
                        Ok((pls, _)) => {
                            let mut lns = lines.borrow_mut();
                            let mut pls_vec = polylines_ref.borrow_mut();
                            lns.clear();
                            pls_vec.clear();
                            for pl in pls {
                                if pl.vertices.len() == 2 {
                                    lns.push((pl.vertices[0], pl.vertices[1]));
                                } else {
                                    pls_vec.push(pl);
                                }
                            }
                            let count = lns.len() + pls_vec.len();
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {count} polylines"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                    #[cfg(not(feature = "shapefile"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("SHP support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let polygons_ref = polygons.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_polygons_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    match survey_cad::io::shp::read_polygons_shp(p) {
                        Ok((polys, _)) => {
                            let len = {
                                let mut pg = polygons_ref.borrow_mut();
                                pg.clear();
                                pg.extend(polys);
                                pg.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} polygons"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                    #[cfg(not(feature = "shapefile"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("SHP support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_las(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LAS", &["las", "laz"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "las")]
                    match survey_cad::io::las::read_points_las(p) {
                        Ok(pts3) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts3.into_iter().map(|p3| Point::new(p3.x, p3.y)));
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                    #[cfg(not(feature = "las"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("LAS support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_e57(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("E57", &["e57"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "e57")]
                    match survey_cad::io::e57::read_points_e57(p) {
                        Ok(pts3) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts3.into_iter().map(|p3| Point::new(p3.x, p3.y)));
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                    #[cfg(not(feature = "e57"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("E57 support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        app.on_export_geojson(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("GeoJSON", &["geojson", "json"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    if let Err(e) =
                        survey_cad::io::write_points_geojson(p, &point_db.borrow(), None, None)
                    {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Exported"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        app.on_export_kml(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("KML", &["kml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "kml")]
                    if let Err(e) = survey_cad::io::kml::write_points_kml(p, &point_db.borrow()) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Exported"));
                    }
                    #[cfg(not(feature = "kml"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("KML support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        app.on_export_dxf(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DXF", &["dxf"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    if let Err(e) =
                        survey_cad::io::write_points_dxf(p, &point_db.borrow(), None, None)
                    {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Exported"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        app.on_export_dwg(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DWG", &["dwg"])
                .add_filter("DGN", &["dgn"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let ents: Vec<survey_cad::io::DxfEntity> = point_db
                        .borrow()
                        .iter()
                        .map(|pt| survey_cad::io::DxfEntity::Point { point: *pt, layer: None })
                        .collect();
                    match survey_cad::io::write_dwg(p, &ents) {
                        Ok(()) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Exported"));
                            }
                        }
                        Err(e) => {
                            let msg = if e.to_string().contains("dxf2dwg") {
                                "dxf2dwg tool not found".to_string()
                            } else {
                                format!("Failed to export: {e}")
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(msg));
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        app.on_export_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    if let Err(e) =
                        survey_cad::io::shp::write_points_shp(p, &point_db.borrow(), None)
                    {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Exported"));
                    }
                    #[cfg(not(feature = "shapefile"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("SHP support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let lines_ref = lines.clone();
        let polylines_ref = polylines.clone();
        app.on_export_polylines_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    {
                        let mut out = Vec::new();
                        for (s, e) in lines_ref.borrow().iter() {
                            out.push(Polyline::new(vec![*s, *e]));
                        }
                        out.extend(polylines_ref.borrow().iter().cloned());
                        if let Err(e) = survey_cad::io::shp::write_polylines_shp(p, &out, None) {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("Failed to export: {e}")));
                            }
                        } else if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from("Exported"));
                        }
                    }
                    #[cfg(not(feature = "shapefile"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("SHP support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let polygons_ref = polygons.clone();
        app.on_export_polygons_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    if let Err(e) = survey_cad::io::shp::write_polygons_shp(p, &polygons_ref.borrow(), None) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Exported"));
                    }
                    #[cfg(not(feature = "shapefile"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("SHP support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        app.on_export_las(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LAS", &["las", "laz"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "las")]
                    {
                        let pts3: Vec<survey_cad::geometry::Point3> = point_db
                            .borrow()
                            .iter()
                            .map(|pt| survey_cad::geometry::Point3::new(pt.x, pt.y, 0.0))
                            .collect();
                        if let Err(e) = survey_cad::io::las::write_points_las(p, &pts3) {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to export: {e}"
                                )));
                            }
                        } else if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from("Exported"));
                        }
                    }
                    #[cfg(not(feature = "las"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("LAS support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        app.on_export_e57(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("E57", &["e57"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "e57")]
                    {
                        let pts3: Vec<survey_cad::geometry::Point3> = point_db
                            .borrow()
                            .iter()
                            .map(|pt| survey_cad::geometry::Point3::new(pt.x, pt.y, 0.0))
                            .collect();
                        if let Err(e) = survey_cad::io::e57::write_points_e57(p, &pts3) {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to export: {e}"
                                )));
                            }
                        } else if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from("Exported"));
                        }
                    }
                    #[cfg(not(feature = "e57"))]
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("E57 support not enabled"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let surfaces = surfaces.clone();
        let surface_units_clone = surface_units.clone();
        let surface_styles_clone = surface_styles.clone();
        let surface_descriptions_clone = surface_descriptions.clone();
        app.on_export_landxml_surface(move || {
            if surfaces.borrow().is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("No surface to export"));
                }
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let tin = &surfaces.borrow()[0];
                    let extras = survey_cad::io::landxml::LandxmlExtras {
                        units: surface_units_clone.borrow().first().cloned(),
                        style: surface_styles_clone.borrow().first().cloned(),
                        description: surface_descriptions_clone.borrow().first().cloned(),
                    };
                    if let Err(e) = survey_cad::io::landxml::write_landxml_surface(p, tin, Some(&extras)) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Exported"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let alignments = alignments.clone();
        app.on_export_landxml_alignment(move || {
            if alignments.borrow().is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("No alignment to export"));
                }
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let al = &alignments.borrow()[0];
                    if let Err(e) = survey_cad::io::landxml::write_landxml_alignment(p, &al.horizontal, None) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Exported"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let surfaces = surfaces.clone();
        let surface_units_clone = surface_units.clone();
        let surface_styles_clone = surface_styles.clone();
        let surface_descriptions_clone = surface_descriptions.clone();
        let alignments = alignments.clone();
        app.on_export_landxml_sections(move || {
            if surfaces.borrow().is_empty() || alignments.borrow().is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("No sections to export"));
                }
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let al = &alignments.borrow()[0];
                    let secs = corridor::extract_cross_sections(&surfaces.borrow()[0], al, 10.0, 10.0, 1.0);
                    let extras = survey_cad::io::landxml::LandxmlExtras {
                        units: surface_units_clone.borrow().first().cloned(),
                        style: surface_styles_clone.borrow().first().cloned(),
                        description: surface_descriptions_clone.borrow().first().cloned(),
                    };
                    if let Err(e) = survey_cad::io::landxml::write_landxml_cross_sections(p, &secs, Some(&extras)) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Exported"));
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        let point_style_names = point_style_names.clone();
        let render_image_pm = render_image.clone();
        let backend_render = backend.clone();
        app.on_point_manager(move || {
            let render_image = render_image_pm.clone();
            let dlg = PointManager::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let model = Rc::new(VecModel::<PointRow>::from(
                point_db
                    .borrow()
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        if point_style_indices.borrow().len() <= i {
                            point_style_indices.borrow_mut().push(0);
                        }
                        PointRow {
                            number: SharedString::from((i + 1).to_string()),
                            name: SharedString::from(""),
                            x: SharedString::from(format!("{:.3}", p.x)),
                            y: SharedString::from(format!("{:.3}", p.y)),
                            group_index: 0,
                            style_index: point_style_indices.borrow()[i] as i32,
                        }
                    })
                    .collect::<Vec<_>>(),
            ));
            dlg.set_points_model(model.clone().into());
            let groups_model = Rc::new(VecModel::<SharedString>::from(
                point_db
                    .borrow()
                    .iter_groups()
                    .map(|(_, g)| SharedString::from(g.name.clone()))
                    .collect::<Vec<_>>(),
            ));
            dlg.set_groups_model(groups_model.clone().into());
            dlg.set_styles_model(Rc::new(VecModel::from(point_style_names.clone())).into());
            dlg.set_selected_index(-1);

            let headers = Rc::new(RefCell::new(vec![
                SharedString::from("#"),
                SharedString::from("Name"),
                SharedString::from("X"),
                SharedString::from("Y"),
                SharedString::from("Group"),
                SharedString::from("Style"),
            ]));
            dlg.set_number_header(headers.borrow()[0].clone());
            dlg.set_name_header(headers.borrow()[1].clone());
            dlg.set_x_header(headers.borrow()[2].clone());
            dlg.set_y_header(headers.borrow()[3].clone());
            dlg.set_group_header(headers.borrow()[4].clone());
            dlg.set_style_header(headers.borrow()[5].clone());

            dlg.set_label_font(SharedString::from(point_label_style.borrow().text_style.font.clone()));
            dlg.set_offset_x(SharedString::from(format!("{:.1}", point_label_style.borrow().offset[0])));
            dlg.set_offset_y(SharedString::from(format!("{:.1}", point_label_style.borrow().offset[1])));

            let rename_in_model: Rc<dyn Fn(usize, SharedString)> = {
                let groups_model = groups_model.clone();
                Rc::new(move |idx: usize, name: SharedString| {
                    if idx < groups_model.row_count() {
                        groups_model.set_row_data(idx, name.clone());
                    }
                })
            };

            {
                let model = model.clone();
                let point_db = point_db.clone();
                let backend_render = backend_render.clone();
                dlg.on_edit_x(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx as usize) {
                            p.x = v;
                            if let Some(row) = model.row_data(idx as usize) {
                                let mut r = row.clone();
                                r.x = SharedString::from(format!("{v:.3}"));
                                model.set_row_data(idx as usize, r);
                            }
                            backend_render
                                .borrow_mut()
                                .update_point(idx as usize, p.x, p.y, 0.0);
                        }
                    }
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                let backend_render = backend_render.clone();
                dlg.on_edit_y(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx as usize) {
                            p.y = v;
                            if let Some(row) = model.row_data(idx as usize) {
                                let mut r = row.clone();
                                r.y = SharedString::from(format!("{v:.3}"));
                                model.set_row_data(idx as usize, r);
                            }
                            backend_render
                                .borrow_mut()
                                .update_point(idx as usize, p.x, p.y, 0.0);
                        }
                    }
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                let psi = point_style_indices.clone();
                let backend_render = backend_render.clone();
                dlg.on_add_point(move || {
                    point_db.borrow_mut().push(Point::new(0.0, 0.0));
                    psi.borrow_mut().push(0);
                    backend_render.borrow_mut().add_point(0.0, 0.0, 0.0);
                    let idx = point_db.borrow().len();
                    model.push(PointRow {
                        number: SharedString::from(format!("{idx}")),
                        name: SharedString::from(""),
                        x: SharedString::from("0.000"),
                        y: SharedString::from("0.000"),
                        group_index: 0,
                        style_index: 0,
                    });
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                let psi = point_style_indices.clone();
                let backend_render = backend_render.clone();
                dlg.on_remove_point(move |idx| {
                    if idx >= 0 && (idx as usize) < point_db.borrow().len() {
                        point_db.borrow_mut().remove(idx as usize);
                        psi.borrow_mut().remove(idx as usize);
                        model.remove(idx as usize);
                        backend_render.borrow_mut().remove_point(idx as usize);
                    }
                });
            }
            {
                let model = model.clone();
                let style_indices = point_style_indices.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                dlg.on_style_changed(move |idx, style_idx| {
                    if let Some(row) = model.row_data(idx as usize) {
                        let mut r = row.clone();
                        r.style_index = style_idx;
                        model.set_row_data(idx as usize, r);
                        if style_indices.borrow().len() > idx as usize {
                            style_indices.borrow_mut()[idx as usize] = style_idx as usize;
                        }
                        if let Some(app) = weak.upgrade() {
                            if app.get_workspace_mode() == 0 {
                                app.set_workspace_image(render_image());
                                app.window().request_redraw();
                            }
                        }
                    }
                });
            }
            {
                let groups_model = groups_model.clone();
                let point_db = point_db.clone();
                dlg.on_create_group(move || {
                    let name = format!("Group {}", groups_model.row_count() + 1);
                    point_db.borrow_mut().add_group(name.clone());
                    groups_model.push(SharedString::from(name));
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                let rename_in_model = rename_in_model.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_rename_group(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let row = d.get_selected_index();
                        if row >= 0 {
                            if let Some(r) = model.row_data(row as usize) {
                                let g_idx = r.group_index as usize;
                                let new_name = format!("Group {}", g_idx + 1);
                                if point_db.borrow_mut().rename_group(g_idx, new_name.clone()) {
                                    rename_in_model(g_idx, SharedString::from(new_name));
                                }
                            }
                        }
                    }
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                dlg.on_group_changed(move |p_idx, g_idx| {
                    if let Some(row) = model.row_data(p_idx as usize) {
                        point_db
                            .borrow_mut()
                            .remove_point_from_group(p_idx as usize, row.group_index as usize);
                        point_db
                            .borrow_mut()
                            .assign_point(p_idx as usize, g_idx as usize);
                        let mut r = row.clone();
                        r.group_index = g_idx;
                        model.set_row_data(p_idx as usize, r);
                    }
                });
            }
            { 
                let headers = headers.clone();
                dlg.on_header_changed(move |col, text| {
                    if let Some(h) = headers.borrow_mut().get_mut(col as usize) {
                        *h = text.clone();
                    }
                });
            }
            {
                let pls = point_label_style.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                let backend_render = backend_render.clone();
                dlg.on_label_font_changed(move |text| {
                    pls.borrow_mut().text_style.font = text.to_string();
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let pls = point_label_style.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                let backend_render = backend_render.clone();
                dlg.on_offset_x_changed(move |val| {
                    if let Ok(v) = val.parse::<f32>() {
                        pls.borrow_mut().offset[0] = v;
                        if let Some(app) = weak.upgrade() {
                            refresh_workspace(&app, &render_image, &backend_render);
                        }
                    }
                });
            }
            {
                let pls = point_label_style.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                let backend_render = backend_render.clone();
                dlg.on_offset_y_changed(move |val| {
                    if let Ok(v) = val.parse::<f32>() {
                        pls.borrow_mut().offset[1] = v;
                        if let Some(app) = weak.upgrade() {
                            refresh_workspace(&app, &render_image, &backend_render);
                        }
                    }
                });
            }

            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let lines = lines.clone();
        let line_style_indices = line_style_indices.clone();
        let line_style_names = line_style_names.clone();
        let render_image = render_image.clone();
        let dialogs = open_line_style_managers.clone();
        app.on_line_style_manager(move || {
            let dlg = LineStyleManager::new().unwrap();
            dialogs.borrow_mut().push(dlg.as_weak());

            let needed = line_style_names.len();
            {
                let mut idx = line_style_indices.borrow_mut();
                if idx.len() < needed {
                    idx.resize(needed, 0);
                }
            }
            let current_indices = line_style_indices.borrow().clone();
            let current_lines = lines.borrow().clone();
            let rows = current_indices
                .iter()
                .enumerate()
                .map(|(i, s_idx)| {
                    if let Some((s, e)) = current_lines.get(i) {
                        LineRow {
                            start: SharedString::from(format!("{:.2},{:.2}", s.x, s.y)),
                            end: SharedString::from(format!("{:.2},{:.2}", e.x, e.y)),
                            style_index: *s_idx as i32,
                        }
                    } else {
                        LineRow {
                            start: SharedString::from(""),
                            end: SharedString::from(""),
                            style_index: *s_idx as i32,
                        }
                    }
                })
                .collect::<Vec<_>>();
            let model = Rc::new(VecModel::<LineRow>::from(rows));
            dlg.set_lines_model(model.clone().into());
            dlg.set_styles_model(Rc::new(VecModel::from((*line_style_names).clone())).into());
            dlg.set_selected_index(-1);

            {
                let model = model.clone();
                let indices = line_style_indices.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                dlg.on_style_changed(move |idx, style_idx| {
                    if let Some(row) = model.row_data(idx as usize) {
                        let mut r = row.clone();
                        r.style_index = style_idx;
                        model.set_row_data(idx as usize, r);
                        {
                            let mut iref = indices.borrow_mut();
                            if iref.len() <= idx as usize {
                                iref.resize(idx as usize + 1, 0);
                            }
                            iref[idx as usize] = style_idx as usize;
                        }
                        if let Some(app) = weak.upgrade() {
                            if app.get_workspace_mode() == 0 {
                                app.set_workspace_image(render_image());
                                app.window().request_redraw();
                            }
                        }
                    }
                });
            }

            dlg.show().unwrap();
        });
    }

    {
        let layers_ref = layers.clone();
        let layer_names_ref = layer_names.clone();
        let line_type_model = line_type_names.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        let weak = app.as_weak();
        app.on_layer_manager(move || {
            let dlg = LayerManager::new().unwrap();
            dlg.set_line_types_model(line_type_model.clone().into());
            let rows = {
                let mgr = layers_ref.borrow();
                let names = layer_names_ref.borrow();
                names
                    .iter()
                    .map(|n| {
                        let layer = mgr.layer(n).unwrap();
                        LayerRow {
                            name: SharedString::from(n.clone()),
                            on: layer.is_on,
                            locked: layer.is_locked,
                            line_type_index: match layer.line_type.unwrap_or(LineType::Solid) {
                                LineType::Solid => 0,
                                LineType::Dashed => 1,
                                LineType::Dotted => 2,
                            },
                            color: SharedString::from(
                                layer
                                    .line_color
                                    .map(|c| format!("{},{},{}", c[0], c[1], c[2]))
                                    .unwrap_or_default(),
                            ),
                            weight: SharedString::from(
                                layer
                                    .line_weight
                                    .map(|w| format!("{:.2}", w.0))
                                    .unwrap_or_default(),
                            ),
                            text_style: SharedString::from(
                                layer
                                    .text_style
                                    .as_ref()
                                    .map(|t| t.name.clone())
                                    .unwrap_or_default(),
                            ),
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let model = Rc::new(VecModel::<LayerRow>::from(rows));
            dlg.set_layers_model(model.clone().into());
            dlg.set_selected_index(-1);

            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_toggle_on(move |idx, val| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        layers.borrow_mut().set_layer_state(&name, val);
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.on = val;
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_toggle_lock(move |idx, val| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            layer.is_locked = val;
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.locked = val;
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_line_type_changed(move |idx, val| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            layer.line_type = Some(match val {
                                0 => LineType::Solid,
                                1 => LineType::Dashed,
                                _ => LineType::Dotted,
                            });
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.line_type_index = val;
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_color_changed(move |idx, text| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            let vals: Vec<u8> = text
                                .split(',')
                                .filter_map(|v| v.trim().parse::<u8>().ok())
                                .collect();
                            if vals.len() == 3 {
                                layer.line_color = Some([vals[0], vals[1], vals[2]]);
                            }
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.color = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_weight_changed(move |idx, text| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            if let Ok(v) = text.parse::<f32>() {
                                layer.line_weight = Some(LineWeight(v));
                            } else {
                                layer.line_weight = None;
                            }
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.weight = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_text_style_changed(move |idx, text| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            if text.is_empty() {
                                layer.text_style = None;
                            } else {
                                layer.text_style = Some(ScTextStyle::new(&text, "Arial", 1.0));
                            }
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.text_style = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }

            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let sup_data = superelevation.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_superelevation_editor(move || {
            let dlg = SuperelevationEditor::new().unwrap();
            let model = Rc::new(VecModel::<SuperelevationRow>::from(
                sup_data
                    .borrow()
                    .iter()
                    .map(|p| SuperelevationRow {
                        station: SharedString::from(format!("{:.2}", p.station)),
                        left: SharedString::from(format!("{:.4}", p.left_slope)),
                        right: SharedString::from(format!("{:.4}", p.right_slope)),
                    })
                    .collect::<Vec<_>>(),
            ));
            dlg.set_rows_model(model.clone().into());
            dlg.set_selected_index(-1);

            let update_design = {
                let sup_data = sup_data.clone();
                let surfaces = surfaces.clone();
                let alignments = alignments.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                let backend_render = backend_render.clone();
                move || {
                    if alignments.borrow().is_empty() {
                        return;
                    }
                    let al = &alignments.borrow()[0];
                    let lane = subassembly::lane(3.5, -0.02);
                    let shoulder = subassembly::shoulder(1.0, -0.04);
                    let subs = subassembly::symmetric_section(&[lane, shoulder]);
                    let tin = corridor::build_design_surface_dynamic(
                        al,
                        &subs,
                        Some(&sup_data.borrow()),
                        10.0,
                    );
                    let verts: Vec<Point3> = tin
                        .vertices
                        .iter()
                        .map(|p| Point3::new(p.x, p.y, p.z))
                        .collect();
                    if surfaces.borrow().is_empty() {
                        backend_render.borrow_mut().add_surface(&verts, &tin.triangles);
                        surfaces.borrow_mut().push(tin);
                    } else {
                        backend_render.borrow_mut().update_surface(0, &verts, &tin.triangles);
                        surfaces.borrow_mut()[0] = tin;
                    }
                    if let Some(app) = weak.upgrade() {
                        if app.get_workspace_mode() == 0 {
                            app.set_workspace_image(render_image());
                        } else {
                            let image = backend_render.borrow_mut().render();
                            app.set_workspace_texture(image);
                        }
                        app.window().request_redraw();
                    }
                }
            };

            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_add_row(move || {
                    sup_data.borrow_mut().push(SuperelevationPoint { station: 0.0, left_slope: 0.0, right_slope: 0.0 });
                    model.push(SuperelevationRow { station: "0.0".into(), left: "0.0000".into(), right: "0.0000".into() });
                    update_design();
                });
            }
            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_remove_row(move |idx| {
                    if idx >= 0 && (idx as usize) < sup_data.borrow().len() {
                        sup_data.borrow_mut().remove(idx as usize);
                        model.remove(idx as usize);
                        update_design();
                    }
                });
            }
            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_edit_station(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(pt) = sup_data.borrow_mut().get_mut(idx as usize) {
                            pt.station = v;
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.station = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                        update_design();
                    }
                });
            }
            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_edit_left(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(pt) = sup_data.borrow_mut().get_mut(idx as usize) {
                            pt.left_slope = v;
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.left = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                        update_design();
                    }
                });
            }
            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_edit_right(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(pt) = sup_data.borrow_mut().get_mut(idx as usize) {
                            pt.right_slope = v;
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.right = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                        update_design();
                    }
                });
            }

            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let grid_settings = grid_settings.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let workspace_crs = workspace_crs.clone();
        app.on_settings(move || {
            let dlg = SettingsDialog::new().unwrap();
            let gs = grid_settings.borrow();
            dlg.set_spacing_value(SharedString::from(format!("{:.1}", gs.spacing)));
            dlg.set_color_r(SharedString::from(gs.color[0].to_string()));
            dlg.set_color_g(SharedString::from(gs.color[1].to_string()));
            dlg.set_color_b(SharedString::from(gs.color[2].to_string()));
            dlg.set_show_grid(gs.visible);
            dlg.set_crs_epsg(SharedString::from(workspace_crs.borrow().to_string()));
            drop(gs);
            let dlg_weak = dlg.as_weak();
            let gs_ref = grid_settings.clone();
            let weak_app = weak.clone();
            let render_image = render_image.clone();
            let backend_render = backend_render.clone();
            let crs_ref = workspace_crs.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let Ok(v) = d.get_spacing_value().parse::<f32>() {
                        gs_ref.borrow_mut().spacing = v;
                    }
                    let r = d.get_color_r().parse::<u8>().unwrap_or(60);
                    let g = d.get_color_g().parse::<u8>().unwrap_or(60);
                    let b = d.get_color_b().parse::<u8>().unwrap_or(60);
                    gs_ref.borrow_mut().color = [r, g, b];
                    gs_ref.borrow_mut().visible = d.get_show_grid();
                    if let Ok(epsg) = d.get_crs_epsg().parse::<u32>() {
                        *crs_ref.borrow_mut() = epsg;
                    }
                    d.hide().unwrap();
                }
                if let Some(app) = weak_app.upgrade() {
                    refresh_workspace(&app, &render_image, &backend_render);
                }
            });
            let cancel_weak = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = cancel_weak.upgrade() {
                    d.hide().unwrap();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let snap_prefs_ref = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_settings(move || {
            let dlg = SnapSettingsDialog::new().unwrap();
            let prefs = snap_prefs_ref.borrow();
            dlg.set_tolerance(SharedString::from(format!("{:.1}", prefs.snap_tolerance)));
            dlg.set_snap_points(prefs.snap_points);
            dlg.set_snap_endpoints(prefs.snap_endpoints);
            dlg.set_snap_midpoints(prefs.snap_midpoints);
            dlg.set_snap_intersections(prefs.snap_intersections);
            dlg.set_snap_nearest(prefs.snap_nearest);
            drop(prefs);
            let dlg_weak = dlg.as_weak();
            let prefs_ref = snap_prefs_ref.clone();
            let cfg_ref = cfg.clone();
            let app_weak = weak.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let Ok(v) = d.get_tolerance().parse::<f32>() {
                        prefs_ref.borrow_mut().snap_tolerance = v;
                        cfg_ref.borrow_mut().snap.snap_tolerance = v;
                    }
                    prefs_ref.borrow_mut().snap_points = d.get_snap_points();
                    cfg_ref.borrow_mut().snap.snap_points = d.get_snap_points();
                    prefs_ref.borrow_mut().snap_endpoints = d.get_snap_endpoints();
                    cfg_ref.borrow_mut().snap.snap_endpoints = d.get_snap_endpoints();
                    prefs_ref.borrow_mut().snap_midpoints = d.get_snap_midpoints();
                    cfg_ref.borrow_mut().snap.snap_midpoints = d.get_snap_midpoints();
                    prefs_ref.borrow_mut().snap_intersections = d.get_snap_intersections();
                    cfg_ref.borrow_mut().snap.snap_intersections = d.get_snap_intersections();
                    prefs_ref.borrow_mut().snap_nearest = d.get_snap_nearest();
                    cfg_ref.borrow_mut().snap.snap_nearest = d.get_snap_nearest();
                    if let Some(a) = app_weak.upgrade() {
                        a.set_snap_points(d.get_snap_points());
                        a.set_snap_endpoints(d.get_snap_endpoints());
                        a.set_snap_midpoints(d.get_snap_midpoints());
                        a.set_snap_intersections(d.get_snap_intersections());
                        a.set_snap_nearest(d.get_snap_nearest());
                        if let Ok(v) = d.get_tolerance().parse::<f32>() {
                            a.set_snap_tolerance(v);
                        }
                    }
                    save_config(&cfg_ref.borrow());
                    d.hide().unwrap();
                }
            });
            let cancel_weak = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = cancel_weak.upgrade() {
                    d.hide().unwrap();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let surfaces = surfaces.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let surface_units = surface_units.clone();
        let surface_styles = surface_styles.clone();
        let surface_descriptions = surface_descriptions.clone();
        app.on_import_landxml_surface(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::landxml::read_landxml_surface(p) {
                        Ok((tin, extras)) => {
                            let verts: Vec<Point3> = tin
                                .vertices
                                .iter()
                                .map(|p| Point3::new(p.x, p.y, p.z))
                                .collect();
                            backend_render
                                .borrow_mut()
                                .add_surface(&verts, &tin.triangles);
                            surfaces.borrow_mut().push(tin);
                            surface_units.borrow_mut().push(extras.units.unwrap_or_default());
                            surface_styles.borrow_mut().push(extras.style.unwrap_or_default());
                            surface_descriptions.borrow_mut().push(extras.description.unwrap_or_default());
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Imported surface"));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_landxml_alignment(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::landxml::read_landxml_alignment(p) {
                        Ok((hal, _)) => {
                            let val = survey_cad::io::landxml::read_landxml_profile(p)
                                .unwrap_or_else(|_| VerticalAlignment::new(vec![(0.0, 0.0), (hal.length(), 0.0)]));
                            alignments.borrow_mut().push(Alignment::new(hal, val));
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Imported alignment"));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
                                )));
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let render_image = render_image.clone();
        let point_style_indices = point_style_indices.clone();
        let drawing_mode = drawing_mode.clone();
        let offset_ref = offset.clone();
        let zoom_ref = zoom.clone();
        let lines_ref = lines.clone();
        let polygons_ref = polygons.clone();
        let arcs_ref = arcs.clone();
        let last_click = last_click.clone();
        let backend_render = backend.clone();
        let macro_playing = macro_playing.clone();
        let macro_recorder = macro_recorder.clone();
        let snap_target = snap_target.clone();
        app.on_workspace_clicked(move |x, y| {
            if *drawing_mode.borrow() != DrawingMode::None {
                if let Some(app) = weak.upgrade() {
                    let size = app.window().size();
                    let mut p = screen_to_workspace(
                        x,
                        y,
                        &offset_ref,
                        &zoom_ref,
                        size.width as f32,
                        size.height as f32,
                    );
                    let zoom_factor = *zoom_ref.borrow();
                    if app.get_snap_to_entities() {
                        let scene = snap::Scene {
                            points: &point_db.borrow(),
                            lines: &lines_ref.borrow(),
                            polygons: &polygons_ref.borrow(),
                            polylines: &polylines.borrow(),
                            arcs: &arcs_ref.borrow(),
                        };
                        let opts = snap::SnapOptions {
                            snap_points: app.get_snap_points(),
                            snap_endpoints: app.get_snap_endpoints(),
                            snap_midpoints: app.get_snap_midpoints(),
                            snap_intersections: app.get_snap_intersections(),
                            snap_nearest: app.get_snap_nearest(),
                        };
                        if let Some(sp) = snap::resolve_snap(
                            p,
                            &scene,
                            app.get_snap_tolerance() as f64 / (zoom_factor as f64),
                            opts,
                        ) {
                            *snap_target.borrow_mut() = Some(sp);
                            p = sp;
                        } else {
                            *snap_target.borrow_mut() = None;
                        }
                    } else {
                        *snap_target.borrow_mut() = None;
                    }
                    if app.get_snap_to_grid() {
                        p.x = p.x.round();
                        p.y = p.y.round();
                    }
                    let mut mode = drawing_mode.borrow_mut();
                    match &mut *mode {
                        DrawingMode::Line { start: Some(s) } => {
                            lines_ref.borrow_mut().push((*s, p));
                            if !macro_playing.borrow().0 {
                                let sx = s.x;
                                let sy = s.y;
                                let px = p.x;
                                let py = p.y;
                                record_macro(
                                    &mut macro_recorder.borrow_mut(),
                                    &format!("line {sx} {sy} {px} {py}"),
                                );
                            }
                            *mode = DrawingMode::None;
                        }
                        DrawingMode::Line { start: None } => {}
                        DrawingMode::Polygon { vertices } => {
                            let now = Instant::now();
                            let double = last_click
                                .borrow()
                                .map(|t| now.duration_since(t).as_millis() < 500)
                                .unwrap_or(false);
                            *last_click.borrow_mut() = Some(now);
                            vertices.push(p);
                            if double && vertices.len() > 2 {
                                polygons_ref.borrow_mut().push(vertices.clone());
                                *mode = DrawingMode::None;
                            }
                        }
                        DrawingMode::ArcCenter {
                            center,
                            radius,
                            start_angle,
                        } => {
                            if let (Some(c), Some(r), Some(sa)) = (*center, *radius, *start_angle) {
                                let ea = (p.y - c.y).atan2(p.x - c.x);
                                let arc = Arc::new(c, r, sa, ea);
                                arcs_ref.borrow_mut().push(arc);
                                *mode = DrawingMode::None;
                            }
                        }
                        DrawingMode::ArcThreePoint { p1, p2 } => {
                            if let (Some(a), Some(b)) = (*p1, *p2) {
                                if let Some(arc) = arc_from_three_points(a, b, p) {
                                    arcs_ref.borrow_mut().push(arc);
                                }
                                *mode = DrawingMode::None;
                            }
                        }
                        DrawingMode::ArcStartEndRadius { start, end, radius } => {
                            if let (Some(s), Some(e)) = (*start, *end) {
                                let r = radius.unwrap_or_else(|| {
                                    ((p.x - s.x).powi(2) + (p.y - s.y).powi(2)).sqrt()
                                });
                                if let Some(arc) = arc_from_start_end_radius(s, e, r, p) {
                                    arcs_ref.borrow_mut().push(arc);
                                }
                                *mode = DrawingMode::None;
                            }
                        }
                        _ => {}
                    }
                    drop(mode);
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            } else if let Some(app) = weak.upgrade() {
                if app.get_workspace_click_mode() {
                    let size = app.window().size();
                    let mut p = screen_to_workspace(
                        x,
                        y,
                        &offset_ref,
                        &zoom_ref,
                        size.width as f32,
                        size.height as f32,
                    );
                    let zoom_factor = *zoom_ref.borrow();
                    if app.get_snap_to_entities() {
                        let scene = snap::Scene {
                            points: &point_db.borrow(),
                            lines: &lines.borrow(),
                            polygons: &polygons.borrow(),
                            polylines: &polylines.borrow(),
                            arcs: &arcs.borrow(),
                        };
                        let opts = snap::SnapOptions {
                            snap_points: app.get_snap_points(),
                            snap_endpoints: app.get_snap_endpoints(),
                            snap_midpoints: app.get_snap_midpoints(),
                            snap_intersections: app.get_snap_intersections(),
                            snap_nearest: app.get_snap_nearest(),
                        };
                        if let Some(sp) = snap::resolve_snap(
                            p,
                            &scene,
                            app.get_snap_tolerance() as f64 / (zoom_factor as f64),
                            opts,
                        ) {
                            *snap_target.borrow_mut() = Some(sp);
                            p = sp;
                        } else {
                            *snap_target.borrow_mut() = None;
                        }
                    } else {
                        *snap_target.borrow_mut() = None;
                    }
                    if app.get_snap_to_grid() {
                        p.x = p.x.round();
                        p.y = p.y.round();
                    }
                    point_db.borrow_mut().push(p);
                    point_style_indices.borrow_mut().push(0);
                    backend_render.borrow_mut().add_point(p.x, p.y, 0.0);
                    if !macro_playing.borrow().0 {
                        let px = p.x;
                        let py = p.y;
                        record_macro(
                            &mut macro_recorder.borrow_mut(),
                            &format!("point {px} {py}"),
                        );
                    }
                    command_stack.borrow_mut().push(Command::RemovePoint {
                        index: point_db.borrow().len() - 1,
                        point: p,
                    });
                    app.set_workspace_click_mode(false);
                    app.set_status(SharedString::from(format!(
                        "Total points: {}",
                        point_db.borrow().len()
                    )));
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                    refresh_workspace(&app, &render_image, &backend_render);
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        let point_style_indices = point_style_indices.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let dimensions = dimensions.clone();
        let selected_dimensions = selected_dimensions.clone();
        let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
        let backend_render = backend.clone();
        app.on_clear_workspace(move || {
            point_db.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            dimensions.borrow_mut().clear();
            point_style_indices.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            surface_units.borrow_mut().clear();
            surface_styles.borrow_mut().clear();
            surface_descriptions.borrow_mut().clear();
            alignments.borrow_mut().clear();
            selected_indices.borrow_mut().clear();
            selected_lines.borrow_mut().clear();
            selected_polygons.borrow_mut().clear();
            selected_polylines.borrow_mut().clear();
            selected_arcs.borrow_mut().clear();
            selected_dimensions.borrow_mut().clear();
            backend_render.borrow_mut().clear();
            refresh_line_style_dialogs();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Cleared workspace"));
                if app.get_workspace_mode() == 0 {
                    app.set_workspace_image(render_image());
                    app.window().request_redraw();
                }
            }
        });
    }

    let backend_render = backend.clone();
    let window_size_rc = window_size.clone();
    app.window()
        .set_rendering_notifier(move |state, _| {
            if let slint::RenderingState::BeforeRendering = state {
                if let Some(app) = weak.upgrade() {
                    let current_size = app.window().size();
                    if *window_size_rc.borrow() != current_size {
                        backend_render
                            .borrow_mut()
                            .resize(current_size.width, current_size.height);
                        *window_size_rc.borrow_mut() = current_size;
                    }
                    let image = backend_render.borrow_mut().render();
                    app.set_workspace_texture(image);
                    app.window().request_redraw();
                }
            }
        })
        .unwrap();

    {
        use slint::{Timer, TimerMode};
        use std::rc::Rc;

        let cursor_feedback = cursor_feedback.clone();
        let weak = app.as_weak();
        let timer = Rc::new(Timer::default());
        let timer_handle = timer.clone();
        timer.start(
            TimerMode::Repeated,
            core::time::Duration::from_millis(16),
            move || {
                if let Some(app) = weak.upgrade() {
                    if let Some(ref mut cf) = *cursor_feedback.borrow_mut() {
                        cf.frame = cf.frame.wrapping_add(1);
                        if cf.frame < 60 {
                            app.window().request_redraw();
                        } else {
                            *cursor_feedback.borrow_mut() = None;
                            timer_handle.stop();
                        }
                    } else {
                        timer_handle.stop();
                    }
                } else {
                    timer_handle.stop();
                }
            },
        );
    }

    app.window().request_redraw();

    app.run()
}
