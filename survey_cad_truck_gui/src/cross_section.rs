use slint::Image;
use log::error;

use crate::render::draw_text;
use crate::FONT;

use crate::error::GuiError;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

use survey_cad::alignment::{VerticalAlignment, VerticalElement};
use survey_cad::corridor;
use survey_cad::subassembly;
use survey_cad::geometry::Point3 as ScPoint3;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Lane {
    pub width: f64,
    pub slope: f64,
}

#[allow(dead_code)]
impl Lane {
    pub fn to_subassembly(self) -> corridor::Subassembly {
        subassembly::lane(self.width, self.slope)
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Shoulder {
    pub width: f64,
    pub slope: f64,
}

#[allow(dead_code)]
impl Shoulder {
    pub fn to_subassembly(self) -> corridor::Subassembly {
        subassembly::shoulder(self.width, self.slope)
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Ditch {
    pub depth: f64,
    pub bottom_width: f64,
    pub side_slope: f64,
}

#[allow(dead_code)]
impl Ditch {
    pub fn to_subassembly(self) -> corridor::Subassembly {
        subassembly::ditch(self.depth, self.bottom_width, self.side_slope)
    }
}

/// Parameters for mapping section coordinates to screen coordinates.
pub struct SectionParams {
    pub dir: (f64, f64),
    pub center: ScPoint3,
    pub scale: f32,
    pub ox: f32,
    pub oy: f32,
}

pub fn render_cross_section(
    section: &corridor::CrossSection,
    width: u32,
    height: u32,
    grid_x: f32,
    grid_y: f32,
) -> Result<Image, GuiError> {
    if width == 0 || height == 0 {
        return Ok(Image::default());
    }
    let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
        error!("Failed to create pixmap {width}x{height}");
        GuiError::from("failed to create pixmap")
    })?;
    pixmap.fill(Color::from_rgba8(32, 32, 32, 255));
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(0, 255, 0, 255));
    paint.anti_alias = true;

    if section.points.len() >= 2 {
        let Some(first) = section.points.first() else {
            return Ok(Image::default());
        };
        let Some(last) = section.points.last() else {
            return Ok(Image::default());
        };
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

        // draw grid lines
        let mut grid_paint = Paint::default();
        grid_paint.set_color(Color::from_rgba8(60, 60, 60, 255));
        grid_paint.anti_alias = true;
        let grid_stroke = Stroke { width: 1.0, ..Stroke::default() };

        let start_x = (min_x / grid_x).floor() as i32;
        let end_x = (max_x / grid_x).ceil() as i32;
        for i in start_x..=end_x {
            let gx = i as f32 * grid_x;
            let px = ox + gx * scale;
            let mut pb = PathBuilder::new();
            pb.move_to(px, 0.0);
            pb.line_to(px, height as f32);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &grid_paint, &grid_stroke, Transform::identity(), None);
            }
        }

        let start_y = (min_y / grid_y).floor() as i32;
        let end_y = (max_y / grid_y).ceil() as i32;
        for i in start_y..=end_y {
            let gy = i as f32 * grid_y;
            let py = oy - gy * scale;
            let mut pb = PathBuilder::new();
            pb.move_to(0.0, py);
            pb.line_to(width as f32, py);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &grid_paint, &grid_stroke, Transform::identity(), None);
            }
        }

        // axis lines
        grid_paint.set_color(Color::from_rgba8(90, 90, 90, 255));
        let mut pb = PathBuilder::new();
        pb.move_to(ox, 0.0);
        pb.line_to(ox, height as f32);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &grid_paint, &grid_stroke, Transform::identity(), None);
        }
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, oy);
        pb.line_to(width as f32, oy);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &grid_paint, &grid_stroke, Transform::identity(), None);
        }

        // axis ticks and labels
        for i in start_x..=end_x {
            let gx = i as f32 * grid_x;
            let px = ox + gx * scale;
            let mut pb = PathBuilder::new();
            pb.move_to(px, oy - 5.0);
            pb.line_to(px, oy + 5.0);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &grid_paint, &grid_stroke, Transform::identity(), None);
            }
            let lbl = format!("{:.2}", gx);
            draw_text(
                &mut pixmap,
                &lbl,
                &FONT,
                px + 2.0,
                oy + 7.0,
                Color::from_rgba8(200, 200, 200, 255),
                12.0,
            );
        }
        for i in start_y..=end_y {
            let gy = i as f32 * grid_y;
            let py = oy - gy * scale;
            let mut pb = PathBuilder::new();
            pb.move_to(ox - 5.0, py);
            pb.line_to(ox + 5.0, py);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &grid_paint, &grid_stroke, Transform::identity(), None);
            }
            let lbl = format!("{:.2}", gy);
            draw_text(
                &mut pixmap,
                &lbl,
                &FONT,
                ox + 7.0,
                py - 6.0,
                Color::from_rgba8(200, 200, 200, 255),
                12.0,
            );
        }

        // section polyline
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
    Ok(Image::from_rgba8_premultiplied(buffer))
}

