use std::cell::RefCell;
use std::rc::Rc;

use slint::{Image, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use survey_cad::crs::{list_known_crs, Crs};
use survey_cad::geometry::{Arc, Point, Polyline};
use survey_cad::surveying::{
    bearing, forward, level_elevation, line_intersection, station_distance, vertical_angle, Station,
};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

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

import { Button, VerticalBox, HorizontalBox, ComboBox, LineEdit } from "std-widgets.slint";

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
    in-out property <bool> workspace_click_mode;

    callback workspace_clicked(length, length);

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

    MenuBar {
        Menu {
            title: "File";
            MenuItem { title: "New"; activated => { root.new_project(); } }
            MenuItem { title: "Open"; activated => { root.open_project(); } }
            MenuItem { title: "Save"; activated => { root.save_project(); } }
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

    VerticalBox {
        if root.workspace_mode == 0 : Workspace2D {
            image <=> root.workspace_image;
            click_mode <=> root.workspace_click_mode;
            clicked(x, y) => { root.workspace_clicked(x, y); }
        }
        if root.workspace_mode == 1 : Rectangle {
            height: 100%;
            width: 100%;
            background: #202020;
            Text {
                text: "3D Workspace Placeholder";
                color: white;
                vertical-alignment: center;
                horizontal-alignment: center;
            }
        }
        Text { text: root.status; }
    }
}
}

fn render_workspace(points: &[Point], lines: &[(Point, Point)]) -> Image {
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

fn main() -> Result<(), slint::PlatformError> {
    let app = MainWindow::new()?;
    let points: Rc<RefCell<Vec<Point>>> = Rc::new(RefCell::new(Vec::new()));
    let lines: Rc<RefCell<Vec<(Point, Point)>>> = Rc::new(RefCell::new(Vec::new()));
    let polygons: Rc<RefCell<Vec<Vec<Point>>>> = Rc::new(RefCell::new(Vec::new()));
    let polylines: Rc<RefCell<Vec<Polyline>>> = Rc::new(RefCell::new(Vec::new()));
    let arcs: Rc<RefCell<Vec<Arc>>> = Rc::new(RefCell::new(Vec::new()));
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
    app.set_workspace_image(render_workspace(&points.borrow(), &lines.borrow()));
    app.set_workspace_click_mode(false);

    let update_image = {
        let points = points.clone();
        let lines = lines.clone();
        let weak = app.as_weak();
        std::rc::Rc::new(move || {
            if let Some(app) = weak.upgrade() {
                let img = render_workspace(&points.borrow(), &lines.borrow());
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
        let update_image = update_image.clone();
        app.on_new_project(move || {
            points.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
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
                    key_dlg.run().unwrap();
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
            dlg.run().unwrap();
        });
    }

    let weak = app.as_weak();
    {
        let lines = lines.clone();
        let update_image = update_image.clone();
        app.on_add_line(move || {
            lines
                .borrow_mut()
                .push((Point::new(0.0, 0.0), Point::new(1.0, 1.0)));
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from(format!(
                    "Total lines: {}",
                    lines.borrow().len()
                )));
            }
            (update_image.clone())();
        });
    }

    let weak = app.as_weak();
    {
        let polygons = polygons.clone();
        app.on_add_polygon(move || {
            polygons.borrow_mut().push(vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 1.0),
            ]);
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from(format!(
                    "Total polygons: {}",
                    polygons.borrow().len()
                )));
            }
        });
    }

    let weak = app.as_weak();
    {
        let polylines = polylines.clone();
        app.on_add_polyline(move || {
            polylines.borrow_mut().push(Polyline::new(vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(1.0, 1.0),
            ]));
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from(format!(
                    "Total polylines: {}",
                    polylines.borrow().len()
                )));
            }
        });
    }

    let weak = app.as_weak();
    {
        let arcs = arcs.clone();
        app.on_add_arc(move || {
            arcs.borrow_mut().push(Arc::new(
                Point::new(0.0, 0.0),
                1.0,
                0.0,
                std::f64::consts::FRAC_PI_2,
            ));
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from(format!(
                    "Total arcs: {}",
                    arcs.borrow().len()
                )));
            }
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
            dlg.run().unwrap();
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
            dlg.run().unwrap();
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
        let update_image = update_image.clone();
        app.on_workspace_clicked(move |x, y| {
            if let Some(app) = weak.upgrade() {
                if app.get_workspace_click_mode() {
                    const WIDTH: f64 = 600.0;
                    const HEIGHT: f64 = 400.0;
                    let px = x as f64 - WIDTH / 2.0;
                    let py = HEIGHT / 2.0 - y as f64;
                    points.borrow_mut().push(Point::new(px, py));
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

    app.run()
}
