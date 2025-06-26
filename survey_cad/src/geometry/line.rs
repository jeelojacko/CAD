//! Basic 2D line types used throughout the crate.

use super::{distance, Point};
use crate::styles::LineWeight;

/// Available drawing styles for a line entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LineType {
    /// Continuous solid line.
    Solid,
    /// Dashed line style.
    Dashed,
    /// Dotted line style.
    Dotted,
}

/// Style information for rendering a line.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LineStyle {
    pub line_type: LineType,
    pub color: [u8; 3],
    pub weight: LineWeight,
}

impl LineStyle {
    pub fn new(line_type: LineType, color: [u8; 3], weight: LineWeight) -> Self {
        Self {
            line_type,
            color,
            weight,
        }
    }
}

impl Default for LineStyle {
    fn default() -> Self {
        Self {
            line_type: LineType::Solid,
            color: [255, 255, 255],
            weight: LineWeight::default(),
        }
    }
}

/// Representation of a 2D line segment between two points.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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

    /// Returns the closest point on the line segment to `p`.
    pub fn nearest_point(&self, p: Point) -> Point {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let len_sq = dx * dx + dy * dy;
        if len_sq.abs() < f64::EPSILON {
            return self.start;
        }
        let t = ((p.x - self.start.x) * dx + (p.y - self.start.y) * dy) / len_sq;
        if t <= 0.0 {
            self.start
        } else if t >= 1.0 {
            self.end
        } else {
            Point::new(self.start.x + t * dx, self.start.y + t * dy)
        }
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
