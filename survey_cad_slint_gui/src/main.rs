#![allow(unused_variables)]

use std::cell::RefCell;
use std::rc::Rc;

mod bevy_adapter;
mod workspace3d;

use slint::{Image, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use survey_cad::crs::{list_known_crs, Crs};
use survey_cad::geometry::{Arc, Line, Point, Polyline};
use survey_cad::dtm::Tin;
use survey_cad::alignment::HorizontalAlignment;
use survey_cad::alignment::{Alignment, VerticalAlignment};
use survey_cad::corridor::corridor_volume;
use survey_cad::io::DxfEntity;
use survey_cad::snap::snap_point;
#[cfg(any(feature = "las", feature = "e57"))]
use survey_cad::geometry::Point3;
use survey_cad::surveying::{
    bearing, forward, level_elevation, line_intersection, station_distance, vertical_angle, Station,
};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};
use bevy::prelude as bevy_prelude;
use spin_on::spin_on;

slint::slint! {
component Workspace2D inherits Rectangle {
    in-out property <image> image;
    in-out property <bool> click_mode;
    callback clicked(length, length);
    background: #202020;
    Image {
        source: root.image;
        image-fit: fill;
        width: 100%;
        height: 100%;
    }
    if root.click_mode : TouchArea {
        width: 100%;
        height: 100%;
        clicked => { root.clicked(self.mouse-x, self.mouse-y); }
    }
}

component Workspace3D inherits Rectangle {
    in-out property <image> texture <=> img.source;
    out property <length> requested-texture-width: img.width;
    out property <length> requested-texture-height: img.height;
    callback mouse_moved(length, length);
    background: #202020;
    img := Image {
        width: 100%;
        height: 100%;
        image-fit: fill;
    }
    TouchArea {
        width: 100%;
        height: 100%;
        moved => { root.mouse_moved(self.mouse-x, self.mouse-y); }
    }
}

import { Button, VerticalBox, HorizontalBox, ComboBox, LineEdit, ListView, CheckBox } from "std-widgets.slint";

export component AddPointDialog inherits Window {
    callback from_file();
    callback manual_keyin();
    callback manual_click();
    title: "Add Point";
    VerticalBox {
        spacing: 6px;
        Button { text: "From File"; clicked => { root.from_file(); } }
        Button { text: "Manual (Key In)"; clicked => { root.manual_keyin(); } }
        Button { text: "Manual (Click on Screen)"; clicked => { root.manual_click(); } }
    }
}

export component KeyInDialog inherits Window {
    in-out property <string> x_value;
    in-out property <string> y_value;
    callback accept();
    callback cancel();
    title: "Enter Point";
    VerticalBox {
        spacing: 6px;
        HorizontalBox {
            Text { text: "X:"; }
            LineEdit { text <=> root.x_value; }
        }
        HorizontalBox {
            Text { text: "Y:"; }
            LineEdit { text <=> root.y_value; }
        }
        HorizontalBox {
            spacing: 6px;
            Button { text: "OK"; clicked => { root.accept(); } }
            Button { text: "Cancel"; clicked => { root.cancel(); } }
        }
    }
}

export component StationDistanceDialog inherits Window {
    in-out property <string> x1;
    in-out property <string> y1;
    in-out property <string> x2;
    in-out property <string> y2;
    callback accept();
    callback cancel();
    title: "Station Distance";
    VerticalBox {
        spacing: 6px;
        HorizontalBox {
            Text { text: "X1:"; }
            LineEdit { text <=> root.x1; }
        }
        HorizontalBox {
            Text { text: "Y1:"; }
            LineEdit { text <=> root.y1; }
        }
        HorizontalBox {
            Text { text: "X2:"; }
            LineEdit { text <=> root.x2; }
        }
        HorizontalBox {
            Text { text: "Y2:"; }
            LineEdit { text <=> root.y2; }
        }
        HorizontalBox {
            spacing: 6px;
            Button { text: "OK"; clicked => { root.accept(); } }
            Button { text: "Cancel"; clicked => { root.cancel(); } }
        }
    }
}

export component LevelElevationDialog inherits Window {
    in-out property <string> start_elev;
    in-out property <string> backsight;
    in-out property <string> foresight;
    callback accept();
    callback cancel();
    title: "Level Elevation";
    VerticalBox {
        spacing: 6px;
        HorizontalBox {
            Text { text: "Start Elev:"; }
            LineEdit { text <=> root.start_elev; }
        }
        HorizontalBox {
            Text { text: "Backsight:"; }
            LineEdit { text <=> root.backsight; }
        }
        HorizontalBox {
            Text { text: "Foresight:"; }
            LineEdit { text <=> root.foresight; }
        }
        HorizontalBox {
            spacing: 6px;
            Button { text: "OK"; clicked => { root.accept(); } }
            Button { text: "Cancel"; clicked => { root.cancel(); } }
        }
    }
}

export component CorridorVolumeDialog inherits Window {
    in-out property <string> width_value;
    in-out property <string> interval_value;
    in-out property <string> offset_step_value;
    callback accept();
    callback cancel();
    title: "Corridor Volume";
    VerticalBox {
        spacing: 6px;
        HorizontalBox {
            Text { text: "Width:"; }
            LineEdit { text <=> root.width_value; }
        }
        HorizontalBox {
            Text { text: "Interval:"; }
            LineEdit { text <=> root.interval_value; }
        }
        HorizontalBox {
            Text { text: "Offset Step:"; }
            LineEdit { text <=> root.offset_step_value; }
        }
        HorizontalBox {
            spacing: 6px;
            Button { text: "OK"; clicked => { root.accept(); } }
            Button { text: "Cancel"; clicked => { root.cancel(); } }
        }
    }
}

export component AddLineDialog inherits Window {
    callback from_file();
    callback manual();
    title: "Add Line";
    VerticalBox {
        spacing: 6px;
        Button { text: "From File"; clicked => { root.from_file(); } }
        Button { text: "Manual"; clicked => { root.manual(); } }
    }
}

export component LineKeyInDialog inherits Window {
    in-out property <string> x1;
    in-out property <string> y1;
    in-out property <string> x2;
    in-out property <string> y2;
    callback accept();
    callback cancel();
    title: "Enter Line";
    VerticalBox {
        spacing: 6px;
        HorizontalBox {
            Text { text: "X1:"; }
            LineEdit { text <=> root.x1; }
            Text { text: "Y1:"; }
            LineEdit { text <=> root.y1; }
        }
        HorizontalBox {
            Text { text: "X2:"; }
            LineEdit { text <=> root.x2; }
            Text { text: "Y2:"; }
            LineEdit { text <=> root.y2; }
        }
        HorizontalBox {
            spacing: 6px;
            Button { text: "OK"; clicked => { root.accept(); } }
            Button { text: "Cancel"; clicked => { root.cancel(); } }
        }
    }
}

export component AddPolygonDialog inherits Window {
    callback from_file();
    callback manual();
    title: "Add Polygon";
    VerticalBox {
        spacing: 6px;
        Button { text: "From File"; clicked => { root.from_file(); } }
        Button { text: "Manual"; clicked => { root.manual(); } }
    }
}

export component AddPolylineDialog inherits Window {
    callback from_file();
    callback manual();
    title: "Add Polyline";
    VerticalBox {
        spacing: 6px;
        Button { text: "From File"; clicked => { root.from_file(); } }
        Button { text: "Manual"; clicked => { root.manual(); } }
    }
}

export component PointsDialog inherits Window {
    in-out property <string> x_value;
    in-out property <string> y_value;
    in-out property <[string]> points_model;
    callback add_point();
    callback accept();
    callback cancel();
    title: "Enter Points";
    VerticalBox {
        spacing: 6px;
        HorizontalBox {
            Text { text: "X:"; }
            LineEdit { text <=> root.x_value; }
            Text { text: "Y:"; }
            LineEdit { text <=> root.y_value; }
            Button { text: "Add"; clicked => { root.add_point(); } }
        }
        ListView {
            for p in root.points_model : Text { text: p; }
            height: 100px;
        }
        HorizontalBox {
            spacing: 6px;
            Button { text: "OK"; clicked => { root.accept(); } }
            Button { text: "Cancel"; clicked => { root.cancel(); } }
        }
    }
}

export component AddArcDialog inherits Window {
    callback from_file();
    callback manual();
    title: "Add Arc";
    VerticalBox {
        spacing: 6px;
        Button { text: "From File"; clicked => { root.from_file(); } }
        Button { text: "Manual"; clicked => { root.manual(); } }
    }
}

export component ArcKeyInDialog inherits Window {
    in-out property <string> cx;
    in-out property <string> cy;
    in-out property <string> radius;
    in-out property <string> start_angle;
    in-out property <string> end_angle;
    callback accept();
    callback cancel();
    title: "Enter Arc";
    VerticalBox {
        spacing: 6px;
        HorizontalBox {
            Text { text: "Cx:"; }
            LineEdit { text <=> root.cx; }
            Text { text: "Cy:"; }
            LineEdit { text <=> root.cy; }
        }
        HorizontalBox {
            Text { text: "Radius:"; }
            LineEdit { text <=> root.radius; }
        }
        HorizontalBox {
            Text { text: "Start:"; }
            LineEdit { text <=> root.start_angle; }
            Text { text: "End:"; }
            LineEdit { text <=> root.end_angle; }
        }
        HorizontalBox {
            spacing: 6px;
            Button { text: "OK"; clicked => { root.accept(); } }
            Button { text: "Cancel"; clicked => { root.cancel(); } }
        }
    }
}

export component MainWindow inherits Window {
    preferred-width: 800px;
    preferred-height: 600px;

    in-out property <string> status;
    in property <[string]> crs_list;
    in-out property <int> crs_index;
    in property <[string]> cogo_list;
    in-out property <int> cogo_index;
    in-out property <int> workspace_mode;
    in-out property <image> workspace_image;
    in-out property <image> workspace_texture;
    in-out property <bool> workspace_click_mode;
    in-out property <bool> snap_to_grid;
    in-out property <bool> snap_to_entities;

    callback workspace_clicked(length, length);
    callback workspace_mouse_moved(length, length);

    callback crs_changed(int);
    callback cogo_selected(int);

    callback new_project();
    callback open_project();
    callback save_project();
    callback add_point();
    callback add_line();
    callback add_polygon();
    callback add_polyline();
    callback add_arc();
    callback clear_workspace();
    callback view_changed(int);
    callback station_distance();
    callback traverse_area();
    callback level_elevation_tool();
    callback corridor_volume();
    callback import_geojson();
    callback import_kml();
    callback import_dxf();
    callback import_shp();
    callback import_las();
    callback import_e57();
    callback export_geojson();
    callback export_kml();
    callback export_dxf();
    callback export_shp();
    callback export_las();
    callback export_e57();
    callback import_landxml_surface();
    callback import_landxml_alignment();

    MenuBar {
        Menu {
            title: "File";
            MenuItem { title: "New"; activated => { root.new_project(); } }
            MenuItem { title: "Open"; activated => { root.open_project(); } }
            MenuItem { title: "Save"; activated => { root.save_project(); } }
            Menu {
                title: "Import";
                MenuItem { title: "GeoJSON"; activated => { root.import_geojson(); } }
                MenuItem { title: "KML"; activated => { root.import_kml(); } }
                MenuItem { title: "DXF"; activated => { root.import_dxf(); } }
                MenuItem { title: "SHP"; activated => { root.import_shp(); } }
                MenuItem { title: "LAS"; activated => { root.import_las(); } }
                MenuItem { title: "E57"; activated => { root.import_e57(); } }
                MenuItem { title: "LandXML Surface"; activated => { root.import_landxml_surface(); } }
                MenuItem { title: "LandXML Alignment"; activated => { root.import_landxml_alignment(); } }
            }
            Menu {
                title: "Export";
                MenuItem { title: "GeoJSON"; activated => { root.export_geojson(); } }
                MenuItem { title: "KML"; activated => { root.export_kml(); } }
                MenuItem { title: "DXF"; activated => { root.export_dxf(); } }
                MenuItem { title: "SHP"; activated => { root.export_shp(); } }
                MenuItem { title: "LAS"; activated => { root.export_las(); } }
                MenuItem { title: "E57"; activated => { root.export_e57(); } }
            }
        }
        Menu {
            title: "Edit";
            MenuItem { title: "Add Point"; activated => { root.add_point(); } }
            MenuItem { title: "Add Line"; activated => { root.add_line(); } }
            MenuItem { title: "Add Polygon"; activated => { root.add_polygon(); } }
            MenuItem { title: "Add Polyline"; activated => { root.add_polyline(); } }
            MenuItem { title: "Add Arc"; activated => { root.add_arc(); } }
            MenuItem { title: "Clear"; activated => { root.clear_workspace(); } }
        }
        Menu {
            title: "Tools";
            MenuItem { title: "Station Distance"; activated => { root.station_distance(); } }
            MenuItem { title: "Traverse Area"; activated => { root.traverse_area(); } }
            MenuItem { title: "Level Elevation"; activated => { root.level_elevation_tool(); } }
            MenuItem { title: "Corridor Volume"; activated => { root.corridor_volume(); } }
        }
        Menu {
            title: "View";
            MenuItem { title: "2D Workspace"; activated => { root.view_changed(0); } }
            MenuItem { title: "3D Workspace"; activated => { root.view_changed(1); } }
        }
    }

    HorizontalBox {
        spacing: 6px;
        Button { text: "New"; clicked => { root.new_project(); } }
        Button { text: "Open"; clicked => { root.open_project(); } }
        Button { text: "Save"; clicked => { root.save_project(); } }
        Button { text: "Add Point"; clicked => { root.add_point(); } }
        Button { text: "Add Line"; clicked => { root.add_line(); } }
        Button { text: "Add Polygon"; clicked => { root.add_polygon(); } }
        Button { text: "Add Polyline"; clicked => { root.add_polyline(); } }
        Button { text: "Add Arc"; clicked => { root.add_arc(); } }
        Button { text: "Load LandXML Surface"; clicked => { root.import_landxml_surface(); } }
        Button { text: "Load LandXML Alignment"; clicked => { root.import_landxml_alignment(); } }
        Button { text: "Corridor Volume"; clicked => { root.corridor_volume(); } }
        Button { text: "Clear"; clicked => { root.clear_workspace(); } }
        Text { text: "View:"; }
        ComboBox {
            model: ["2D", "3D"];
            current-index <=> root.workspace_mode;
            selected => { root.view_changed(root.workspace_mode); }
        }
        Text { text: "Cogo:"; }
        ComboBox {
            model: root.cogo_list;
            current-index <=> root.cogo_index;
            selected => { root.cogo_selected(root.cogo_index); }
        }
    }

    HorizontalBox {
        spacing: 6px;
        Text { text: "CRS:"; }
        ComboBox {
            model: root.crs_list;
            current-index <=> root.crs_index;
            selected => { root.crs_changed(root.crs_index); }
        }
    }

    HorizontalBox {
        spacing: 6px;
        CheckBox { text: "Snap Grid"; checked <=> root.snap_to_grid; }
        CheckBox { text: "Snap Objects"; checked <=> root.snap_to_entities; }
    }

    VerticalBox {
        if root.workspace_mode == 0 : Workspace2D {
            image <=> root.workspace_image;
            click_mode <=> root.workspace_click_mode;
            clicked(x, y) => { root.workspace_clicked(x, y); }
        }
        if root.workspace_mode == 1 : Workspace3D {
            texture <=> root.workspace_texture;
            mouse_moved(x, y) => { root.workspace_mouse_moved(x, y); }
        }
        Text { text: root.status; }
    }
}
}

