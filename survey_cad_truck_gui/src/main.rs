#![allow(unused_variables)]

use i_slint_common::sharedfontdb;
use slint::platform::PointerEventButton;
use slint::{Image, Model, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use survey_cad::alignment::HorizontalAlignment;
use survey_cad::crs::list_known_crs;
use survey_cad::dtm::Tin;
use survey_cad::geometry::point::PointStyle;
use survey_cad::geometry::{
    Arc, Line, LineAnnotation, LineStyle, LineType, Point, PointSymbol, Polyline,
};
use survey_cad::point_database::PointDatabase;
use survey_cad::styles::{LineLabelPosition, LineLabelStyle};

mod truck_backend;
use truck_backend::TruckBackend;

use once_cell::sync::Lazy;
use rusttype::{point, Font, Scale};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

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
    surfaces: &'a [Tin],
    alignments: &'a [HorizontalAlignment],
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
    Arc {
        center: Option<Point>,
        radius: Option<f64>,
        start_angle: Option<f64>,
    },
}

struct RenderState<'a> {
    offset: &'a Rc<RefCell<Vec2>>,
    zoom: &'a Rc<RefCell<f32>>,
    selected: &'a Rc<RefCell<Vec<usize>>>,
    selected_lines: &'a Rc<RefCell<Vec<(Point, Point)>>>,
    drag: &'a Rc<RefCell<DragSelect>>,
    cursor_feedback: &'a Rc<RefCell<Option<CursorFeedback>>>,
}

struct RenderStyles<'a> {
    point_styles: &'a [PointStyle],
    style_indices: &'a Rc<RefCell<Vec<usize>>>,
    line_styles: &'a [LineStyle],
    line_style_indices: &'a Rc<RefCell<Vec<usize>>>,
    show_labels: bool,
    label_style: &'a LineLabelStyle,
}

