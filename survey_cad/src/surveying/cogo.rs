//! Basic coordinate geometry (COGO) utilities used in surveying operations.

use crate::geometry::Point;

/// Computes the bearing in radians from point `a` to point `b` measured from the
/// positive X axis.
pub fn bearing(a: Point, b: Point) -> f64 {
    (b.y - a.y).atan2(b.x - a.x)
}

/// Computes a new point from a starting point, a bearing (radians from the
/// positive X axis) and a distance.
pub fn forward(start: Point, bearing: f64, distance: f64) -> Point {
    Point::new(
        start.x + distance * bearing.cos(),
        start.y + distance * bearing.sin(),
    )
}

/// Determines the intersection of two infinite lines defined by points
/// `(p1, p2)` and `(p3, p4)`. Returns `None` if the lines are parallel.
pub fn line_intersection(p1: Point, p2: Point, p3: Point, p4: Point) -> Option<Point> {
    let denom = (p1.x - p2.x) * (p3.y - p4.y) - (p1.y - p2.y) * (p3.x - p4.x);
    if denom.abs() < f64::EPSILON {
        return None;
    }
    let x_num =
        (p1.x * p2.y - p1.y * p2.x) * (p3.x - p4.x) - (p1.x - p2.x) * (p3.x * p4.y - p3.y * p4.x);
    let y_num =
        (p1.x * p2.y - p1.y * p2.x) * (p3.y - p4.y) - (p1.y - p2.y) * (p3.x * p4.y - p3.y * p4.x);
    Some(Point::new(x_num / denom, y_num / denom))
}

/// Determines the intersection of a line defined by `(p1, p2)` and a line
/// starting at `start` with a specified `bearing`. Returns `None` if the lines
/// are parallel.
pub fn line_bearing_intersection(
    p1: Point,
    p2: Point,
    start: Point,
    bearing: f64,
) -> Option<Point> {
    let dir_end = Point::new(start.x + bearing.cos(), start.y + bearing.sin());
    line_intersection(p1, p2, start, dir_end)
}

/// Determines the intersection of two lines each defined by a starting point
/// and a bearing. Returns `None` if the lines are parallel.
pub fn bearing_bearing_intersection(p1: Point, b1: f64, p2: Point, b2: f64) -> Option<Point> {
    let p1_end = Point::new(p1.x + b1.cos(), p1.y + b1.sin());
    let p2_end = Point::new(p2.x + b2.cos(), p2.y + b2.sin());
    line_intersection(p1, p1_end, p2, p2_end)
}

/// Calculates the intersection points of two circles. Each circle is defined by
/// its center and radius. Returns `None` if the circles do not intersect or are
/// coincident.
pub fn circle_circle_intersection(c0: Point, r0: f64, c1: Point, r1: f64) -> Option<Vec<Point>> {
    let d = crate::geometry::distance(c0, c1);
    if d.abs() < f64::EPSILON && (r0 - r1).abs() < f64::EPSILON {
        return None;
    }
    if d > r0 + r1 || d < (r0 - r1).abs() {
        return None;
    }
    let a = (r0 * r0 - r1 * r1 + d * d) / (2.0 * d);
    let h_sq = r0 * r0 - a * a;
    if h_sq < 0.0 {
        return None;
    }
    let h = h_sq.sqrt();
    let x2 = c0.x + a * (c1.x - c0.x) / d;
    let y2 = c0.y + a * (c1.y - c0.y) / d;
    if h.abs() < f64::EPSILON {
        return Some(vec![Point::new(x2, y2)]);
    }
    let rx = -(c1.y - c0.y) * (h / d);
    let ry = (c1.x - c0.x) * (h / d);
    let p1_int = Point::new(x2 + rx, y2 + ry);
    let p2_int = Point::new(x2 - rx, y2 - ry);
    Some(vec![p1_int, p2_int])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearing_works() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(1.0, 1.0);
        let bng = bearing(a, b);
        assert!((bng - std::f64::consts::FRAC_PI_4).abs() < 1e-6);
    }

    #[test]
    fn forward_works() {
        let start = Point::new(0.0, 0.0);
        let p = forward(start, std::f64::consts::FRAC_PI_2, 2.0);
        assert!((p.x - 0.0).abs() < 1e-6);
        assert!((p.y - 2.0).abs() < 1e-6);
    }

    #[test]
    fn line_intersection_works() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(1.0, 1.0);
        let p3 = Point::new(0.0, 1.0);
        let p4 = Point::new(1.0, 0.0);
        let int = line_intersection(p1, p2, p3, p4).unwrap();
        assert!((int.x - 0.5).abs() < 1e-6);
        assert!((int.y - 0.5).abs() < 1e-6);
    }

    #[test]
    fn line_bearing_intersection_works() {
        let p1 = Point::new(-1.0, 0.0);
        let p2 = Point::new(1.0, 0.0);
        let start = Point::new(0.0, -1.0);
        let int = line_bearing_intersection(p1, p2, start, std::f64::consts::FRAC_PI_2).unwrap();
        assert!(int.x.abs() < 1e-6 && int.y.abs() < 1e-6);
    }

    #[test]
    fn bearing_bearing_intersection_works() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(1.0, 1.0);
        let int = bearing_bearing_intersection(a, 0.0, b, -std::f64::consts::FRAC_PI_2).unwrap();
        assert!((int.x - 1.0).abs() < 1e-6 && int.y.abs() < 1e-6);
    }

    #[test]
    fn circle_circle_intersection_works() {
        let c0 = Point::new(0.0, 0.0);
        let c1 = Point::new(1.0, 0.0);
        let pts = circle_circle_intersection(c0, 1.0, c1, 1.0).unwrap();
        assert_eq!(pts.len(), 2);
        let y = (3.0_f64).sqrt() / 2.0;
        assert!(pts
            .iter()
            .any(|p| (p.x - 0.5).abs() < 1e-6 && (p.y - y).abs() < 1e-6));
        assert!(pts
            .iter()
            .any(|p| (p.x - 0.5).abs() < 1e-6 && (p.y + y).abs() < 1e-6));
    }
}
