//! Basic 3D line types used throughout the crate.

use super::{distance3, Point3};

/// Representation of a 3D line segment between two points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line3 {
    pub start: Point3,
    pub end: Point3,
}

impl Line3 {
    /// Creates a new line segment.
    pub fn new(start: Point3, end: Point3) -> Self {
        Self { start, end }
    }

    /// Returns the length of the line segment.
    pub fn length(&self) -> f64 {
        distance3(self.start, self.end)
    }

    /// Returns the midpoint of the line segment.
    pub fn midpoint(&self) -> Point3 {
        Point3::new(
            (self.start.x + self.end.x) / 2.0,
            (self.start.y + self.end.y) / 2.0,
            (self.start.z + self.end.z) / 2.0,
        )
    }
}