fn draw_text(pixmap: &mut Pixmap, text: &str, x: f32, y: f32, color: Color, size: f32) {
    let scale = Scale::uniform(size);
    let v_metrics = FONT.v_metrics(scale);
    let mut cursor = x;
    for ch in text.chars() {
        let glyph = FONT
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

fn render_workspace(
    data: &WorkspaceRenderData,
    state: &RenderState,
    styles: &RenderStyles,
    drawing: &DrawingMode,
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
    paint.set_color(Color::from_rgba8(60, 60, 60, 255));
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
    let step = 50.0 * zoom_val;
    let mut x = origin_x;
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
            let text = format!("{:.2} m\n{:.1}\u{00B0}", ann.distance, angle);
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

    for poly in data.polygons {
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
            let stroke = Stroke {
                width: 1.0,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    for pl in data.polylines {
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
            let stroke = Stroke {
                width: 1.0,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    for arc in data.arcs {
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
            let stroke = Stroke {
                width: 1.0,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
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
    for hal in data.alignments {
        for elem in &hal.elements {
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
            DrawingMode::Arc {
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
            _ => {}
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

fn read_line_csv(path: &str) -> std::io::Result<(Point, Point)> {
    let pts = survey_cad::io::read_points_csv(path, None, None)?;
    if pts.len() != 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected exactly two points",
        ));
    }
    Ok((pts[0], pts[1]))
}

fn read_points_list(path: &str) -> std::io::Result<Vec<Point>> {
    survey_cad::io::read_points_csv(path, None, None)
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

fn main() -> Result<(), slint::PlatformError> {
    let backend = Rc::new(RefCell::new(TruckBackend::new(640, 480)));
    // Register bundled font before creating the window. If registration fails
    // we fall back to the system fonts so the application remains usable.
    if let Err(err) = sharedfontdb::register_font_from_memory(FONT_DATA) {
        eprintln!("Failed to register bundled font: {err}. Falling back to system fonts");
    }
    let app = MainWindow::new()?;
    let window_size = Rc::new(RefCell::new(app.window().size()));

    // example data so the 2D workspace has something to draw
    let example_line = Line::new(Point::new(0.0, 0.0), Point::new(50.0, 50.0));
    let point_db = Rc::new(RefCell::new(PointDatabase::new()));
    let lines = Rc::new(RefCell::new(vec![(example_line.start, example_line.end)]));
    let polygons = Rc::new(RefCell::new(Vec::<Vec<Point>>::new()));
    let polylines = Rc::new(RefCell::new(Vec::<Polyline>::new()));
    let arcs = Rc::new(RefCell::new(Vec::<Arc>::new()));
    let surfaces = Rc::new(RefCell::new(Vec::<Tin>::new()));
    let alignments = Rc::new(RefCell::new(Vec::<HorizontalAlignment>::new()));

    let zoom = Rc::new(RefCell::new(1.0_f32));
    let offset = Rc::new(RefCell::new(Vec2::default()));
    let pan_2d_flag = Rc::new(RefCell::new(false));
    let last_pos_2d = Rc::new(RefCell::new((0.0_f64, 0.0_f64)));
    let rotate_flag = Rc::new(RefCell::new(false));
    let pan_flag = Rc::new(RefCell::new(false));
    let last_pos = Rc::new(RefCell::new((0.0_f64, 0.0_f64)));
    let selected_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_lines = Rc::new(RefCell::new(Vec::<(Point, Point)>::new()));
    let drag_select = Rc::new(RefCell::new(DragSelect::default()));
    let cursor_feedback = Rc::new(RefCell::new(None));
    let drawing_mode = Rc::new(RefCell::new(DrawingMode::None));
    let last_click = Rc::new(RefCell::new(None));
    let point_style_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let point_styles = survey_cad::styles::default_point_styles();
    let point_style_names: Vec<SharedString> = point_styles
        .iter()
        .map(|(n, _)| SharedString::from(n.clone()))
        .collect();
    let point_style_values: Vec<PointStyle> = point_styles.iter().map(|(_, s)| *s).collect();

    let line_style_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let line_styles = survey_cad::styles::default_line_styles();
    let line_label_styles = survey_cad::styles::default_line_label_styles();
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
        Rc::new(move || {
            let model = Rc::new(VecModel::from((*style_names).clone()));
            dialogs.borrow_mut().retain(|d| {
                if let Some(dlg) = d.upgrade() {
                    dlg.set_styles_model(model.clone().into());
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
        let alignments = alignments.clone();
        let zoom = zoom.clone();
        let offset = offset.clone();
        let selected_indices = selected_indices.clone();
        let drag_select = drag_select.clone();
        let selected_lines = selected_lines.clone();
        let style_indices = point_style_indices.clone();
        let point_styles = point_style_values.clone();
        let line_styles_vals = line_style_values.clone();
        let line_style_indices = line_style_indices.clone();
        let cursor_feedback = cursor_feedback.clone();
        let drawing_mode = drawing_mode.clone();
        let label_style = line_label_styles[0].1.clone();
        move || {
            let size = app_weak.upgrade().map(|a| a.window().size()).unwrap();
            render_workspace(
                &WorkspaceRenderData {
                    points: &point_db.borrow(),
                    lines: &lines.borrow(),
                    polygons: &polygons.borrow(),
                    polylines: &polylines.borrow(),
                    arcs: &arcs.borrow(),
                    surfaces: &surfaces.borrow(),
                    alignments: &alignments.borrow(),
                },
                &RenderState {
                    offset: &offset,
                    zoom: &zoom,
                    selected: &selected_indices,
                    selected_lines: &selected_lines,
                    drag: &drag_select,
                    cursor_feedback: &cursor_feedback,
                },
                &RenderStyles {
                    point_styles: &point_styles,
                    style_indices: &style_indices,
                    line_styles: &line_styles_vals,
                    line_style_indices: &line_style_indices,
                    show_labels: true,
                    label_style: &label_style,
                },
                &drawing_mode.borrow(),
                size.width,
                size.height,
            )
        }
    };

    // basic CRS list as before
    let crs_entries = list_known_crs();
    let crs_model = Rc::new(VecModel::from(
        crs_entries
            .iter()
            .map(|e| SharedString::from(format!("{} - {}", e.code, e.name)))
            .collect::<Vec<_>>(),
    ));
    app.set_crs_list(crs_model.into());
    app.set_crs_index(0);
    app.set_workspace_mode(1); // start with 3D mode to show truck rendering

    // show length of example line in the status bar so Line import is used
    app.set_status(SharedString::from(format!(
        "Example line length: {:.1}",
        example_line.length()
    )));

    // prepare initial 2D workspace image
    app.set_workspace_image(render_image());
    app.window().request_redraw();

    {
        use slint::{Timer, TimerMode};
        use std::rc::Rc;

        let weak = app.as_weak();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let timer = Rc::new(Timer::default());
        let timer_handle = timer.clone();
        timer.start(
            TimerMode::Repeated,
            core::time::Duration::from_millis(16),
            move || {
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                    } else {
                        let image = backend_render.borrow_mut().render();
                        app.set_workspace_texture(image);
                    }
                    app.window().request_redraw();
                } else {
                    timer_handle.stop();
                }
            },
        );

        use slint::CloseRequestResponse;
        let timer_handle = timer.clone();
        app.window().on_close_requested(move || {
            timer_handle.stop();
            CloseRequestResponse::HideWindow
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
            *drawing_mode.borrow_mut() = DrawingMode::Arc {
                center: None,
                radius: None,
                start_angle: None,
            };
        });
    }

    let weak = app.as_weak();

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
        let zoom = zoom.clone();
        app.on_view_changed(move |mode| {
            if let Some(app) = weak.upgrade() {
                app.set_workspace_mode(mode);
                if mode == 0 {
                    app.set_workspace_image(render_image());
                    app.window().request_redraw();
                    app.set_zoom_level(*zoom.borrow());
                }
            }
        });
    }

    // camera interaction callbacks for the 3D workspace
    {
        let rotate_flag = rotate_flag.clone();
        let last_pos = last_pos.clone();
        app.on_workspace_left_pressed(move |x, y| {
            *rotate_flag.borrow_mut() = true;
            *last_pos.borrow_mut() = (x as f64, y as f64);
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
        let arcs_ref = arcs.clone();
        let render_image = render_image.clone();
        let weak = app.as_weak();
        app.on_workspace_pointer_pressed(move |x, y, ev| {
            if *drawing_mode.borrow() != DrawingMode::None {
                if ev.button == PointerEventButton::Left {
                    if let Some(app) = weak.upgrade() {
                        let size = app.window().size();
                        let p = screen_to_workspace(
                            x,
                            y,
                            &offset,
                            &zoom,
                            size.width as f32,
                            size.height as f32,
                        );
                        match &mut *drawing_mode.borrow_mut() {
                            DrawingMode::Line { start } => {
                                if start.is_none() {
                                    *start = Some(p);
                                } else if let Some(s) = start.take() {
                                    lines_ref.borrow_mut().push((s, p));
                                    *drawing_mode.borrow_mut() = DrawingMode::None;
                                } else {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(
                                            "No start point, line cancelled",
                                        ));
                                    }
                                    *drawing_mode.borrow_mut() = DrawingMode::None;
                                    return;
                                }
                            }
                            DrawingMode::Polygon { vertices } => {
                                vertices.push(p);
                            }
                            DrawingMode::Arc {
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
                                    arcs_ref.borrow_mut().push(Arc::new(c, r, sa, ea));
                                    *drawing_mode.borrow_mut() = DrawingMode::None;
                                }
                            }
                            _ => {}
                        }
                        if app.get_workspace_mode() == 0 {
                            app.set_workspace_image(render_image());
                            app.window().request_redraw();
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
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let offset = offset.clone();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        let cursor_feedback = cursor_feedback.clone();
        let weak = app.as_weak();
        app.on_workspace_pointer_released(move || {
            *rotate_flag.borrow_mut() = false;
            *pan_flag.borrow_mut() = false;
            *pan_2d_flag.borrow_mut() = false;
            *cursor_feedback.borrow_mut() = None;

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
            }
        });
    }

    {
        let backend = backend.clone();
        let rotate_flag = rotate_flag.clone();
        let pan_flag = pan_flag.clone();
        let last_pos = last_pos.clone();
        let pan_2d_flag = pan_2d_flag.clone();
        let last_pos_2d = last_pos_2d.clone();
        let offset = offset.clone();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        let drag_select = drag_select.clone();
        let cursor_feedback = cursor_feedback.clone();
        let weak = app.as_weak();
        app.on_workspace_mouse_moved(move |x, y| {
            let mut last = last_pos.borrow_mut();
            let dx = x as f64 - last.0;
            let dy = y as f64 - last.1;
            *last = (x as f64, y as f64);
            if *rotate_flag.borrow() {
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
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
        app.on_new_project(move || {
            point_db.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            alignments.borrow_mut().clear();
            selected_indices.borrow_mut().clear();
            selected_lines.borrow_mut().clear();
            refresh_line_style_dialogs();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("New project created"));
                if app.get_workspace_mode() == 0 {
                    app.set_workspace_image(render_image());
                    app.window().request_redraw();
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let render_image = render_image.clone();
        app.on_open_project(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    let (result, len) = {
                        let mut db_ref = point_db.borrow_mut();
                        let res =
                            survey_cad::io::read_point_database_csv(p, &mut db_ref, None, None);
                        let len = db_ref.len();
                        (res, len)
                    };
                    match result {
                        Ok(()) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Loaded {} points",
                                    len
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
                                    "Failed to open: {}",
                                    e
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
        app.on_save_project(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    if let Err(e) =
                        survey_cad::io::write_points_csv(p, &point_db.borrow(), None, None)
                    {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to save: {}", e)));
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
        app.on_add_line(move || {
            let line_style_indices = line_style_indices.clone();
            let dlg = AddLineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let lines = lines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let line_style_indices = line_style_indices.clone();
                let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_line_csv(p) {
                                Ok(l) => {
                                    lines.borrow_mut().push(l);
                                    line_style_indices.borrow_mut().push(0);
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
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {}",
                                            e
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
                                    line_style_indices.borrow_mut().push(0);
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
        app.on_add_point(move || {
            let dlg = AddPointDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let point_db = point_db.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let point_style_indices = point_style_indices.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match survey_cad::io::read_points_csv(p, None, None) {
                                Ok(pts) => {
                                    let len = {
                                        let mut db = point_db.borrow_mut();
                                        db.clear();
                                        db.extend(pts);
                                        point_style_indices.borrow_mut().clear();
                                        point_style_indices
                                            .borrow_mut()
                                            .extend(std::iter::repeat_n(0, db.len()));
                                        db.len()
                                    };
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Loaded {} points",
                                            len
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
                                            "Failed to open: {}",
                                            e
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
                        key_dlg.on_accept(move || {
                            if let Some(dlg) = key_weak2.upgrade() {
                                if let (Ok(x), Ok(y)) = (
                                    dlg.get_x_value().parse::<f64>(),
                                    dlg.get_y_value().parse::<f64>(),
                                ) {
                                    point_db.borrow_mut().push(Point::new(x, y));
                                    psi.borrow_mut().push(0);
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
        app.on_add_polygon(move || {
            let dlg = AddPolygonDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let polygons = polygons.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_points_list(p) {
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
                                            "Failed to open: {}",
                                            e
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
                                    model.push(SharedString::from(format!("{:.3},{:.3}", x, y)));
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
        app.on_add_polyline(move || {
            let dlg = AddPolylineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let polylines = polylines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_points_list(p) {
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
                                            "Failed to open: {}",
                                            e
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
                                    model.push(SharedString::from(format!("{:.3},{:.3}", x, y)));
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
                                            "Failed to open: {}",
                                            e
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
            let mut verts: Vec<Point> = selected_indices
                .borrow()
                .iter()
                .filter_map(|&i| point_db.borrow().get(i).copied())
                .collect();
            let mut line_chain: Vec<Line> = selected_lines
                .borrow()
                .iter()
                .map(|(s, e)| Line::new(*s, *e))
                .collect();
            if !line_chain.is_empty() {
                let mut chain = Vec::new();
                let current = line_chain.pop().unwrap();
                chain.push(current.start);
                chain.push(current.end);
                let mut last = current.end;
                while !line_chain.is_empty() {
                    if let Some(pos) = line_chain
                        .iter()
                        .position(|l| l.start == last || l.end == last)
                    {
                        let l = line_chain.remove(pos);
                        if l.start == last {
                            chain.push(l.end);
                            last = l.end;
                        } else {
                            chain.push(l.start);
                            last = l.start;
                        }
                    } else {
                        break;
                    }
                }
                verts.extend(chain);
            }
            if verts.len() >= 3 {
                polygons.borrow_mut().push(verts);
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
                            app.set_status(SharedString::from(format!("Distance: {:.3}", dist)));
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
        app.on_traverse_area(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                if let (Some(p), Some(app)) = (path.to_str(), weak.upgrade()) {
                    match survey_cad::io::read_points_csv(p, None, None) {
                        Ok(pts) => {
                            let trav = survey_cad::surveying::Traverse::new(pts);
                            app.set_status(SharedString::from(format!("Area: {:.3}", trav.area())));
                        }
                        Err(e) => {
                            app.set_status(SharedString::from(format!("Failed: {}", e)));
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
                            app.set_status(SharedString::from(format!("Elevation: {:.3}", elev)));
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
                        let hal = &aligns[0];
                        let len = hal.length();
                        let val = survey_cad::alignment::VerticalAlignment::new(vec![
                            (0.0, 0.0),
                            (len, 0.0),
                        ]);
                        let al = survey_cad::alignment::Alignment::new(hal.clone(), val);
                        Some(survey_cad::corridor::corridor_volume(
                            design, ground, &al, width, interval, step,
                        ))
                    })();
                    if let Some(app) = weak2.upgrade() {
                        if let Some(vol) = res {
                            app.set_status(SharedString::from(format!("Volume: {:.3}", vol)));
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
        let render_image = render_image.clone();
        app.on_import_geojson(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("GeoJSON", &["geojson", "json"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::read_points_geojson(p, None, None) {
                        Ok(pts) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts);
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {} points",
                                    len
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
                                    "Failed to import: {}",
                                    e
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
                                    "Imported {} points",
                                    len
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
                                    "Failed to import: {}",
                                    e
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
                                    "Imported {} points",
                                    len
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
                                    "Failed to import: {}",
                                    e
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
        app.on_import_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    match survey_cad::io::shp::read_points_shp(p) {
                        Ok((pts, _)) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts);
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {} points",
                                    len
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
                                    "Failed to import: {}",
                                    e
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
                                    "Imported {} points",
                                    len
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
                                    "Failed to import: {}",
                                    e
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
                                    "Imported {} points",
                                    len
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
                                    "Failed to import: {}",
                                    e
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
                            app.set_status(SharedString::from(format!("Failed to export: {}", e)));
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
                            app.set_status(SharedString::from(format!("Failed to export: {}", e)));
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
                            app.set_status(SharedString::from(format!("Failed to export: {}", e)));
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
                            app.set_status(SharedString::from(format!("Failed to export: {}", e)));
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
                                    "Failed to export: {}",
                                    e
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
                                    "Failed to export: {}",
                                    e
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
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        let point_style_names = point_style_names.clone();
        let render_image_pm = render_image.clone();
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
                            number: SharedString::from(format!("{}", i + 1)),
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
            dlg.set_groups_model(
                Rc::new(VecModel::<SharedString>::from(
                    point_db
                        .borrow()
                        .iter_groups()
                        .map(|(_, g)| SharedString::from(g.name.clone()))
                        .collect::<Vec<_>>(),
                ))
                .into(),
            );
            dlg.set_styles_model(Rc::new(VecModel::from(point_style_names.clone())).into());
            dlg.set_selected_index(-1);

            {
                let model = model.clone();
                let point_db = point_db.clone();
                dlg.on_edit_x(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx as usize) {
                            p.x = v;
                            if let Some(row) = model.row_data(idx as usize) {
                                let mut r = row.clone();
                                r.x = SharedString::from(format!("{:.3}", v));
                                model.set_row_data(idx as usize, r);
                            }
                        }
                    }
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                dlg.on_edit_y(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx as usize) {
                            p.y = v;
                            if let Some(row) = model.row_data(idx as usize) {
                                let mut r = row.clone();
                                r.y = SharedString::from(format!("{:.3}", v));
                                model.set_row_data(idx as usize, r);
                            }
                        }
                    }
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                let psi = point_style_indices.clone();
                dlg.on_add_point(move || {
                    point_db.borrow_mut().push(Point::new(0.0, 0.0));
                    psi.borrow_mut().push(0);
                    let idx = point_db.borrow().len();
                    model.push(PointRow {
                        number: SharedString::from(format!("{}", idx)),
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
                dlg.on_remove_point(move |idx| {
                    if idx >= 0 && (idx as usize) < point_db.borrow().len() {
                        point_db.borrow_mut().remove(idx as usize);
                        psi.borrow_mut().remove(idx as usize);
                        model.remove(idx as usize);
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
            let model = Rc::new(VecModel::<LineRow>::from(
                lines
                    .borrow()
                    .iter()
                    .enumerate()
                    .map(|(i, (s, e))| {
                        if line_style_indices.borrow().len() <= i {
                            line_style_indices.borrow_mut().push(0);
                        }
                        LineRow {
                            start: SharedString::from(format!("{:.2},{:.2}", s.x, s.y)),
                            end: SharedString::from(format!("{:.2},{:.2}", e.x, e.y)),
                            style_index: line_style_indices.borrow()[i] as i32,
                        }
                    })
                    .collect::<Vec<_>>(),
            ));
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
                        if indices.borrow().len() > idx as usize {
                            indices.borrow_mut()[idx as usize] = style_idx as usize;
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
        let weak = app.as_weak();
        let surfaces = surfaces.clone();
        let render_image = render_image.clone();
        app.on_import_landxml_surface(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::landxml::read_landxml_surface(p) {
                        Ok(tin) => {
                            surfaces.borrow_mut().push(tin);
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Imported surface"));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                    app.window().request_redraw();
                                }
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {}",
                                    e
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
        app.on_import_landxml_alignment(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::landxml::read_landxml_alignment(p) {
                        Ok(al) => {
                            alignments.borrow_mut().push(al);
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Imported alignment"));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
                                    app.window().request_redraw();
                                }
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {}",
                                    e
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
        app.on_workspace_clicked(move |x, y| {
            if *drawing_mode.borrow() != DrawingMode::None {
                if let Some(app) = weak.upgrade() {
                    let size = app.window().size();
                    let p = screen_to_workspace(
                        x,
                        y,
                        &offset_ref,
                        &zoom_ref,
                        size.width as f32,
                        size.height as f32,
                    );
                    match &mut *drawing_mode.borrow_mut() {
                        DrawingMode::Line { start: Some(s) } => {
                            lines_ref.borrow_mut().push((*s, p));
                            *drawing_mode.borrow_mut() = DrawingMode::None;
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
                                *drawing_mode.borrow_mut() = DrawingMode::None;
                            }
                        }
                        DrawingMode::Arc {
                            center,
                            radius,
                            start_angle,
                        } => {
                            if let (Some(c), Some(r), Some(sa)) = (*center, *radius, *start_angle) {
                                let ea = (p.y - c.y).atan2(p.x - c.x);
                                arcs_ref.borrow_mut().push(Arc::new(c, r, sa, ea));
                                *drawing_mode.borrow_mut() = DrawingMode::None;
                            }
                        }
                        _ => {}
                    }
                    if app.get_workspace_mode() == 0 {
                        app.set_workspace_image(render_image());
                        app.window().request_redraw();
                    }
                }
            } else if let Some(app) = weak.upgrade() {
                if app.get_workspace_click_mode() {
                    const WIDTH: f64 = 600.0;
                    const HEIGHT: f64 = 400.0;
                    let mut p = Point::new(x as f64 - WIDTH / 2.0, HEIGHT / 2.0 - y as f64);
                    if app.get_snap_to_grid() {
                        p.x = p.x.round();
                        p.y = p.y.round();
                    }
                    if app.get_snap_to_entities() {
                        let mut ents: Vec<survey_cad::io::DxfEntity> = Vec::new();
                        for pt in point_db.borrow().iter() {
                            ents.push(survey_cad::io::DxfEntity::Point {
                                point: *pt,
                                layer: None,
                            });
                        }
                        for (s, e) in lines.borrow().iter() {
                            ents.push(survey_cad::io::DxfEntity::Line {
                                line: Line::new(*s, *e),
                                layer: None,
                            });
                        }
                        for poly in polygons.borrow().iter() {
                            ents.push(survey_cad::io::DxfEntity::Polyline {
                                polyline: Polyline::new(poly.clone()),
                                layer: None,
                            });
                        }
                        for pl in polylines.borrow().iter() {
                            ents.push(survey_cad::io::DxfEntity::Polyline {
                                polyline: pl.clone(),
                                layer: None,
                            });
                        }
                        for arc in arcs.borrow().iter() {
                            ents.push(survey_cad::io::DxfEntity::Arc {
                                arc: *arc,
                                layer: None,
                            });
                        }
                        if let Some(sp) = survey_cad::snap::snap_point(p, &ents, 5.0) {
                            p = sp;
                        }
                    }
                    point_db.borrow_mut().push(p);
                    point_style_indices.borrow_mut().push(0);
                    app.set_workspace_click_mode(false);
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
        let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
        app.on_clear_workspace(move || {
            point_db.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            point_style_indices.borrow_mut().clear();
            line_style_indices.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            alignments.borrow_mut().clear();
            selected_indices.borrow_mut().clear();
            selected_lines.borrow_mut().clear();
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
