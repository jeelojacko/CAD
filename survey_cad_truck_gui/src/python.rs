use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use slint::{Image, SharedString, ComponentHandle};

use survey_cad::dtm::Tin;
use survey_cad::geometry::Point;
use survey_cad::point_database::PointDatabase;

use crate::commands::{MacroPlaying, MacroRecorder, spawn_point, spawn_line};
use crate::truck_backend::TruckBackend;
use crate::ui_state::Vec2;
use crate::MainWindow;

pub struct MacroContext {
    pub playing: Rc<RefCell<MacroPlaying>>,
    pub recorder: Rc<RefCell<MacroRecorder>>,
    pub point_db: Rc<RefCell<PointDatabase>>,
    pub point_styles: Rc<RefCell<Vec<usize>>>,
    pub lines: Rc<RefCell<Vec<(Point, Point)>>>,
    pub line_styles: Rc<RefCell<Vec<usize>>>,
    pub backend: Rc<RefCell<TruckBackend>>,
    pub render_image: Rc<dyn Fn() -> Result<Image, Box<dyn std::error::Error>>>,
    pub weak: slint::Weak<MainWindow>,
}

pub struct PythonContext {
    pub weak: slint::Weak<MainWindow>,
    pub point_db: Rc<RefCell<PointDatabase>>,
    pub lines: Rc<RefCell<Vec<(Point, Point)>>>,
    pub surfaces: Rc<RefCell<Vec<Tin>>>,
    pub selected_points: Rc<RefCell<Vec<usize>>>,
    pub selected_lines: Rc<RefCell<Vec<(Point, Point)>>>,
    pub offset: Rc<RefCell<Vec2>>,
    pub zoom: Rc<RefCell<f32>>,
}

pub fn play_macro_file(path: &Path, ctx: &MacroContext) {
    if let Ok(content) = std::fs::read_to_string(path) {
        ctx.playing.borrow_mut().0 = true;
        for line in content.lines() {
            let parts = shell_words::split(line).unwrap_or_default();
            if parts.is_empty() {
                continue;
            }
            match parts[0].as_str() {
                "point" if parts.len() >= 3 => {
                    if let (Ok(x), Ok(y)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
                        spawn_point(&ctx.point_db, &ctx.point_styles, &ctx.backend, Point::new(x, y));
                    }
                }
                "line" if parts.len() >= 5 => {
                    if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                        parts[1].parse::<f64>(),
                        parts[2].parse::<f64>(),
                        parts[3].parse::<f64>(),
                        parts[4].parse::<f64>(),
                    ) {
                        spawn_line(
                            &ctx.point_db,
                            &ctx.lines,
                            &ctx.point_styles,
                            &ctx.line_styles,
                            &ctx.backend,
                            Point::new(x1, y1),
                            Point::new(x2, y2),
                        );
                    }
                }
                _ => {}
            }
        }
        ctx.playing.borrow_mut().0 = false;
        ctx.recorder.borrow_mut().file = None;
        if let Some(app) = ctx.weak.upgrade() {
            if app.get_workspace_mode() == 0 {
                crate::set_workspace_image_result(&app, &*ctx.render_image);
                app.window().request_redraw();
            }
            crate::refresh_workspace(&app, &*ctx.render_image, &ctx.backend);
        }
    }
}

pub fn run_python_file(path: &Path, ctx: &PythonContext) {
    match std::fs::read_to_string(path) {
        Ok(code) => {
            let result = Python::with_gil(|py| {
                let module = PyModule::new_bound(py, "survey_cad_python")?;
                survey_cad_python::init(py, &module)?;

                let pts: Vec<Py<survey_cad_python::Point>> = ctx
                    .point_db
                    .borrow()
                    .iter()
                    .map(|p| Py::new(py, survey_cad_python::Point::new(p.x, p.y)))
                    .collect::<PyResult<_>>()?;

                let lines_py: Vec<(Py<survey_cad_python::Point>, Py<survey_cad_python::Point>)> =
                    ctx.lines
                        .borrow()
                        .iter()
                        .map(|(a, b)| {
                            Ok((
                                Py::new(py, survey_cad_python::Point::new(a.x, a.y))?,
                                Py::new(py, survey_cad_python::Point::new(b.x, b.y))?,
                            ))
                        })
                        .collect::<PyResult<_>>()?;

                let surfs: Vec<Py<PyAny>> = ctx
                    .surfaces
                    .borrow()
                    .iter()
                    .map(|s| {
                        let dict = PyDict::new_bound(py);
                        let verts: Vec<(f64, f64, f64)> =
                            s.vertices.iter().map(|v| (v.x, v.y, v.z)).collect();
                        let tris: Vec<(usize, usize, usize)> =
                            s.triangles.iter().map(|t| (t[0], t[1], t[2])).collect();
                        dict.set_item("vertices", verts)?;
                        dict.set_item("triangles", tris)?;
                        Ok(dict.into())
                    })
                    .collect::<PyResult<_>>()?;

                let selected_pts: Vec<usize> = ctx.selected_points.borrow().to_vec();
                let selected_lines_py: Vec<(Py<survey_cad_python::Point>, Py<survey_cad_python::Point>)> = ctx
                    .selected_lines
                    .borrow()
                    .iter()
                    .map(|(a, b)| {
                        Ok((
                            Py::new(py, survey_cad_python::Point::new(a.x, a.y))?,
                            Py::new(py, survey_cad_python::Point::new(b.x, b.y))?,
                        ))
                    })
                    .collect::<PyResult<_>>()?;

                let view = PyDict::new_bound(py);
                let off = ctx.offset.borrow();
                view.set_item("offset", (off.x, off.y))?;
                view.set_item("zoom", *ctx.zoom.borrow())?;

                let globals = PyDict::new_bound(py);
                globals.set_item("survey_cad_python", module)?;
                globals.set_item("points", pts)?;
                globals.set_item("lines", lines_py)?;
                globals.set_item("surfaces", surfs)?;
                globals.set_item("selected_points", selected_pts)?;
                globals.set_item("selected_lines", selected_lines_py)?;
                globals.set_item("view", view)?;

                py.run_bound(&code, Some(&globals), None)
            });

            match result {
                Ok(_) => {
                    if let Some(app) = ctx.weak.upgrade() {
                        app.set_status(SharedString::from("Python script finished"));
                    }
                }
                Err(e) => {
                    if let Some(app) = ctx.weak.upgrade() {
                        app.set_status(SharedString::from(format!("Python error: {e}")));
                    }
                }
            }
        }
        Err(e) => {
            if let Some(app) = ctx.weak.upgrade() {
                app.set_status(SharedString::from(format!("Failed to read: {e}")));
            }
        }
    }
}
