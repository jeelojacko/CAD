#![allow(unused_variables)]

use i_slint_common::sharedfontdb;
use slint::platform::PointerEventButton;
use slint::{Model, PhysicalSize, SharedString, VecModel, Image};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use survey_cad::alignment::{Alignment, AlignmentGroup, VerticalAlignment};
use survey_cad::corridor;
use survey_cad::crs::list_known_crs;
use survey_cad::dtm::{SurfaceGroup, Tin};
use survey_cad::geometry::point::PointStyle;
use survey_cad::geometry::{
    convex_hull, Arc, Line, LineStyle, LineType, LinearDimension, Point, Point3 as ScPoint3,
    Polyline,
};
use survey_cad::io::project::{read_project_json, write_project_json, GridSettings, Project};
use survey_cad::layers::{Layer, LayerManager as ScLayerManager};
use survey_cad::point_database::PointDatabase;
use survey_cad::styles::{default_alignment_styles, LineWeight, TextStyle as ScTextStyle};
use survey_cad::subassembly;
use survey_cad::superelevation::SuperelevationPoint;
mod snap;
use std::fs;
use std::path::Path;
use truck_modeling::base::InnerSpace;
use truck_modeling::base::Point3;
use truck_modeling::base::Vector3;

/// Default directory for recorded macros and Python scripts.
/// Can be overridden using the `--macro-dir` command line option or the
/// `macro_dir` entry in the saved configuration.
const DEFAULT_MACRO_DIR: &str = "macros";

mod truck_backend;
use truck_backend::{HitObject, TruckBackend};
mod persistence;
use persistence::{load_layers, load_styles, save_layers, save_styles, StyleSettings};
mod commands;
mod error;
mod cross_section;
mod inspector;
mod io_utils;
#[cfg(feature = "python")]
mod python;
#[cfg(not(feature = "python"))]
mod python {
    use crate::error::GuiError;
    use std::path::Path;

    #[derive(Clone)]
    pub struct MacroContext;
    #[derive(Clone)]
    pub struct PythonContext;

    pub fn play_macro_file(_path: &Path, _ctx: &MacroContext) {}
    pub fn run_python_file(_path: &Path, _ctx: &PythonContext) -> Result<(), GuiError> {
        Err(GuiError::Msg("Python support disabled".into()))
    }
}
mod render;
mod ui_state;
mod workspace;
mod dialogs;
mod geometry;
mod pipe_editor;

use commands::{
    record_macro, spawn_line, spawn_point, Command, CommandStack, Context, MacroPlaying,
    MacroRecorder,
};
pub use cross_section::{
    calc_section_params, grade_at, handle_positions, nearest_point, render_cross_section,
    screen_to_world, SectionParams,
};
pub use inspector::{
    has_selection, show_context_menu, show_inspector_for_point, show_inspector_for_polygon,
};
pub use io_utils::{read_arc_csv, read_line_csv, read_points_list};
use crate::pipe_editor::{PipeEditor, Pipe, Structure};
use once_cell::sync::Lazy;
#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::PyDict;
use python::{play_macro_file, run_python_file, MacroContext, PythonContext};
use render::{
    arc_from_start_end_radius, arc_from_three_points, polyline_to_solid, screen_to_workspace,
    workspace_to_screen, RenderState, RenderStyles, WorkspaceRenderData,
};
use error::GuiError;
use rusttype::Font;
use tiny_skia::Pixmap;
use ui_state::{
    load_config, save_config, CursorFeedback, DragSelect, DrawingMode, Theme, Vec2,
    WorkspaceProfile,
};
pub use workspace::{refresh_workspace, render_workspace, set_workspace_image_result};

slint::include_modules!();

// The default font embedded at build time. The path is provided by build.rs via
// the `DEFAULT_FONT_PATH` environment variable, which can be overridden with
// `SURVEY_CAD_FONT` when building.
static FONT_DATA: &[u8] = include_bytes!(env!("DEFAULT_FONT_PATH"));
static FONT: Lazy<Font<'static>> = Lazy::new(|| Font::try_from_bytes(FONT_DATA).unwrap());

