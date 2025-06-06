//! Basic 2D line types used throughout the crate.

use super::{distance, Point};

/// Available drawing styles for a line entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    /// Continuous solid line.
    Solid,
    /// Dashed line style.
    Dashed,
    /// Dotted line style.
    Dotted,
}

/// Representation of a 2D line segment between two points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

impl Line {
    /// Creates a new line segment.
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    /// Returns the length of the line segment.
    pub fn length(&self) -> f64 {
        distance(self.start, self.end)
    }

    /// Returns the midpoint of the line segment.
    pub fn midpoint(&self) -> Point {
        Point::new(
            (self.start.x + self.end.x) / 2.0,
            (self.start.y + self.end.y) / 2.0,
        )
    }

    /// Returns the azimuth from the start point to the end point in radians.
    pub fn azimuth(&self) -> f64 {
        (self.end.y - self.start.y).atan2(self.end.x - self.start.x)
    }
}

/// Annotation describing line distance and azimuth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineAnnotation {
    pub distance: f64,
    pub azimuth: f64,
}

impl LineAnnotation {
    /// Creates a new annotation using the properties of `line`.
    pub fn from_line(line: &Line) -> Self {
        Self {
            distance: line.length(),
            azimuth: line.azimuth(),
        }
    }
}