fn render_workspace(
    points: &[Point],
    lines: &[(Point, Point)],
    polygons: &[Vec<Point>],
    polylines: &[Polyline],
    arcs: &[Arc],
    surfaces: &[survey_cad::dtm::Tin],
    alignments: &[survey_cad::alignment::HorizontalAlignment],
) -> Image {
    const WIDTH: u32 = 600;
    const HEIGHT: u32 = 400;
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(Color::from_rgba8(32, 32, 32, 255));

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(255, 0, 0, 255));
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 2.0,
        ..Stroke::default()
    };

    for (s, e) in lines {
        let mut pb = PathBuilder::new();
        pb.move_to(
            (s.x as f32) + WIDTH as f32 / 2.0,
            HEIGHT as f32 / 2.0 - s.y as f32,
        );
        pb.line_to(
            (e.x as f32) + WIDTH as f32 / 2.0,
            HEIGHT as f32 / 2.0 - e.y as f32,
        );
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    for poly in polygons {
        if poly.len() < 2 {
            continue;
        }
        let mut pb = PathBuilder::new();
        let first = poly.first().unwrap();
        pb.move_to(
            (first.x as f32) + WIDTH as f32 / 2.0,
            HEIGHT as f32 / 2.0 - first.y as f32,
        );
        for p in &poly[1..] {
            pb.line_to(
                (p.x as f32) + WIDTH as f32 / 2.0,
                HEIGHT as f32 / 2.0 - p.y as f32,
            );
        }
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    for pl in polylines {
        if pl.vertices.len() < 2 {
            continue;
        }
        let mut pb = PathBuilder::new();
        let first = &pl.vertices[0];
        pb.move_to(
            (first.x as f32) + WIDTH as f32 / 2.0,
            HEIGHT as f32 / 2.0 - first.y as f32,
        );
        for p in &pl.vertices[1..] {
            pb.line_to(
                (p.x as f32) + WIDTH as f32 / 2.0,
                HEIGHT as f32 / 2.0 - p.y as f32,
            );
        }
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    for arc in arcs {
        let steps = 32;
        let mut pb = PathBuilder::new();
        for i in 0..=steps {
            let t = arc.start_angle + (arc.end_angle - arc.start_angle) * (i as f64 / steps as f64);
            let x = arc.center.x + arc.radius * t.cos();
            let y = arc.center.y + arc.radius * t.sin();
            let px = (x as f32) + WIDTH as f32 / 2.0;
            let py = HEIGHT as f32 / 2.0 - y as f32;
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
    for tin in surfaces {
        for tri in &tin.triangles {
            let a = tin.vertices[tri[0]];
            let b = tin.vertices[tri[1]];
            let c = tin.vertices[tri[2]];
            let mut pb = PathBuilder::new();
            pb.move_to((a.x as f32) + WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0 - a.y as f32);
            pb.line_to((b.x as f32) + WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0 - b.y as f32);
            pb.line_to((c.x as f32) + WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0 - c.y as f32);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    paint.set_color(Color::from_rgba8(0, 200, 255, 255));
    for hal in alignments {
        for elem in &hal.elements {
            match elem {
                survey_cad::alignment::HorizontalElement::Tangent { start, end } => {
                    let mut pb = PathBuilder::new();
                    pb.move_to((start.x as f32) + WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0 - start.y as f32);
                    pb.line_to((end.x as f32) + WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0 - end.y as f32);
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
                        let px = (x as f32) + WIDTH as f32 / 2.0;
                        let py = HEIGHT as f32 / 2.0 - y as f32;
                        if i == 0 { pb.move_to(px, py); } else { pb.line_to(px, py); }
                    }
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
                survey_cad::alignment::HorizontalElement::Spiral { spiral } => {
                    let mut pb = PathBuilder::new();
                    let sp = spiral.start_point();
                    let ep = spiral.end_point();
                    pb.move_to((sp.x as f32) + WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0 - sp.y as f32);
                    pb.line_to((ep.x as f32) + WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0 - ep.y as f32);
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
            }
        }
    }

    paint.set_color(Color::from_rgba8(0, 255, 0, 255));
    for p in points {
        if let Some(circle) = PathBuilder::from_circle(
            (p.x as f32) + WIDTH as f32 / 2.0,
            HEIGHT as f32 / 2.0 - p.y as f32,
            3.0,
        ) {
            pixmap.fill_path(
                &circle,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pixmap.data(), WIDTH, HEIGHT);
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
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "empty file"));
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
    let (ui_tx, ui_rx) = crossbeam_channel::unbounded::<workspace3d::UiEvent>();
    let (data_tx, data_rx) = crossbeam_channel::unbounded::<workspace3d::BevyData>();

    let (bevy_texture_receiver, bevy_control_sender) =
        spin_on(bevy_adapter::run_bevy_app_with_slint(
            |_| {},
            |mut bapp| {
                workspace3d::bevy_app(&mut bapp);
                bapp.insert_resource(bevy_prelude::ClearColor(bevy_prelude::Color::srgb(0.1, 0.1, 0.1)))
                    .run();
            },
            ui_rx,
            data_tx,
        ))?;

    let app = MainWindow::new()?;
    let points: Rc<RefCell<Vec<Point>>> = Rc::new(RefCell::new(Vec::new()));
    let lines: Rc<RefCell<Vec<(Point, Point)>>> = Rc::new(RefCell::new(Vec::new()));
    let polygons: Rc<RefCell<Vec<Vec<Point>>>> = Rc::new(RefCell::new(Vec::new()));
    let polylines: Rc<RefCell<Vec<Polyline>>> = Rc::new(RefCell::new(Vec::new()));
    let arcs: Rc<RefCell<Vec<Arc>>> = Rc::new(RefCell::new(Vec::new()));
    let surfaces: Rc<RefCell<Vec<Tin>>> = Rc::new(RefCell::new(Vec::new()));
    let alignments: Rc<RefCell<Vec<HorizontalAlignment>>> = Rc::new(RefCell::new(Vec::new()));
    let surfaces_for_corridor = surfaces.clone();
    let alignments_for_corridor = alignments.clone();
    let crs_entries = list_known_crs();
    let crs_model = Rc::new(VecModel::from(
        crs_entries
            .iter()
            .map(|e| SharedString::from(format!("{} - {}", e.code, e.name)))
            .collect::<Vec<_>>(),
    ));
    app.set_crs_list(crs_model.clone().into());
    app.set_crs_index(0);
    let cogo_model = Rc::new(VecModel::from(vec![
        SharedString::from("Bearing"),
        SharedString::from("Forward"),
        SharedString::from("Intersection"),
        SharedString::from("Level Elev"),
        SharedString::from("Vert Angle"),
    ]));
    app.set_cogo_list(cogo_model.clone().into());
    app.set_cogo_index(0);
    let working_crs = Rc::new(RefCell::new(Crs::wgs84()));

    app.set_workspace_mode(0);
    app.set_workspace_image(render_workspace(
        &points.borrow(),
        &lines.borrow(),
        &polygons.borrow(),
        &polylines.borrow(),
        &arcs.borrow(),
        &surfaces.borrow(),
        &alignments.borrow(),
    ));
    app.set_workspace_click_mode(false);
    app.set_workspace_texture(Image::default());
    app.set_snap_to_grid(true);
    app.set_snap_to_entities(true);

    let app_for_notifier = app.as_weak();
    app.window().set_rendering_notifier(move |state, _| {
        if let slint::RenderingState::BeforeRendering = state {
            let Some(app) = app_for_notifier.upgrade() else { return; };
            app.window().request_redraw();
            let Ok(new_texture) = bevy_texture_receiver.try_recv() else { return; };
            if let Some(old_texture) = app.get_workspace_texture().to_wgpu_24_texture() {
                let sender = bevy_control_sender.clone();
                slint::spawn_local(async move {
                    sender
                        .send(bevy_adapter::ControlMessage::ReleaseFrontBufferTexture { texture: old_texture })
                        .await
                        .unwrap();
                })
                .unwrap();
            }
            // Fixed texture size for the 3D workspace
            if let Ok(image) = new_texture.try_into() {
                app.set_workspace_texture(image);
            }
        }
    }).unwrap();

    let update_image = {
        let points = points.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let weak = app.as_weak();
        std::rc::Rc::new(move || {
            if let Some(app) = weak.upgrade() {
                let img = render_workspace(
                    &points.borrow(),
                    &lines.borrow(),
                    &polygons.borrow(),
                    &polylines.borrow(),
                    &arcs.borrow(),
                    &surfaces.borrow(),
                    &alignments.borrow(),
                );
                app.set_workspace_image(img);
            }
        })
    };

    let weak = app.as_weak();
    {
        let points = points.clone();
        let weak = weak.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let update_image = update_image.clone();
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
            }
            (update_image.clone())();
        });
    }

    let weak = app.as_weak();
    {
        let points = points.clone();
        let update_image = update_image.clone();
        app.on_open_project(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                if let Some(path_str) = path.to_str() {
                    match survey_cad::io::read_points_csv(path_str, None, None) {
                        Ok(pts) => {
                            *points.borrow_mut() = pts;
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Loaded {} points",
                                    points.borrow().len()
                                )));
                            }
                            (update_image.clone())();
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

    let weak = app.as_weak();
    {
        let points = points.clone();
        app.on_save_project(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .save_file()
            {
                if let Some(path_str) = path.to_str() {
                    if let Err(e) =
                        survey_cad::io::write_points_csv(path_str, &points.borrow(), None, None)
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

    // Import handlers
    {
        let weak = app.as_weak();
        let points = points.clone();
        let update_image = update_image.clone();
        app.on_import_geojson(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("GeoJSON", &["geojson", "json"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::read_points_geojson(p, None, None) {
                        Ok(pts) => {
                            *points.borrow_mut() = pts;
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {} points",
                                    points.borrow().len()
                                )));
                            }
                            (update_image.clone())();
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
        let points = points.clone();
        let update_image = update_image.clone();
        app.on_import_kml(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("KML", &["kml", "kmz"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "kml")]
                    match survey_cad::io::kml::read_points_kml(p) {
                        Ok(pts) => {
                            *points.borrow_mut() = pts;
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {} points",
                                    points.borrow().len()
                                )));
                            }
                            (update_image.clone())();
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
        let points = points.clone();
        let update_image = update_image.clone();
        app.on_import_dxf(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DXF", &["dxf"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::read_dxf(p) {
                        Ok(ents) => {
                            *points.borrow_mut() = ents
                                .into_iter()
                                .filter_map(|e| match e {
                                    survey_cad::io::DxfEntity::Point { point, .. } => Some(point),
                                    _ => None,
                                })
                                .collect();
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {} points",
                                    points.borrow().len()
                                )));
                            }
                            (update_image.clone())();
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
        let points = points.clone();
        let update_image = update_image.clone();
        app.on_import_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    match survey_cad::io::shp::read_points_shp(p) {
                        Ok((pts, _)) => {
                            *points.borrow_mut() = pts;
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {} points",
                                    points.borrow().len()
                                )));
                            }
                            (update_image.clone())();
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
        let points = points.clone();
        let update_image = update_image.clone();
        app.on_import_las(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LAS", &["las", "laz"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "las")]
                    match survey_cad::io::las::read_points_las(p) {
                        Ok(pts3) => {
                            *points.borrow_mut() = pts3
                                .into_iter()
                                .map(|p3| Point::new(p3.x, p3.y))
                                .collect();
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {} points",
                                    points.borrow().len()
                                )));
                            }
                            (update_image.clone())();
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
        let points = points.clone();
        let update_image = update_image.clone();
        app.on_import_e57(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("E57", &["e57"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "e57")]
                    match survey_cad::io::e57::read_points_e57(p) {
                        Ok(pts3) => {
                            *points.borrow_mut() = pts3
                                .into_iter()
                                .map(|p3| Point::new(p3.x, p3.y))
                                .collect();
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {} points",
                                    points.borrow().len()
                                )));
                            }
                            (update_image.clone())();
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
        let surfaces = surfaces.clone();
        let update_image = update_image.clone();
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
                            }
                            (update_image.clone())();
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("Failed to import: {}", e)));
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
        let update_image = update_image.clone();
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
                            }
                            (update_image.clone())();
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("Failed to import: {}", e)));
                            }
                        }
                    }
                }
            }
        });
    }

    // Export handlers
    {
        let weak = app.as_weak();
        let points = points.clone();
        app.on_export_geojson(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("GeoJSON", &["geojson", "json"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    if let Err(e) = survey_cad::io::write_points_geojson(p, &points.borrow(), None, None) {
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
        let points = points.clone();
        app.on_export_kml(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("KML", &["kml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "kml")]
                    if let Err(e) = survey_cad::io::kml::write_points_kml(p, &points.borrow()) {
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
        let points = points.clone();
        app.on_export_dxf(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DXF", &["dxf"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    if let Err(e) = survey_cad::io::write_points_dxf(p, &points.borrow(), None, None) {
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
        let points = points.clone();
        app.on_export_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    if let Err(e) = survey_cad::io::shp::write_points_shp(p, &points.borrow(), None) {
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
        let points = points.clone();
        app.on_export_las(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LAS", &["las", "laz"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "las")]
                    {
                        let pts3: Vec<Point3> = points.borrow().iter().map(|pt| Point3::new(pt.x, pt.y, 0.0)).collect();
                        if let Err(e) = survey_cad::io::las::write_points_las(p, &pts3) {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("Failed to export: {}", e)));
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
        let points = points.clone();
        app.on_export_e57(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("E57", &["e57"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "e57")]
                    {
                        let pts3: Vec<Point3> = points.borrow().iter().map(|pt| Point3::new(pt.x, pt.y, 0.0)).collect();
                        if let Err(e) = survey_cad::io::e57::write_points_e57(p, &pts3) {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("Failed to export: {}", e)));
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

    let weak = app.as_weak();
    {
        let points = points.clone();
        let update_image = update_image.clone();
        let main_weak = weak.clone();
        app.on_add_point(move || {
            let dlg = AddPointDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let points = points.clone();
                let update_image = update_image.clone();
                let main_weak = main_weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(path_str) = path.to_str() {
                            match survey_cad::io::read_points_csv(path_str, None, None) {
                                Ok(pts) => {
                                    *points.borrow_mut() = pts;
                                    if let Some(app) = main_weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Loaded {} points",
                                            points.borrow().len()
                                        )));
                                    }
                                    (update_image.clone())();
                                }
                                Err(e) => {
                                    if let Some(app) = main_weak.upgrade() {
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
                let points = points.clone();
                let update_image = update_image.clone();
                let main_weak = main_weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual_keyin(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let key_dlg = KeyInDialog::new().unwrap();
                    let key_weak = key_dlg.as_weak();
                    let key_dlg_weak = key_dlg.as_weak();
                    {
                        let points = points.clone();
                        let update_image = update_image.clone();
                        let main_weak = main_weak.clone();
                        let key_weak2 = key_weak.clone();
                        let key_dlg_weak2 = key_dlg_weak.clone();
                        key_dlg.on_accept(move || {
                            if let Some(dlg) = key_dlg_weak2.upgrade() {
                                if let (Ok(x), Ok(y)) = (
                                    dlg.get_x_value().parse::<f64>(),
                                    dlg.get_y_value().parse::<f64>(),
                                ) {
                                    points.borrow_mut().push(Point::new(x, y));
                                    if let Some(app) = main_weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total points: {}",
                                            points.borrow().len()
                                        )));
                                    }
                                    (update_image.clone())();
                                }
                            }
                            if let Some(k) = key_weak2.upgrade() {
                                let _ = k.hide();
                            }
                        });
                    }
                    {
                        let key_weak2 = key_weak.clone();
                        key_dlg.on_cancel(move || {
                            if let Some(k) = key_weak2.upgrade() {
                                let _ = k.hide();
                            }
                        });
                    }
                    key_dlg.show().unwrap();
                });
            }
            {
                let main_weak = main_weak.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual_click(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    if let Some(app) = main_weak.upgrade() {
                        app.set_workspace_click_mode(true);
                    }
                });
            }
            dlg.show().unwrap();
        });
    }

    let weak = app.as_weak();
    {
        let lines = lines.clone();
        let update_image = update_image.clone();
        let weak_main = weak.clone();
        app.on_add_line(move || {
            let dlg = AddLineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let lines = lines.clone();
                let update_image = update_image.clone();
                let weak_main = weak_main.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_line_csv(p) {
                                Ok(l) => {
                                    lines.borrow_mut().push(l);
                                    if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total lines: {}",
                                            lines.borrow().len()
                                        )));
                                    }
                                    (update_image.clone())();
                                }
                                Err(e) => {
                                    if let Some(app) = weak_main.upgrade() {
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
                let update_image = update_image.clone();
                let weak_main = weak_main.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let kd = LineKeyInDialog::new().unwrap();
                    let kd_weak = kd.as_weak();
                    let kd_weak2 = kd.as_weak();
                    {
                        let lines = lines.clone();
                        let update_image = update_image.clone();
                        let weak_main = weak_main.clone();
                        kd.on_accept(move || {
                            if let Some(dlg) = kd_weak2.upgrade() {
                                if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                                    dlg.get_x1().parse::<f64>(),
                                    dlg.get_y1().parse::<f64>(),
                                    dlg.get_x2().parse::<f64>(),
                                    dlg.get_y2().parse::<f64>(),
                                ) {
                                    lines.borrow_mut().push((Point::new(x1, y1), Point::new(x2, y2)));
                                    if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total lines: {}",
                                            lines.borrow().len()
                                        )));
                                    }
                                    (update_image.clone())();
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

    let weak = app.as_weak();
    {
        let polygons = polygons.clone();
        let update_image = update_image.clone();
        let weak_main = weak.clone();
        app.on_add_polygon(move || {
            let dlg = AddPolygonDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let polygons = polygons.clone();
                let update_image = update_image.clone();
                let weak_main = weak_main.clone();
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
                                        if let Some(app) = weak_main.upgrade() {
                                            app.set_status(SharedString::from(format!(
                                                "Total polygons: {}",
                                                polygons.borrow().len()
                                            )));
                                        }
                                        (update_image.clone())();
                                    } else if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from("Need at least 3 points"));
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from(format!("Failed to open: {}", e)));
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
                let update_image = update_image.clone();
                let weak_main = weak_main.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let pd = PointsDialog::new().unwrap();
                    let _pd_weak = pd.as_weak();
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
                        let update_image = update_image.clone();
                        let weak_main = weak_main.clone();
                        let pd_weak2 = pd.as_weak();
                        let pts = pts.clone();
                        pd.on_accept(move || {
                            if pts.borrow().len() >= 3 {
                                polygons.borrow_mut().push(pts.borrow().clone());
                                if let Some(app) = weak_main.upgrade() {
                                    app.set_status(SharedString::from(format!(
                                        "Total polygons: {}",
                                        polygons.borrow().len()
                                    )));
                                }
                                (update_image.clone())();
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

    let weak = app.as_weak();
    {
        let polylines = polylines.clone();
        let update_image = update_image.clone();
        let weak_main = weak.clone();
        app.on_add_polyline(move || {
            let dlg = AddPolylineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let polylines = polylines.clone();
                let update_image = update_image.clone();
                let weak_main = weak_main.clone();
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
                                        if let Some(app) = weak_main.upgrade() {
                                            app.set_status(SharedString::from(format!(
                                                "Total polylines: {}",
                                                polylines.borrow().len()
                                            )));
                                        }
                                        (update_image.clone())();
                                    } else if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from("Need at least 2 points"));
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from(format!("Failed to open: {}", e)));
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
                let update_image = update_image.clone();
                let weak_main = weak_main.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_manual(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                    let pd = PointsDialog::new().unwrap();
                    let model = Rc::new(VecModel::<SharedString>::from(Vec::<SharedString>::new()));
                    pd.set_points_model(model.clone().into());
                    let _pd_weak = pd.as_weak();
                    let pts = Rc::new(RefCell::new(Vec::<Point>::new()));
                    {
                        let model = model.clone();
                        let pd_weak2 = pd.as_weak();
                        let pts = pts.clone();
                        pd.on_add_point(move || {
                            if let Some(d) = pd_weak2.upgrade() {
                                if let (Ok(x), Ok(y)) = (d.get_x_value().parse::<f64>(), d.get_y_value().parse::<f64>()) {
                                    pts.borrow_mut().push(Point::new(x, y));
                                    model.push(SharedString::from(format!("{:.3},{:.3}", x, y)));
                                }
                            }
                        });
                    }
                    {
                        let polylines = polylines.clone();
                        let update_image = update_image.clone();
                        let weak_main = weak_main.clone();
                        let pd_weak2 = pd.as_weak();
                        let pts = pts.clone();
                        pd.on_accept(move || {
                            if pts.borrow().len() >= 2 {
                                polylines.borrow_mut().push(Polyline::new(pts.borrow().clone()));
                                if let Some(app) = weak_main.upgrade() {
                                    app.set_status(SharedString::from(format!(
                                        "Total polylines: {}",
                                        polylines.borrow().len()
                                    )));
                                }
                                (update_image.clone())();
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

    let weak = app.as_weak();
    {
        let arcs = arcs.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let update_image = update_image.clone();
        let weak_main = weak.clone();
        app.on_add_arc(move || {
            let dlg = AddArcDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let arcs = arcs.clone();
                let update_image = update_image.clone();
                let weak_main = weak_main.clone();
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
                                    if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total arcs: {}",
                                            arcs.borrow().len()
                                        )));
                                    }
                                    (update_image.clone())();
                                }
                                Err(e) => {
                                    if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from(format!("Failed to open: {}", e)));
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
                let update_image = update_image.clone();
                let weak_main = weak_main.clone();
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
                        let update_image = update_image.clone();
                        let weak_main = weak_main.clone();
                        ad.on_accept(move || {
                            if let Some(dlg) = ad_weak2.upgrade() {
                                if let (Ok(cx), Ok(cy), Ok(r), Ok(sa), Ok(ea)) = (
                                    dlg.get_cx().parse::<f64>(),
                                    dlg.get_cy().parse::<f64>(),
                                    dlg.get_radius().parse::<f64>(),
                                    dlg.get_start_angle().parse::<f64>(),
                                    dlg.get_end_angle().parse::<f64>(),
                                ) {
                                    arcs.borrow_mut().push(Arc::new(Point::new(cx, cy), r, sa, ea));
                                    if let Some(app) = weak_main.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total arcs: {}",
                                            arcs.borrow().len()
                                        )));
                                    }
                                    (update_image.clone())();
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

    let weak = app.as_weak();
    {
        let points = points.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let update_image = update_image.clone();
        app.on_clear_workspace(move || {
            points.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            alignments.borrow_mut().clear();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Workspace cleared"));
            }
            (update_image.clone())();
        });
    }

    {
        let entries = crs_entries.clone();
        let working_crs = working_crs.clone();
        let weak = app.as_weak();
        app.on_crs_changed(move |idx| {
            if let Some(entry) = entries.get(idx as usize) {
                if let Some(code) = entry.code.split(':').nth(1) {
                    if let Ok(epsg) = code.parse::<u32>() {
                        *working_crs.borrow_mut() = Crs::from_epsg(epsg);
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!(
                                "Selected CRS {}",
                                entry.code
                            )));
                        }
                    }
                }
            }
        });
    }

    {
        let points = points.clone();
        let weak = app.as_weak();
        app.on_cogo_selected(move |idx| {
            if let Some(app) = weak.upgrade() {
                match idx {
                    0 => {
                        let pts = points.borrow();
                        if pts.len() >= 2 {
                            let bng = bearing(pts[0], pts[1]);
                            app.set_status(SharedString::from(format!("Bearing: {:.3} rad", bng)));
                        } else {
                            app.set_status(SharedString::from("Need 2 points for bearing"));
                        }
                    }
                    1 => {
                        if let Some(start) = { points.borrow().first().copied() } {
                            let p = forward(start, 0.0, 10.0);
                            points.borrow_mut().push(p);
                            app.set_status(SharedString::from(format!(
                                "Forward point: {:.3},{:.3}",
                                p.x, p.y
                            )));
                        } else {
                            app.set_status(SharedString::from("Need start point"));
                        }
                    }
                    2 => {
                        let pts = points.borrow();
                        if pts.len() >= 4 {
                            match line_intersection(pts[0], pts[1], pts[2], pts[3]) {
                                Some(p) => {
                                    app.set_status(SharedString::from(format!(
                                        "Intersection: {:.3},{:.3}",
                                        p.x, p.y
                                    )));
                                }
                                None => {
                                    app.set_status(SharedString::from("Lines are parallel"));
                                }
                            }
                        } else {
                            app.set_status(SharedString::from("Need 4 points for intersection"));
                        }
                    }
                    3 => {
                        let elev = level_elevation(100.0, 1.2, 0.8);
                        app.set_status(SharedString::from(format!("New elevation: {:.3}", elev)));
                    }
                    4 => {
                        let pts = points.borrow();
                        if pts.len() >= 2 {
                            let a_stn = Station::new("A", pts[0]);
                            let b_stn = Station::new("B", pts[1]);
                            let ang = vertical_angle(&a_stn, 10.0, &b_stn, 14.0);
                            app.set_status(SharedString::from(format!(
                                "Vert angle: {:.3} rad",
                                ang
                            )));
                        } else {
                            app.set_status(SharedString::from("Need 2 points for vert angle"));
                        }
                    }
                    _ => {}
                }
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
                        Some(station_distance(
                            &Station::new("A", Point::new(x1, y1)),
                            &Station::new("B", Point::new(x2, y2)),
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
                        Some(level_elevation(start, bs, fs))
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
        let surfaces_clone = surfaces_for_corridor.clone();
        let alignments_clone = alignments_for_corridor.clone();
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
                        let val = VerticalAlignment::new(vec![(0.0, 0.0), (len, 0.0)]);
                        let al = Alignment::new(hal.clone(), val);
                        Some(corridor_volume(design, ground, &al, width, interval, step))
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
        let update_image = update_image.clone();
        app.on_view_changed(move |mode| {
            if let Some(app) = weak.upgrade() {
                app.set_workspace_mode(mode);
                if mode == 0 {
                    (update_image.clone())();
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
        let update_image = update_image.clone();
        app.on_workspace_clicked(move |x, y| {
            if let Some(app) = weak.upgrade() {
                if app.get_workspace_click_mode() {
                    const WIDTH: f64 = 600.0;
                    const HEIGHT: f64 = 400.0;
                    let mut p = Point::new(x as f64 - WIDTH / 2.0, HEIGHT / 2.0 - y as f64);

                    if app.get_snap_to_entities() {
                        let mut ents: Vec<DxfEntity> = Vec::new();
                        for pt in points.borrow().iter() {
                            ents.push(DxfEntity::Point { point: *pt, layer: None });
                        }
                        for (s, e) in lines.borrow().iter() {
                            ents.push(DxfEntity::Line { line: Line::new(*s, *e), layer: None });
                        }
                        for poly in polygons.borrow().iter() {
                            ents.push(DxfEntity::Polyline { polyline: Polyline::new(poly.clone()), layer: None });
                        }
                        for pl in polylines.borrow().iter() {
                            ents.push(DxfEntity::Polyline { polyline: pl.clone(), layer: None });
                        }
                        for arc in arcs.borrow().iter() {
                            ents.push(DxfEntity::Arc { arc: *arc, layer: None });
                        }
                        if let Some(sp) = snap_point(p, &ents, 5.0) {
                            p = sp;
                        }
                    }

                    if app.get_snap_to_grid() {
                        p.x = p.x.round();
                        p.y = p.y.round();
                    }

                    points.borrow_mut().push(p);
                    app.set_workspace_click_mode(false);
                    app.set_status(SharedString::from(format!(
                        "Total points: {}",
                        points.borrow().len()
                    )));
                    (update_image.clone())();
                }
            }
        });
    }

    {
        let sender = ui_tx.clone();
        app.on_workspace_mouse_moved(move |x, y| {
            let _ = sender.send(workspace3d::UiEvent::MouseMove {
                dx: x as f32,
                dy: y as f32,
            });
        });
    }

    {
        let weak_main = app.as_weak();
        std::thread::spawn(move || {
            for data in data_rx {
                let workspace3d::BevyData::CameraPosition(pos) = data;
                let txt = SharedString::from(format!("Camera: {:.1}, {:.1}, {:.1}", pos.x, pos.y, pos.z));
                let weak = weak_main.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(a) = weak.upgrade() {
                        a.set_status(txt.clone());
                    }
                });
            }
        });
    }

    app.run()
}