fn main() -> Result<(), slint::PlatformError> {
    let mut cmd_font: Option<String> = None;
    let mut cmd_macro_dir: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--font-path" => {
                if let Some(p) = args.next() {
                    cmd_font = Some(p);
                }
            }
            "--macro-dir" => {
                if let Some(p) = args.next() {
                    cmd_macro_dir = Some(p);
                }
            }
            _ => {}
        }
    }

    let mut cfg = load_config();
    if let Some(p) = cmd_font {
        cfg.font_path = Some(p);
        save_config(&cfg);
    }
    if let Some(p) = cmd_macro_dir {
        cfg.macro_dir = Some(p);
        save_config(&cfg);
    }
    let config = Rc::new(RefCell::new(cfg));

    let macro_dir: Rc<String> = Rc::new(
        config
            .borrow()
            .macro_dir
            .clone()
            .unwrap_or_else(|| DEFAULT_MACRO_DIR.to_string()),
    );
    let _ = fs::create_dir_all(&*macro_dir);

    let backend = Rc::new(RefCell::new(TruckBackend::new(
        config.borrow().window_width,
        config.borrow().window_height,
    )));
    // Always populate the font database with the system fonts first so that the
    // embedded font can complement, rather than replace, them. This ensures
    // that built-in controls can resolve their default fonts while we still
    // provide our bundled DejaVuSans.
    sharedfontdb::FONT_DB.with_borrow_mut(|db| db.make_mut().load_system_fonts());
    if let Some(path) = config.borrow().font_path.as_ref() {
        if sharedfontdb::register_font_from_path(Path::new(path)).is_err() {
            sharedfontdb::register_font_from_memory(FONT_DATA)
                .expect("failed to register embedded font");
        }
    } else {
        sharedfontdb::register_font_from_memory(FONT_DATA)
            .expect("failed to register embedded font");
    }
    match config.borrow().theme {
        Theme::Dark => std::env::set_var("SLINT_STYLE", "fluent-dark"),
        Theme::Light => std::env::set_var("SLINT_STYLE", "fluent-light"),
    }
    let app = MainWindow::new()?;
    app.window().set_size(PhysicalSize::new(
        config.borrow().window_width,
        config.borrow().window_height,
    ));

    let snap_prefs = Rc::new(RefCell::new(config.borrow().snap.clone()));
    {
        let p = snap_prefs.borrow();
        app.set_snap_to_grid(p.snap_to_grid);
        app.set_snap_to_entities(p.snap_to_entities);
        app.set_snap_endpoints(p.snap_endpoints);
        app.set_snap_points(p.snap_points);
        app.set_snap_intersections(p.snap_intersections);
        app.set_snap_midpoints(p.snap_midpoints);
        app.set_snap_nearest(p.snap_nearest);
        app.set_snap_surfaces(p.snap_surfaces);
        app.set_snap_solids(p.snap_solids);
        app.set_snap_tolerance(p.snap_tolerance);
    }
    let last_folder = Rc::new(RefCell::new(config.borrow().last_open_dir.clone()));
    let window_size = Rc::new(RefCell::new(app.window().size()));

    // example data so the 2D workspace has something to draw
    let example_line = Line::new(Point::new(0.0, 0.0), Point::new(50.0, 50.0));
    let point_db = Rc::new(RefCell::new(PointDatabase::new()));
    let lines = Rc::new(RefCell::new(vec![(example_line.start, example_line.end)]));
    let polygons = Rc::new(RefCell::new(Vec::<Vec<Point>>::new()));
    let polylines = Rc::new(RefCell::new(Vec::<Polyline>::new()));
    let arcs = Rc::new(RefCell::new(Vec::<Arc>::new()));
    let dimensions = Rc::new(RefCell::new(Vec::<LinearDimension>::new()));
    let pipe_editor = Rc::new(RefCell::new(PipeEditor::new(backend.clone())));
    let surfaces = Rc::new(RefCell::new(Vec::<Tin>::new()));
    let surface_groups = Rc::new(RefCell::new(Vec::<SurfaceGroup>::new()));
    let surface_units = Rc::new(RefCell::new(Vec::<String>::new()));
    let surface_styles = Rc::new(RefCell::new(Vec::<String>::new()));
    let surface_descriptions = Rc::new(RefCell::new(Vec::<String>::new()));
    let alignments = Rc::new(RefCell::new(Vec::<Alignment>::new()));
    let alignment_groups = Rc::new(RefCell::new(Vec::<AlignmentGroup>::new()));
    let superelevation = Rc::new(RefCell::new(Vec::<SuperelevationPoint>::new()));
    let layers = Rc::new(RefCell::new(ScLayerManager::new()));
    let layer_names = Rc::new(RefCell::new(Vec::<String>::new()));
    if let Some(saved) = load_layers(Path::new("layers.json")) {
        *layers.borrow_mut() = saved;
        layer_names
            .borrow_mut()
            .extend(layers.borrow().iter().map(|l| l.name.clone()));
    } else {
        let mut mgr = layers.borrow_mut();
        let default = Layer::new("DEFAULT");
        mgr.add_layer(default);
        layer_names.borrow_mut().push("DEFAULT".to_string());
    }

    let zoom = Rc::new(RefCell::new(1.0_f32));
    let offset = Rc::new(RefCell::new(Vec2::default()));
    let grid_settings = Rc::new(RefCell::new(GridSettings::default()));
    let workspace_crs = Rc::new(RefCell::new(4326u32));
    let workspace_pixmap = Rc::new(RefCell::new(Pixmap::new(1, 1).unwrap()));
    let pan_2d_flag = Rc::new(RefCell::new(false));
    let last_pos_2d = Rc::new(RefCell::new((0.0_f64, 0.0_f64)));
    let rotate_flag = Rc::new(RefCell::new(false));
    let pan_flag = Rc::new(RefCell::new(false));
    let last_pos = Rc::new(RefCell::new((0.0_f64, 0.0_f64)));
    let selected_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_lines = Rc::new(RefCell::new(Vec::<(Point, Point)>::new()));
    let selected_polygons = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_polylines = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_arcs = Rc::new(RefCell::new(Vec::<usize>::new()));
    let selected_dimensions = Rc::new(RefCell::new(Vec::<usize>::new()));
    let drag_select = Rc::new(RefCell::new(DragSelect::default()));
    let cursor_feedback = Rc::new(RefCell::new(None));
    let snap_target = Rc::new(RefCell::new(None::<Point>));
    let drawing_mode = Rc::new(RefCell::new(DrawingMode::None));
    let last_click = Rc::new(RefCell::new(None));
    let selected_surface = Rc::new(RefCell::new(None::<usize>));
    let click_pos_3d = Rc::new(RefCell::new(None::<(f64, f64)>));
    let active_handle = Rc::new(RefCell::new(None::<usize>));
    let context_menu = Rc::new(RefCell::new(None::<slint::Weak<ContextMenu>>));
    let current_line: Rc<RefCell<Option<Polyline>>> = Rc::new(RefCell::new(None));
    let point_style_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let point_layers = Rc::new(RefCell::new(Vec::<usize>::new()));
    let line_layers = Rc::new(RefCell::new(Vec::<usize>::new()));
    let polygon_layers = Rc::new(RefCell::new(Vec::<usize>::new()));
    let point_metadata = Rc::new(RefCell::new(Vec::<String>::new()));
    let line_metadata = Rc::new(RefCell::new(Vec::<String>::new()));
    let point_elevation = Rc::new(RefCell::new(Vec::<String>::new()));
    let point_measurement = Rc::new(RefCell::new(Vec::<String>::new()));
    let point_data_sets = Rc::new(RefCell::new(Vec::<usize>::new()));
    let data_set_names = Rc::new(RefCell::new(vec![String::from("Default")]));
    let inspector_window: Rc<RefCell<Option<slint::Weak<EntityInspector>>>> =
        Rc::new(RefCell::new(None));
    let style_settings = load_styles(Path::new("styles.json")).unwrap_or_else(|| StyleSettings {
        point_styles: survey_cad::styles::default_point_styles(),
        line_styles: survey_cad::styles::default_line_styles(),
        polygon_styles: survey_cad::styles::default_polygon_styles(),
        alignment_styles: survey_cad::styles::default_alignment_styles(),
        line_label_styles: survey_cad::styles::default_line_label_styles(),
        point_label_styles: survey_cad::styles::default_point_label_styles(),
    });
    let point_styles = style_settings.point_styles.clone();
    let point_style_names: Vec<SharedString> = point_styles
        .iter()
        .map(|(n, _)| SharedString::from(n.clone()))
        .collect();
    let point_style_values: Vec<PointStyle> = point_styles.iter().map(|(_, s)| *s).collect();

    let line_styles = style_settings.line_styles.clone();
    let line_style_indices = Rc::new(RefCell::new(vec![0; line_styles.len()]));

    let polygon_styles = style_settings.polygon_styles.clone();
    let polygon_style_indices = Rc::new(RefCell::new(Vec::<usize>::new()));
    let polygon_style_names: Vec<SharedString> = polygon_styles
        .iter()
        .map(|(n, _)| SharedString::from(n.clone()))
        .collect();
    let polygon_style_values: Vec<survey_cad::styles::PolygonStyle> =
        polygon_styles.iter().map(|(_, s)| *s).collect();

    let alignment_styles = style_settings.alignment_styles.clone();
    let alignment_style = alignment_styles
        .first()
        .map(|(_, s)| *s)
        .unwrap_or_else(|| default_alignment_styles()[0].1);
    let command_stack = Rc::new(RefCell::new(CommandStack::new()));
    let macro_recorder = Rc::new(RefCell::new(MacroRecorder::default()));
    let macro_playing = Rc::new(RefCell::new(MacroPlaying::default()));
    let command_history = Rc::new(VecModel::<SharedString>::from(Vec::new()));
    let line_type_names = Rc::new(VecModel::from(vec![
        SharedString::from("Solid"),
        SharedString::from("Dashed"),
        SharedString::from("Dotted"),
    ]));
    let line_label_styles = style_settings.line_label_styles.clone();
    let point_label_styles = style_settings.point_label_styles.clone();
    let point_label_style = Rc::new(RefCell::new(point_label_styles[0].1.clone()));
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
        let lines = lines.clone();
        let indices = line_style_indices.clone();
        Rc::new(move || {
            let needed = style_names.len();
            {
                let mut idx = indices.borrow_mut();
                if idx.len() < needed {
                    idx.resize(needed, 0);
                }
            }
            let style_model = Rc::new(VecModel::from((*style_names).clone()));
            let current_indices = indices.borrow().clone();
            let current_lines = lines.borrow().clone();
            let rows = current_indices
                .iter()
                .enumerate()
                .map(|(i, s_idx)| {
                    if let Some((s, e)) = current_lines.get(i) {
                        LineRow {
                            start: SharedString::from(format!(
                                "{sx:.2},{sy:.2}",
                                sx = s.x,
                                sy = s.y
                            )),
                            end: SharedString::from(format!("{ex:.2},{ey:.2}", ex = e.x, ey = e.y)),
                            style_index: *s_idx as i32,
                        }
                    } else {
                        LineRow {
                            start: SharedString::from(""),
                            end: SharedString::from(""),
                            style_index: *s_idx as i32,
                        }
                    }
                })
                .collect::<Vec<_>>();
            let line_model = Rc::new(VecModel::from(rows));
            dialogs.borrow_mut().retain(|d| {
                if let Some(dlg) = d.upgrade() {
                    dlg.set_styles_model(style_model.clone().into());
                    dlg.set_lines_model(line_model.clone().into());
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
        let surface_units_ref = surface_units.clone();
        let surface_styles_ref = surface_styles.clone();
        let surface_descriptions_ref = surface_descriptions.clone();
        let alignments = alignments.clone();
        let zoom = zoom.clone();
        let offset = offset.clone();
        let pixmap_ref = workspace_pixmap.clone();
        let selected_indices = selected_indices.clone();
        let drag_select = drag_select.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let dimensions = dimensions.clone();
        let selected_dimensions = selected_dimensions.clone();
        let style_indices = point_style_indices.clone();
        let point_styles = point_style_values.clone();
        let line_styles_vals = line_style_values.clone();
        let line_style_indices = line_style_indices.clone();
        let polygon_style_indices = polygon_style_indices.clone();
        let cursor_feedback = cursor_feedback.clone();
        let snap_target = snap_target.clone();
        let drawing_mode = drawing_mode.clone();
        let label_style = line_label_styles[0].1.clone();
        let point_label_style = point_label_style.clone();
        let grid_settings_ref = grid_settings.clone();
        move || {
            let Some(app) = app_weak.upgrade() else {
                return Err(GuiError::Msg("window closed".into()));
            };
            let size = app.window().size();
            if size.width == 0 || size.height == 0 {
                return Ok(Image::default());
            }
            let show_numbers = app_weak
                .upgrade()
                .map(|a| a.get_show_point_numbers())
                .unwrap_or(true);
            render_workspace(
                &WorkspaceRenderData {
                    points: &point_db.borrow(),
                    lines: &lines.borrow(),
                    polygons: &polygons.borrow(),
                    polylines: &polylines.borrow(),
                    arcs: &arcs.borrow(),
                    dimensions: &dimensions.borrow(),
                    surfaces: &surfaces.borrow(),
                    alignments: &alignments.borrow(),
                },
                &RenderState {
                    offset: &offset,
                    zoom: &zoom,
                    pixmap: &pixmap_ref,
                    selected: &selected_indices,
                    selected_lines: &selected_lines,
                    selected_polygons: &selected_polygons,
                    selected_polylines: &selected_polylines,
                    selected_arcs: &selected_arcs,
                    selected_dimensions: &selected_dimensions,
                    drag: &drag_select,
                    cursor_feedback: &cursor_feedback,
                    snap_target: &snap_target,
                },
                &RenderStyles {
                    point_styles: &point_styles,
                    style_indices: &style_indices,
                    line_styles: &line_styles_vals,
                    line_style_indices: &line_style_indices,
                    polygon_styles: &polygon_style_values,
                    polygon_style_indices: &polygon_style_indices,
                    alignment_style: &alignment_style,
                    show_labels: true,
                    label_style: &label_style,
                    point_label_style: &point_label_style.borrow(),
                    show_point_numbers: show_numbers,
                },
                &drawing_mode.borrow(),
                &grid_settings_ref.borrow(),
                size.width,
                size.height,
            )
        }
    };

    // basic CRS list as before
    let crs_entries = list_known_crs();
    let crs_entries_rc = Rc::new(crs_entries);
    let crs_model = Rc::new(VecModel::from(
        crs_entries_rc
            .iter()
            .map(|e| SharedString::from(format!("{} - {}", e.code, e.name)))
            .collect::<Vec<_>>(),
    ));
    let default_idx = crs_entries_rc
        .iter()
        .position(|e| e.code == format!("EPSG:{}", *workspace_crs.borrow()))
        .unwrap_or(0);
    app.set_crs_list(crs_model.into());
    app.set_crs_index(default_idx as i32);
    app.set_workspace_mode(0); // start with 2D mode
    app.set_show_point_numbers(true);
    app.set_command_history(command_history.clone().into());
    app.set_command_text(SharedString::from(""));
    app.set_input_value(SharedString::from(""));

    {
        let weak = app.as_weak();
        app.on_button_hovered(move |txt| {
            if let Some(app) = weak.upgrade() {
                app.set_status(txt.clone());
            }
        });
    }

    {
        let weak = app.as_weak();
        app.on_menu_hovered(move |txt| {
            if let Some(app) = weak.upgrade() {
                app.set_status(txt.clone());
            }
        });
    }

    app.on_open_help(move |path| {
        if let Err(e) = open::that(path.as_str()) {
            eprintln!("Failed to open help: {e}");
        }
    });

    // show camera controls in the status bar
    app.set_status(SharedString::from(
        "Camera: Left drag orbit, middle drag pan, scroll zoom",
    ));

    // prepare initial 2D workspace image and schedule continuous redraws
    crate::set_workspace_image_result(&app, &render_image);
    app.window().request_redraw();

    {
        use slint::{Timer, TimerMode};
        use std::rc::Rc;

        let weak = app.as_weak();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let command_stack = command_stack.clone();
        let timer = Rc::new(Timer::default());
        let timer_handle = timer.clone();

        // Perform an initial refresh immediately
        if let Some(app) = weak.upgrade() {
            refresh_workspace(&app, &render_image, &backend_render);
        }

        timer.start(
            TimerMode::Repeated,
            core::time::Duration::from_millis(16),
            move || {
                if let Some(app) = weak.upgrade() {
                    refresh_workspace(&app, &render_image, &backend_render);
                } else {
                    timer_handle.stop();
                }
            },
        );

        use slint::CloseRequestResponse;
        let timer_handle = timer.clone();
        let cfg = config.clone();
        let snap = snap_prefs.clone();
        let win = window_size.clone();
        let last_dir = last_folder.clone();
        app.window().on_close_requested(move || {
            timer_handle.stop();
            {
                let mut c = cfg.borrow_mut();
                c.window_width = win.borrow().width;
                c.window_height = win.borrow().height;
                c.last_open_dir = last_dir.borrow().clone();
                c.snap = snap.borrow().clone();
                save_config(&c);
            }
            CloseRequestResponse::HideWindow
        });
    }

    {
        let recorder = macro_recorder.clone();
        let macro_dir = macro_dir.clone();
        app.on_macro_record(move || {
            if recorder.borrow().file.is_some() {
                recorder.borrow_mut().file = None;
            } else if let Some(path) = rfd::FileDialog::new()
                .add_filter("Text", &["txt"])
                .set_directory(&*macro_dir)
                .save_file()
            {
                if let Ok(f) = std::fs::File::create(&path) {
                    recorder.borrow_mut().file = Some(f);
                }
            }
        });
    }

    {
        let recorder = macro_recorder.clone();
        let playing = macro_playing.clone();
        let point_db = point_db.clone();
        let point_styles = point_style_indices.clone();
        let lines_ref = lines.clone();
        let line_styles = line_style_indices.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        let macro_dir = macro_dir.clone();
        let weak = app.as_weak();
        app.on_macro_play(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Text", &["txt"])
                .set_directory(&*macro_dir)
                .pick_file()
            {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    playing.borrow_mut().0 = true;
                    for line in content.lines() {
                        let parts = shell_words::split(line).unwrap_or_default();
                        if parts.is_empty() {
                            continue;
                        }
                        match parts[0].as_str() {
                            "point" if parts.len() >= 3 => {
                                if let (Ok(x), Ok(y)) =
                                    (parts[1].parse::<f64>(), parts[2].parse::<f64>())
                                {
                                    spawn_point(
                                        &point_db,
                                        &point_styles,
                                        &backend_render,
                                        Point::new(x, y),
                                    );
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
                                        &point_db,
                                        &lines_ref,
                                        &point_styles,
                                        &line_styles,
                                        &backend_render,
                                        Point::new(x1, y1),
                                        Point::new(x2, y2),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    playing.borrow_mut().0 = false;
                    recorder.borrow_mut().file = None;
                    if let Some(app) = weak.upgrade() {
                        if app.get_workspace_mode() == 0 {
                            crate::set_workspace_image_result(&app, &render_image);
                            app.window().request_redraw();
                        }
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let surfaces_ref = surfaces.clone();
        let macro_dir = macro_dir.clone();
        #[cfg(feature = "python")]
        app.on_run_python_script(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Python", &["py"])
                .set_directory(&*macro_dir)
                .pick_file()
            {
                match std::fs::read_to_string(&path) {
                    Ok(code) => {
                        let result = Python::with_gil(|py| {
                            let module = PyModule::new_bound(py, "survey_cad_python")?;
                            survey_cad_python::init(py, &module)?;

                            let pts: Vec<Py<survey_cad_python::Point>> = point_db
                                .borrow()
                                .iter()
                                .map(|p| Py::new(py, survey_cad_python::Point::new(p.x, p.y)))
                                .collect::<PyResult<_>>()?;

                            let lines_py: Vec<(
                                Py<survey_cad_python::Point>,
                                Py<survey_cad_python::Point>,
                            )> = lines_ref
                                .borrow()
                                .iter()
                                .map(|(a, b)| {
                                    Ok((
                                        Py::new(py, survey_cad_python::Point::new(a.x, a.y))?,
                                        Py::new(py, survey_cad_python::Point::new(b.x, b.y))?,
                                    ))
                                })
                                .collect::<PyResult<_>>()?;

                            let surfs: Vec<Py<PyAny>> = surfaces_ref
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

                            let globals = PyDict::new_bound(py);
                            globals.set_item("survey_cad_python", module)?;
                            globals.set_item("points", pts)?;
                            globals.set_item("lines", lines_py)?;
                            globals.set_item("surfaces", surfs)?;

                            py.run_bound(&code, Some(&globals), None)
                        });

                        match result {
                            Ok(_) => {
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from("Python script finished"));
                                }
                            }
                            Err(e) => {
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from(format!(
                                        "Python error: {e}"
                                    )));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to read: {e}")));
                        }
                    }
                }
            }
        });
        #[cfg(not(feature = "python"))]
        app.on_run_python_script(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Python support disabled"));
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let surfaces_ref = surfaces.clone();
        let playing = macro_playing.clone();
        let recorder = macro_recorder.clone();
        let point_styles = point_style_indices.clone();
        let line_styles = line_style_indices.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        let cfg = config.clone();
        let macro_dir = macro_dir.clone();
        let selected_indices_ml = selected_indices.clone();
        let selected_lines_ml = selected_lines.clone();
        let offset_ml = offset.clone();
        let zoom_ml = zoom.clone();
        app.on_show_macro_list(move || {
            let mut items = Vec::new();
            if let Ok(rd) = fs::read_dir(&*macro_dir) {
                for ent in rd.flatten() {
                    if let Some(ext) = ent.path().extension().and_then(|e| e.to_str()) {
                        if ext == "txt" || ext == "py" {
                            if let Some(n) = ent.file_name().to_str() {
                                items.push(SharedString::from(n.to_string()));
                            }
                        }
                    }
                }
            }

            let dlg = MacroListDialog::new().unwrap();
            dlg.set_files(Rc::new(VecModel::from(items.clone())).into());
            dlg.set_selected_index(0);
            let dlg_weak_run = dlg.as_weak();
            let weak_run = weak.clone();
            let point_db_run = point_db.clone();
            let lines_run = lines_ref.clone();
            let surfaces_run = surfaces_ref.clone();
            let playing_run = playing.clone();
            let recorder_run = recorder.clone();
            let point_styles_run = point_styles.clone();
            let line_styles_run = line_styles.clone();
            let backend_run = backend_render.clone();
            let render_image_run = render_image.clone();
            let items_run = items.clone();
            let selected_indices = selected_indices_ml.clone();
            let selected_lines = selected_lines_ml.clone();
            let offset = offset_ml.clone();
            let zoom = zoom_ml.clone();
            let macro_dir = macro_dir.clone();
            dlg.on_run(move |idx| {
                if let Some(name) = items_run.get(idx as usize) {
                    let path = Path::new(&*macro_dir).join(name.as_str());
                    if name.ends_with(".py") {
                        #[cfg(feature = "python")]
                        let ctx_py = PythonContext {
                            weak: weak_run.clone(),
                            point_db: point_db_run.clone(),
                            lines: lines_run.clone(),
                            surfaces: surfaces_run.clone(),
                            selected_points: selected_indices.clone(),
                            selected_lines: selected_lines.clone(),
                            offset: offset.clone(),
                            zoom: zoom.clone(),
                        };
                        #[cfg(not(feature = "python"))]
                        let ctx_py = PythonContext;
                        if let Err(e) = run_python_file(&path, &ctx_py) {
                            if let Some(app) = weak_run.upgrade() {
                                app.set_status(SharedString::from(format!("Python error: {e}")));
                            }
                        }
                    } else {
                        #[cfg(feature = "python")]
                        let ctx = MacroContext {
                            playing: playing_run.clone(),
                            recorder: recorder_run.clone(),
                            point_db: point_db_run.clone(),
                            point_styles: point_styles_run.clone(),
                            lines: lines_run.clone(),
                            line_styles: line_styles_run.clone(),
                            backend: backend_run.clone(),
                            render_image: Rc::new(render_image_run.clone()),
                            weak: weak_run.clone(),
                        };
                        #[cfg(not(feature = "python"))]
                        let ctx = MacroContext;
                        play_macro_file(&path, &ctx);
                    }
                }
                if let Some(d) = dlg_weak_run.upgrade() {
                    let _ = d.hide();
                }
            });

            let cfg_assign = cfg.clone();
            let items_assign = items.clone();
            dlg.on_assign(move |idx, slot| {
                if let Some(name) = items_assign.get(idx as usize) {
                    let mut cfg_borrow = cfg_assign.borrow_mut();
                    if slot as usize >= cfg_borrow.quick_scripts.len() {
                        cfg_borrow
                            .quick_scripts
                            .resize(slot as usize + 1, String::new());
                    }
                    cfg_borrow.quick_scripts[slot as usize] = name.to_string();
                    save_config(&cfg_borrow);
                }
            });

            let dlg_weak_cancel = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = dlg_weak_cancel.upgrade() {
                    let _ = d.hide();
                }
            });

            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let surfaces_ref = surfaces.clone();
        let selected_indices_ref = selected_indices.clone();
        let selected_lines_ref = selected_lines.clone();
        let offset_ref = offset.clone();
        let zoom_ref = zoom.clone();
        let macro_dir = macro_dir.clone();
        app.on_open_script_panel(move || {
            let mut items = Vec::new();
            if let Ok(rd) = fs::read_dir(&*macro_dir) {
                for ent in rd.flatten() {
                    if let Some(ext) = ent.path().extension().and_then(|e| e.to_str()) {
                        if ext == "py" {
                            if let Some(n) = ent.file_name().to_str() {
                                items.push(SharedString::from(n.to_string()));
                            }
                        }
                    }
                }
            }

            let panel = ScriptPanel::new().unwrap();
            panel.set_files(Rc::new(VecModel::from(items.clone())).into());
            panel.set_selected_index(0);
            let weak_run = weak.clone();
            let point_db_run = point_db.clone();
            let lines_run = lines_ref.clone();
            let surfaces_run = surfaces_ref.clone();
            let panel_weak = panel.as_weak();
            let items_run = items.clone();
            let selected_indices_run = selected_indices_ref.clone();
            let selected_lines_run = selected_lines_ref.clone();
            let offset_run = offset_ref.clone();
            let zoom_run = zoom_ref.clone();
            let macro_dir = macro_dir.clone();
            panel.on_run(move |idx| {
                if let Some(name) = items_run.get(idx as usize) {
                    let path = Path::new(&*macro_dir).join(name.as_str());
                    #[cfg(feature = "python")]
                    let ctx_py = PythonContext {
                        weak: weak_run.clone(),
                        point_db: point_db_run.clone(),
                        lines: lines_run.clone(),
                        surfaces: surfaces_run.clone(),
                        selected_points: selected_indices_run.clone(),
                        selected_lines: selected_lines_run.clone(),
                        offset: offset_run.clone(),
                        zoom: zoom_run.clone(),
                    };
                    #[cfg(not(feature = "python"))]
                    let ctx_py = PythonContext;
                    if let Err(e) = run_python_file(&path, &ctx_py) {
                        if let Some(app) = weak_run.upgrade() {
                            app.set_status(SharedString::from(format!("Python error: {e}")));
                        }
                    }
                }
            });

            panel.on_close(move || {
                if let Some(p) = panel_weak.upgrade() {
                    let _ = p.hide();
                }
            });

            panel.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let cfg = config.clone();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let surfaces_ref = surfaces.clone();
        let playing = macro_playing.clone();
        let recorder = macro_recorder.clone();
        let point_styles = point_style_indices.clone();
        let line_styles = line_style_indices.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let offset = offset.clone();
        let zoom = zoom.clone();
        let macro_dir = macro_dir.clone();
        app.on_run_quick_script(move |slot| {
            let scripts = &cfg.borrow().quick_scripts;
            if let Some(name) = scripts.get(slot as usize) {
                if name.is_empty() {
                    return;
                }
                let path = Path::new(&*macro_dir).join(name);
                if name.ends_with(".py") {
                    #[cfg(feature = "python")]
                    let ctx_py = PythonContext {
                        weak: weak.clone(),
                        point_db: point_db.clone(),
                        lines: lines_ref.clone(),
                        surfaces: surfaces_ref.clone(),
                        selected_points: selected_indices.clone(),
                        selected_lines: selected_lines.clone(),
                        offset: offset.clone(),
                        zoom: zoom.clone(),
                    };
                    #[cfg(not(feature = "python"))]
                    let ctx_py = PythonContext;
                    if let Err(e) = run_python_file(&path, &ctx_py) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Python error: {e}")));
                        }
                    }
                } else {
                    #[cfg(feature = "python")]
                    let ctx = MacroContext {
                        playing: playing.clone(),
                        recorder: recorder.clone(),
                        point_db: point_db.clone(),
                        point_styles: point_styles.clone(),
                        lines: lines_ref.clone(),
                        line_styles: line_styles.clone(),
                        backend: backend_render.clone(),
                        render_image: Rc::new(render_image.clone()),
                        weak: weak.clone(),
                    };
                    #[cfg(not(feature = "python"))]
                    let ctx = MacroContext;
                    play_macro_file(&path, &ctx);
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let surfaces = surfaces.clone();
        let selected_indices = selected_indices.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_create_surface_from_selection(move || {
            let sc_pts: Vec<ScPoint3> = selected_indices
                .borrow()
                .iter()
                .filter_map(|&i| point_db.borrow().get(i).copied())
                .map(|p| ScPoint3::new(p.x, p.y, 0.0))
                .collect();
            if sc_pts.len() >= 3 {
                let tin = survey_cad::dtm::Tin::from_points(sc_pts.clone());
                let verts: Vec<Point3> = tin
                    .vertices
                    .iter()
                    .map(|p| Point3::new(p.x, p.y, p.z))
                    .collect();
                backend_render
                    .borrow_mut()
                    .add_surface(&verts, &tin.triangles);
                surfaces.borrow_mut().push(tin);
                selected_indices.borrow_mut().clear();
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from(format!(
                        "Total surfaces: {}",
                        surfaces.borrow().len()
                    )));
                    if app.get_workspace_mode() == 0 {
                        crate::set_workspace_image_result(&app, &render_image);
                    } else {
                        let image = backend_render.borrow_mut().render();
                        app.set_workspace_texture(image);
                    }
                    app.window().request_redraw();
                }
            } else if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Need at least 3 points"));
            }
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
            let dlg = ArcModeDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let dm = drawing_mode.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_center_start_end(move || {
                    *dm.borrow_mut() = DrawingMode::ArcCenter {
                        center: None,
                        radius: None,
                        start_angle: None,
                    };
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let dm = drawing_mode.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_three_point(move || {
                    *dm.borrow_mut() = DrawingMode::ArcThreePoint { p1: None, p2: None };
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            {
                let dm = drawing_mode.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_start_end_radius(move || {
                    *dm.borrow_mut() = DrawingMode::ArcStartEndRadius {
                        start: None,
                        end: None,
                        radius: None,
                    };
                    if let Some(d) = dlg_weak.upgrade() {
                        let _ = d.hide();
                    }
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let drawing_mode = drawing_mode.clone();
        app.on_draw_dimension_mode(move || {
            *drawing_mode.borrow_mut() = DrawingMode::Dimension { start: None };
        });
    }

    {
        let weak = app.as_weak();
        let layer_names = layer_names.clone();
        let point_style_names = point_style_names.clone();
        let point_layers = point_layers.clone();
        let point_style_indices = point_style_indices.clone();
        let point_metadata = point_metadata.clone();
        let inspector_ref = inspector_window.clone();
        let selected_indices = selected_indices.clone();
        let selected_polygons = selected_polygons.clone();
        let polygon_style_names = polygon_style_names.clone();
        let polygon_layers = polygon_layers.clone();
        let polygon_style_indices = polygon_style_indices.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_inspector(move || {
            if let Some(app) = weak.upgrade() {
                if let Some(idx) = selected_indices.borrow().first().copied() {
                    show_inspector_for_point(
                        idx,
                        &app,
                        &layer_names,
                        &point_style_names,
                        &point_layers,
                        &point_style_indices,
                        &point_metadata,
                        &point_elevation,
                        &point_measurement,
                        &point_data_sets,
                        &data_set_names,
                        &inspector_ref,
                        Rc::new(render_image.clone()),
                        &backend_render,
                    );
                } else if let Some(idx) = selected_polygons.borrow().first().copied() {
                    show_inspector_for_polygon(
                        idx,
                        &app,
                        &layer_names,
                        &polygon_style_names,
                        &polygon_layers,
                        &polygon_style_indices,
                        &point_measurement,
                        &point_data_sets,
                        &data_set_names,
                        &inspector_ref,
                        Rc::new(render_image.clone()),
                        &backend_render,
                    );
                }
            }
        });
    }

    let weak = app.as_weak();

    {
        let command_stack = command_stack.clone();
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        let lines = lines.clone();
        let line_style_indices = line_style_indices.clone();
        let backend = backend.clone();
        let render_image = render_image.clone();
        let dimensions = dimensions.clone();
        let weak = app.as_weak();
        app.on_undo(move || {
            let ctx = Context {
                points: &point_db,
                point_styles: &point_style_indices,
                lines: &lines,
                line_styles: &line_style_indices,
                dimensions: &dimensions,
                backend: &backend,
            };
            command_stack.borrow_mut().undo(&ctx);
            if let Some(app) = weak.upgrade() {
                refresh_workspace(&app, &render_image, &backend);
            }
        });
    }

    {
        let command_stack = command_stack.clone();
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        let lines = lines.clone();
        let line_style_indices = line_style_indices.clone();
        let backend = backend.clone();
        let render_image = render_image.clone();
        let dimensions = dimensions.clone();
        let weak = app.as_weak();
        app.on_redo(move || {
            let ctx = Context {
                points: &point_db,
                point_styles: &point_style_indices,
                lines: &lines,
                line_styles: &line_style_indices,
                dimensions: &dimensions,
                backend: &backend,
            };
            command_stack.borrow_mut().redo(&ctx);
            if let Some(app) = weak.upgrade() {
                refresh_workspace(&app, &render_image, &backend);
            }
        });
    }

    {
        let history_model = command_history.clone();
        let command_stack = command_stack.clone();
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        let lines = lines.clone();
        let line_style_indices = line_style_indices.clone();
        let backend = backend.clone();
        let render_image = render_image.clone();
        let dimensions = dimensions.clone();
        let weak = app.as_weak();
        app.on_command_entered(move |cmd| {
            history_model.push(cmd.clone());
            let parts = shell_words::split(&cmd).unwrap_or_default();
            if parts.is_empty() {
                return;
            }
            let ctx = Context {
                points: &point_db,
                point_styles: &point_style_indices,
                lines: &lines,
                line_styles: &line_style_indices,
                dimensions: &dimensions,
                backend: &backend,
            };
            match parts[0].as_str() {
                "point" if parts.len() >= 3 => {
                    if let (Ok(x), Ok(y)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
                        point_db.borrow_mut().push(Point::new(x, y));
                        point_style_indices.borrow_mut().push(0);
                        backend.borrow_mut().add_point(x, y, 0.0);
                        command_stack.borrow_mut().push(Command::RemovePoint {
                            index: point_db.borrow().len() - 1,
                            point: Point::new(x, y),
                        });
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
                            &point_db,
                            &lines,
                            &point_style_indices,
                            &line_style_indices,
                            &backend,
                            Point::new(x1, y1),
                            Point::new(x2, y2),
                        );
                        command_stack.borrow_mut().push(Command::RemoveLine {
                            index: lines.borrow().len() - 1,
                            line: (Point::new(x1, y1), Point::new(x2, y2)),
                        });
                    }
                }
                "undo" => {
                    command_stack.borrow_mut().undo(&ctx);
                }
                "redo" => {
                    command_stack.borrow_mut().redo(&ctx);
                }
                _ => {}
            }
            if let Some(app) = weak.upgrade() {
                refresh_workspace(&app, &render_image, &backend);
            }
        });
        {
            let weak = app.as_weak();
            let offset_ref = offset.clone();
            let zoom_ref = zoom.clone();
            app.on_input_entered(move |text| {
                let parts: Vec<_> = text
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .collect();
                if parts.len() >= 2 {
                    if let (Ok(x), Ok(y)) = (
                        parts[0].trim().parse::<f64>(),
                        parts[1].trim().parse::<f64>(),
                    ) {
                        if let Some(app) = weak.upgrade() {
                            let size = app.window().size();
                            let (sx, sy) = workspace_to_screen(
                                Point::new(x, y),
                                &offset_ref,
                                &zoom_ref,
                                size.width as f32,
                                size.height as f32,
                            );
                            app.invoke_workspace_clicked(sx, sy);
                        }
                    } else if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Invalid input"));
                    }
                } else if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("Enter X Y"));
                }
            });
        }
    }

    {
        let weak = app.as_weak();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        app.on_zoom_in(move || {
            *zoom.borrow_mut() *= 1.2;
            if let Some(app) = weak.upgrade() {
                app.set_zoom_level(*zoom.borrow());
                if app.get_workspace_mode() == 0 {
                    crate::set_workspace_image_result(&app, &render_image);
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
                    crate::set_workspace_image_result(&app, &render_image);
                    app.window().request_redraw();
                }
            }
        });
    }

    {
        let weak = app.as_weak();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let zoom = zoom.clone();
        app.on_view_changed(move |mode| {
            if let Some(app) = weak.upgrade() {
                app.set_workspace_mode(mode);
                app.set_zoom_level(*zoom.borrow());
                if mode == 0 {
                    crate::set_workspace_image_result(&app, &render_image);
                    app.set_status(SharedString::from("Camera: Middle drag pan, scroll zoom"));
                } else {
                    let image = backend_render.borrow_mut().render();
                    app.set_workspace_texture(image);
                    app.set_status(SharedString::from(
                        "Camera: Left drag orbit, middle drag pan, scroll zoom",
                    ));
                }
                app.window().request_redraw();
            }
        });
    }

    {
        let weak = app.as_weak();
        let render_image = render_image.clone();
        app.on_point_numbers_changed(move |_| {
            if let Some(app) = weak.upgrade() {
                if app.get_workspace_mode() == 0 {
                    crate::set_workspace_image_result(&app, &render_image);
                    app.window().request_redraw();
                }
            }
        });
    }

    {
        let workspace_crs = workspace_crs.clone();
        let crs_entries_rc = crs_entries_rc.clone();
        app.on_crs_changed(move |idx| {
            if let Some(entry) = crs_entries_rc.get(idx as usize) {
                if let Some(code) = entry.code.split(':').nth(1) {
                    if let Ok(epsg) = code.parse::<u32>() {
                        *workspace_crs.borrow_mut() = epsg;
                    }
                }
            }
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_grid_changed(move |val| {
            prefs.borrow_mut().snap_to_grid = val;
            cfg.borrow_mut().snap.snap_to_grid = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_objects_changed(move |val| {
            prefs.borrow_mut().snap_to_entities = val;
            cfg.borrow_mut().snap.snap_to_entities = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_endpoints_changed(move |val| {
            prefs.borrow_mut().snap_endpoints = val;
            cfg.borrow_mut().snap.snap_endpoints = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_intersections_changed(move |val| {
            prefs.borrow_mut().snap_intersections = val;
            cfg.borrow_mut().snap.snap_intersections = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let prefs = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_points_changed(move |val| {
            prefs.borrow_mut().snap_points = val;
            cfg.borrow_mut().snap.snap_points = val;
            save_config(&cfg.borrow());
        });
    }

    {
        let drawing_mode = drawing_mode.clone();
        let polygons = polygons.clone();
        let render_image = render_image.clone();
        let weak = app.as_weak();
        let point_db = point_db.clone();
        let lines = lines.clone();
        let line_style_indices = line_style_indices.clone();
        let point_style_indices = point_style_indices.clone();
        let backend = backend.clone();
        let command_stack = command_stack.clone();
        let dimensions = dimensions.clone();
        app.on_key_pressed(move |key| {
            if key.as_str() == "\u{001a}" {
                let ctx = Context {
                    points: &point_db,
                    point_styles: &point_style_indices,
                    lines: &lines,
                    line_styles: &line_style_indices,
                    dimensions: &dimensions,
                    backend: &backend,
                };
                command_stack.borrow_mut().undo(&ctx);
                if let Some(app) = weak.upgrade() {
                    refresh_workspace(&app, &render_image, &backend);
                }
            } else if key.as_str() == "\u{0019}" {
                let ctx = Context {
                    points: &point_db,
                    point_styles: &point_style_indices,
                    lines: &lines,
                    line_styles: &line_style_indices,
                    dimensions: &dimensions,
                    backend: &backend,
                };
                command_stack.borrow_mut().redo(&ctx);
                if let Some(app) = weak.upgrade() {
                    refresh_workspace(&app, &render_image, &backend);
                }
            } else if key.as_str() == "\u{001b}" {
                *drawing_mode.borrow_mut() = DrawingMode::None;
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
                    }
                }
            } else if key.as_str() == "\u{000a}" {
                let mut dm = drawing_mode.borrow_mut();
                if let DrawingMode::Polygon { vertices } = &mut *dm {
                    if vertices.len() > 2 {
                        vertices.push(vertices[0]);
                        polygons.borrow_mut().push(vertices.clone());
                        *dm = DrawingMode::None;
                    }
                }
                drop(dm);
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
                    }
                }
            } else if key.as_str() == "F5" {
                if let Some(app) = weak.upgrade() {
                    app.invoke_run_quick_script(0);
                }
            } else if key.as_str() == "F6" {
                if let Some(app) = weak.upgrade() {
                    app.invoke_run_quick_script(1);
                }
            } else if key.as_str() == "F7" {
                if let Some(app) = weak.upgrade() {
                    app.invoke_run_quick_script(2);
                }
            }
        });
    }

    // camera interaction callbacks for the 3D workspace
    {
        let rotate_flag = rotate_flag.clone();
        let last_pos = last_pos.clone();
        let click_pos = click_pos_3d.clone();
        let backend = backend.clone();
        let weak = app.as_weak();
        let active_handle_ref = active_handle.clone();
        app.on_workspace_left_pressed(move |x, y| {
            *last_pos.borrow_mut() = (x as f64, y as f64);
            if let Some(HitObject::Handle(i)) = backend.borrow_mut().hit_test(x as f64, y as f64) {
                *rotate_flag.borrow_mut() = false;
                *active_handle_ref.borrow_mut() = Some(i);
                backend.borrow_mut().highlight_handle(i, true);
                if let Some(app) = weak.upgrade() {
                    let image = backend.borrow_mut().render();
                    app.set_workspace_texture(image);
                    app.window().request_redraw();
                }
            } else {
                *rotate_flag.borrow_mut() = true;
                *active_handle_ref.borrow_mut() = None;
                *click_pos.borrow_mut() = Some((x as f64, y as f64));
            }
        });
    }

    {
        let pan_flag = pan_flag.clone();
        let last_pos = last_pos.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let selected_dimensions = selected_dimensions.clone();
        let menu_state = context_menu.clone();
        let weak = app.as_weak();
        app.on_workspace_right_pressed(move |x, y| {
            if has_selection(
                &selected_indices,
                &selected_lines,
                &selected_polygons,
                &selected_polylines,
                &selected_arcs,
                &selected_dimensions,
            ) {
                if let Some(app) = weak.upgrade() {
                    show_context_menu(&app, &menu_state, x, y);
                }
            } else {
                *pan_flag.borrow_mut() = true;
                *last_pos.borrow_mut() = (x as f64, y as f64);
            }
        });
    }

    {
        let pan_flag = pan_flag.clone();
        let last_pos = last_pos.clone();
        app.on_workspace_middle_pressed(move |x, y| {
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
        let polylines = polylines.clone();
        let point_db = point_db.clone();
        let arcs_ref = arcs.clone();
        let dimensions = dimensions.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let selected_dimensions = selected_dimensions.clone();
        let last_click = last_click.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let command_stack = command_stack.clone();
        let weak = app.as_weak();
        let context_menu = context_menu.clone();
        let macro_playing = macro_playing.clone();
        let macro_recorder = macro_recorder.clone();
        let snap_target = snap_target.clone();
        app.on_workspace_pointer_pressed(move |x, y, ev| {
            if ev.button == PointerEventButton::Right && *drawing_mode.borrow() == DrawingMode::None
            {
                if has_selection(
                    &selected_indices,
                    &selected_lines,
                    &selected_polygons,
                    &selected_polylines,
                    &selected_arcs,
                    &selected_dimensions,
                ) {
                    if let Some(app) = weak.upgrade() {
                        show_context_menu(&app, &context_menu, x, y);
                    }
                } else {
                    *pan_2d_flag.borrow_mut() = true;
                    *last_pos_2d.borrow_mut() = (x as f64, y as f64);
                }
                return;
            }
            if *drawing_mode.borrow() != DrawingMode::None {
                if ev.button == PointerEventButton::Left {
                    if let Some(app) = weak.upgrade() {
                        let size = app.window().size();
                        let mut p = screen_to_workspace(
                            x,
                            y,
                            &offset,
                            &zoom,
                            size.width as f32,
                            size.height as f32,
                        );
                        let zoom_factor = *zoom.borrow();
                        if app.get_snap_to_entities() {
                            let scene = snap::Scene {
                                points: &point_db.borrow(),
                                lines: &lines_ref.borrow(),
                                polygons: &polygons_ref.borrow(),
                                polylines: &polylines.borrow(),
                                arcs: &arcs_ref.borrow(),
                            };
                            let opts = snap::SnapOptions {
                                snap_points: app.get_snap_points(),
                                snap_endpoints: app.get_snap_endpoints(),
                                snap_midpoints: app.get_snap_midpoints(),
                                snap_intersections: app.get_snap_intersections(),
                                snap_nearest: app.get_snap_nearest(),
                                snap_surfaces: app.get_snap_surfaces(),
                                snap_solids: app.get_snap_solids(),
                            };
                            if let Some(sp) = snap::resolve_snap(
                                p,
                                &scene,
                                app.get_snap_tolerance() as f64 / (zoom_factor as f64),
                                opts,
                            ) {
                                *snap_target.borrow_mut() = Some(sp);
                                p = sp;
                            } else {
                                *snap_target.borrow_mut() = None;
                            }
                        } else {
                            *snap_target.borrow_mut() = None;
                        }
                        if app.get_snap_to_grid() {
                            p.x = p.x.round();
                            p.y = p.y.round();
                        }
                        let mut mode = drawing_mode.borrow_mut();
                        match &mut *mode {
                            DrawingMode::Line { start } => {
                                if start.is_none() {
                                    *start = Some(p);
                                } else if let Some(s) = start.take() {
                                    lines_ref.borrow_mut().push((s, p));
                                    backend_render.borrow_mut().add_line(
                                        [s.x, s.y, 0.0],
                                        [p.x, p.y, 0.0],
                                        [1.0, 1.0, 1.0, 1.0],
                                        1.0,
                                    );
                                    if !macro_playing.borrow().0 {
                                        let sx = s.x;
                                        let sy = s.y;
                                        let px = p.x;
                                        let py = p.y;
                                        record_macro(
                                            &mut macro_recorder.borrow_mut(),
                                            &format!("line {sx} {sy} {px} {py}"),
                                        );
                                    }
                                    *mode = DrawingMode::None;
                                } else {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(
                                            "No start point, line cancelled",
                                        ));
                                    }
                                    *mode = DrawingMode::None;
                                    return;
                                }
                            }
                            DrawingMode::Dimension { start } => {
                                if start.is_none() {
                                    *start = Some(p);
                                } else if let Some(s) = start.take() {
                                    dimensions.borrow_mut().push(LinearDimension::new(s, p));
                                    backend_render.borrow_mut().add_dimension(
                                        [s.x, s.y, 0.0],
                                        [p.x, p.y, 0.0],
                                        [1.0, 1.0, 1.0, 1.0],
                                        1.0,
                                    );
                                    command_stack.borrow_mut().push(Command::RemoveDimension {
                                        index: dimensions.borrow().len() - 1,
                                        dim: LinearDimension::new(s, p),
                                    });
                                    *mode = DrawingMode::None;
                                }
                            }
                            DrawingMode::Polygon { vertices } => {
                                let now = Instant::now();
                                let double = last_click
                                    .borrow()
                                    .map(|t| now.duration_since(t).as_millis() < 500)
                                    .unwrap_or(false);
                                *last_click.borrow_mut() = Some(now);
                                vertices.push(p);
                                if double && vertices.len() > 2 {
                                    vertices.push(vertices[0]);
                                    polygons_ref.borrow_mut().push(vertices.clone());
                                    *mode = DrawingMode::None;
                                }
                            }
                            DrawingMode::ArcCenter {
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
                                    let arc = Arc::new(c, r, sa, ea);
                                    arcs_ref.borrow_mut().push(arc);
                                    *mode = DrawingMode::None;
                                }
                            }
                            DrawingMode::ArcThreePoint { p1, p2 } => {
                                if p1.is_none() {
                                    *p1 = Some(p);
                                } else if p2.is_none() {
                                    *p2 = Some(p);
                                } else if let (Some(a), Some(b)) = (*p1, *p2) {
                                    if let Some(arc) = arc_from_three_points(a, b, p) {
                                        arcs_ref.borrow_mut().push(arc);
                                    }
                                    *mode = DrawingMode::None;
                                }
                            }
                            DrawingMode::ArcStartEndRadius { start, end, radius } => {
                                if start.is_none() {
                                    *start = Some(p);
                                } else if end.is_none() {
                                    *end = Some(p);
                                } else if radius.is_none() {
                                    if let (Some(s), Some(e)) = (*start, *end) {
                                        let r = ((p.x - s.x).powi(2) + (p.y - s.y).powi(2)).sqrt();
                                        if let Some(arc) = arc_from_start_end_radius(s, e, r, p) {
                                            arcs_ref.borrow_mut().push(arc);
                                        }
                                        *mode = DrawingMode::None;
                                    }
                                }
                            }
                            _ => {}
                        }
                        drop(mode);
                        if app.get_workspace_mode() == 0 {
                            crate::set_workspace_image_result(&app, &render_image);
                            app.window().request_redraw();
                        }
                        if let Some(app) = weak.upgrade() {
                            refresh_workspace(&app, &render_image, &backend_render);
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
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let polygons_ref = polygons.clone();
        let polylines = polylines.clone();
        let arcs_ref = arcs.clone();
        let offset = offset.clone();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        let cursor_feedback = cursor_feedback.clone();
        let weak = app.as_weak();
        let click_pos = click_pos_3d.clone();
        let selected_surface_ref = selected_surface.clone();
        let backend_inner = backend.clone();
        let dimensions = dimensions.clone();
        let selected_dimensions = selected_dimensions.clone();
        let active_handle_ref = active_handle.clone();
        app.on_workspace_pointer_released(move || {
            *rotate_flag.borrow_mut() = false;
            *pan_flag.borrow_mut() = false;
            *pan_2d_flag.borrow_mut() = false;
            *cursor_feedback.borrow_mut() = None;

            if let Some(i) = active_handle_ref.borrow_mut().take() {
                backend_inner.borrow_mut().highlight_handle(i, false);
                if let Some(app) = weak.upgrade() {
                    let image = backend_inner.borrow_mut().render();
                    app.set_workspace_texture(image);
                    app.window().request_redraw();
                }
            }

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
                        selected_polygons.borrow_mut().clear();
                        selected_polylines.borrow_mut().clear();
                        selected_arcs.borrow_mut().clear();
                        selected_dimensions.borrow_mut().clear();
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
                        for (i, poly) in polygons_ref.borrow().iter().enumerate() {
                            if poly.iter().all(|p| {
                                p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y
                            }) {
                                selected_polygons.borrow_mut().push(i);
                            }
                        }
                        for (i, pl) in polylines.borrow().iter().enumerate() {
                            if pl.vertices.iter().all(|p| {
                                p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y
                            }) {
                                selected_polylines.borrow_mut().push(i);
                            }
                        }
                        for (i, arc) in arcs_ref.borrow().iter().enumerate() {
                            let min_ax = arc.center.x - arc.radius;
                            let max_ax = arc.center.x + arc.radius;
                            let min_ay = arc.center.y - arc.radius;
                            let max_ay = arc.center.y + arc.radius;
                            if min_ax >= min_x
                                && max_ax <= max_x
                                && min_ay >= min_y
                                && max_ay <= max_y
                            {
                                selected_arcs.borrow_mut().push(i);
                            }
                        }
                        for (i, dim) in dimensions.borrow().iter().enumerate() {
                            let min_dx = dim.start.x.min(dim.end.x);
                            let max_dx = dim.start.x.max(dim.end.x);
                            let min_dy = dim.start.y.min(dim.end.y);
                            let max_dy = dim.start.y.max(dim.end.y);
                            if min_dx >= min_x
                                && max_dx <= max_x
                                && min_dy >= min_y
                                && max_dy <= max_y
                            {
                                selected_dimensions.borrow_mut().push(i);
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
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
                    }
                }
            } else if let Some(start) = click_pos.borrow_mut().take() {
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 1 {
                        if let Some(hit) = backend_inner.borrow_mut().hit_test(start.0, start.1) {
                            match hit {
                                HitObject::Surface(i) => {
                                    if let Some(prev) = selected_surface_ref.replace(Some(i)) {
                                        backend_inner.borrow_mut().highlight_surface(prev, false);
                                    }
                                    backend_inner.borrow_mut().highlight_surface(i, true);
                                    backend_inner.borrow_mut().show_surface_handles(i);
                                }
                                HitObject::Point(i) => {
                                    backend_inner.borrow_mut().show_point_handles(i);
                                }
                                HitObject::Line(i) => {
                                    backend_inner.borrow_mut().show_line_handles(i);
                                }
                                _ => {
                                    if let Some(prev) = selected_surface_ref.take() {
                                        backend_inner.borrow_mut().highlight_surface(prev, false);
                                        backend_inner.borrow_mut().hide_handles();
                                    }
                                }
                            }
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
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
        let click_pos = click_pos_3d.clone();
        let pan_2d_flag = pan_2d_flag.clone();
        let last_pos_2d = last_pos_2d.clone();
        let offset = offset.clone();
        let zoom = zoom.clone();
        let render_image = render_image.clone();
        let drag_select = drag_select.clone();
        let cursor_feedback = cursor_feedback.clone();
        let drawing_mode = drawing_mode.clone();
        let point_db = point_db.clone();
        let lines_ref = lines.clone();
        let polygons_ref = polygons.clone();
        let polylines = polylines.clone();
        let arcs_ref = arcs.clone();
        let current_line = current_line.clone();
        let snap_target = snap_target.clone();
        let weak = app.as_weak();
        let active_handle_ref = active_handle.clone();
        let backend_move = backend.clone();
        app.on_workspace_mouse_moved(move |x, y| {
            let _ = backend_move.borrow_mut().hit_test(x as f64, y as f64);
            let mut last = last_pos.borrow_mut();
            let dx = x as f64 - last.0;
            let dy = y as f64 - last.1;
            *last = (x as f64, y as f64);
            if let Some(i) = *active_handle_ref.borrow() {
                if let Some(pos) = backend_move.borrow().handle_position(i) {
                    let mut new_p = backend_move
                        .borrow()
                        .screen_to_plane(x as f64, y as f64, pos.z);
                    if let Some(app_ref) = weak.upgrade() {
                        if app_ref.get_snap_to_entities() {
                            let scene = backend_move.borrow().snap_scene();
                            let opts = snap::SnapOptions {
                                snap_points: app_ref.get_snap_points(),
                                snap_endpoints: app_ref.get_snap_endpoints(),
                                snap_midpoints: app_ref.get_snap_midpoints(),
                                snap_intersections: app_ref.get_snap_intersections(),
                                snap_nearest: app_ref.get_snap_nearest(),
                                snap_surfaces: app_ref.get_snap_surfaces(),
                                snap_solids: app_ref.get_snap_solids(),
                            };
                            if let Some(sp) = snap::resolve_snap_3d(
                                new_p,
                                &scene,
                                app_ref.get_snap_tolerance() as f64,
                                opts,
                            ) {
                                new_p = sp;
                            }
                        }
                    }
                    backend_move.borrow_mut().move_handle(i, new_p);
                    if let Some(app) = weak.upgrade() {
                        let image = backend_move.borrow_mut().render();
                        app.set_workspace_texture(image);
                        app.window().request_redraw();
                    }
                    return;
                }
            }
            if *rotate_flag.borrow() {
                if let Some(start) = *click_pos.borrow() {
                    if (x as f64 - start.0).abs() > 3.0 || (y as f64 - start.1).abs() > 3.0 {
                        *click_pos.borrow_mut() = None;
                    }
                }
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
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
                    }
                }
            }

            if drag_select.borrow().active {
                drag_select.borrow_mut().end = (x, y);
                if let Some(app) = weak.upgrade() {
                    if app.get_workspace_mode() == 0 {
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
                    }
                }
            }

            if matches!(*drawing_mode.borrow(), DrawingMode::Line { .. }) {
                if let Some(app) = weak.upgrade() {
                    let size = app.window().size();
                    let mut p = screen_to_workspace(
                        x,
                        y,
                        &offset,
                        &zoom,
                        size.width as f32,
                        size.height as f32,
                    );
                    let zoom_factor = *zoom.borrow();
                    if app.get_snap_to_entities() {
                        let scene = snap::Scene {
                            points: &point_db.borrow(),
                            lines: &lines_ref.borrow(),
                            polygons: &polygons_ref.borrow(),
                            polylines: &polylines.borrow(),
                            arcs: &arcs_ref.borrow(),
                        };
                        let opts = snap::SnapOptions {
                            snap_points: app.get_snap_points(),
                            snap_endpoints: app.get_snap_endpoints(),
                            snap_midpoints: app.get_snap_midpoints(),
                            snap_intersections: app.get_snap_intersections(),
                            snap_nearest: app.get_snap_nearest(),
                            snap_surfaces: app.get_snap_surfaces(),
                            snap_solids: app.get_snap_solids(),
                        };
                        if let Some(sp) = snap::resolve_snap(
                            p,
                            &scene,
                            app.get_snap_tolerance() as f64 / (zoom_factor as f64),
                            opts,
                        ) {
                            *snap_target.borrow_mut() = Some(sp);
                            p = sp;
                        } else {
                            *snap_target.borrow_mut() = None;
                        }
                    } else {
                        *snap_target.borrow_mut() = None;
                    }
                    if app.get_snap_to_grid() {
                        p.x = p.x.round();
                        p.y = p.y.round();
                    }
                    if let Some(cl) = current_line.borrow_mut().as_mut() {
                        if let Some(last) = cl.vertices.last_mut() {
                            *last = p;
                        }
                    }
                    if app.get_workspace_mode() == 0 {
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
                    }
                }
            }

            *cursor_feedback.borrow_mut() = Some(CursorFeedback {
                pos: (x, y),
                frame: 0,
            });

            if let Some(app) = weak.upgrade() {
                if app.get_workspace_mode() == 0 {
                    let size = app.window().size();
                    let p = screen_to_workspace(
                        x,
                        y,
                        &offset,
                        &zoom,
                        size.width as f32,
                        size.height as f32,
                    );
                    app.set_status(SharedString::from(format!("X: {:.3} Y: {:.3}", p.x, p.y)));
                } else {
                    let p = {
                        let b = backend_move.borrow();
                        b.screen_to_plane(x as f64, y as f64, 0.0)
                    };
                    app.set_status(SharedString::from(format!(
                        "X: {:.3} Y: {:.3} Z: {:.3}",
                        p.x, p.y, p.z
                    )));
                    let image = backend_move.borrow_mut().render();
                    app.set_workspace_texture(image);
                    app.window().request_redraw();
                }
            }
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
                    crate::set_workspace_image_result(&app, &render_image);
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
        let surface_units_np = surface_units.clone();
        let surface_styles_np = surface_styles.clone();
        let surface_descriptions_np = surface_descriptions.clone();
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
        let dimensions = dimensions.clone();
        let selected_dimensions = selected_dimensions.clone();
        let workspace_crs = workspace_crs.clone();
        let crs_entries_rc = crs_entries_rc.clone();
        app.on_new_project(move || {
            point_db.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            dimensions.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            surface_units_np.borrow_mut().clear();
            surface_styles_np.borrow_mut().clear();
            surface_descriptions_np.borrow_mut().clear();
            alignments.borrow_mut().clear();
            selected_indices.borrow_mut().clear();
            selected_lines.borrow_mut().clear();
            selected_polygons.borrow_mut().clear();
            selected_polylines.borrow_mut().clear();
            selected_arcs.borrow_mut().clear();
            selected_dimensions.borrow_mut().clear();
            backend_render.borrow_mut().clear();
            refresh_line_style_dialogs();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("New project created"));
                *workspace_crs.borrow_mut() = 4326;
                if let Some(idx) = crs_entries_rc.iter().position(|e| e.code == "EPSG:4326") {
                    app.set_crs_index(idx as i32);
                }
                refresh_workspace(&app, &render_image, &backend_render);
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
        let surface_units_ref = surface_units.clone();
        let surface_styles_ref = surface_styles.clone();
        let surface_descriptions_ref = surface_descriptions.clone();
        let layers_ref = layers.clone();
        let layer_names_ref = layer_names.clone();
        let line_style_indices = line_style_indices.clone();
        let point_style_indices = point_style_indices.clone();
        let polygon_style_indices = polygon_style_indices.clone();
        let grid_settings = grid_settings.clone();
        let point_label_style = point_label_style.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let dimensions = dimensions.clone();
        let last_dir = last_folder.clone();
        let config_rc = config.clone();
        let workspace_crs = workspace_crs.clone();
        let crs_entries_rc = crs_entries_rc.clone();
        let alignments = alignments.clone();
        let surface_groups = surface_groups.clone();
        let alignment_groups = alignment_groups.clone();
        app.on_open_project(move || {
            if let Some(path) = dialogs::open_project_file(last_dir.borrow().as_deref().map(Path::new)) {
                *last_dir.borrow_mut() = path.parent().map(|p| p.to_string_lossy().to_string());
                config_rc.borrow_mut().last_open_dir = last_dir.borrow().clone();
                save_config(&config_rc.borrow());
                if let Some(p) = path.to_str() {
                    let p = p.to_string();
                    match read_project_json(&p) {
                        Ok(proj) => {
                            *workspace_crs.borrow_mut() = proj.crs_epsg;
                            if let Some(idx) = crs_entries_rc
                                .iter()
                                .position(|e| e.code == format!("EPSG:{}", proj.crs_epsg))
                            {
                                if let Some(app) = weak.upgrade() {
                                    app.set_crs_index(idx as i32);
                                }
                            }
                            point_db.borrow_mut().clear();
                            point_db.borrow_mut().extend_from_slice(&proj.points);
                            lines.borrow_mut().clear();
                            lines
                                .borrow_mut()
                                .extend(proj.lines.iter().map(|l| (l.start, l.end)));
                            polygons.borrow_mut().clear();
                            polygons.borrow_mut().extend(proj.polygons.clone());
                            polylines.borrow_mut().clear();
                            polylines.borrow_mut().extend(proj.polylines.clone());
                            arcs.borrow_mut().clear();
                            arcs.borrow_mut().extend(proj.arcs.clone());
                            dimensions.borrow_mut().clear();
                            dimensions.borrow_mut().extend(proj.dimensions.clone());
                            surfaces.borrow_mut().clear();
                            surfaces.borrow_mut().extend(proj.surfaces.clone());
                            surface_groups.borrow_mut().clear();
                            surface_groups
                                .borrow_mut()
                                .extend(proj.surface_groups.clone());
                            surface_units_ref.borrow_mut().clear();
                            surface_units_ref
                                .borrow_mut()
                                .extend(proj.surface_units.clone());
                            surface_styles_ref.borrow_mut().clear();
                            surface_styles_ref
                                .borrow_mut()
                                .extend(proj.surface_styles.clone());
                            surface_descriptions_ref.borrow_mut().clear();
                            surface_descriptions_ref
                                .borrow_mut()
                                .extend(proj.surface_descriptions.clone());
                            alignments.borrow_mut().clear();
                            alignments.borrow_mut().extend(proj.alignments.clone());
                            alignment_groups.borrow_mut().clear();
                            alignment_groups
                                .borrow_mut()
                                .extend(proj.alignment_groups.clone());
                            *line_style_indices.borrow_mut() = proj.line_style_indices.clone();
                            *point_style_indices.borrow_mut() = proj.point_style_indices.clone();
                            *polygon_style_indices.borrow_mut() =
                                proj.polygon_style_indices.clone();
                            *grid_settings.borrow_mut() = proj.grid.clone();
                            {
                                let mut pls = point_label_style.borrow_mut();
                                pls.text_style.font = proj.point_label_font.clone();
                                pls.offset = proj.point_label_offset;
                            }

                            let mut mgr = ScLayerManager::new();
                            layer_names_ref.borrow_mut().clear();
                            let order = if !proj.layer_order.is_empty() {
                                proj.layer_order.clone()
                            } else {
                                proj.layers.iter().map(|l| l.name.clone()).collect()
                            };
                            for name in &order {
                                if let Some(l) = proj.layers.iter().find(|x| x.name == *name) {
                                    layer_names_ref.borrow_mut().push(name.clone());
                                    mgr.add_layer(l.clone());
                                }
                            }
                            *layers_ref.borrow_mut() = mgr;

                            backend_render.borrow_mut().clear();
                            for pt in point_db.borrow().iter() {
                                backend_render.borrow_mut().add_point(pt.x, pt.y, 0.0);
                            }
                            for tin in surfaces.borrow().iter() {
                                let verts: Vec<Point3> = tin
                                    .vertices
                                    .iter()
                                    .map(|p| Point3::new(p.x, p.y, p.z))
                                    .collect();
                                backend_render
                                    .borrow_mut()
                                    .add_surface(&verts, &tin.triangles);
                            }
                            for dim in dimensions.borrow().iter() {
                                backend_render.borrow_mut().add_dimension(
                                    [dim.start.x, dim.start.y, 0.0],
                                    [dim.end.x, dim.end.y, 0.0],
                                    [1.0, 1.0, 1.0, 1.0],
                                    1.0,
                                );
                            }

                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Project loaded"));
                                refresh_workspace(&app, &render_image, &backend_render);
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("Failed to open: {e}")));
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
        let surfaces = surfaces.clone();
        let layers_ref = layers.clone();
        let line_style_indices = line_style_indices.clone();
        let point_style_indices = point_style_indices.clone();
        let polygon_style_indices = polygon_style_indices.clone();
        let grid_settings = grid_settings.clone();
        let point_label_style = point_label_style.clone();
        let point_styles = point_styles.clone();
        let line_styles = line_styles.clone();
        let dimensions = dimensions.clone();
        let last_dir = last_folder.clone();
        let config_rc = config.clone();
        let workspace_crs = workspace_crs.clone();
        let surface_units_ref = surface_units.clone();
        let surface_styles_ref = surface_styles.clone();
        let surface_descriptions_ref = surface_descriptions.clone();
        let surface_groups_ref = surface_groups.clone();
        let alignments_save = alignments.clone();
        let alignment_groups_save = alignment_groups.clone();
        let layer_names_save = layer_names.clone();
        app.on_save_project(move || {
            let mut dialog = rfd::FileDialog::new();
            if let Some(dir) = last_dir.borrow().as_ref() {
                dialog = dialog.set_directory(dir);
            }
            if let Some(path) = dialog.add_filter("Project", &["json"]).save_file() {
                *last_dir.borrow_mut() = path.parent().map(|p| p.to_string_lossy().to_string());
                config_rc.borrow_mut().last_open_dir = last_dir.borrow().clone();
                save_config(&config_rc.borrow());
                if let Some(p) = path.to_str() {
                    let proj = Project {
                        points: point_db.borrow().points().to_vec(),
                        lines: lines.borrow().iter().map(|l| Line::new(l.0, l.1)).collect(),
                        polygons: polygons.borrow().clone(),
                        polylines: polylines.borrow().clone(),
                        arcs: arcs.borrow().clone(),
                        dimensions: dimensions.borrow().clone(),
                        alignments: alignments_save.borrow().clone(),
                        alignment_groups: alignment_groups_save.borrow().clone(),
                        surfaces: surfaces.borrow().clone(),
                        surface_groups: surface_groups_ref.borrow().clone(),
                        surface_units: surface_units_ref.borrow().clone(),
                        surface_styles: surface_styles_ref.borrow().clone(),
                        surface_descriptions: surface_descriptions_ref.borrow().clone(),
                        layers: layer_names_save
                            .borrow()
                            .iter()
                            .filter_map(|n| layers_ref.borrow().layer(n).cloned())
                            .collect(),
                        layer_order: layer_names_save.borrow().clone(),
                        point_style_indices: point_style_indices.borrow().clone(),
                        line_style_indices: line_style_indices.borrow().clone(),
                        polygon_style_indices: polygon_style_indices.borrow().clone(),
                        grid: grid_settings.borrow().clone(),
                        crs_epsg: *workspace_crs.borrow(),
                        point_label_font: point_label_style.borrow().text_style.font.clone(),
                        point_label_offset: point_label_style.borrow().offset,
                    };
                    let base = Path::new(p);
                    let _ = save_layers(&base.with_extension("layers.json"), &layers_ref.borrow());
                    let style_settings = StyleSettings {
                        point_styles: point_styles.clone(),
                        line_styles: line_styles.clone(),
                        polygon_styles: polygon_styles.clone(),
                        alignment_styles: alignment_styles.clone(),
                        line_label_styles: line_label_styles.clone(),
                        point_label_styles: point_label_styles.clone(),
                    };
                    let _ = save_styles(&base.with_extension("styles.json"), &style_settings);

                    if let Err(e) = write_project_json(p, &proj) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to save: {e}")));
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
        let backend_render = backend.clone();
        let command_stack_outer = command_stack.clone();
        let macro_playing_outer = macro_playing.clone();
        let macro_recorder_outer = macro_recorder.clone();
        let workspace_crs_line = workspace_crs.clone();
        app.on_add_line(move || {
            let macro_playing = macro_playing_outer.clone();
            let macro_recorder = macro_recorder_outer.clone();
            let line_style_indices = line_style_indices.clone();
            let dlg = AddLineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let lines = lines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let workspace_crs = workspace_crs_line.clone();
                let line_style_indices = line_style_indices.clone();
                let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
                let backend_render = backend_render.clone();
                let command_stack = command_stack_outer.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("DWG", &["dwg"])
                        .add_filter("DGN", &["dgn"])
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_line_csv(p, *workspace_crs.borrow()) {
                                Ok(l) => {
                                    lines.borrow_mut().push(l);
                                    let (s, e) = l;
                                    backend_render.borrow_mut().add_line(
                                        [s.x, s.y, 0.0],
                                        [e.x, e.y, 0.0],
                                        [1.0, 1.0, 1.0, 1.0],
                                        1.0,
                                    );
                                    command_stack.borrow_mut().push(Command::RemoveLine {
                                        index: lines.borrow().len() - 1,
                                        line: (s, e),
                                    });
                                    let count = lines.borrow().len();
                                    let mut idx = line_style_indices.borrow_mut();
                                    if idx.len() < count {
                                        idx.resize(count, 0);
                                    }
                                    refresh_line_style_dialogs();
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total lines: {}",
                                            lines.borrow().len()
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            crate::set_workspace_image_result(&app, &render_image);
                                            app.window().request_redraw();
                                        }
                                        refresh_workspace(&app, &render_image, &backend_render);
                                        refresh_workspace(&app, &render_image, &backend_render);
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {e}"
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
                let backend_render = backend_render.clone();
                let command_stack_outer = command_stack_outer.clone();
                let macro_playing = macro_playing.clone();
                let macro_recorder = macro_recorder.clone();
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
                        let backend_render = backend_render.clone();
                        let command_stack = command_stack_outer.clone();
                        let macro_playing = macro_playing.clone();
                        let macro_recorder = macro_recorder.clone();
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
                                    backend_render.borrow_mut().add_line(
                                        [x1, y1, 0.0],
                                        [x2, y2, 0.0],
                                        [1.0, 1.0, 1.0, 1.0],
                                        1.0,
                                    );
                                    if !macro_playing.borrow().0 {
                                        record_macro(
                                            &mut macro_recorder.borrow_mut(),
                                            &format!("line {x1} {y1} {x2} {y2}"),
                                        );
                                    }
                                    command_stack.borrow_mut().push(Command::RemoveLine {
                                        index: lines.borrow().len() - 1,
                                        line: (Point::new(x1, y1), Point::new(x2, y2)),
                                    });
                                    let count = lines.borrow().len();
                                    let mut idx = line_style_indices.borrow_mut();
                                    if idx.len() < count {
                                        idx.resize(count, 0);
                                    }
                                    refresh_line_style_dialogs();
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total lines: {}",
                                            lines.borrow().len()
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            crate::set_workspace_image_result(&app, &render_image);
                                            app.window().request_redraw();
                                        }
                                        refresh_workspace(&app, &render_image, &backend_render);
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
        let backend_render = backend.clone();
        let command_stack_outer = command_stack.clone();
        let macro_playing_outer = macro_playing.clone();
        let macro_recorder_outer = macro_recorder.clone();
        let workspace_crs_point = workspace_crs.clone();
        app.on_add_point(move || {
            let macro_playing = macro_playing_outer.clone();
            let macro_recorder = macro_recorder_outer.clone();
            let dlg = AddPointDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let point_db = point_db.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let point_style_indices = point_style_indices.clone();
                let workspace_crs = workspace_crs_point.clone();
                let backend_render = backend_render.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("DWG", &["dwg"])
                        .add_filter("DGN", &["dgn"])
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match survey_cad::io::read_points_csv(
                                p,
                                Some(4326),
                                Some(*workspace_crs.borrow()),
                            ) {
                                Ok(pts) => {
                                    let len = {
                                        let mut db = point_db.borrow_mut();
                                        db.clear();
                                        db.extend(pts);
                                        point_style_indices.borrow_mut().clear();
                                        point_style_indices
                                            .borrow_mut()
                                            .extend(std::iter::repeat_n(0, db.len()));
                                        backend_render.borrow_mut().clear();
                                        for p in db.iter() {
                                            backend_render.borrow_mut().add_point(p.x, p.y, 0.0);
                                        }
                                        db.len()
                                    };
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Loaded {len} points"
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            crate::set_workspace_image_result(&app, &render_image);
                                            app.window().request_redraw();
                                        }
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {e}"
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
                let backend_render = backend_render.clone();
                let cs_inner = command_stack_outer.clone();
                let macro_playing = macro_playing.clone();
                let macro_recorder = macro_recorder.clone();
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
                        let backend_render = backend_render.clone();
                        let command_stack = cs_inner.clone();
                        let macro_playing = macro_playing.clone();
                        let macro_recorder = macro_recorder.clone();
                        key_dlg.on_accept(move || {
                            if let Some(dlg) = key_weak2.upgrade() {
                                if let (Ok(x), Ok(y)) = (
                                    dlg.get_x_value().parse::<f64>(),
                                    dlg.get_y_value().parse::<f64>(),
                                ) {
                                    point_db.borrow_mut().push(Point::new(x, y));
                                    psi.borrow_mut().push(0);
                                    backend_render.borrow_mut().add_point(x, y, 0.0);
                                    if !macro_playing.borrow().0 {
                                        record_macro(
                                            &mut macro_recorder.borrow_mut(),
                                            &format!("point {x} {y}"),
                                        );
                                    }
                                    command_stack.borrow_mut().push(Command::RemovePoint {
                                        index: point_db.borrow().len() - 1,
                                        point: Point::new(x, y),
                                    });
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Total points: {}",
                                            point_db.borrow().len()
                                        )));
                                        if app.get_workspace_mode() == 0 {
                                            crate::set_workspace_image_result(&app, &render_image);
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
        let workspace_crs_polygon = workspace_crs.clone();
        app.on_add_polygon(move || {
            let dlg = AddPolygonDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let polygons = polygons.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let workspace_crs = workspace_crs_polygon.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("DWG", &["dwg"])
                        .add_filter("DGN", &["dgn"])
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_points_list(p, *workspace_crs.borrow()) {
                                Ok(pts) => {
                                    if pts.len() >= 3 {
                                        polygons.borrow_mut().push(pts);
                                        if let Some(app) = weak.upgrade() {
                                            app.set_status(SharedString::from(format!(
                                                "Total polygons: {}",
                                                polygons.borrow().len()
                                            )));
                                            if app.get_workspace_mode() == 0 {
                                                crate::set_workspace_image_result(
                                                    &app,
                                                    &render_image,
                                                );
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
                                            "Failed to open: {e}"
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
                                    model.push(SharedString::from(format!("{x:.3},{y:.3}")));
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
                                        crate::set_workspace_image_result(&app, &render_image);
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
        let workspace_crs_polyline = workspace_crs.clone();
        app.on_add_polyline(move || {
            let dlg = AddPolylineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            {
                let polylines = polylines.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                let dlg_weak = dlg_weak.clone();
                let workspace_crs = workspace_crs_polyline.clone();
                dlg.on_from_file(move || {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            match read_points_list(p, *workspace_crs.borrow()) {
                                Ok(pts) => {
                                    if pts.len() >= 2 {
                                        polylines.borrow_mut().push(Polyline::new(pts));
                                        if let Some(app) = weak.upgrade() {
                                            app.set_status(SharedString::from(format!(
                                                "Total polylines: {}",
                                                polylines.borrow().len()
                                            )));
                                            if app.get_workspace_mode() == 0 {
                                                crate::set_workspace_image_result(
                                                    &app,
                                                    &render_image,
                                                );
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
                                            "Failed to open: {e}"
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
                                    model.push(SharedString::from(format!("{x:.3},{y:.3}")));
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
                                        crate::set_workspace_image_result(&app, &render_image);
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
                                            crate::set_workspace_image_result(&app, &render_image);
                                            app.window().request_redraw();
                                        }
                                    }
                                }
                                Err(e) => {
                                    if let Some(app) = weak.upgrade() {
                                        app.set_status(SharedString::from(format!(
                                            "Failed to open: {e}"
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
                                            crate::set_workspace_image_result(&app, &render_image);
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
            let mut pts: Vec<Point> = selected_indices
                .borrow()
                .iter()
                .filter_map(|&i| point_db.borrow().get(i).copied())
                .collect();
            for (s, e) in selected_lines.borrow().iter() {
                pts.push(*s);
                pts.push(*e);
            }
            let hull = convex_hull(&pts);
            if hull.len() >= 3 {
                polygons.borrow_mut().push(hull);
                selected_indices.borrow_mut().clear();
                selected_lines.borrow_mut().clear();
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from(format!(
                        "Total polygons: {}",
                        polygons.borrow().len()
                    )));
                    if app.get_workspace_mode() == 0 {
                        crate::set_workspace_image_result(&app, &render_image);
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
                            app.set_status(SharedString::from(format!("Distance: {dist:.3}")));
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
        let workspace_crs = workspace_crs.clone();
        app.on_traverse_area(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DWG", &["dwg"])
                .add_filter("DGN", &["dgn"])
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                if let (Some(p), Some(app)) = (path.to_str(), weak.upgrade()) {
                    match survey_cad::io::read_points_csv(
                        p,
                        Some(4326),
                        Some(*workspace_crs.borrow()),
                    ) {
                        Ok(pts) => {
                            let trav = survey_cad::surveying::Traverse::new(pts);
                            app.set_status(SharedString::from(format!("Area: {:.3}", trav.area())));
                        }
                        Err(e) => {
                            app.set_status(SharedString::from(format!("Failed: {e}")));
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
                            app.set_status(SharedString::from(format!("Elevation: {elev:.3}")));
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
                        let al = &aligns[0];
                        Some(survey_cad::corridor::corridor_volume(
                            design, ground, al, width, interval, step,
                        ))
                    })();
                    if let Some(app) = weak2.upgrade() {
                        if let Some(vol) = res {
                            app.set_status(SharedString::from(format!("Volume: {vol:.3}")));
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
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        app.on_move_entity(move || {
            let dlg = MoveEntityDialog::new().unwrap();
            dlg.set_dx_value("0".into());
            dlg.set_dy_value("0".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let point_db = point_db.clone();
            let lines = lines.clone();
            let polygons = polygons.clone();
            let polylines = polylines.clone();
            let arcs = arcs.clone();
            let selected_indices = selected_indices.clone();
            let selected_lines = selected_lines.clone();
            let selected_polygons = selected_polygons.clone();
            let selected_polylines = selected_polylines.clone();
            let selected_arcs = selected_arcs.clone();
            let backend_inner = backend_render.clone();
            let render_image = render_image.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let dx = d.get_dx_value().parse::<f64>().unwrap_or(0.0);
                    let dy = d.get_dy_value().parse::<f64>().unwrap_or(0.0);
                    for &idx in selected_indices.borrow().iter() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx) {
                            p.x += dx;
                            p.y += dy;
                            backend_inner.borrow_mut().update_point(idx, p.x, p.y, 0.0);
                        }
                    }
                    for (i, line) in lines.borrow_mut().iter_mut().enumerate() {
                        if selected_lines.borrow().iter().any(|(s, e)| {
                            (*s == line.0 && *e == line.1) || (*s == line.1 && *e == line.0)
                        }) {
                            line.0.x += dx;
                            line.0.y += dy;
                            line.1.x += dx;
                            line.1.y += dy;
                            backend_inner.borrow_mut().update_line(
                                i,
                                [line.0.x, line.0.y, 0.0],
                                [line.1.x, line.1.y, 0.0],
                                [1.0, 1.0, 1.0, 1.0],
                                1.0,
                            );
                        }
                    }
                    for &idx in selected_polygons.borrow().iter() {
                        if let Some(poly) = polygons.borrow_mut().get_mut(idx) {
                            for v in poly.iter_mut() {
                                v.x += dx;
                                v.y += dy;
                            }
                        }
                    }
                    for &idx in selected_polylines.borrow().iter() {
                        if let Some(pl) = polylines.borrow_mut().get_mut(idx) {
                            for v in pl.vertices.iter_mut() {
                                v.x += dx;
                                v.y += dy;
                            }
                        }
                    }
                    for &idx in selected_arcs.borrow().iter() {
                        if let Some(a) = arcs.borrow_mut().get_mut(idx) {
                            a.center.x += dx;
                            a.center.y += dy;
                        }
                    }
                    if let Some(app) = weak2.upgrade() {
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
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
        let lines = lines.clone();
        let polygons = polygons.clone();
        let polylines = polylines.clone();
        let arcs = arcs.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        app.on_rotate_entity(move || {
            let dlg = RotateEntityDialog::new().unwrap();
            dlg.set_angle_value("0".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let point_db = point_db.clone();
            let lines = lines.clone();
            let polygons = polygons.clone();
            let polylines = polylines.clone();
            let arcs = arcs.clone();
            let selected_indices = selected_indices.clone();
            let selected_lines = selected_lines.clone();
            let selected_polygons = selected_polygons.clone();
            let selected_polylines = selected_polylines.clone();
            let selected_arcs = selected_arcs.clone();
            let backend_inner = backend_render.clone();
            let render_image = render_image.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let ang = d
                        .get_angle_value()
                        .parse::<f64>()
                        .unwrap_or(0.0)
                        .to_radians();
                    let cos_a = ang.cos();
                    let sin_a = ang.sin();
                    for &idx in selected_indices.borrow().iter() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx) {
                            let x = p.x * cos_a - p.y * sin_a;
                            let y = p.x * sin_a + p.y * cos_a;
                            p.x = x;
                            p.y = y;
                            backend_inner.borrow_mut().update_point(idx, p.x, p.y, 0.0);
                        }
                    }
                    for (i, line) in lines.borrow_mut().iter_mut().enumerate() {
                        if selected_lines.borrow().iter().any(|(s, e)| {
                            (*s == line.0 && *e == line.1) || (*s == line.1 && *e == line.0)
                        }) {
                            for pt in [&mut line.0, &mut line.1] {
                                let x = pt.x * cos_a - pt.y * sin_a;
                                let y = pt.x * sin_a + pt.y * cos_a;
                                pt.x = x;
                                pt.y = y;
                            }
                            backend_inner.borrow_mut().update_line(
                                i,
                                [line.0.x, line.0.y, 0.0],
                                [line.1.x, line.1.y, 0.0],
                                [1.0, 1.0, 1.0, 1.0],
                                1.0,
                            );
                        }
                    }
                    for &idx in selected_polygons.borrow().iter() {
                        if let Some(poly) = polygons.borrow_mut().get_mut(idx) {
                            for v in poly.iter_mut() {
                                let x = v.x * cos_a - v.y * sin_a;
                                let y = v.x * sin_a + v.y * cos_a;
                                v.x = x;
                                v.y = y;
                            }
                        }
                    }
                    for &idx in selected_polylines.borrow().iter() {
                        if let Some(pl) = polylines.borrow_mut().get_mut(idx) {
                            for v in pl.vertices.iter_mut() {
                                let x = v.x * cos_a - v.y * sin_a;
                                let y = v.x * sin_a + v.y * cos_a;
                                v.x = x;
                                v.y = y;
                            }
                        }
                    }
                    for &idx in selected_arcs.borrow().iter() {
                        if let Some(a) = arcs.borrow_mut().get_mut(idx) {
                            let cx = a.center.x * cos_a - a.center.y * sin_a;
                            let cy = a.center.x * sin_a + a.center.y * cos_a;
                            a.center.x = cx;
                            a.center.y = cy;
                            a.start_angle += ang;
                            a.end_angle += ang;
                        }
                    }
                    if let Some(app) = weak2.upgrade() {
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
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
        let lines_ref = lines.clone();
        let polygons_ref = polygons.clone();
        let polylines_ref = polylines.clone();
        let arcs_ref = arcs.clone();
        let dimensions_ref = dimensions.clone();
        let selected_indices = selected_indices.clone();
        let selected_lines = selected_lines.clone();
        let selected_polygons = selected_polygons.clone();
        let selected_polylines = selected_polylines.clone();
        let selected_arcs = selected_arcs.clone();
        let selected_dimensions = selected_dimensions.clone();
        let backend_render = backend.clone();
        let command_stack = command_stack.clone();
        let psi = point_style_indices.clone();
        let lsi = line_style_indices.clone();
        let render_image = render_image.clone();
        app.on_delete_selected(move || {
            {
                let mut inds = selected_indices.borrow_mut();
                inds.sort();
                let mut points = point_db.borrow_mut();
                let mut styles = psi.borrow_mut();
                for &idx in inds.iter().rev() {
                    if idx < points.len() {
                        let pt = points.remove(idx);
                        if idx < styles.len() {
                            styles.remove(idx);
                        }
                        backend_render.borrow_mut().remove_point(idx);
                        command_stack.borrow_mut().push(Command::AddPoint {
                            index: idx,
                            point: pt,
                        });
                    }
                }
                inds.clear();
            }
            {
                let mut lines = lines_ref.borrow_mut();
                let mut styles = lsi.borrow_mut();
                for i in (0..lines.len()).rev() {
                    let l = lines[i];
                    if selected_lines
                        .borrow()
                        .iter()
                        .any(|(s, e)| (l.0 == *s && l.1 == *e) || (l.0 == *e && l.1 == *s))
                    {
                        lines.remove(i);
                        if i < styles.len() {
                            styles.remove(i);
                        }
                        backend_render.borrow_mut().remove_line(i);
                    }
                }
                selected_lines.borrow_mut().clear();
            }
            {
                let mut polys = polygons_ref.borrow_mut();
                let mut idxs = selected_polygons.borrow_mut();
                idxs.sort();
                for &i in idxs.iter().rev() {
                    if i < polys.len() {
                        polys.remove(i);
                    }
                }
                idxs.clear();
            }
            {
                let mut plines = polylines_ref.borrow_mut();
                let mut idxs = selected_polylines.borrow_mut();
                idxs.sort();
                for &i in idxs.iter().rev() {
                    if i < plines.len() {
                        plines.remove(i);
                    }
                }
                idxs.clear();
            }
            {
                let mut arcs = arcs_ref.borrow_mut();
                let mut idxs = selected_arcs.borrow_mut();
                idxs.sort();
                for &i in idxs.iter().rev() {
                    if i < arcs.len() {
                        arcs.remove(i);
                    }
                }
                idxs.clear();
            }
            {
                let mut dims = dimensions_ref.borrow_mut();
                let mut idxs = selected_dimensions.borrow_mut();
                idxs.sort();
                for &i in idxs.iter().rev() {
                    if i < dims.len() {
                        dims.remove(i);
                        backend_render.borrow_mut().remove_dimension(i);
                    }
                }
                idxs.clear();
            }
            if let Some(app) = weak.upgrade() {
                crate::set_workspace_image_result(&app, &render_image);
                app.window().request_redraw();
            }
        });
    }

    {
        let weak = app.as_weak();
        let polylines_ref = polylines.clone();
        let selected_polylines_ref = selected_polylines.clone();
        let backend_ref = backend.clone();
        app.on_extrude_polyline(move || {
            let dlg = ExtrudePolylineDialog::new().unwrap();
            dlg.set_distance_value("1".into());
            dlg.set_dx_value("0".into());
            dlg.set_dy_value("0".into());
            dlg.set_dz_value("1".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let polylines_inner = polylines_ref.clone();
            let selected_pl = selected_polylines_ref.clone();
            let backend_inner = backend_ref.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let dist = d.get_distance_value().parse::<f64>().unwrap_or(0.0);
                    let dx = d.get_dx_value().parse::<f64>().unwrap_or(0.0);
                    let dy = d.get_dy_value().parse::<f64>().unwrap_or(0.0);
                    let dz = d.get_dz_value().parse::<f64>().unwrap_or(1.0);
                    let mut dir = Vector3::new(dx, dy, dz);
                    if dir.magnitude2() < f64::EPSILON {
                        dir = Vector3::unit_z();
                    } else {
                        dir = dir.normalize();
                    }
                    let vec = dir * dist;
                    for &idx in selected_pl.borrow().iter() {
                        if let Some(pl) = polylines_inner.borrow().get(idx) {
                            if let Some(sol) = polyline_to_solid(pl, vec) {
                                backend_inner.borrow_mut().add_solid(sol);
                            }
                        }
                    }
                    if let Some(app) = weak2.upgrade() {
                        if app.get_workspace_mode() == 1 {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                        }
                        app.window().request_redraw();
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
        let alignments = alignments.clone();
        let backend = backend.clone();
        app.on_design_cross_sections(move || {
            let dlg = DesignSectionDialog::new().unwrap();
            dlg.set_start_station("0".into());
            dlg.set_end_station("100".into());
            dlg.set_interval("10".into());
            dlg.set_lane_width("3.5".into());
            dlg.set_lane_slope("-0.02".into());
            dlg.set_shoulder_width("1.0".into());
            dlg.set_shoulder_slope("-0.04".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let aligns = alignments.clone();
            let backend_inner = backend.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    let res = (|| {
                        let start = d.get_start_station().parse::<f64>().ok()?;
                        let end = d.get_end_station().parse::<f64>().ok()?;
                        let interval = d.get_interval().parse::<f64>().ok()?;
                        let lane_w = d.get_lane_width().parse::<f64>().ok()?;
                        let lane_s = d.get_lane_slope().parse::<f64>().ok()?;
                        let sh_w = d.get_shoulder_width().parse::<f64>().ok()?;
                        let sh_s = d.get_shoulder_slope().parse::<f64>().ok()?;
                        let aligns = aligns.borrow();
                        if aligns.is_empty() {
                            return None;
                        }
                        let al = &aligns[0];
                        let lane = subassembly::lane(lane_w, lane_s);
                        let shoulder = subassembly::shoulder(sh_w, sh_s);
                        let sections = subassembly::symmetric_section(&[lane, shoulder]);
                        let mut cs =
                            corridor::extract_design_cross_sections(al, &sections, None, interval);
                        cs.retain(|c| c.station >= start && c.station <= end);
                        for section in cs {
                            for pair in section.points.windows(2) {
                                backend_inner.borrow_mut().add_line(
                                    [pair[0].x, pair[0].y, pair[0].z],
                                    [pair[1].x, pair[1].y, pair[1].z],
                                    [1.0, 1.0, 1.0, 1.0],
                                    1.0,
                                );
                            }
                        }
                        Some(())
                    })();
                    if let Some(app) = weak2.upgrade() {
                        if app.get_workspace_mode() == 1 {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
                        }
                        if res.is_some() {
                            app.set_status(SharedString::from("Sections generated"));
                        } else {
                            app.set_status(SharedString::from(
                                "Invalid input or missing alignment",
                            ));
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
        let pipe_editor = pipe_editor.clone();
        let weak = app.as_weak();
        app.on_pipe_editor(move || {
            let dlg_s = StructureManager::new().unwrap();
            let dlg_p = PipeManager::new().unwrap();

            let structs = pipe_editor
                .borrow()
                .network
                .structures
                .iter()
                .map(|s| StructureRow {
                    id: SharedString::from(s.id.clone()),
                    x: SharedString::from(format!("{:.2}", s.x)),
                    y: SharedString::from(format!("{:.2}", s.y)),
                    z: SharedString::from(format!("{:.2}", s.z)),
                })
                .collect::<Vec<_>>();
            let struct_model = Rc::new(VecModel::from(structs));
            dlg_s.set_structures_model(struct_model.clone().into());
            dlg_s.set_selected_index(-1);

            let pipes = pipe_editor
                .borrow()
                .network
                .pipes
                .iter()
                .map(|p| PipeRow {
                    id: SharedString::from(p.id.clone()),
                    from: SharedString::from(p.from.clone()),
                    to: SharedString::from(p.to.clone()),
                    diameter: SharedString::from(format!("{:.2}", p.diameter)),
                })
                .collect::<Vec<_>>();
            let pipe_model = Rc::new(VecModel::from(pipes));
            dlg_p.set_pipes_model(pipe_model.clone().into());
            dlg_p.set_selected_index(-1);

            {
                let pipe_editor = pipe_editor.clone();
                let struct_model = struct_model.clone();
                dlg_s.on_add_structure(move || {
                    let mut editor = pipe_editor.borrow_mut();
                    let id = format!("S{}", editor.network.structures.len() + 1);
                    editor.network.structures.push(Structure { id: id.clone(), x: 0.0, y: 0.0, z: 0.0 });
                    struct_model.push(StructureRow { id: id.into(), x: "0".into(), y: "0".into(), z: "0".into() });
                    editor.refresh_render();
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                let struct_model = struct_model.clone();
                dlg_s.on_remove_structure(move |idx| {
                    if idx >= 0 {
                        let mut editor = pipe_editor.borrow_mut();
                        if (idx as usize) < editor.network.structures.len() {
                            editor.network.structures.remove(idx as usize);
                            struct_model.remove(idx as usize);
                            editor.refresh_render();
                        }
                    }
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                dlg_s.on_edit_id(move |idx, text| {
                    if idx >= 0 {
                        if let Some(s) = pipe_editor.borrow_mut().network.structures.get_mut(idx as usize) {
                            s.id = text.to_string();
                        }
                    }
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                dlg_s.on_edit_x(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(s) = pipe_editor.borrow_mut().network.structures.get_mut(idx as usize) {
                            s.x = v;
                            pipe_editor.borrow_mut().refresh_render();
                        }
                    }
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                dlg_s.on_edit_y(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(s) = pipe_editor.borrow_mut().network.structures.get_mut(idx as usize) {
                            s.y = v;
                            pipe_editor.borrow_mut().refresh_render();
                        }
                    }
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                dlg_s.on_edit_z(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(s) = pipe_editor.borrow_mut().network.structures.get_mut(idx as usize) {
                            s.z = v;
                            pipe_editor.borrow_mut().refresh_render();
                        }
                    }
                });
            }

            {
                let pipe_editor = pipe_editor.clone();
                let pipe_model = pipe_model.clone();
                dlg_p.on_add_pipe(move || {
                    let mut editor = pipe_editor.borrow_mut();
                    let id = format!("P{}", editor.network.pipes.len() + 1);
                    let from = editor.network.structures.first().map(|s| s.id.clone()).unwrap_or_default();
                    let to = from.clone();
                    editor.network.pipes.push(Pipe {
                        id: id.clone(),
                        from: from.clone(),
                        to: to.clone(),
                        diameter: 0.3,
                        c: 100.0,
                        start_invert: 0.0,
                        end_invert: 0.0,
                        design_flow: 0.0,
                    });
                    pipe_model.push(PipeRow { id: id.into(), from: from.into(), to: to.into(), diameter: "0.3".into() });
                    editor.refresh_render();
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                let pipe_model = pipe_model.clone();
                dlg_p.on_remove_pipe(move |idx| {
                    if idx >= 0 {
                        let mut editor = pipe_editor.borrow_mut();
                        if (idx as usize) < editor.network.pipes.len() {
                            editor.network.pipes.remove(idx as usize);
                            pipe_model.remove(idx as usize);
                            editor.refresh_render();
                        }
                    }
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                dlg_p.on_edit_id(move |idx, text| {
                    if let Some(p) = pipe_editor.borrow_mut().network.pipes.get_mut(idx as usize) {
                        p.id = text.to_string();
                    }
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                dlg_p.on_edit_from(move |idx, text| {
                    if let Some(p) = pipe_editor.borrow_mut().network.pipes.get_mut(idx as usize) {
                        p.from = text.to_string();
                        pipe_editor.borrow_mut().refresh_render();
                    }
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                dlg_p.on_edit_to(move |idx, text| {
                    if let Some(p) = pipe_editor.borrow_mut().network.pipes.get_mut(idx as usize) {
                        p.to = text.to_string();
                        pipe_editor.borrow_mut().refresh_render();
                    }
                });
            }
            {
                let pipe_editor = pipe_editor.clone();
                dlg_p.on_edit_diameter(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(p) = pipe_editor.borrow_mut().network.pipes.get_mut(idx as usize) {
                            p.diameter = v;
                            pipe_editor.borrow_mut().refresh_render();
                        }
                    }
                });
            }

            dlg_s.show().unwrap();
            dlg_p.show().unwrap();
            if let Some(app) = weak.upgrade() {
                app.window().request_redraw();
            }
        });
    }

    {
        let weak = app.as_weak();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let backend = backend.clone();
        app.on_view_cross_sections(move || {
            let surfs = surfaces.borrow();
            let aligns = alignments.borrow();
            if surfs.is_empty() || aligns.is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("Need surface and alignment"));
                }
                return;
            }
            let al = aligns[0].clone();
            let sections = corridor::extract_cross_sections(&surfs[0], &al, 10.0, 10.0, 1.0);
            if sections.is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("No cross sections"));
                }
                return;
            }
            let viewer = CrossSectionViewer::new().unwrap();
            let current = Rc::new(RefCell::new(0usize));
            viewer.set_station_label(SharedString::from(format!(
                "Station: {:.2}",
                sections[0].station
            )));
            let elev = al.vertical.elevation_at(sections[0].station).unwrap_or(0.0);
            let grade = grade_at(&al.vertical, sections[0].station).unwrap_or(0.0);
            viewer.set_elevation_label(SharedString::from(format!("Elev: {elev:.2}")));
            viewer.set_slope_label(SharedString::from(format!("Slope: {grade:.4}")));
            if let Ok(img) = render_cross_section(&sections[0], 600, 300) {
                viewer.set_section_image(img);
            }
            let handles0: Vec<HandlePoint> = handle_positions(&sections[0], 600.0, 300.0)
                .into_iter()
                .map(|(x, y)| HandlePoint { x, y })
                .collect();
            viewer.set_handles_model(Rc::new(VecModel::from(handles0)).into());
            let viewer_weak = viewer.as_weak();
            let secs = Rc::new(RefCell::new(sections));
            let drag_index = Rc::new(RefCell::new(None::<usize>));
            {
                let current = current.clone();
                let secs = secs.clone();
                let viewer_weak = viewer_weak.clone();
                let al = al.clone();
                viewer.on_prev(move || {
                    if *current.borrow() > 0 {
                        *current.borrow_mut() -= 1;
                        let i = *current.borrow();
                        if let Some(v) = viewer_weak.upgrade() {
                            let secs_b = secs.borrow();
                            v.set_station_label(SharedString::from(format!(
                                "Station: {:.2}",
                                secs_b[i].station
                            )));
                            let elev = al.vertical.elevation_at(secs_b[i].station).unwrap_or(0.0);
                            let grade = grade_at(&al.vertical, secs_b[i].station).unwrap_or(0.0);
                            v.set_elevation_label(SharedString::from(format!("Elev: {elev:.2}")));
                            v.set_slope_label(SharedString::from(format!("Slope: {grade:.4}")));
                            if let Ok(img) = render_cross_section(&secs_b[i], 600, 300) {
                                v.set_section_image(img);
                            }
                            let handles: Vec<HandlePoint> = handle_positions(&secs_b[i], 600.0, 300.0)
                                .into_iter()
                                .map(|(x, y)| HandlePoint { x, y })
                                .collect();
                            v.set_handles_model(Rc::new(VecModel::from(handles)).into());
                        }
                    }
                });
            }
            {
                let current = current.clone();
                let secs = secs.clone();
                let viewer_weak = viewer_weak.clone();
                let al = al.clone();
                viewer.on_next(move || {
                    if *current.borrow() + 1 < secs.borrow().len() {
                        *current.borrow_mut() += 1;
                        let i = *current.borrow();
                        if let Some(v) = viewer_weak.upgrade() {
                            let secs_b = secs.borrow();
                            v.set_station_label(SharedString::from(format!(
                                "Station: {:.2}",
                                secs_b[i].station
                            )));
                            let elev = al.vertical.elevation_at(secs_b[i].station).unwrap_or(0.0);
                            let grade = grade_at(&al.vertical, secs_b[i].station).unwrap_or(0.0);
                            v.set_elevation_label(SharedString::from(format!("Elev: {elev:.2}")));
                            v.set_slope_label(SharedString::from(format!("Slope: {grade:.4}")));
                            if let Ok(img) = render_cross_section(&secs_b[i], 600, 300) {
                                v.set_section_image(img);
                            }
                        }
                    }
                });
            }
            {
                let secs_p = secs.clone();
                let current_p = current.clone();
                let drag_p = drag_index.clone();
                viewer.on_pointer_pressed(move |x, y| {
                    let secs_b = secs_p.borrow();
                    if let Some(idx) =
                        nearest_point(&secs_b[*current_p.borrow()], x, y, 600.0, 300.0)
                    {
                        *drag_p.borrow_mut() = Some(idx);
                    }
                });

                let secs_m = secs.clone();
                let current_m = current.clone();
                let drag_m = drag_index.clone();
                let viewer_weak_m = viewer_weak.clone();
                let surfaces_m = surfaces.clone();
                let backend_m = backend.clone();
                viewer.on_pointer_moved(move |x, y| {
                    if let Some(idx) = *drag_m.borrow() {
                        if let Some(p) = screen_to_world(
                            &secs_m.borrow()[*current_m.borrow()],
                            x,
                            y,
                            600.0,
                            300.0,
                        ) {
                            secs_m.borrow_mut()[*current_m.borrow()].points[idx] = p;
                            if let Some(v) = viewer_weak_m.upgrade() {
                                if let Ok(img) = render_cross_section(
                                    &secs_m.borrow()[*current_m.borrow()],
                                    600,
                                    300,
                                ) {
                                    v.set_section_image(img);
                                }
                                let handles: Vec<HandlePoint> = handle_positions(
                                    &secs_m.borrow()[*current_m.borrow()],
                                    600.0,
                                    300.0,
                                )
                                .into_iter()
                                .map(|(hx, hy)| HandlePoint { x: hx, y: hy })
                                .collect();
                                v.set_handles_model(Rc::new(VecModel::from(handles)).into());
                            }
                            let tin = corridor::surface_from_cross_sections(&secs_m.borrow());
                            let verts: Vec<Point3> = tin
                                .vertices
                                .iter()
                                .map(|p| Point3::new(p.x, p.y, p.z))
                                .collect();
                            if surfaces_m.borrow().is_empty() {
                                backend_m.borrow_mut().add_surface(&verts, &tin.triangles);
                                surfaces_m.borrow_mut().push(tin);
                            } else {
                                backend_m
                                    .borrow_mut()
                                    .update_surface(0, &verts, &tin.triangles);
                                surfaces_m.borrow_mut()[0] = tin;
                            }
                        }
                    }
                });

                let secs_r = secs.clone();
                let surfaces_r = surfaces.clone();
                let backend_r = backend.clone();
                let drag_r = drag_index.clone();
                let current = current.clone();
                let viewer_weak = viewer_weak.clone();
                viewer.on_pointer_released(move || {
                    if drag_r.borrow().is_some() {
                        *drag_r.borrow_mut() = None;
                        let tin = corridor::surface_from_cross_sections(&secs_r.borrow());
                        let verts: Vec<Point3> = tin
                            .vertices
                            .iter()
                            .map(|p| Point3::new(p.x, p.y, p.z))
                            .collect();
                        if surfaces_r.borrow().is_empty() {
                            backend_r.borrow_mut().add_surface(&verts, &tin.triangles);
                            surfaces_r.borrow_mut().push(tin);
                        } else {
                            backend_r
                                .borrow_mut()
                                .update_surface(0, &verts, &tin.triangles);
                            surfaces_r.borrow_mut()[0] = tin;
                        }
                        if let Some(v) = viewer_weak.upgrade() {
                            let handles: Vec<HandlePoint> = handle_positions(
                                &secs_r.borrow()[*current.borrow()],
                                600.0,
                                300.0,
                            )
                            .into_iter()
                            .map(|(hx, hy)| HandlePoint { x: hx, y: hy })
                            .collect();
                            v.set_handles_model(Rc::new(VecModel::from(handles)).into());
                        }
                    }
                });
            }
            viewer.show().unwrap();
        });
    }

    {
        let backend = backend.clone();
        let weak = app.as_weak();
        let cs_outer = command_stack.clone();
        let surface_groups = surface_groups.clone();
        app.on_tin_add_vertex(move || {
            let dlg = TinVertexDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let command_stack = cs_outer.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(x), Ok(y), Ok(z)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_x_val().parse::<f64>(),
                        d.get_y_val().parse::<f64>(),
                        d.get_z_val().parse::<f64>(),
                    ) {
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            if let Some(idx) = backend_inner
                                .borrow_mut()
                                .add_vertex(sidx, Point3::new(x, y, z))
                            {
                                command_stack.borrow_mut().push(Command::TinDeleteVertex {
                                    surface: sidx,
                                    index: idx,
                                    point: Point3::new(x, y, z),
                                });
                            }
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend = backend.clone();
        let weak = app.as_weak();
        let surface_groups = surface_groups.clone();
        app.on_tin_move_vertex(move || {
            let dlg = TinVertexDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(idx), Ok(x), Ok(y), Ok(z)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_vertex_index().parse::<usize>(),
                        d.get_x_val().parse::<f64>(),
                        d.get_y_val().parse::<f64>(),
                        d.get_z_val().parse::<f64>(),
                    ) {
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            backend_inner
                                .borrow_mut()
                                .move_vertex(sidx, idx, Point3::new(x, y, z));
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend = backend.clone();
        let weak = app.as_weak();
        let surface_groups = surface_groups.clone();
        app.on_tin_delete_vertex(move || {
            let dlg = TinVertexDialog::new().unwrap();
            dlg.set_x_val("0".into());
            dlg.set_y_val("0".into());
            dlg.set_z_val("0".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(idx)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_vertex_index().parse::<usize>(),
                    ) {
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            backend_inner.borrow_mut().delete_vertex(sidx, idx);
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend = backend.clone();
        let weak = app.as_weak();
        let surface_groups = surface_groups.clone();
        app.on_tin_add_triangle(move || {
            let dlg = TinTriangleDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(a), Ok(b), Ok(c)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_v1().parse::<usize>(),
                        d.get_v2().parse::<usize>(),
                        d.get_v3().parse::<usize>(),
                    ) {
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            backend_inner.borrow_mut().add_triangle(sidx, [a, b, c]);
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend = backend.clone();
        let weak = app.as_weak();
        let surface_groups = surface_groups.clone();
        app.on_tin_delete_triangle(move || {
            let dlg = TinTriangleDialog::new().unwrap();
            dlg.set_v1("0".into());
            dlg.set_v2("0".into());
            dlg.set_v3("0".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(idx)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_tri_index().parse::<usize>(),
                    ) {
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            backend_inner.borrow_mut().delete_triangle(sidx, idx);
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend = backend.clone();
        let weak = app.as_weak();
        let surface_groups = surface_groups.clone();
        app.on_tin_add_breakline(move || {
            let dlg = TinBreaklineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(a), Ok(b)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_v1().parse::<usize>(),
                        d.get_v2().parse::<usize>(),
                    ) {
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            backend_inner.borrow_mut().add_breakline(sidx, a, b);
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend = backend.clone();
        let weak = app.as_weak();
        let surface_groups = surface_groups.clone();
        app.on_tin_remove_breakline(move || {
            let dlg = TinBreaklineDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let (Ok(surf), Ok(a), Ok(b)) = (
                        d.get_surface_index().parse::<usize>(),
                        d.get_v1().parse::<usize>(),
                        d.get_v2().parse::<usize>(),
                    ) {
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            backend_inner.borrow_mut().remove_breakline(sidx, a, b);
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend = backend.clone();
        let weak = app.as_weak();
        let surface_groups = surface_groups.clone();
        app.on_tin_set_boundary(move || {
            let dlg = TinBoundaryDialog::new().unwrap();
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let Ok(surf) = d.get_surface_index().parse::<usize>() {
                        let verts: Vec<usize> = d
                            .get_verts()
                            .split(|c: char| c == ',' || c.is_whitespace())
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            backend_inner.borrow_mut().set_boundary(sidx, verts.clone());
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend = backend.clone();
        let weak = app.as_weak();
        let surface_groups = surface_groups.clone();
        app.on_tin_clear_boundary(move || {
            let dlg = TinBoundaryDialog::new().unwrap();
            dlg.set_verts("".into());
            let dlg_weak = dlg.as_weak();
            let weak2 = weak.clone();
            let backend_inner = backend.clone();
            let surface_groups_inner = surface_groups.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let Ok(surf) = d.get_surface_index().parse::<usize>() {
                        let targets = if let Some(g) = surface_groups_inner
                            .borrow()
                            .iter()
                            .find(|g| g.surface_ids.contains(&surf))
                        {
                            g.surface_ids.clone()
                        } else {
                            vec![surf]
                        };
                        for sidx in targets {
                            backend_inner.borrow_mut().clear_boundary(sidx);
                        }
                        if let Some(app) = weak2.upgrade() {
                            let image = backend_inner.borrow_mut().render();
                            app.set_workspace_texture(image);
                            app.window().request_redraw();
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
        let backend_render = backend.clone();
        let workspace_crs = workspace_crs.clone();
        app.on_import_geojson(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("GeoJSON", &["geojson", "json"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    let dst = *workspace_crs.borrow();
                    match survey_cad::io::read_points_geojson(p, Some(4326), Some(dst)) {
                        Ok(pts) => {
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts);
                                backend_render.borrow_mut().clear();
                                for pt in db.iter() {
                                    backend_render.borrow_mut().add_point(pt.x, pt.y, 0.0);
                                }
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
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
        let backend_render = backend.clone();
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
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
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
        let backend_render = backend.clone();
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
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
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
        let backend_render = backend.clone();
        app.on_import_dwg(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DWG", &["dwg"])
                .add_filter("DGN", &["dgn"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::read_dwg(p) {
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
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            let msg = if e.to_string().contains("dwg2dxf") {
                                "dwg2dxf tool not found".to_string()
                            } else {
                                format!("Failed to import: {e}")
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(msg));
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
        let backend_render = backend.clone();
        let workspace_crs = workspace_crs.clone();
        app.on_import_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    match survey_cad::io::shp::read_points_shp(p) {
                        Ok((mut pts, _)) => {
                            let dst = *workspace_crs.borrow();
                            let src = survey_cad::crs::Crs::from_epsg(4326);
                            let dst_crs = survey_cad::crs::Crs::from_epsg(dst);
                            for p in &mut pts {
                                if let Some((x, y)) = src.transform_point(&dst_crs, p.x, p.y) {
                                    p.x = x;
                                    p.y = y;
                                }
                            }
                            let len = {
                                let mut db = point_db.borrow_mut();
                                db.clear();
                                db.extend(pts);
                                db.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} points"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
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
        let lines = lines.clone();
        let polylines_ref = polylines.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_polylines_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    match survey_cad::io::shp::read_polylines_shp(p) {
                        Ok((pls, _)) => {
                            let mut lns = lines.borrow_mut();
                            let mut pls_vec = polylines_ref.borrow_mut();
                            lns.clear();
                            pls_vec.clear();
                            for pl in pls {
                                if pl.vertices.len() == 2 {
                                    lns.push((pl.vertices[0], pl.vertices[1]));
                                } else {
                                    pls_vec.push(pl);
                                }
                            }
                            let count = lns.len() + pls_vec.len();
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {count} polylines"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
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
        let polygons_ref = polygons.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_import_polygons_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    match survey_cad::io::shp::read_polygons_shp(p) {
                        Ok((polys, _)) => {
                            let len = {
                                let mut pg = polygons_ref.borrow_mut();
                                pg.clear();
                                pg.extend(polys);
                                pg.len()
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Imported {len} polygons"
                                )));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
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
        let backend_render = backend.clone();
        let config_ref = config.clone();
        let surfaces_ref = surfaces.clone();
        app.on_import_las(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LAS", &["las", "laz"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    let p = p.to_string();
                    #[cfg(feature = "las")]
                    {
                        use std::sync::{
                            atomic::{AtomicBool, Ordering},
                            Arc,
                        };
                        let dlg = match ImportProgressDialog::new() {
                            Ok(d) => d,
                            Err(e) => {
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from(format!("UI error: {e}")));
                                }
                                return;
                            }
                        };
                        dlg.set_message(SharedString::from("Importing LAS"));
                        dlg.set_progress(0.0);
                        let cancel = Arc::new(AtomicBool::new(false));
                        let cancel_dlg = cancel.clone();
                        let dlg_weak = dlg.as_weak();
                        dlg.on_cancel(move || {
                            cancel_dlg.store(true, Ordering::SeqCst);
                        });
                        if let Err(e) = dlg.show() {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("UI error: {e}")));
                            }
                            return;
                        }
                        use slint::{Timer, TimerMode};
                        use std::sync::mpsc;

                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            let res = survey_cad::io::las::read_points_las_progress(&p, |prog| {
                                let dweak = dlg_weak.clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(d) = dweak.upgrade() {
                                        d.set_progress(prog);
                                    }
                                });
                                !cancel.load(Ordering::SeqCst)
                            });
                            let _ = tx.send(res);
                            slint::invoke_from_event_loop(move || {
                                if let Some(d) = dlg_weak.upgrade() {
                                    let _ = d.hide();
                                }
                            })
                            .unwrap();
                        });

                        let weak_app = weak.clone();
                        let point_db = point_db.clone();
                        let render_image = render_image.clone();
                        let backend_render = backend_render.clone();
                        let config = config_ref.clone();
                        let surfaces = surfaces_ref.clone();
                        let timer = Rc::new(Timer::default());
                        let timer_handle = timer.clone();
                        timer.start(
                            TimerMode::Repeated,
                            core::time::Duration::from_millis(50),
                            move || {
                                if let Ok(res) = rx.try_recv() {
                                    timer_handle.stop();
                                    match res {
                                        Ok(pts3) => {
                                            let len = {
                                                let mut db = point_db.borrow_mut();
                                                db.clear();
                                                db.extend(
                                                    pts3.iter().map(|p3| Point::new(p3.x, p3.y)),
                                                );
                                                db.len()
                                            };
                                            if config.borrow().auto_tin && len >= 3 {
                                                let verts_sc: Vec<ScPoint3> = pts3
                                                    .iter()
                                                    .map(|p| ScPoint3::new(p.x, p.y, p.z))
                                                    .collect();
                                                let tin = survey_cad::dtm::Tin::from_points(
                                                    verts_sc.clone(),
                                                );
                                                let verts: Vec<Point3> = tin
                                                    .vertices
                                                    .iter()
                                                    .map(|p| Point3::new(p.x, p.y, p.z))
                                                    .collect();
                                                backend_render
                                                    .borrow_mut()
                                                    .add_surface(&verts, &tin.triangles);
                                                surfaces.borrow_mut().push(tin);
                                            }
                                            if let Some(app) = weak_app.upgrade() {
                                                app.set_status(SharedString::from(format!(
                                                    "Imported {len} points"
                                                )));
                                                if app.get_workspace_mode() == 0 {
                                                    crate::set_workspace_image_result(
                                                        &app,
                                                        &render_image,
                                                    );
                                                } else {
                                                    let image =
                                                        backend_render.borrow_mut().render();
                                                    app.set_workspace_texture(image);
                                                }
                                                app.window().request_redraw();
                                            }
                                        }
                                        Err(e) => {
                                            if let Some(app) = weak_app.upgrade() {
                                                app.set_status(SharedString::from(format!(
                                                    "Failed to import: {e}"
                                                )));
                                            }
                                        }
                                    }
                                }
                            },
                        );
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
        let backend_render = backend.clone();
        let config_ref = config.clone();
        let surfaces_ref = surfaces.clone();
        app.on_import_e57(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("E57", &["e57"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    let p = p.to_string();
                    #[cfg(feature = "e57")]
                    {
                        use std::sync::{
                            atomic::{AtomicBool, Ordering},
                            Arc,
                        };
                        let dlg = match ImportProgressDialog::new() {
                            Ok(d) => d,
                            Err(e) => {
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from(format!("UI error: {e}")));
                                }
                                return;
                            }
                        };
                        dlg.set_message(SharedString::from("Importing E57"));
                        dlg.set_progress(0.0);
                        let cancel = Arc::new(AtomicBool::new(false));
                        let cancel_dlg = cancel.clone();
                        let dlg_weak = dlg.as_weak();
                        dlg.on_cancel(move || {
                            cancel_dlg.store(true, Ordering::SeqCst);
                        });
                        if let Err(e) = dlg.show() {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!("UI error: {e}")));
                            }
                            return;
                        }
                        use slint::{Timer, TimerMode};
                        use std::sync::mpsc;

                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            let res = survey_cad::io::e57::read_points_e57_progress(&p, |prog| {
                                let dweak = dlg_weak.clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(d) = dweak.upgrade() {
                                        d.set_progress(prog);
                                    }
                                });
                                !cancel.load(Ordering::SeqCst)
                            });
                            let _ = tx.send(res);
                            slint::invoke_from_event_loop(move || {
                                if let Some(d) = dlg_weak.upgrade() {
                                    let _ = d.hide();
                                }
                            })
                            .unwrap();
                        });

                        let weak_app = weak.clone();
                        let point_db = point_db.clone();
                        let render_image = render_image.clone();
                        let backend_render = backend_render.clone();
                        let config = config_ref.clone();
                        let surfaces = surfaces_ref.clone();
                        let timer = Rc::new(Timer::default());
                        let timer_handle = timer.clone();
                        timer.start(
                            TimerMode::Repeated,
                            core::time::Duration::from_millis(50),
                            move || {
                                if let Ok(res) = rx.try_recv() {
                                    timer_handle.stop();
                                    match res {
                                        Ok(pts3) => {
                                            let len = {
                                                let mut db = point_db.borrow_mut();
                                                db.clear();
                                                db.extend(
                                                    pts3.iter().map(|p3| Point::new(p3.x, p3.y)),
                                                );
                                                db.len()
                                            };
                                            if config.borrow().auto_tin && len >= 3 {
                                                let verts_sc: Vec<ScPoint3> = pts3
                                                    .iter()
                                                    .map(|p| ScPoint3::new(p.x, p.y, p.z))
                                                    .collect();
                                                let tin = survey_cad::dtm::Tin::from_points(
                                                    verts_sc.clone(),
                                                );
                                                let verts: Vec<Point3> = tin
                                                    .vertices
                                                    .iter()
                                                    .map(|p| Point3::new(p.x, p.y, p.z))
                                                    .collect();
                                                backend_render
                                                    .borrow_mut()
                                                    .add_surface(&verts, &tin.triangles);
                                                surfaces.borrow_mut().push(tin);
                                            }
                                            if let Some(app) = weak_app.upgrade() {
                                                app.set_status(SharedString::from(format!(
                                                    "Imported {len} points"
                                                )));
                                                if app.get_workspace_mode() == 0 {
                                                    crate::set_workspace_image_result(
                                                        &app,
                                                        &render_image,
                                                    );
                                                } else {
                                                    let image =
                                                        backend_render.borrow_mut().render();
                                                    app.set_workspace_texture(image);
                                                }
                                                app.window().request_redraw();
                                            }
                                        }
                                        Err(e) => {
                                            if let Some(app) = weak_app.upgrade() {
                                                app.set_status(SharedString::from(format!(
                                                    "Failed to import: {e}"
                                                )));
                                            }
                                        }
                                    }
                                }
                            },
                        );
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
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
        app.on_export_dwg(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DWG", &["dwg"])
                .add_filter("DGN", &["dgn"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let ents: Vec<survey_cad::io::DxfEntity> = point_db
                        .borrow()
                        .iter()
                        .map(|pt| survey_cad::io::DxfEntity::Point {
                            point: *pt,
                            layer: None,
                        })
                        .collect();
                    match survey_cad::io::write_dwg(p, &ents) {
                        Ok(()) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Exported"));
                            }
                        }
                        Err(e) => {
                            let msg = if e.to_string().contains("dxf2dwg") {
                                "dxf2dwg tool not found".to_string()
                            } else {
                                format!("Failed to export: {e}")
                            };
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(msg));
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
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
        let lines_ref = lines.clone();
        let polylines_ref = polylines.clone();
        app.on_export_polylines_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    {
                        let mut out = Vec::new();
                        for (s, e) in lines_ref.borrow().iter() {
                            out.push(Polyline::new(vec![*s, *e]));
                        }
                        out.extend(polylines_ref.borrow().iter().cloned());
                        if let Err(e) = survey_cad::io::shp::write_polylines_shp(p, &out, None) {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to export: {e}"
                                )));
                            }
                        } else if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from("Exported"));
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
        let polygons_ref = polygons.clone();
        app.on_export_polygons_shp(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SHP", &["shp"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    #[cfg(feature = "shapefile")]
                    if let Err(e) =
                        survey_cad::io::shp::write_polygons_shp(p, &polygons_ref.borrow(), None)
                    {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
                                    "Failed to export: {e}"
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
                                    "Failed to export: {e}"
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
        let alignments = alignments.clone();
        let surfaces = surfaces.clone();
        app.on_export_landxml(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let pts = point_db.borrow().points().to_vec();
                    let als = alignments.borrow().clone();
                    let surfs = surfaces.borrow().clone();
                    if let Err(e) =
                        survey_cad::io::landxml::write_landxml(p, &pts, &als, &surfs)
                    {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
        let surfaces = surfaces.clone();
        let surface_units_clone = surface_units.clone();
        let surface_styles_clone = surface_styles.clone();
        let surface_descriptions_clone = surface_descriptions.clone();
        app.on_export_landxml_surface(move || {
            if surfaces.borrow().is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("No surface to export"));
                }
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let tin = &surfaces.borrow()[0];
                    let extras = survey_cad::io::landxml::LandxmlExtras {
                        units: surface_units_clone.borrow().first().cloned(),
                        style: surface_styles_clone.borrow().first().cloned(),
                        description: surface_descriptions_clone.borrow().first().cloned(),
                    };
                    if let Err(e) =
                        survey_cad::io::landxml::write_landxml_surface(p, tin, Some(&extras))
                    {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
        let alignments = alignments.clone();
        app.on_export_landxml_alignment(move || {
            if alignments.borrow().is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("No alignment to export"));
                }
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let al = &alignments.borrow()[0];
                    if let Err(e) =
                        survey_cad::io::landxml::write_landxml_alignment(p, &al.horizontal, None)
                    {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
        let surfaces = surfaces.clone();
        let surface_units_clone = surface_units.clone();
        let surface_styles_clone = surface_styles.clone();
        let surface_descriptions_clone = surface_descriptions.clone();
        let alignments = alignments.clone();
        app.on_export_landxml_sections(move || {
            if surfaces.borrow().is_empty() || alignments.borrow().is_empty() {
                if let Some(app) = weak.upgrade() {
                    app.set_status(SharedString::from("No sections to export"));
                }
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .save_file()
            {
                if let Some(p) = path.to_str() {
                    let al = &alignments.borrow()[0];
                    let secs = corridor::extract_cross_sections(
                        &surfaces.borrow()[0],
                        al,
                        10.0,
                        10.0,
                        1.0,
                    );
                    let extras = survey_cad::io::landxml::LandxmlExtras {
                        units: surface_units_clone.borrow().first().cloned(),
                        style: surface_styles_clone.borrow().first().cloned(),
                        description: surface_descriptions_clone.borrow().first().cloned(),
                    };
                    if let Err(e) = survey_cad::io::landxml::write_landxml_cross_sections(
                        p,
                        &secs,
                        Some(&extras),
                    ) {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(SharedString::from(format!("Failed to export: {e}")));
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
        let point_style_indices = point_style_indices.clone();
        let point_style_names = point_style_names.clone();
        let render_image_pm = render_image.clone();
        let backend_render = backend.clone();
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
                            number: SharedString::from((i + 1).to_string()),
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
            let groups_model = Rc::new(VecModel::<SharedString>::from(
                point_db
                    .borrow()
                    .iter_groups()
                    .map(|(_, g)| SharedString::from(g.name.clone()))
                    .collect::<Vec<_>>(),
            ));
            dlg.set_groups_model(groups_model.clone().into());
            dlg.set_styles_model(Rc::new(VecModel::from(point_style_names.clone())).into());
            dlg.set_selected_index(-1);

            let headers = Rc::new(RefCell::new(vec![
                SharedString::from("#"),
                SharedString::from("Name"),
                SharedString::from("X"),
                SharedString::from("Y"),
                SharedString::from("Group"),
                SharedString::from("Style"),
            ]));
            dlg.set_number_header(headers.borrow()[0].clone());
            dlg.set_name_header(headers.borrow()[1].clone());
            dlg.set_x_header(headers.borrow()[2].clone());
            dlg.set_y_header(headers.borrow()[3].clone());
            dlg.set_group_header(headers.borrow()[4].clone());
            dlg.set_style_header(headers.borrow()[5].clone());

            dlg.set_label_font(SharedString::from(
                point_label_style.borrow().text_style.font.clone(),
            ));
            dlg.set_offset_x(SharedString::from(format!(
                "{:.1}",
                point_label_style.borrow().offset[0]
            )));
            dlg.set_offset_y(SharedString::from(format!(
                "{:.1}",
                point_label_style.borrow().offset[1]
            )));

            let rename_in_model: Rc<dyn Fn(usize, SharedString)> = {
                let groups_model = groups_model.clone();
                Rc::new(move |idx: usize, name: SharedString| {
                    if idx < groups_model.row_count() {
                        groups_model.set_row_data(idx, name.clone());
                    }
                })
            };

            {
                let model = model.clone();
                let point_db = point_db.clone();
                let backend_render = backend_render.clone();
                dlg.on_edit_x(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx as usize) {
                            p.x = v;
                            if let Some(row) = model.row_data(idx as usize) {
                                let mut r = row.clone();
                                r.x = SharedString::from(format!("{v:.3}"));
                                model.set_row_data(idx as usize, r);
                            }
                            backend_render
                                .borrow_mut()
                                .update_point(idx as usize, p.x, p.y, 0.0);
                        }
                    }
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                let backend_render = backend_render.clone();
                dlg.on_edit_y(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(p) = point_db.borrow_mut().get_mut(idx as usize) {
                            p.y = v;
                            if let Some(row) = model.row_data(idx as usize) {
                                let mut r = row.clone();
                                r.y = SharedString::from(format!("{v:.3}"));
                                model.set_row_data(idx as usize, r);
                            }
                            backend_render
                                .borrow_mut()
                                .update_point(idx as usize, p.x, p.y, 0.0);
                        }
                    }
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                let psi = point_style_indices.clone();
                let backend_render = backend_render.clone();
                dlg.on_add_point(move || {
                    point_db.borrow_mut().push(Point::new(0.0, 0.0));
                    psi.borrow_mut().push(0);
                    backend_render.borrow_mut().add_point(0.0, 0.0, 0.0);
                    let idx = point_db.borrow().len();
                    model.push(PointRow {
                        number: SharedString::from(format!("{idx}")),
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
                let backend_render = backend_render.clone();
                dlg.on_remove_point(move |idx| {
                    if idx >= 0 && (idx as usize) < point_db.borrow().len() {
                        point_db.borrow_mut().remove(idx as usize);
                        psi.borrow_mut().remove(idx as usize);
                        model.remove(idx as usize);
                        backend_render.borrow_mut().remove_point(idx as usize);
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
                                crate::set_workspace_image_result(&app, &render_image);
                                app.window().request_redraw();
                            }
                        }
                    }
                });
            }
            {
                let groups_model = groups_model.clone();
                let point_db = point_db.clone();
                dlg.on_create_group(move || {
                    let name = format!("Group {}", groups_model.row_count() + 1);
                    point_db.borrow_mut().add_group(name.clone());
                    groups_model.push(SharedString::from(name));
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                let rename_in_model = rename_in_model.clone();
                let dlg_weak = dlg_weak.clone();
                dlg.on_rename_group(move || {
                    if let Some(d) = dlg_weak.upgrade() {
                        let row = d.get_selected_index();
                        if row >= 0 {
                            if let Some(r) = model.row_data(row as usize) {
                                let g_idx = r.group_index as usize;
                                let new_name = format!("Group {}", g_idx + 1);
                                if point_db.borrow_mut().rename_group(g_idx, new_name.clone()) {
                                    rename_in_model(g_idx, SharedString::from(new_name));
                                }
                            }
                        }
                    }
                });
            }
            {
                let model = model.clone();
                let point_db = point_db.clone();
                dlg.on_group_changed(move |p_idx, g_idx| {
                    if let Some(row) = model.row_data(p_idx as usize) {
                        point_db
                            .borrow_mut()
                            .remove_point_from_group(p_idx as usize, row.group_index as usize);
                        point_db
                            .borrow_mut()
                            .assign_point(p_idx as usize, g_idx as usize);
                        let mut r = row.clone();
                        r.group_index = g_idx;
                        model.set_row_data(p_idx as usize, r);
                    }
                });
            }
            {
                let headers = headers.clone();
                dlg.on_header_changed(move |col, text| {
                    if let Some(h) = headers.borrow_mut().get_mut(col as usize) {
                        *h = text.clone();
                    }
                });
            }
            {
                let pls = point_label_style.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                let backend_render = backend_render.clone();
                dlg.on_label_font_changed(move |text| {
                    pls.borrow_mut().text_style.font = text.to_string();
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let pls = point_label_style.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                let backend_render = backend_render.clone();
                dlg.on_offset_x_changed(move |val| {
                    if let Ok(v) = val.parse::<f32>() {
                        pls.borrow_mut().offset[0] = v;
                        if let Some(app) = weak.upgrade() {
                            refresh_workspace(&app, &render_image, &backend_render);
                        }
                    }
                });
            }
            {
                let pls = point_label_style.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                let backend_render = backend_render.clone();
                dlg.on_offset_y_changed(move |val| {
                    if let Ok(v) = val.parse::<f32>() {
                        pls.borrow_mut().offset[1] = v;
                        if let Some(app) = weak.upgrade() {
                            refresh_workspace(&app, &render_image, &backend_render);
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

            let needed = line_style_names.len();
            {
                let mut idx = line_style_indices.borrow_mut();
                if idx.len() < needed {
                    idx.resize(needed, 0);
                }
            }
            let current_indices = line_style_indices.borrow().clone();
            let current_lines = lines.borrow().clone();
            let rows = current_indices
                .iter()
                .enumerate()
                .map(|(i, s_idx)| {
                    if let Some((s, e)) = current_lines.get(i) {
                        LineRow {
                            start: SharedString::from(format!("{:.2},{:.2}", s.x, s.y)),
                            end: SharedString::from(format!("{:.2},{:.2}", e.x, e.y)),
                            style_index: *s_idx as i32,
                        }
                    } else {
                        LineRow {
                            start: SharedString::from(""),
                            end: SharedString::from(""),
                            style_index: *s_idx as i32,
                        }
                    }
                })
                .collect::<Vec<_>>();
            let model = Rc::new(VecModel::<LineRow>::from(rows));
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
                        {
                            let mut iref = indices.borrow_mut();
                            if iref.len() <= idx as usize {
                                iref.resize(idx as usize + 1, 0);
                            }
                            iref[idx as usize] = style_idx as usize;
                        }
                        if let Some(app) = weak.upgrade() {
                            if app.get_workspace_mode() == 0 {
                                crate::set_workspace_image_result(&app, &render_image);
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
        let layers_ref = layers.clone();
        let layer_names_ref = layer_names.clone();
        let line_type_model = line_type_names.clone();
        let backend_render = backend.clone();
        let render_image = render_image.clone();
        let weak = app.as_weak();
        app.on_layer_manager(move || {
            let dlg = LayerManager::new().unwrap();
            dlg.set_line_types_model(line_type_model.clone().into());
            let rows = {
                let mgr = layers_ref.borrow();
                let names = layer_names_ref.borrow();
                names
                    .iter()
                    .map(|n| {
                        let layer = mgr.layer(n).unwrap();
                        LayerRow {
                            name: SharedString::from(n.clone()),
                            on: layer.is_on,
                            locked: layer.is_locked,
                            line_type_index: match layer.line_type.unwrap_or(LineType::Solid) {
                                LineType::Solid => 0,
                                LineType::Dashed => 1,
                                LineType::Dotted => 2,
                            },
                            color: SharedString::from(
                                layer
                                    .line_color
                                    .map(|c| format!("{},{},{}", c[0], c[1], c[2]))
                                    .unwrap_or_default(),
                            ),
                            weight: SharedString::from(
                                layer
                                    .line_weight
                                    .map(|w| format!("{:.2}", w.0))
                                    .unwrap_or_default(),
                            ),
                            text_style: SharedString::from(
                                layer
                                    .text_style
                                    .as_ref()
                                    .map(|t| t.name.clone())
                                    .unwrap_or_default(),
                            ),
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let all_rows = Rc::new(RefCell::new(rows));
            let model = Rc::new(VecModel::<LayerRow>::from(all_rows.borrow().clone()));
            dlg.set_layers_model(model.clone().into());
            dlg.set_search_text(SharedString::default());
            dlg.set_selected_index(-1);

            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_toggle_on(move |idx, val| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        layers.borrow_mut().set_layer_state(&name, val);
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.on = val;
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let rows = all_rows.clone();
                dlg.on_search_changed(move |text| {
                    let text = text.to_lowercase();
                    let filtered: Vec<LayerRow> = rows
                        .borrow()
                        .iter()
                        .filter(|r| r.name.to_lowercase().contains(&text))
                        .cloned()
                        .collect();
                    model.set_vec(filtered);
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_toggle_lock(move |idx, val| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            layer.is_locked = val;
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.locked = val;
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_line_type_changed(move |idx, val| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            layer.line_type = Some(match val {
                                0 => LineType::Solid,
                                1 => LineType::Dashed,
                                _ => LineType::Dotted,
                            });
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.line_type_index = val;
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_color_changed(move |idx, text| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            let vals: Vec<u8> = text
                                .split(',')
                                .filter_map(|v| v.trim().parse::<u8>().ok())
                                .collect();
                            if vals.len() == 3 {
                                layer.line_color = Some([vals[0], vals[1], vals[2]]);
                            }
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.color = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_weight_changed(move |idx, text| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            if let Ok(v) = text.parse::<f32>() {
                                layer.line_weight = Some(LineWeight(v));
                            } else {
                                layer.line_weight = None;
                            }
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.weight = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_text_style_changed(move |idx, text| {
                    if let Some(name) = names.borrow().get(idx as usize).cloned() {
                        if let Some(layer) = layers.borrow_mut().layer_mut(&name) {
                            if text.is_empty() {
                                layer.text_style = None;
                            } else {
                                layer.text_style = Some(ScTextStyle::new(&text, "Arial", 1.0));
                            }
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.text_style = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }

            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_freeze_all(move || {
                    for n in names.borrow().iter() {
                        layers.borrow_mut().set_layer_state(n, false);
                    }
                    for i in 0..model.row_count() {
                        if let Some(row) = model.row_data(i) {
                            let mut r = row.clone();
                            r.on = false;
                            model.set_row_data(i, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_thaw_all(move || {
                    for n in names.borrow().iter() {
                        layers.borrow_mut().set_layer_state(n, true);
                    }
                    for i in 0..model.row_count() {
                        if let Some(row) = model.row_data(i) {
                            let mut r = row.clone();
                            r.on = true;
                            model.set_row_data(i, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_lock_all(move || {
                    for n in names.borrow().iter() {
                        if let Some(l) = layers.borrow_mut().layer_mut(n) {
                            l.is_locked = true;
                        }
                    }
                    for i in 0..model.row_count() {
                        if let Some(row) = model.row_data(i) {
                            let mut r = row.clone();
                            r.locked = true;
                            model.set_row_data(i, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }
            {
                let model = model.clone();
                let layers = layers_ref.clone();
                let names = layer_names_ref.clone();
                let backend_render = backend_render.clone();
                let render_image = render_image.clone();
                let weak = weak.clone();
                dlg.on_unlock_all(move || {
                    for n in names.borrow().iter() {
                        if let Some(l) = layers.borrow_mut().layer_mut(n) {
                            l.is_locked = false;
                        }
                    }
                    for i in 0..model.row_count() {
                        if let Some(row) = model.row_data(i) {
                            let mut r = row.clone();
                            r.locked = false;
                            model.set_row_data(i, r);
                        }
                    }
                    if let Some(app) = weak.upgrade() {
                        refresh_workspace(&app, &render_image, &backend_render);
                    }
                });
            }

            dlg.show().unwrap();
        });
    }

    {
        let surface_groups = surface_groups.clone();
        let weak = app.as_weak();
        app.on_surface_group_manager(move || {
            let dlg = SurfaceGroupManager::new().unwrap();
            let model = Rc::new(VecModel::<SharedString>::from(
                surface_groups
                    .borrow()
                    .iter()
                    .map(|g| SharedString::from(g.name.clone()))
                    .collect::<Vec<_>>(),
            ));
            dlg.set_groups_model(model.clone().into());
            dlg.set_selected_index(-1);
            {
                let surface_groups = surface_groups.clone();
                let model = model.clone();
                dlg.on_create_group(move || {
                    let name = format!("Group {}", surface_groups.borrow().len() + 1);
                    surface_groups.borrow_mut().push(SurfaceGroup {
                        name: name.clone(),
                        surface_ids: Vec::new(),
                    });
                    model.push(SharedString::from(name));
                });
            }
            {
                let surface_groups = surface_groups.clone();
                let model = model.clone();
                dlg.on_rename_group(move |idx| {
                    if idx >= 0 {
                        if let Some(g) = surface_groups.borrow_mut().get_mut(idx as usize) {
                            let new_name = format!("Group {}", idx + 1);
                            g.name = new_name.clone();
                            model.set_row_data(idx as usize, SharedString::from(new_name));
                        }
                    }
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let alignment_groups = alignment_groups.clone();
        let weak = app.as_weak();
        app.on_alignment_group_manager(move || {
            let dlg = AlignmentGroupManager::new().unwrap();
            let model = Rc::new(VecModel::<SharedString>::from(
                alignment_groups
                    .borrow()
                    .iter()
                    .map(|g| SharedString::from(g.name.clone()))
                    .collect::<Vec<_>>(),
            ));
            dlg.set_groups_model(model.clone().into());
            dlg.set_selected_index(-1);
            {
                let alignment_groups = alignment_groups.clone();
                let model = model.clone();
                dlg.on_create_group(move || {
                    let name = format!("Group {}", alignment_groups.borrow().len() + 1);
                    alignment_groups.borrow_mut().push(AlignmentGroup {
                        name: name.clone(),
                        alignment_ids: Vec::new(),
                    });
                    model.push(SharedString::from(name));
                });
            }
            {
                let alignment_groups = alignment_groups.clone();
                let model = model.clone();
                dlg.on_rename_group(move |idx| {
                    if idx >= 0 {
                        if let Some(g) = alignment_groups.borrow_mut().get_mut(idx as usize) {
                            let new_name = format!("Group {}", idx + 1);
                            g.name = new_name.clone();
                            model.set_row_data(idx as usize, SharedString::from(new_name));
                        }
                    }
                });
            }
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let sup_data = superelevation.clone();
        let surfaces = surfaces.clone();
        let alignments = alignments.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        app.on_superelevation_editor(move || {
            let dlg = SuperelevationEditor::new().unwrap();
            let model = Rc::new(VecModel::<SuperelevationRow>::from(
                sup_data
                    .borrow()
                    .iter()
                    .map(|p| SuperelevationRow {
                        station: SharedString::from(format!("{:.2}", p.station)),
                        left: SharedString::from(format!("{:.4}", p.left_slope)),
                        right: SharedString::from(format!("{:.4}", p.right_slope)),
                    })
                    .collect::<Vec<_>>(),
            ));
            dlg.set_rows_model(model.clone().into());
            dlg.set_selected_index(-1);

            let update_design = {
                let sup_data = sup_data.clone();
                let surfaces = surfaces.clone();
                let alignments = alignments.clone();
                let weak = weak.clone();
                let render_image = render_image.clone();
                let backend_render = backend_render.clone();
                move || {
                    if alignments.borrow().is_empty() {
                        return;
                    }
                    let al = &alignments.borrow()[0];
                    let lane = subassembly::lane(3.5, -0.02);
                    let shoulder = subassembly::shoulder(1.0, -0.04);
                    let subs = subassembly::symmetric_section(&[lane, shoulder]);
                    if let Some(app) = weak.upgrade() {
                        app.set_status(SharedString::from("Generating design surface... 0%"));
                        app.window().request_redraw();
                    }
                    let mut last = 0f32;
                    let tin = corridor::build_design_surface_dynamic_with_progress(
                        al,
                        &subs,
                        Some(&sup_data.borrow()),
                        10.0,
                        |p| {
                            let pct = (p * 100.0).round();
                            if pct - last >= 1.0 {
                                if let Some(app) = weak.upgrade() {
                                    app.set_status(SharedString::from(format!(
                                        "Generating design surface... {}%",
                                        pct as i32
                                    )));
                                    app.window().request_redraw();
                                }
                                last = pct;
                            }
                        },
                    );
                    let verts: Vec<Point3> = tin
                        .vertices
                        .iter()
                        .map(|p| Point3::new(p.x, p.y, p.z))
                        .collect();
                    if surfaces.borrow().is_empty() {
                        backend_render
                            .borrow_mut()
                            .add_surface(&verts, &tin.triangles);
                        surfaces.borrow_mut().push(tin);
                    } else {
                        backend_render
                            .borrow_mut()
                            .update_surface(0, &verts, &tin.triangles);
                        surfaces.borrow_mut()[0] = tin;
                    }
                    if let Some(app) = weak.upgrade() {
                        if app.get_workspace_mode() == 0 {
                            crate::set_workspace_image_result(&app, &render_image);
                        } else {
                            let image = backend_render.borrow_mut().render();
                            app.set_workspace_texture(image);
                        }
                        app.window().request_redraw();
                        app.set_status(SharedString::from("Design surface updated"));
                    }
                }
            };

            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_add_row(move || {
                    sup_data.borrow_mut().push(SuperelevationPoint {
                        station: 0.0,
                        left_slope: 0.0,
                        right_slope: 0.0,
                    });
                    model.push(SuperelevationRow {
                        station: "0.0".into(),
                        left: "0.0000".into(),
                        right: "0.0000".into(),
                    });
                    update_design();
                });
            }
            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_remove_row(move |idx| {
                    if idx >= 0 && (idx as usize) < sup_data.borrow().len() {
                        sup_data.borrow_mut().remove(idx as usize);
                        model.remove(idx as usize);
                        update_design();
                    }
                });
            }
            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_edit_station(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(pt) = sup_data.borrow_mut().get_mut(idx as usize) {
                            pt.station = v;
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.station = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                        update_design();
                    }
                });
            }
            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_edit_left(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(pt) = sup_data.borrow_mut().get_mut(idx as usize) {
                            pt.left_slope = v;
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.left = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                        update_design();
                    }
                });
            }
            {
                let model = model.clone();
                let sup_data = sup_data.clone();
                let update_design = update_design.clone();
                dlg.on_edit_right(move |idx, text| {
                    if let Ok(v) = text.parse::<f64>() {
                        if let Some(pt) = sup_data.borrow_mut().get_mut(idx as usize) {
                            pt.right_slope = v;
                        }
                        if let Some(row) = model.row_data(idx as usize) {
                            let mut r = row.clone();
                            r.right = text.clone();
                            model.set_row_data(idx as usize, r);
                        }
                        update_design();
                    }
                });
            }

            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let grid_settings = grid_settings.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let workspace_crs = workspace_crs.clone();
        let config_ref = config.clone();
        app.on_settings(move || {
            let dlg = SettingsDialog::new().unwrap();
            let gs = grid_settings.borrow();
            dlg.set_spacing_value(SharedString::from(format!("{:.1}", gs.spacing)));
            dlg.set_color_r(SharedString::from(gs.color[0].to_string()));
            dlg.set_color_g(SharedString::from(gs.color[1].to_string()));
            dlg.set_color_b(SharedString::from(gs.color[2].to_string()));
            dlg.set_show_grid(gs.visible);
            dlg.set_auto_tin(config_ref.borrow().auto_tin);
            dlg.set_crs_epsg(SharedString::from(workspace_crs.borrow().to_string()));
            dlg.set_profile_index(config_ref.borrow().profile as i32);
            dlg.set_theme_index(config_ref.borrow().theme as i32);
            let mut fonts: Vec<String> = Vec::new();
            if let Ok(dir) = fs::read_dir("assets") {
                for e in dir.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|s| s.to_str()) == Some("ttf") {
                        fonts.push(p.to_string_lossy().to_string());
                    }
                }
            }
            if let Some(fp) = config_ref.borrow().font_path.clone() {
                if !fonts.contains(&fp) {
                    fonts.push(fp);
                }
            }
            let font_items: Vec<SharedString> =
                fonts.iter().map(|s| SharedString::from(s.as_str())).collect();
            dlg.set_font_list(Rc::new(VecModel::from(font_items)).into());
            let current_idx = config_ref
                .borrow()
                .font_path
                .as_ref()
                .and_then(|p| fonts.iter().position(|f| f == p))
                .unwrap_or(0);
            dlg.set_font_index(current_idx as i32);
            drop(gs);
            let fonts_cloned = fonts.clone();
            let dlg_weak = dlg.as_weak();
            let gs_ref = grid_settings.clone();
            let weak_app = weak.clone();
            let render_image = render_image.clone();
            let backend_render = backend_render.clone();
            let crs_ref = workspace_crs.clone();
            let config_acc = config_ref.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let Ok(v) = d.get_spacing_value().parse::<f32>() {
                        gs_ref.borrow_mut().spacing = v;
                    }
                    let r = d.get_color_r().parse::<u8>().unwrap_or(60);
                    let g = d.get_color_g().parse::<u8>().unwrap_or(60);
                    let b = d.get_color_b().parse::<u8>().unwrap_or(60);
                    gs_ref.borrow_mut().color = [r, g, b];
                    gs_ref.borrow_mut().visible = d.get_show_grid();
                    let mut cfg = config_acc.borrow_mut();
                    cfg.auto_tin = d.get_auto_tin();
                    cfg.profile = match d.get_profile_index() {
                        1 => WorkspaceProfile::Engineer,
                        2 => WorkspaceProfile::Gis,
                        _ => WorkspaceProfile::Surveyor,
                    };
                    let theme = match d.get_theme_index() {
                        1 => Theme::Light,
                        _ => Theme::Dark,
                    };
                    cfg.theme = theme;
                    cfg.font_path = fonts_cloned.get(d.get_font_index() as usize).cloned();
                    drop(cfg);
                    if let Ok(epsg) = d.get_crs_epsg().parse::<u32>() {
                        *crs_ref.borrow_mut() = epsg;
                    }
                    d.hide().unwrap();
                }
                if let Some(app) = weak_app.upgrade() {
                    match config_acc.borrow().theme {
                        Theme::Dark => std::env::set_var("SLINT_STYLE", "fluent-dark"),
                        Theme::Light => std::env::set_var("SLINT_STYLE", "fluent-light"),
                    }
                    refresh_workspace(&app, &render_image, &backend_render);
                }
                save_config(&config_acc.borrow());
            });
            let cancel_weak = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = cancel_weak.upgrade() {
                    d.hide().unwrap();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let snap_prefs_ref = snap_prefs.clone();
        let cfg = config.clone();
        app.on_snap_settings(move || {
            let dlg = SnapSettingsDialog::new().unwrap();
            let prefs = snap_prefs_ref.borrow();
            dlg.set_tolerance(SharedString::from(format!("{:.1}", prefs.snap_tolerance)));
            dlg.set_snap_points(prefs.snap_points);
            dlg.set_snap_endpoints(prefs.snap_endpoints);
            dlg.set_snap_midpoints(prefs.snap_midpoints);
            dlg.set_snap_intersections(prefs.snap_intersections);
            dlg.set_snap_nearest(prefs.snap_nearest);
            dlg.set_snap_surfaces(prefs.snap_surfaces);
            dlg.set_snap_solids(prefs.snap_solids);
            drop(prefs);
            let dlg_weak = dlg.as_weak();
            let prefs_ref = snap_prefs_ref.clone();
            let cfg_ref = cfg.clone();
            let app_weak = weak.clone();
            dlg.on_accept(move || {
                if let Some(d) = dlg_weak.upgrade() {
                    if let Ok(v) = d.get_tolerance().parse::<f32>() {
                        prefs_ref.borrow_mut().snap_tolerance = v;
                        cfg_ref.borrow_mut().snap.snap_tolerance = v;
                    }
                    prefs_ref.borrow_mut().snap_points = d.get_snap_points();
                    cfg_ref.borrow_mut().snap.snap_points = d.get_snap_points();
                    prefs_ref.borrow_mut().snap_endpoints = d.get_snap_endpoints();
                    cfg_ref.borrow_mut().snap.snap_endpoints = d.get_snap_endpoints();
                    prefs_ref.borrow_mut().snap_midpoints = d.get_snap_midpoints();
                    cfg_ref.borrow_mut().snap.snap_midpoints = d.get_snap_midpoints();
                    prefs_ref.borrow_mut().snap_intersections = d.get_snap_intersections();
                    cfg_ref.borrow_mut().snap.snap_intersections = d.get_snap_intersections();
                    prefs_ref.borrow_mut().snap_nearest = d.get_snap_nearest();
                    cfg_ref.borrow_mut().snap.snap_nearest = d.get_snap_nearest();
                    prefs_ref.borrow_mut().snap_surfaces = d.get_snap_surfaces();
                    cfg_ref.borrow_mut().snap.snap_surfaces = d.get_snap_surfaces();
                    prefs_ref.borrow_mut().snap_solids = d.get_snap_solids();
                    cfg_ref.borrow_mut().snap.snap_solids = d.get_snap_solids();
                    if let Some(a) = app_weak.upgrade() {
                        a.set_snap_points(d.get_snap_points());
                        a.set_snap_endpoints(d.get_snap_endpoints());
                        a.set_snap_midpoints(d.get_snap_midpoints());
                        a.set_snap_intersections(d.get_snap_intersections());
                        a.set_snap_nearest(d.get_snap_nearest());
                        a.set_snap_surfaces(d.get_snap_surfaces());
                        a.set_snap_solids(d.get_snap_solids());
                        if let Ok(v) = d.get_tolerance().parse::<f32>() {
                            a.set_snap_tolerance(v);
                        }
                    }
                    save_config(&cfg_ref.borrow());
                    d.hide().unwrap();
                }
            });
            let cancel_weak = dlg.as_weak();
            dlg.on_cancel(move || {
                if let Some(d) = cancel_weak.upgrade() {
                    d.hide().unwrap();
                }
            });
            dlg.show().unwrap();
        });
    }

    {
        let weak = app.as_weak();
        let surfaces = surfaces.clone();
        let render_image = render_image.clone();
        let backend_render = backend.clone();
        let surface_units = surface_units.clone();
        let surface_styles = surface_styles.clone();
        let surface_descriptions = surface_descriptions.clone();
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        app.on_import_landxml_surface(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::landxml::read_landxml_surface(p) {
                        Ok((tin, extras)) => {
                            let verts: Vec<Point3> = tin
                                .vertices
                                .iter()
                                .map(|p| Point3::new(p.x, p.y, p.z))
                                .collect();
                            backend_render
                                .borrow_mut()
                                .add_surface(&verts, &tin.triangles);
                            surfaces.borrow_mut().push(tin);
                            for v in &verts {
                                point_db.borrow_mut().push(Point::new(v.x, v.y));
                                point_style_indices.borrow_mut().push(0);
                            }
                            surface_units
                                .borrow_mut()
                                .push(extras.units.unwrap_or_default());
                            surface_styles
                                .borrow_mut()
                                .push(extras.style.unwrap_or_default());
                            surface_descriptions
                                .borrow_mut()
                                .push(extras.description.unwrap_or_default());
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Imported surface"));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
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
        let backend_render = backend.clone();
        let point_db = point_db.clone();
        let point_style_indices = point_style_indices.clone();
        app.on_import_landxml_alignment(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("LandXML", &["xml"])
                .pick_file()
            {
                if let Some(p) = path.to_str() {
                    match survey_cad::io::landxml::read_landxml_alignment(p) {
                        Ok((hal, _)) => {
                            let val = survey_cad::io::landxml::read_landxml_profile(p)
                                .unwrap_or_else(|_| {
                                    VerticalAlignment::new(vec![(0.0, 0.0), (hal.length(), 0.0)])
                                });
                            for elem in &hal.elements {
                                use survey_cad::alignment::HorizontalElement::*;
                                match elem {
                                    Tangent { start, end } => {
                                        point_db.borrow_mut().push(*start);
                                        point_style_indices.borrow_mut().push(0);
                                        point_db.borrow_mut().push(*end);
                                        point_style_indices.borrow_mut().push(0);
                                    }
                                    Curve { arc } => {
                                        let s = Point::new(
                                            arc.center.x + arc.radius * arc.start_angle.cos(),
                                            arc.center.y + arc.radius * arc.start_angle.sin(),
                                        );
                                        let e = Point::new(
                                            arc.center.x + arc.radius * arc.end_angle.cos(),
                                            arc.center.y + arc.radius * arc.end_angle.sin(),
                                        );
                                        point_db.borrow_mut().push(s);
                                        point_style_indices.borrow_mut().push(0);
                                        point_db.borrow_mut().push(e);
                                        point_style_indices.borrow_mut().push(0);
                                    }
                                    Spiral { spiral } => {
                                        point_db.borrow_mut().push(spiral.start_point());
                                        point_style_indices.borrow_mut().push(0);
                                        point_db.borrow_mut().push(spiral.end_point());
                                        point_style_indices.borrow_mut().push(0);
                                    }
                                }
                            }
                            alignments.borrow_mut().push(Alignment::new(hal, val));
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from("Imported alignment"));
                                if app.get_workspace_mode() == 0 {
                                    crate::set_workspace_image_result(&app, &render_image);
                                } else {
                                    let image = backend_render.borrow_mut().render();
                                    app.set_workspace_texture(image);
                                }
                                app.window().request_redraw();
                            }
                        }
                        Err(e) => {
                            if let Some(app) = weak.upgrade() {
                                app.set_status(SharedString::from(format!(
                                    "Failed to import: {e}"
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
        let backend_render = backend.clone();
        let macro_playing = macro_playing.clone();
        let macro_recorder = macro_recorder.clone();
        let snap_target = snap_target.clone();
        app.on_workspace_clicked(move |x, y| {
            if *drawing_mode.borrow() != DrawingMode::None {
                if let Some(app) = weak.upgrade() {
                    let size = app.window().size();
                    let mut p = screen_to_workspace(
                        x,
                        y,
                        &offset_ref,
                        &zoom_ref,
                        size.width as f32,
                        size.height as f32,
                    );
                    let zoom_factor = *zoom_ref.borrow();
                    if app.get_snap_to_entities() {
                        let scene = snap::Scene {
                            points: &point_db.borrow(),
                            lines: &lines_ref.borrow(),
                            polygons: &polygons_ref.borrow(),
                            polylines: &polylines.borrow(),
                            arcs: &arcs_ref.borrow(),
                        };
                        let opts = snap::SnapOptions {
                            snap_points: app.get_snap_points(),
                            snap_endpoints: app.get_snap_endpoints(),
                            snap_midpoints: app.get_snap_midpoints(),
                            snap_intersections: app.get_snap_intersections(),
                            snap_nearest: app.get_snap_nearest(),
                            snap_surfaces: app.get_snap_surfaces(),
                            snap_solids: app.get_snap_solids(),
                        };
                        if let Some(sp) = snap::resolve_snap(
                            p,
                            &scene,
                            app.get_snap_tolerance() as f64 / (zoom_factor as f64),
                            opts,
                        ) {
                            *snap_target.borrow_mut() = Some(sp);
                            p = sp;
                        } else {
                            *snap_target.borrow_mut() = None;
                        }
                    } else {
                        *snap_target.borrow_mut() = None;
                    }
                    if app.get_snap_to_grid() {
                        p.x = p.x.round();
                        p.y = p.y.round();
                    }
                    let mut mode = drawing_mode.borrow_mut();
                    match &mut *mode {
                        DrawingMode::Line { start: Some(s) } => {
                            lines_ref.borrow_mut().push((*s, p));
                            if !macro_playing.borrow().0 {
                                let sx = s.x;
                                let sy = s.y;
                                let px = p.x;
                                let py = p.y;
                                record_macro(
                                    &mut macro_recorder.borrow_mut(),
                                    &format!("line {sx} {sy} {px} {py}"),
                                );
                            }
                            *mode = DrawingMode::None;
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
                                *mode = DrawingMode::None;
                            }
                        }
                        DrawingMode::ArcCenter {
                            center,
                            radius,
                            start_angle,
                        } => {
                            if let (Some(c), Some(r), Some(sa)) = (*center, *radius, *start_angle) {
                                let ea = (p.y - c.y).atan2(p.x - c.x);
                                let arc = Arc::new(c, r, sa, ea);
                                arcs_ref.borrow_mut().push(arc);
                                *mode = DrawingMode::None;
                            }
                        }
                        DrawingMode::ArcThreePoint { p1, p2 } => {
                            if let (Some(a), Some(b)) = (*p1, *p2) {
                                if let Some(arc) = arc_from_three_points(a, b, p) {
                                    arcs_ref.borrow_mut().push(arc);
                                }
                                *mode = DrawingMode::None;
                            }
                        }
                        DrawingMode::ArcStartEndRadius { start, end, radius } => {
                            if let (Some(s), Some(e)) = (*start, *end) {
                                let r = radius.unwrap_or_else(|| {
                                    ((p.x - s.x).powi(2) + (p.y - s.y).powi(2)).sqrt()
                                });
                                if let Some(arc) = arc_from_start_end_radius(s, e, r, p) {
                                    arcs_ref.borrow_mut().push(arc);
                                }
                                *mode = DrawingMode::None;
                            }
                        }
                        _ => {}
                    }
                    drop(mode);
                    if app.get_workspace_mode() == 0 {
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
                    }
                }
            } else if let Some(app) = weak.upgrade() {
                if app.get_workspace_click_mode() {
                    let size = app.window().size();
                    let mut p = screen_to_workspace(
                        x,
                        y,
                        &offset_ref,
                        &zoom_ref,
                        size.width as f32,
                        size.height as f32,
                    );
                    let zoom_factor = *zoom_ref.borrow();
                    if app.get_snap_to_entities() {
                        let scene = snap::Scene {
                            points: &point_db.borrow(),
                            lines: &lines.borrow(),
                            polygons: &polygons.borrow(),
                            polylines: &polylines.borrow(),
                            arcs: &arcs.borrow(),
                        };
                        let opts = snap::SnapOptions {
                            snap_points: app.get_snap_points(),
                            snap_endpoints: app.get_snap_endpoints(),
                            snap_midpoints: app.get_snap_midpoints(),
                            snap_intersections: app.get_snap_intersections(),
                            snap_nearest: app.get_snap_nearest(),
                            snap_surfaces: app.get_snap_surfaces(),
                            snap_solids: app.get_snap_solids(),
                        };
                        if let Some(sp) = snap::resolve_snap(
                            p,
                            &scene,
                            app.get_snap_tolerance() as f64 / (zoom_factor as f64),
                            opts,
                        ) {
                            *snap_target.borrow_mut() = Some(sp);
                            p = sp;
                        } else {
                            *snap_target.borrow_mut() = None;
                        }
                    } else {
                        *snap_target.borrow_mut() = None;
                    }
                    if app.get_snap_to_grid() {
                        p.x = p.x.round();
                        p.y = p.y.round();
                    }
                    point_db.borrow_mut().push(p);
                    point_style_indices.borrow_mut().push(0);
                    backend_render.borrow_mut().add_point(p.x, p.y, 0.0);
                    if !macro_playing.borrow().0 {
                        let px = p.x;
                        let py = p.y;
                        record_macro(
                            &mut macro_recorder.borrow_mut(),
                            &format!("point {px} {py}"),
                        );
                    }
                    command_stack.borrow_mut().push(Command::RemovePoint {
                        index: point_db.borrow().len() - 1,
                        point: p,
                    });
                    app.set_workspace_click_mode(false);
                    app.set_status(SharedString::from(format!(
                        "Total points: {}",
                        point_db.borrow().len()
                    )));
                    if app.get_workspace_mode() == 0 {
                        crate::set_workspace_image_result(&app, &render_image);
                        app.window().request_redraw();
                    }
                    refresh_workspace(&app, &render_image, &backend_render);
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
        let dimensions = dimensions.clone();
        let selected_dimensions = selected_dimensions.clone();
        let refresh_line_style_dialogs = refresh_line_style_dialogs.clone();
        let backend_render = backend.clone();
        app.on_clear_workspace(move || {
            point_db.borrow_mut().clear();
            lines.borrow_mut().clear();
            polygons.borrow_mut().clear();
            polylines.borrow_mut().clear();
            arcs.borrow_mut().clear();
            dimensions.borrow_mut().clear();
            point_style_indices.borrow_mut().clear();
            surfaces.borrow_mut().clear();
            surface_units.borrow_mut().clear();
            surface_styles.borrow_mut().clear();
            surface_descriptions.borrow_mut().clear();
            alignments.borrow_mut().clear();
            selected_indices.borrow_mut().clear();
            selected_lines.borrow_mut().clear();
            selected_polygons.borrow_mut().clear();
            selected_polylines.borrow_mut().clear();
            selected_arcs.borrow_mut().clear();
            selected_dimensions.borrow_mut().clear();
            backend_render.borrow_mut().clear();
            refresh_line_style_dialogs();
            if let Some(app) = weak.upgrade() {
                app.set_status(SharedString::from("Cleared workspace"));
                if app.get_workspace_mode() == 0 {
                    crate::set_workspace_image_result(&app, &render_image);
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