pub fn calc_section_params(
    section: &corridor::CrossSection,
    width: f32,
    height: f32,
) -> Option<SectionParams> {
    if section.points.len() < 2 {
        return None;
    }
    let first = section.points.first()?;
    let last = section.points.last()?;
    let dx = last.x - first.x;
    let dy = last.y - first.y;
    let len = (dx * dx + dy * dy).sqrt();
    let dir = if len.abs() < f64::EPSILON {
        (1.0, 0.0)
    } else {
        (dx / len, dy / len)
    };
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
    Some(SectionParams {
        dir,
        center,
        scale,
        ox,
        oy,
    })
}

pub fn screen_to_world(
    section: &corridor::CrossSection,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Option<ScPoint3> {
    let params = calc_section_params(section, width, height)?;
    let off = (x - params.ox) / params.scale;
    let elev = (params.oy - y) / params.scale;
    Some(ScPoint3::new(
        params.center.x + off as f64 * params.dir.0,
        params.center.y + off as f64 * params.dir.1,
        params.center.z + elev as f64,
    ))
}

pub fn nearest_point(
    section: &corridor::CrossSection,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Option<usize> {
    let params = calc_section_params(section, width, height)?;
    let mut best = None;
    let mut best_dist = f32::MAX;
    for (i, p) in section.points.iter().enumerate() {
        let off = ((p.x - params.center.x) * params.dir.0 + (p.y - params.center.y) * params.dir.1)
            as f32;
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
    if best_dist.sqrt() <= 10.0 {
        best
    } else {
        None
    }
}

pub fn handle_positions(
    section: &corridor::CrossSection,
    width: f32,
    height: f32,
) -> Vec<(f32, f32)> {
    let Some(params) = calc_section_params(section, width, height) else {
        return Vec::new();
    };
    section
        .points
        .iter()
        .map(|p| {
            let off = ((p.x - params.center.x) * params.dir.0
                + (p.y - params.center.y) * params.dir.1) as f32;
            let elev = (p.z - params.center.z) as f32;
            (
                params.ox + off * params.scale,
                params.oy - elev * params.scale,
            )
        })
        .collect()
}

pub fn grade_at(profile: &VerticalAlignment, station: f64) -> Option<f64> {
    for elem in &profile.elements {
        match *elem {
            VerticalElement::Grade {
                start_station,
                end_station,
                start_elev,
                end_elev,
            } => {
                if station >= start_station && station <= end_station {
                    if (end_station - start_station).abs() < f64::EPSILON {
                        return Some(0.0);
                    }
                    return Some((end_elev - start_elev) / (end_station - start_station));
                }
            }
            VerticalElement::Parabola {
                start_station,
                end_station,
                start_grade,
                end_grade,
                ..
            } => {
                if station >= start_station && station <= end_station {
                    let t = (station - start_station) / (end_station - start_station);
                    return Some(start_grade + (end_grade - start_grade) * t);
                }
            }
        }
    }
    None
}
