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
}
