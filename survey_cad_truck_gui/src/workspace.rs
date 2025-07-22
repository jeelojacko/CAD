use std::cell::RefCell;
use std::rc::Rc;

use log::error;

use slint::{ComponentHandle, Image};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use survey_cad::geometry::{Line, LineAnnotation, LineType};
use survey_cad::io::project::GridSettings;
use survey_cad::styles::{format_dms, HatchPattern, LineLabelPosition};

use crate::error::GuiError;
use crate::render::{draw_text, RenderState, RenderStyles, WorkspaceRenderData};
use crate::truck_backend::TruckBackend;
use crate::ui_state::DrawingMode;
use crate::{MainWindow, FONT};

pub fn render_workspace(
    data: &WorkspaceRenderData,
    state: &RenderState,
    styles: &RenderStyles,
    drawing: &DrawingMode,
    grid: &GridSettings,
    width: u32,
    height: u32,
) -> Result<Image, GuiError> {
    if width == 0 || height == 0 {
        return Ok(Image::default());
    }
    let mut pixmap = state.pixmap.borrow_mut();
    if pixmap.width() != width || pixmap.height() != height {
        *pixmap = Pixmap::new(width, height).ok_or_else(|| {
            error!("Failed to create pixmap {width}x{height}");
            GuiError::from("failed to create pixmap")
        })?;
    } else {
        pixmap.fill(Color::from_rgba8(32, 32, 32, 255));
    }
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
            LineType::Dashed => stroke.dash = StrokeDash::new(vec![10.0, 10.0], 0.0),
            LineType::Dotted => stroke.dash = StrokeDash::new(vec![2.0, 6.0], 0.0),
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
        let Some(first) = poly.first() else { continue };
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
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );

            if pstyle.hatch_pattern != HatchPattern::None {
                paint.set_color(Color::from_rgba8(
                    pstyle.hatch_color[0],
                    pstyle.hatch_color[1],
                    pstyle.hatch_color[2],
                    255,
                ));
                let stroke = Stroke {
                    width: 1.0,
                    ..Stroke::default()
                };
                {
                    let bb = path.bounds();
                    let step = 10.0;
                    if matches!(
                        pstyle.hatch_pattern,
                        HatchPattern::Cross | HatchPattern::Grid
                    ) {
                        let mut x = bb.left();
                        while x <= bb.right() {
                            let mut pb = PathBuilder::new();
                            pb.move_to(x, bb.top());
                            pb.line_to(x, bb.bottom());
                            if let Some(p) = pb.finish() {
                                pixmap.stroke_path(
                                    &p,
                                    &paint,
                                    &stroke,
                                    Transform::identity(),
                                    None,
                                );
                            }
                            x += step;
                        }
                    }
                    if matches!(
                        pstyle.hatch_pattern,
                        HatchPattern::ForwardDiagonal
                            | HatchPattern::BackwardDiagonal
                            | HatchPattern::Cross
                    ) {
                        let mut x = bb.left();
                        while x <= bb.right() {
                            let mut pb = PathBuilder::new();
                            pb.move_to(x, bb.top());
                            pb.line_to(x + bb.height(), bb.bottom());
                            if let Some(p) = pb.finish() {
                                pixmap.stroke_path(
                                    &p,
                                    &paint,
                                    &stroke,
                                    Transform::identity(),
                                    None,
                                );
                            }
                            x += step;
                        }
                    }
                }
            }
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

    if let Some(sp) = *state.snap_target.borrow() {
        paint.set_color(Color::from_rgba8(255, 255, 0, 255));
        let sx = tx(sp.x as f32);
        let sy = ty(sp.y as f32);
        let mut pb = PathBuilder::new();
        pb.move_to(sx - 4.0, sy);
        pb.line_to(sx + 4.0, sy);
        pb.move_to(sx, sy - 4.0);
        pb.line_to(sx, sy + 4.0);
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
    Ok(Image::from_rgba8_premultiplied(buffer))
}

pub fn refresh_workspace(
    app: &MainWindow,
    render_image: &dyn Fn() -> Result<Image, GuiError>,
    backend_render: &Rc<RefCell<TruckBackend>>,
) {
    if app.get_workspace_mode() == 0 {
        match render_image() {
            Ok(img) => app.set_workspace_image(img),
            Err(e) => {
                eprintln!("Failed to render workspace: {e}");
                app.set_workspace_image(Image::default());
            }
        }
    } else {
        let image = backend_render.borrow_mut().render();
        app.set_workspace_texture(image);
    }
    app.window().request_redraw();
}

pub fn set_workspace_image_result(
    app: &MainWindow,
    render_image: &dyn Fn() -> Result<Image, GuiError>,
) {
    match render_image() {
        Ok(img) => app.set_workspace_image(img),
        Err(e) => {
            eprintln!("Failed to render workspace: {e}");
            app.set_workspace_image(Image::default());
        }
    }
}
