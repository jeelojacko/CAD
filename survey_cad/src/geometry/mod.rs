//! Basic geometry primitives for CAD operations.

/// Representation of a 2D point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
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
}

/// Calculates the Euclidean distance between two points.
pub fn distance(a: Point, b: Point) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
}

/// Calculates the area of a simple polygon using the shoelace formula.
pub fn polygon_area(vertices: &[Point]) -> f64 {
    if vertices.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..vertices.len() {
        let j = (i + 1) % vertices.len();
        sum += vertices[i].x * vertices[j].y - vertices[j].x * vertices[i].y;
    }
    sum.abs() * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_length_midpoint() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(3.0, 4.0);
        let line = Line::new(a, b);
        assert_eq!(line.length(), 5.0);
        let mid = line.midpoint();
        assert_eq!(mid, Point::new(1.5, 2.0));
    }

    #[test]
    fn polygon_area_square() {
        let square = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        assert!((polygon_area(&square) - 1.0).abs() < 1e-6);
    }
}
