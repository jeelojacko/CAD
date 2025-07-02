use super::{Point, Point3};

/// Linear dimension annotation between two 2D points.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LinearDimension {
    pub start: Point,
    pub end: Point,
    /// Optional user supplied text overriding the measured distance
    pub text: Option<String>,
    /// Offset distance from the measured line
    pub offset: f64,
}

impl LinearDimension {
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end, text: None, offset: 0.0 }
    }

    /// Returns the measured length of the dimension
    pub fn length(&self) -> f64 {
        super::distance(self.start, self.end)
    }
}

/// Linear dimension annotation between two 3D points.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LinearDimension3 {
    pub start: Point3,
    pub end: Point3,
    pub text: Option<String>,
    pub offset: f64,
}

impl LinearDimension3 {
    pub fn new(start: Point3, end: Point3) -> Self {
        Self { start, end, text: None, offset: 0.0 }
    }

    pub fn length(&self) -> f64 {
        super::distance3(self.start, self.end)
    }
}
