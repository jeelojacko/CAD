#![allow(unused_variables)]

use slint::{Image, SharedString, VecModel};
use std::cell::RefCell;
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

    paint.set_color(Color::from_rgba8(255, 0, 0, 255));
    let stroke = Stroke {
        width: 2.0,
        ..Stroke::default()
    };

    for (s, e) in data.lines {
        let mut pb = PathBuilder::new();
        pb.move_to(tx(s.x as f32), ty(s.y as f32));
        pb.line_to(tx(e.x as f32), ty(e.y as f32));
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
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
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
                survey_cad::alignment::HorizontalElement::Curve { arc } => {
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
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
            }
        }
    }

    paint.set_color(Color::from_rgba8(0, 255, 0, 255));
    for p in data.points {
        if let Some(circle) = PathBuilder::from_circle(tx(p.x as f32), ty(p.y as f32), 3.0) {
            pixmap.fill_path(
                &circle,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
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

    // example data so the 2D workspace has something to draw
    let example_line = Line::new(Point::new(0.0, 0.0), Point::new(50.0, 50.0));
    let points = Rc::new(RefCell::new(Vec::<Point>::new()));
    let lines = Rc::new(RefCell::new(vec![(example_line.start, example_line.end)]));
    let polygons = Rc::new(RefCell::new(Vec::<Vec<Point>>::new()));
    let polylines = Rc::new(RefCell::new(Vec::<Polyline>::new()));
    let arcs = Rc::new(RefCell::new(Vec::<Arc>::new()));
    let surfaces = Rc::new(RefCell::new(Vec::<Tin>::new()));
    let alignments = Rc::new(RefCell::new(Vec::<HorizontalAlignment>::new()));

    let zoom = Rc::new(RefCell::new(1.0_f32));

    let render_image = {
        let points = points.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let zoom = zoom.clone();
        move || {
            render_workspace(
                &WorkspaceRenderData {
                    points: &points.borrow(),
                    lines: &lines.borrow(),
                    polygons: &polygons.borrow(),
                    polylines: &polylines.borrow(),
                    arcs: &arcs.borrow(),
                    surfaces: &surfaces.borrow(),
                    alignments: &alignments.borrow(),
                },
                *zoom.borrow(),
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
                    app.set_zoom_level(*zoom.borrow());
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let points = points.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        app.on_new_project(move || {
            points.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            alignments.borrow_mut().clear();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("New project created"));
                if app.get_workspace_mode() == 0 {
                    app.set_workspace_image(render_image());
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let points = points.clone();
        let render_image = render_image.clone();
        app.on_open_project(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::read_points_csv(p, None, None) {
                        Ok(pts) => {
                            *points.borrow_mut() = pts;
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Loaded {} points",
                                    points.borrow().len()
                                )));
                                if app.get_workspace_mode() == 0 {
                                    app.set_workspace_image(render_image());
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
        let points = points.clone();
        app.on_save_project(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    if let Err(e) =
                        survey_cad::io::write_points_csv(p, &points.borrow(), None, None)
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
        app.on_add_line(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Add Line not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_add_point(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Add Point not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_add_polygon(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Add Polygon not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_add_polyline(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Add Polyline not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_add_arc(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Add Arc not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_station_distance(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Station Distance not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_traverse_area(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Traverse Area not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_level_elevation_tool(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Level Elevation not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_corridor_volume(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Corridor Volume not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_import_geojson(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Import GeoJSON not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_import_kml(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Import KML not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_import_dxf(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Import DXF not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_import_shp(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Import SHP not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_import_las(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Import LAS not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_import_e57(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Import E57 not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_export_geojson(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Export GeoJSON not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_export_kml(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Export KML not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_export_dxf(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Export DXF not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_export_shp(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Export SHP not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_export_las(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Export LAS not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_export_e57(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Export E57 not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_import_landxml_surface(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Import LandXML Surface not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_import_landxml_alignment(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Import LandXML Alignment not implemented"));
            }
        });
    }

    {
        let weak = app.as_weak();
        let points = points.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        app.on_clear_workspace(move || {
            points.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            alignments.borrow_mut().clear();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Cleared workspace"));
                if app.get_workspace_mode() == 0 {
                    app.set_workspace_image(render_image());
                }
            }
        });
    }

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
