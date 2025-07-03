use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use survey_cad::layers::LayerManager;
use survey_cad::geometry::line::LineStyle;
use survey_cad::geometry::point::PointStyle;
use survey_cad::styles::PolygonStyle;

#[derive(Serialize, Deserialize, Default)]
pub struct StyleSettings {
    pub point_styles: Vec<(String, PointStyle)>,
    pub line_styles: Vec<(String, LineStyle)>,
    pub polygon_styles: Vec<(String, PolygonStyle)>,
}

pub fn save_layers(path: &Path, layers: &LayerManager) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(layers)?;
    fs::write(path, json)
}

pub fn load_layers(path: &Path) -> Option<LayerManager> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_styles(path: &Path, styles: &StyleSettings) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(styles)?;
    fs::write(path, json)
}

pub fn load_styles(path: &Path) -> Option<StyleSettings> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}
