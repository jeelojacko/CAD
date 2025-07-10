use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use survey_cad::layers::LayerManager;
use survey_cad::geometry::line::LineStyle;
use survey_cad::geometry::point::PointStyle;
use survey_cad::styles::{LineLabelStyle, PointLabelStyle, PolygonStyle};

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct StyleSettings {
    pub point_styles: Vec<(String, PointStyle)>,
    pub line_styles: Vec<(String, LineStyle)>,
    pub polygon_styles: Vec<(String, PolygonStyle)>,
    pub alignment_styles: Vec<(String, LineStyle)>,
    pub line_label_styles: Vec<(String, LineLabelStyle)>,
    pub point_label_styles: Vec<(String, PointLabelStyle)>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use survey_cad::layers::{Layer, LayerManager};
    use survey_cad::geometry::{line::LineType, point::PointSymbol};
    use survey_cad::styles::{LineLabelPosition, LineWeight, TextStyle};

    #[test]
    fn layer_manager_round_trip() {
        let mut mgr = LayerManager::new();
        mgr.add_layer(Layer::new("layer1"));
        mgr.add_layer(Layer::new("layer2"));

        let file = NamedTempFile::new().unwrap();
        save_layers(file.path(), &mgr).unwrap();
        let loaded = load_layers(file.path()).unwrap();
        assert_eq!(mgr, loaded);
    }

    #[test]
    fn style_settings_round_trip() {
        let styles = StyleSettings {
            point_styles: vec![(
                "p".into(),
                PointStyle::new(PointSymbol::Circle, [1, 2, 3], 1.0),
            )],
            line_styles: vec![(
                "l".into(),
                LineStyle::new(LineType::Solid, [4, 5, 6], LineWeight(0.5)),
            )],
            polygon_styles: vec![("poly".into(), PolygonStyle::default())],
            alignment_styles: vec![(
                "align".into(),
                LineStyle::new(LineType::Dashed, [7, 8, 9], LineWeight(0.7)),
            )],
            line_label_styles: vec![(
                "ll".into(),
                LineLabelStyle::new(
                    TextStyle::new("txt", "Arial", 10.0),
                    [10, 11, 12],
                    LineLabelPosition::Above,
                ),
            )],
            point_label_styles: vec![(
                "pl".into(),
                PointLabelStyle::new(
                    TextStyle::new("txt", "Arial", 10.0),
                    [13, 14, 15],
                    [1.0, 2.0],
                ),
            )],
        };

        let file = NamedTempFile::new().unwrap();
        save_styles(file.path(), &styles).unwrap();
        let loaded = load_styles(file.path()).unwrap();
        assert_eq!(styles, loaded);
    }
}
