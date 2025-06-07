//! Surveying specific utilities.

use crate::geometry::{self, Point};

pub mod cogo;
pub use cogo::{bearing, forward, line_intersection};

pub mod adjustment;
pub use adjustment::{adjust_network, AdjustResult, Observation};

pub mod field_code;
pub use field_code::{CodeAction, FieldCode};

pub mod point_db;
pub use point_db::{PointDatabase, SurveyPoint};

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

/// Calculates the vertical angle between two stations given their elevations.
/// The result is returned in radians.
pub fn vertical_angle(a: &Station, elev_a: f64, b: &Station, elev_b: f64) -> f64 {
    let horiz = geometry::distance(a.position, b.position);
    let delta_elev = elev_b - elev_a;
    delta_elev.atan2(horiz)
}

/// Performs a simple differential leveling computation returning the new
/// elevation given a starting elevation, a backsight reading and a foresight
/// reading.
pub fn level_elevation(start_elev: f64, backsight: f64, foresight: f64) -> f64 {
    start_elev + backsight - foresight
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

    #[test]
    fn vertical_angle_works() {
        let a = Station::new("A", Point::new(0.0, 0.0));
        let b = Station::new("B", Point::new(3.0, 4.0));
        let ang = vertical_angle(&a, 10.0, &b, 14.0);
        let expected = (4.0f64).atan2(5.0);
        assert!((ang - expected).abs() < 1e-6);
    }

    #[test]
    fn level_elevation_works() {
        let new_elev = level_elevation(100.0, 1.2, 0.8);
        assert!((new_elev - 100.4).abs() < 1e-6);
    }
}
