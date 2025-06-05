//! Basic geometry primitives for CAD operations.

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

/// Representation of a circular arc defined by its center, radius and start/end
/// angles (in radians).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub center: Point,
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

impl Arc {
    /// Creates a new `Arc`.
    pub fn new(center: Point, radius: f64, start_angle: f64, end_angle: f64) -> Self {
        Self {
            center,
            radius,
            start_angle,
            end_angle,
        }
    }

    /// Returns the length of the arc.
    pub fn length(&self) -> f64 {
        let sweep = (self.end_angle - self.start_angle).abs();
        self.radius * sweep
    }
}

/// Representation of a series of connected line segments.
#[derive(Debug, Clone, PartialEq)]
pub struct Polyline {
    pub vertices: Vec<Point>,
}

impl Polyline {
    /// Creates a new polyline from a list of vertices.
    pub fn new(vertices: Vec<Point>) -> Self {
        Self { vertices }
    }

    /// Returns the total length of all segments in the polyline.
    pub fn length(&self) -> f64 {
        self.vertices
            .windows(2)
            .map(|pair| distance(pair[0], pair[1]))
            .sum()
    }
}

/// Representation of a planar polygonal surface.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    pub boundary: Vec<Point>,
}

impl Surface {
    /// Creates a new surface from its boundary vertices.
    pub fn new(boundary: Vec<Point>) -> Self {
        Self { boundary }
    }

    /// Calculates the area enclosed by the surface boundary.
    pub fn area(&self) -> f64 {
        polygon_area(&self.boundary)
    }
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

    #[test]
    fn arc_length_quarter_circle() {
        let arc = Arc::new(Point::new(0.0, 0.0), 1.0, 0.0, std::f64::consts::FRAC_PI_2);
        assert!((arc.length() - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn polyline_length() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(3.0, 4.0),
            Point::new(6.0, 8.0),
        ];
        let pl = Polyline::new(pts);
        assert!((pl.length() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn surface_area() {
        let boundary = vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        let s = Surface::new(boundary);
        assert!((s.area() - 4.0).abs() < 1e-6);
    }
}
