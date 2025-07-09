use std::cell::RefCell;
use std::rc::Rc;

use rusttype::{point, Font, Scale};
use tiny_skia::{Color, Pixmap};

use survey_cad::alignment::Alignment;
use survey_cad::dtm::Tin;
use survey_cad::geometry::{Arc, LineStyle, LinearDimension, Point, Polyline};
use survey_cad::geometry::point::PointStyle;
use survey_cad::styles::{LineLabelStyle, PointLabelStyle};
use truck_modeling::base::{Point3, Vector3};
use truck_modeling::builder;
use truck_modeling::topology::{Solid, Wire};

use crate::ui_state::{CursorFeedback, DragSelect, Vec2};

pub struct WorkspaceRenderData<'a> {
    pub points: &'a [Point],
    pub lines: &'a [(Point, Point)],
    pub polygons: &'a [Vec<Point>],
    pub polylines: &'a [Polyline],
    pub arcs: &'a [Arc],
    pub dimensions: &'a [LinearDimension],
    pub surfaces: &'a [Tin],
    pub alignments: &'a [Alignment],
}

pub struct RenderState<'a> {
    pub offset: &'a Rc<RefCell<Vec2>>,
    pub zoom: &'a Rc<RefCell<f32>>,
    pub selected: &'a Rc<RefCell<Vec<usize>>>,
    pub selected_lines: &'a Rc<RefCell<Vec<(Point, Point)>>>,
    pub selected_polygons: &'a Rc<RefCell<Vec<usize>>>,
    pub selected_polylines: &'a Rc<RefCell<Vec<usize>>>,
    pub selected_arcs: &'a Rc<RefCell<Vec<usize>>>,
    pub selected_dimensions: &'a Rc<RefCell<Vec<usize>>>,
    pub drag: &'a Rc<RefCell<DragSelect>>,
    pub cursor_feedback: &'a Rc<RefCell<Option<CursorFeedback>>>,
    pub snap_target: &'a Rc<RefCell<Option<Point>>>,
}

pub struct RenderStyles<'a> {
    pub point_styles: &'a [PointStyle],
    pub style_indices: &'a Rc<RefCell<Vec<usize>>>,
    pub line_styles: &'a [LineStyle],
    pub line_style_indices: &'a Rc<RefCell<Vec<usize>>>,
    pub polygon_styles: &'a [survey_cad::styles::PolygonStyle],
    pub polygon_style_indices: &'a Rc<RefCell<Vec<usize>>>,
    pub alignment_style: &'a LineStyle,
    pub show_labels: bool,
    pub label_style: &'a LineLabelStyle,
    pub point_label_style: &'a PointLabelStyle,
    pub show_point_numbers: bool,
}

pub fn draw_text(
    pixmap: &mut Pixmap,
    text: &str,
    font: &Font,
    x: f32,
    y: f32,
    color: Color,
    size: f32,
) {
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

pub fn screen_to_workspace(
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

pub fn workspace_to_screen(
    p: Point,
    offset: &Rc<RefCell<Vec2>>,
    zoom: &Rc<RefCell<f32>>,
    width: f32,
    height: f32,
) -> (f32, f32) {
    let origin_x = width / 2.0;
    let origin_y = height / 2.0;
    let z = *zoom.borrow();
    let off = offset.borrow();
    let sx = (p.x as f32 + off.x) * z + origin_x;
    let sy = origin_y - (p.y as f32 + off.y) * z;
    (sx, sy)
}

pub fn arc_from_three_points(p1: Point, p2: Point, p3: Point) -> Option<Arc> {
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

pub fn arc_from_start_end_radius(start: Point, end: Point, r: f64, orient: Point) -> Option<Arc> {
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

pub fn polyline_to_solid(pl: &Polyline, vector: Vector3) -> Option<Solid> {
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
