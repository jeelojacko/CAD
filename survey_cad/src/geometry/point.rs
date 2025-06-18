//! Basic 2D point types used throughout the crate.

/// Symbol used when rendering a point entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointSymbol {
    #[default]
    Circle,
    Square,
    Cross,
}

/// Basic visual style information for a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointStyle {
    pub symbol: PointSymbol,
    pub color: [u8; 3],
    pub size: f32,
}

impl PointStyle {
    /// Creates a new point style.
    pub fn new(symbol: PointSymbol, color: [u8; 3], size: f32) -> Self {
        Self { symbol, color, size }
    }
}

/// Representation of a point with optional name and number.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NamedPoint {
    pub point: Point,
    pub name: Option<String>,
    pub number: Option<u32>,
    #[serde(skip)]
    pub symbol: PointSymbol,
}

impl NamedPoint {
    /// Creates a new named point.
    pub fn new(point: Point, name: Option<String>, number: Option<u32>) -> Self {
        Self {
            point,
            name,
            number,
            symbol: PointSymbol::Circle,
        }
    }
}

/// Representation of a 2D point.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

