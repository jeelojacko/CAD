use std::cell::RefCell;
use std::rc::Rc;

use slint::{SharedString, VecModel};
use survey_cad::crs::{list_known_crs, Crs};
use survey_cad::geometry::{Arc, Point, Polyline};

slint::slint! {
import { Button, VerticalBox, HorizontalBox, ComboBox } from "std-widgets.slint";

export component MainWindow inherits Window {
    preferred-width: 800px;
    preferred-height: 600px;

    in-out property <string> status;
    in property <[string]> crs_list;
    in-out property <int> crs_index;

    callback crs_changed(int);

    callback new_project();
    callback open_project();
    callback save_project();
    callback add_point();
    callback add_line();
    callback add_polygon();
    callback add_polyline();
    callback add_arc();
    callback clear_workspace();

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
        Rectangle {
            height: 100%;
            width: 100%;
            background: #202020;
            Text {
                text: "2D/3D Workspace Placeholder";
                color: white;
                vertical-alignment: center;
                horizontal-alignment: center;
            }
        }
        Text { text: root.status; }
    }
}
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
    let working_crs = Rc::new(RefCell::new(Crs::wgs84()));

    let weak = app.as_weak();
    {
        let points = points.clone();
        let weak = weak.clone();
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        app.on_new_project(move || {
            points.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("New project created"));
            }
        });
    }

    let weak = app.as_weak();
    {
        let points = points.clone();
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
        app.on_add_point(move || {
            points.borrow_mut().push(Point::new(0.0, 0.0));
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from(format!(
                    "Total points: {}",
                    points.borrow().len()
                )));
            }
        });
    }

    let weak = app.as_weak();
    {
        let lines = lines.clone();
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
        app.on_clear_workspace(move || {
            points.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Workspace cleared"));
            }
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

    app.run()
}
