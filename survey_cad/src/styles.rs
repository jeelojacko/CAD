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
