//! Basic geometry primitives for CAD operations.

pub mod line;
pub mod line3;
pub mod point;
pub mod point3;
pub mod dimension;

pub use line::{Line, LineAnnotation, LineType, LineStyle};
pub use line3::Line3;
pub use point::{NamedPoint, Point, PointSymbol};
pub use point3::Point3;
pub use dimension::{LinearDimension, LinearDimension3};

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

fn orientation(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Computes the convex hull of the provided points using the monotonic chain
/// algorithm.
pub fn convex_hull(points: &[Point]) -> Vec<Point> {
    if points.len() <= 1 {
        return points.to_vec();
    }
    let mut pts: Vec<Point> = points.to_vec();
    pts.sort_by(|a, b| {
        a.x
            .partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });
    pts.dedup();

    let mut lower = Vec::new();
    for p in &pts {
        while lower.len() >= 2
            && orientation(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0
        {
            lower.pop();
        }
        lower.push(*p);
    }

    let mut upper = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2
            && orientation(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0
        {
            upper.pop();
        }
        upper.push(*p);
    }

    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
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
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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

    /// Returns the point on the arc at the given angle.
    pub fn point_at(&self, angle: f64) -> Point {
        Point::new(
            self.center.x + self.radius * angle.cos(),
            self.center.y + self.radius * angle.sin(),
        )
    }

    /// Returns the start point of the arc.
    pub fn start_point(&self) -> Point {
        self.point_at(self.start_angle)
    }

    /// Returns the end point of the arc.
    pub fn end_point(&self) -> Point {
        self.point_at(self.end_angle)
    }

    /// Returns the midpoint of the arc.
    pub fn midpoint(&self) -> Point {
        self.point_at((self.start_angle + self.end_angle) / 2.0)
    }

    /// Returns the closest point on the arc to `p`.
    pub fn nearest_point(&self, p: Point) -> Point {
        let mut ang = (p.y - self.center.y).atan2(p.x - self.center.x);
        // Normalize angles to range 0..2PI for comparison
        let mut start = self.start_angle;
        let mut end = self.end_angle;
        while ang < 0.0 {
            ang += 2.0 * std::f64::consts::PI;
        }
        while start < 0.0 {
            start += 2.0 * std::f64::consts::PI;
        }
        while end < 0.0 {
            end += 2.0 * std::f64::consts::PI;
        }
        if start <= end {
            if ang < start {
                return self.start_point();
            }
            if ang > end {
                return self.end_point();
            }
            self.point_at(ang)
        } else {
            // Arc crosses 2PI -> 0 boundary
            if ang > end && ang < start {
                // outside sweep
                let d_start = (ang - start).abs();
                let d_end = (ang - end).abs();
                if d_start < d_end {
                    self.start_point()
                } else {
                    self.end_point()
                }
            } else {
                self.point_at(ang)
            }
        }
    }
}

/// Representation of a series of connected line segments.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

    /// Returns the position at a distance along the polyline.
    pub fn point_at(&self, dist: f64) -> Option<Point> {
        if self.vertices.len() < 2 {
            return None;
        }

        if dist <= 0.0 {
            return Some(self.vertices[0]);
        }

        let mut remaining = dist;
        for pair in self.vertices.windows(2) {
            let seg_len = distance(pair[0], pair[1]);
            if remaining <= seg_len {
                let t = if seg_len.abs() < f64::EPSILON {
                    0.0
                } else {
                    remaining / seg_len
                };
                return Some(Point::new(
                    pair[0].x + t * (pair[1].x - pair[0].x),
                    pair[0].y + t * (pair[1].y - pair[0].y),
                ));
            }
            remaining -= seg_len;
        }

        self.vertices.last().copied()
    }

    /// Returns a unit tangent direction at a distance along the polyline.
    pub fn direction_at(&self, dist: f64) -> Option<(f64, f64)> {
        if self.vertices.len() < 2 {
            return None;
        }

        if dist < 0.0 {
            return None;
        }

        let mut remaining = dist;
        for pair in self.vertices.windows(2) {
            let seg_len = distance(pair[0], pair[1]);
            if remaining <= seg_len {
                let dx = pair[1].x - pair[0].x;
                let dy = pair[1].y - pair[0].y;
                let len = (dx * dx + dy * dy).sqrt();
                if len.abs() < f64::EPSILON {
                    return Some((0.0, 0.0));
                } else {
                    return Some((dx / len, dy / len));
                }
            }
            remaining -= seg_len;
        }

        if let Some(pair) = self.vertices[self.vertices.len().saturating_sub(2)..].windows(2).next() {
            let dx = pair[1].x - pair[0].x;
            let dy = pair[1].y - pair[0].y;
            let len = (dx * dx + dy * dy).sqrt();
            if len.abs() < f64::EPSILON {
                Some((0.0, 0.0))
            } else {
                Some((dx / len, dy / len))
            }
        } else {
            None
        }
    }

    /// Returns the closest point on the polyline to `p`.
    pub fn nearest_point(&self, p: Point) -> Point {
        if self.vertices.len() == 1 {
            return self.vertices[0];
        }

        let mut nearest = self.vertices[0];
        let mut best_dist = f64::MAX;
        for pair in self.vertices.windows(2) {
            let line = Line::new(pair[0], pair[1]);
            let pt = line.nearest_point(p);
            let d = distance(p, pt);
            if d < best_dist {
                best_dist = d;
                nearest = pt;
            }
        }
        nearest
    }

    /// Returns a smoothed version of the polyline using Chaikin's algorithm.
    /// The number of `iterations` controls how many times the refinement is
    /// applied. Values less than 1 return the original polyline.
    pub fn smooth(&self, iterations: usize) -> Self {
        if iterations == 0 || self.vertices.len() < 3 {
            return self.clone();
        }

        let mut verts = self.vertices.clone();
        for _ in 0..iterations {
            let mut new_pts = Vec::with_capacity(verts.len() * 2);
            new_pts.push(verts[0]);
            for pair in verts.windows(2) {
                let p0 = pair[0];
                let p1 = pair[1];
                let q = Point::new(0.75 * p0.x + 0.25 * p1.x, 0.75 * p0.y + 0.25 * p1.y);
                let r = Point::new(0.25 * p0.x + 0.75 * p1.x, 0.25 * p0.y + 0.75 * p1.y);
                new_pts.push(q);
                new_pts.push(r);
            }
            new_pts.push(*verts.last().unwrap());
            verts = new_pts;
        }
        Self { vertices: verts }
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
    fn polyline_point_and_direction() {
        let pts = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
        let pl = Polyline::new(pts);
        let p = pl.point_at(5.0).unwrap();
        assert!((p.x - 5.0).abs() < 1e-6 && p.y.abs() < 1e-6);
        let dir = pl.direction_at(5.0).unwrap();
        assert!((dir.0 - 1.0).abs() < 1e-6 && dir.1.abs() < 1e-6);
    }

    #[test]
    fn polyline_nearest_point() {
        let pts = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
        let pl = Polyline::new(pts);
        let q = Point::new(5.0, 3.0);
        let n = pl.nearest_point(q);
        assert!((n.x - 5.0).abs() < 1e-6 && n.y.abs() < 1e-6);
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
