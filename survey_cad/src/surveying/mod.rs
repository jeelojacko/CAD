//! Surveying specific utilities.

use crate::geometry::{self, Point};

/// Representation of a simple survey station.
#[derive(Debug)]
pub struct Station {
    pub name: String,
    pub position: Point,
}

impl Station {
    pub fn new(name: impl Into<String>, position: Point) -> Self {
        Self {
            name: name.into(),
            position,
        }
    }
}

/// Calculates the horizontal distance between two survey stations.
pub fn station_distance(a: &Station, b: &Station) -> f64 {
    geometry::distance(a.position, b.position)
}

/// Represents a closed traverse consisting of multiple survey points.
#[derive(Debug, Default)]
pub struct Traverse {
    pub points: Vec<Point>,
}

impl Traverse {
    /// Creates a new traverse from a list of points.
    pub fn new(points: Vec<Point>) -> Self {
        Self { points }
    }

    /// Calculates the area of the traverse using the polygon area algorithm.
    pub fn area(&self) -> f64 {
        geometry::polygon_area(&self.points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn station_distance_works() {
        let s1 = Station::new("A", Point::new(0.0, 0.0));
        let s2 = Station::new("B", Point::new(3.0, 4.0));
        assert_eq!(station_distance(&s1, &s2), 5.0);
    }

    #[test]
    fn traverse_area_square() {
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        let t = Traverse::new(points);
        assert!((t.area() - 1.0).abs() < 1e-6);
    }
}
