//! Surveying specific utilities.

use crate::geometry::{self, Point};

pub mod cogo;
pub use cogo::{bearing, forward, line_intersection};

pub mod adjustment;
pub use adjustment::{
    adjust_network, adjust_network_report, AdjustReport, AdjustResult, IterationRecord, Observation,
};

pub mod least_squares;
pub use least_squares::{
    conditional_ls, free_network_ls, parametric_ls, redundancy_analysis, LSAnalysis, LSResult,
};

pub mod field_code;
pub use field_code::{CodeAction, FieldCode};

pub mod code_library;
pub use code_library::{BlockRef, CodeEntry, CodeLibrary};

pub mod point_db;
pub use point_db::{PointDatabase, SurveyPoint};

pub mod observation_db;
pub use observation_db::{
    ObsType, ObservationDB, ObservationData, ObservationRecord, QueryFilter, TraverseLeg,
};

pub mod stakeout;
pub use stakeout::{grid_stakeout_points, optimal_stationing, stakeout_position};

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

    /// Returns the latitude (northing change) and departure (easting change)
    /// for each leg of the traverse. The traverse is not automatically closed,
    /// so the number of legs returned is `points.len() - 1`.
    pub fn lat_departures(&self) -> Vec<(f64, f64)> {
        if self.points.len() < 2 {
            return Vec::new();
        }
        self.points
            .windows(2)
            .map(|pair| {
                let lat = pair[1].y - pair[0].y;
                let dep = pair[1].x - pair[0].x;
                (lat, dep)
            })
            .collect()
    }

    /// Total length of the traverse computed as the sum of the leg lengths
    /// between consecutive points.
    pub fn length(&self) -> f64 {
        if self.points.len() < 2 {
            return 0.0;
        }
        self.points
            .windows(2)
            .map(|pair| geometry::distance(pair[0], pair[1]))
            .sum()
    }

    /// Computes the misclosure vector `(delta_x, delta_y)` and the misclosure
    /// distance. The misclosure is the difference between the starting point and
    /// the final point of the traverse.
    pub fn misclosure(&self) -> (f64, f64, f64) {
        if self.points.len() < 2 {
            return (0.0, 0.0, 0.0);
        }
        let first = self.points.first().unwrap();
        let last = self.points.last().unwrap();
        let dx = first.x - last.x;
        let dy = first.y - last.y;
        let mis = (dx * dx + dy * dy).sqrt();
        (dx, dy, mis)
    }

    /// Computes the closure precision of the traverse expressed as the ratio of
    /// total traverse length to the misclosure distance. A perfectly closed
    /// traverse will return `f64::INFINITY`.
    pub fn closure_precision(&self) -> f64 {
        let (_, _, mis) = self.misclosure();
        if mis.abs() < f64::EPSILON {
            f64::INFINITY
        } else {
            self.length() / mis
        }
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

    #[test]
    fn lat_departures_and_misclosure() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
            Point::new(0.0, 0.0),
        ];
        let t = Traverse::new(pts);
        let ld = t.lat_departures();
        assert_eq!(ld.len(), 4);
        assert!((ld[0].0 - 0.0).abs() < 1e-6 && (ld[0].1 - 1.0).abs() < 1e-6);
        assert!((ld[1].0 - 1.0).abs() < 1e-6 && ld[1].1.abs() < 1e-6);
        assert!((ld[2].0 - 0.0).abs() < 1e-6 && (ld[2].1 + 1.0).abs() < 1e-6);
        assert!((ld[3].0 + 1.0).abs() < 1e-6 && ld[3].1.abs() < 1e-6);
        let (_, _, mis) = t.misclosure();
        assert!(mis.abs() < 1e-6);
    }

    #[test]
    fn closure_precision_works() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 0.9),
        ];
        let t = Traverse::new(pts);
        let (_, _, mis) = t.misclosure();
        assert!((mis - 0.9).abs() < 1e-6);
        let prec = t.closure_precision();
        let expected = t.length() / mis;
        assert!((prec - expected).abs() < 1e-6);
    }
}
