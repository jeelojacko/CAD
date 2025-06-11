use std::cell::RefCell;
use std::rc::Rc;

use slint::SharedString;
use survey_cad::geometry::Point;

slint::slint! {
import { Button, VerticalBox, HorizontalBox } from "std-widgets.slint";

export component MainWindow inherits Window {
    preferred-width: 800px;
    preferred-height: 600px;

    in-out property <string> status;

    callback new_project();
    callback open_project();
    callback save_project();
    callback add_point();
    callback add_line();
    callback add_polygon();

    HorizontalBox {
        spacing: 6px;
        Button { text: "New"; clicked => { root.new_project(); } }
        Button { text: "Open"; clicked => { root.open_project(); } }
        Button { text: "Save"; clicked => { root.save_project(); } }
        Button { text: "Add Point"; clicked => { root.add_point(); } }
        Button { text: "Add Line"; clicked => { root.add_line(); } }
        Button { text: "Add Polygon"; clicked => { root.add_polygon(); } }
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

    let weak = app.as_weak();
    {
        let points = points.clone();
        let weak = weak.clone();
        app.on_new_project(move || {
            points.borrow_mut().clear();
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

    app.run()
}
