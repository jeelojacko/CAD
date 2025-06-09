use crate::dtm::Tin;
use crate::geometry::{polygon_area, Point};
use crate::surveying::{bearing, Traverse};
use std::collections::HashMap;

/// Representation of a land parcel defined by a closed boundary.
#[derive(Debug, Clone)]
pub struct Parcel {
    pub boundary: Vec<Point>,
}

/// Summary of the closure accuracy for a parcel boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosureReport {
    pub delta_x: f64,
    pub delta_y: f64,
    pub misclosure: f64,
}

impl Parcel {
    /// Creates a new parcel from its boundary polygon.
    pub fn new(boundary: Vec<Point>) -> Self {
        Self { boundary }
    }

    /// Calculates the area enclosed by the parcel boundary.
    pub fn area(&self) -> f64 {
        polygon_area(&self.boundary)
    }

    /// Builds a parcel from a survey traverse.
    pub fn from_traverse(tr: &Traverse) -> Self {
        Self::new(tr.points.clone())
    }

    /// Returns the bearing in radians of each boundary segment.
    pub fn deed_bearings(&self) -> Vec<f64> {
        if self.boundary.len() < 2 {
            return Vec::new();
        }
        let mut bearings = Vec::with_capacity(self.boundary.len());
        for win in self.boundary.windows(2) {
            bearings.push(bearing(win[0], win[1]));
        }
        let first = self.boundary.first().unwrap();
        let last = self.boundary.last().unwrap();
        bearings.push(bearing(*last, *first));
        bearings
    }

    /// Generates a closure report summarizing misclosure of the boundary.
    pub fn closure_report(&self) -> ClosureReport {
        if self.boundary.len() < 2 {
            return ClosureReport {
                delta_x: 0.0,
                delta_y: 0.0,
                misclosure: 0.0,
            };
        }
        let mut dx = 0.0;
        let mut dy = 0.0;
        for win in self.boundary.windows(2) {
            dx += win[1].x - win[0].x;
            dy += win[1].y - win[0].y;
        }
        let first = self.boundary.first().unwrap();
        let last = self.boundary.last().unwrap();
        dx += first.x - last.x;
        dy += first.y - last.y;
        ClosureReport {
            delta_x: dx,
            delta_y: dy,
            misclosure: (dx * dx + dy * dy).sqrt(),
        }
    }

    /// Builds a parcel from the outer boundary edges of a TIN surface.
    pub fn from_tin_boundary(tin: &Tin) -> Self {
        let mut edge_count: HashMap<(usize, usize), usize> = HashMap::new();
        for tri in &tin.triangles {
            for &(a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])].iter() {
                let e = if a < b { (a, b) } else { (b, a) };
                *edge_count.entry(e).or_insert(0) += 1;
            }
        }
        let mut edges: Vec<(usize, usize)> = edge_count
            .into_iter()
            .filter_map(|(e, c)| if c == 1 { Some(e) } else { None })
            .collect();
        if edges.is_empty() {
            return Self::new(Vec::new());
        }
        let (a, mut b) = edges.pop().unwrap();
        let mut order = vec![a, b];
        while !edges.is_empty() {
            if let Some(pos) = edges.iter().position(|&(x, _)| x == b) {
                let (_, nb) = edges.remove(pos);
                order.push(nb);
                b = nb;
            } else if let Some(pos) = edges.iter().position(|&(_, y)| y == b) {
                let (na, _) = edges.remove(pos);
                order.push(na);
                b = na;
            } else {
                break;
            }
        }
        let pts = order
            .into_iter()
            .map(|idx| {
                let v = tin.vertices[idx];
                Point::new(v.x, v.y)
            })
            .collect();
        Self::new(pts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point3;

    #[test]
    fn parcel_area_square() {
        let boundary = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        let p = Parcel::new(boundary);
        assert!((p.area() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parcel_from_tin_boundary() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let tin = Tin::from_points(pts);
        let p = Parcel::from_tin_boundary(&tin);
        assert!((p.area() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parcel_bearings_and_closure() {
        let p = Parcel::new(vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ]);
        let b = p.deed_bearings();
        assert_eq!(b.len(), 4);
        assert!((b[0] - 0.0).abs() < 1e-6);
        assert!((b[1] - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((b[2] - std::f64::consts::PI).abs() < 1e-6);
        assert!((b[3] + std::f64::consts::FRAC_PI_2).abs() < 1e-6);
        let rep = p.closure_report();
        assert!(rep.misclosure.abs() < 1e-6);
    }
}
