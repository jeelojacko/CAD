use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use survey_cad::geometry::Point;

#[derive(Default, Clone)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Default, Clone)]
pub struct DragSelect {
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub active: bool,
}

#[derive(Default, Clone)]
pub struct CursorFeedback {
    pub pos: (f32, f32),
    pub frame: u32,
}

#[derive(Default, Clone, PartialEq)]
pub enum DrawingMode {
    #[default]
    None,
    Line { start: Option<Point> },
    Polygon { vertices: Vec<Point> },
    /// Center, start and end order
    ArcCenter { center: Option<Point>, radius: Option<f64>, start_angle: Option<f64> },
    /// Three point arc
    ArcThreePoint { p1: Option<Point>, p2: Option<Point> },
    /// Start, end, then radius via third click
    ArcStartEndRadius { start: Option<Point>, end: Option<Point>, radius: Option<f64> },
    Dimension { start: Option<Point> },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct SnapPrefs {
    #[serde(default)]
    pub snap_to_grid: bool,
    #[serde(default)]
    pub snap_to_entities: bool,
    #[serde(default)]
    pub snap_points: bool,
    #[serde(default)]
    pub snap_endpoints: bool,
    #[serde(default)]
    pub snap_midpoints: bool,
    #[serde(default)]
    pub snap_intersections: bool,
    #[serde(default)]
    pub snap_nearest: bool,
    #[serde(default)]
    pub snap_surfaces: bool,
    #[serde(default)]
    pub snap_solids: bool,
    #[serde(default)]
    pub snap_tolerance: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Debug)]
pub enum WorkspaceProfile {
    #[default]
    Surveyor,
    Engineer,
    Gis,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Debug)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Config {
    pub window_width: u32,
    pub window_height: u32,
    pub last_open_dir: Option<String>,
    pub snap: SnapPrefs,
    pub auto_tin: bool,
    #[serde(default)]
    pub quick_scripts: Vec<String>,
    #[serde(default)]
    pub profile: WorkspaceProfile,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub font_path: Option<String>,
    #[serde(default)]
    pub macro_dir: Option<String>,
    #[serde(default)]
    pub crs_epsg: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_width: 800,
            window_height: 600,
            last_open_dir: None,
            snap: SnapPrefs::default(),
            auto_tin: false,
            quick_scripts: Vec::new(),
            profile: WorkspaceProfile::default(),
            theme: Theme::default(),
            font_path: None,
            macro_dir: None,
            crs_epsg: 4326,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("survey_cad_truck_gui").join("config.json"))
}

pub fn load_config() -> Config {
    if let Some(path) = config_path() {
        if let Ok(data) = fs::read_to_string(path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Config::default()
        }
    } else {
        Config::default()
    }
}

pub fn save_config(cfg: &Config) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(cfg) {
            let _ = fs::write(path, json);
        }
    }
}
