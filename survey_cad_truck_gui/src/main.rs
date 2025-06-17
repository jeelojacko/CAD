#![allow(unused_variables)]

use slint::{Image, SharedString, VecModel};
use std::rc::Rc;

use survey_cad::alignment::HorizontalAlignment;
use survey_cad::crs::list_known_crs;
use survey_cad::dtm::Tin;
use survey_cad::geometry::{Arc, Line, Point, Polyline};

mod truck_backend;
use truck_backend::TruckBackend;

use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

slint::include_modules!();

struct WorkspaceRenderData<'a> {
    points: &'a [Point],
    lines: &'a [(Point, Point)],
    polygons: &'a [Vec<Point>],
    polylines: &'a [Polyline],
    arcs: &'a [Arc],
    surfaces: &'a [Tin],
    alignments: &'a [HorizontalAlignment],
}

fn render_workspace(data: &WorkspaceRenderData, zoom: f32) -> Image {
    const WIDTH: u32 = 600;
    const HEIGHT: u32 = 400;
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(Color::from_rgba8(32, 32, 32, 255));
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(60, 60, 60, 255));
    paint.anti_alias = true;
    let grid_stroke = Stroke {
        width: 1.0,
        ..Stroke::default()
    };
    let origin_x = WIDTH as f32 / 2.0;
    let origin_y = HEIGHT as f32 / 2.0;
    let tx = |x: f32| x * zoom + origin_x;
    let ty = |y: f32| origin_y - y * zoom;
    let step = 50.0 * zoom;
    let mut x = origin_x;
    while x < WIDTH as f32 {
        let mut pb = PathBuilder::new();
        pb.move_to(x, 0.0);
        pb.line_to(x, HEIGHT as f32);
        if let Some(p) = pb.finish() {
            pixmap.stroke_path(&p, &paint, &grid_stroke, Transform::identity(), None);
        }
        x += step;
    }
    x = origin_x - step;
    while x >= 0.0 {
        let mut pb = PathBuilder::new();
        pb.move_to(x, 0.0);
        pb.line_to(x, HEIGHT as f32);
        if let Some(p) = pb.finish() {
            pixmap.stroke_path(&p, &paint, &grid_stroke, Transform::identity(), None);
        }
        x -= step;
    }
    let mut y = origin_y;
    while y < HEIGHT as f32 {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, y);
        pb.line_to(WIDTH as f32, y);
        if let Some(p) = pb.finish() {
            pixmap.stroke_path(&p, &paint, &grid_stroke, Transform::identity(), None);
        }
        y += step;
    }
    y = origin_y - step;
    while y >= 0.0 {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, y);
        pb.line_to(WIDTH as f32, y);
        if let Some(p) = pb.finish() {
            pixmap.stroke_path(&p, &paint, &grid_stroke, Transform::identity(), None);
        }
        y -= step;
    }
    paint.set_color(Color::from_rgba8(90, 90, 90, 255));
    let mut pb = PathBuilder::new();
    pb.move_to(origin_x, 0.0);
    pb.line_to(origin_x, HEIGHT as f32);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &grid_stroke, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, origin_y);
    pb.line_to(WIDTH as f32, origin_y);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &grid_stroke, Transform::identity(), None);
    }
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        pixmap.data(),
        WIDTH,
        HEIGHT,
    );
    Image::from_rgba8_premultiplied(buffer)
}

fn main() -> Result<(), slint::PlatformError> {
    let mut backend = TruckBackend::new(640, 480);
    let app = MainWindow::new()?;

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

    let weak = app.as_weak();

    app.window()
        .set_rendering_notifier(move |state, _| {
            if let slint::RenderingState::BeforeRendering = state {
                if let Some(app) = weak.upgrade() {
                    let image = backend.render();
                    app.set_workspace_texture(image);
                    app.window().request_redraw();
                }
            }
        })
        .unwrap();

    app.run()
}
