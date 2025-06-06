//! Basic geometry primitives for CAD operations.

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

/// Symbol used when rendering a point entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointSymbol {
    #[default]
    Circle,
    Square,
    Cross,
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

/// Representation of a 3D point.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

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

/// Calculates the Euclidean distance between two 3D points.
pub fn distance3(a: Point3, b: Point3) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
}

fn cross(a: Point3, b: Point3) -> Point3 {
    Point3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}

fn subtract(a: Point3, b: Point3) -> Point3 {
    Point3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

/// Calculates the area of a planar polygon in 3D space.
pub fn polygon_area3(vertices: &[Point3]) -> f64 {
    if vertices.len() < 3 {
        return 0.0;
    }
    let mut sum = Point3::new(0.0, 0.0, 0.0);
    for i in 1..(vertices.len() - 1) {
        let v0 = subtract(vertices[i], vertices[0]);
        let v1 = subtract(vertices[i + 1], vertices[0]);
        let c = cross(v0, v1);
        sum.x += c.x;
        sum.y += c.y;
        sum.z += c.z;
    }
    0.5 * (sum.x.powi(2) + sum.y.powi(2) + sum.z.powi(2)).sqrt()
}

/// Representation of a planar polygonal surface in 3D.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface3 {
    pub boundary: Vec<Point3>,
}

impl Surface3 {
    /// Creates a new surface from its boundary vertices.
    pub fn new(boundary: Vec<Point3>) -> Self {
        Self { boundary }
    }

    /// Calculates the area enclosed by the surface boundary.
    pub fn area(&self) -> f64 {
        polygon_area3(&self.boundary)
    }
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

    #[test]
    fn line3_length_midpoint() {
        let a = Point3::new(0.0, 0.0, 0.0);
        let b = Point3::new(1.0, 2.0, 2.0);
        let line = Line3::new(a, b);
        assert!((line.length() - 3.0).abs() < 1e-6);
        let mid = line.midpoint();
        assert_eq!(mid, Point3::new(0.5, 1.0, 1.0));
    }

    #[test]
    fn surface3_area_triangle() {
        let boundary = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let s = Surface3::new(boundary);
        assert!((s.area() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn line_azimuth() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(1.0, 1.0);
        let line = Line::new(a, b);
        assert!((line.azimuth() - std::f64::consts::FRAC_PI_4).abs() < 1e-6);
    }

    #[test]
    fn line_annotation_from_line() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(3.0, 4.0);
        let line = Line::new(a, b);
        let ann = LineAnnotation::from_line(&line);
        assert!((ann.distance - 5.0).abs() < 1e-6);
        assert!((ann.azimuth - (4.0f64).atan2(3.0)).abs() < 1e-6);
    }

    #[test]
    fn named_point_creation() {
        let p = Point::new(1.0, 2.0);
        let np = NamedPoint::new(p, Some("A".into()), Some(1));
        assert_eq!(np.point, p);
        assert_eq!(np.name.as_deref(), Some("A"));
        assert_eq!(np.number, Some(1));
    }
}
