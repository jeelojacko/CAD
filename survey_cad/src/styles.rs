use crate::geometry::point::PointStyle;

/// Basic styling structures for drawing entities.
/// Represents the weight of a line in millimeters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineWeight(pub f32);

impl Default for LineWeight {
    fn default() -> Self {
        Self(0.25)
    }
}

/// Text style definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub name: String,
    pub font: String,
    pub height: f64,
}

impl TextStyle {
    /// Creates a new text style.
    pub fn new(name: &str, font: &str, height: f64) -> Self {
        Self {
            name: name.to_string(),
            font: font.to_string(),
            height,
        }
    }
}

/// Dimension style definition with optional overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionStyle {
    pub name: String,
    pub text_style: TextStyle,
    pub scale: f64,
}

impl DimensionStyle {
    pub fn new(name: &str, text_style: TextStyle, scale: f64) -> Self {
        Self {
            name: name.to_string(),
            text_style,
            scale,
        }
    }

    /// Returns a new style with the provided overrides applied.
    pub fn overridden(&self, ov: &DimensionStyleOverride) -> Self {
        Self {
            name: self.name.clone(),
            text_style: ov
                .text_style
                .clone()
                .unwrap_or_else(|| self.text_style.clone()),
            scale: ov.scale.unwrap_or(self.scale),
        }
    }
}

/// Overrides for a dimension style.
#[derive(Debug, Clone, Default)]
pub struct DimensionStyleOverride {
    pub text_style: Option<TextStyle>,
    pub scale: Option<f64>,
}

/// Style definition for point labels.
#[derive(Debug, Clone, PartialEq)]
pub struct PointLabelStyle {
    pub text_style: TextStyle,
    pub color: [u8; 3],
    pub offset: [f32; 2],
}

impl PointLabelStyle {
    pub fn new(text_style: TextStyle, color: [u8; 3], offset: [f32; 2]) -> Self {
        Self {
            text_style,
            color,
            offset,
        }
    }
}

/// Position of a line label relative to the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineLabelPosition {
    Above,
    Below,
    Center,
}

/// Style definition for line labels.
#[derive(Debug, Clone, PartialEq)]
pub struct LineLabelStyle {
    pub text_style: TextStyle,
    pub color: [u8; 3],
    pub position: LineLabelPosition,
}

impl LineLabelStyle {
    pub fn new(text_style: TextStyle, color: [u8; 3], position: LineLabelPosition) -> Self {
        Self {
            text_style,
            color,
            position,
        }
    }
}

/// Returns a basic set of default point styles.
pub fn default_point_styles() -> Vec<(String, PointStyle)> {
    vec![
        (
            "Green Circle".to_string(),
            PointStyle::new(crate::geometry::PointSymbol::Circle, [0, 255, 0], 3.0),
        ),
        (
            "Red Square".to_string(),
            PointStyle::new(crate::geometry::PointSymbol::Square, [255, 0, 0], 3.0),
        ),
        (
            "Blue Cross".to_string(),
            PointStyle::new(crate::geometry::PointSymbol::Cross, [0, 0, 255], 3.0),
        ),
    ]
}

/// Returns a basic set of default point label styles.
pub fn default_point_label_styles() -> Vec<(String, PointLabelStyle)> {
    vec![
        (
            "Small White".to_string(),
            PointLabelStyle::new(TextStyle::new("small", "Arial", 2.5), [255, 255, 255], [5.0, 5.0]),
        ),
        (
            "Large Yellow".to_string(),
            PointLabelStyle::new(TextStyle::new("large", "Arial", 5.0), [255, 255, 0], [5.0, 5.0]),
        ),
    ]
}

/// Returns a basic set of default line styles.
pub fn default_line_styles() -> Vec<(String, crate::geometry::line::LineStyle)> {
    use crate::geometry::line::{LineStyle, LineType};
    vec![
        (
            "White Solid".to_string(),
            LineStyle::new(LineType::Solid, [255, 255, 255], LineWeight(1.0)),
        ),
        (
            "Red Dashed".to_string(),
            LineStyle::new(LineType::Dashed, [255, 0, 0], LineWeight(1.0)),
        ),
        (
            "Blue Dotted".to_string(),
            LineStyle::new(LineType::Dotted, [0, 0, 255], LineWeight(1.0)),
        ),
    ]
}

/// Returns a basic set of default line label styles.
pub fn default_line_label_styles() -> Vec<(String, LineLabelStyle)> {
    vec![
        (
            "Above Small".to_string(),
            LineLabelStyle::new(
                TextStyle::new("small", "Arial", 8.0),
                [255, 255, 255],
                LineLabelPosition::Above,
            ),
        ),
        (
            "Below Small".to_string(),
            LineLabelStyle::new(
                TextStyle::new("small", "Arial", 8.0),
                [255, 255, 0],
                LineLabelPosition::Below,
            ),
        ),
    ]
}
