use super::{adjust_network, AdjustResult, Observation};
use crate::crs::Crs;
use crate::geometry::{Point, Point3, Polyline};

/// Representation of a survey point with optional number and description.
#[derive(Debug, Clone, PartialEq)]
pub struct SurveyPoint {
    pub number: Option<u32>,
    pub point: Point3,
    pub description: Option<String>,
    pub codes: Vec<String>,
}

impl SurveyPoint {
    pub fn new(
        number: Option<u32>,
        point: Point3,
        description: Option<String>,
        codes: Vec<String>,
    ) -> Self {
        Self {
            number,
            point,
            description,
            codes,
        }
    }
}

/// Simple in-memory database of survey points.
#[derive(Debug, Default)]
pub struct PointDatabase {
    pub points: Vec<SurveyPoint>,
}

impl PointDatabase {
    /// Creates a new empty database.
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    /// Adds a survey point to the database.
    pub fn add_point(&mut self, point: SurveyPoint) {
        self.points.push(point);
    }

    /// Applies a coordinate transformation to all points in the database.
    pub fn transform(&mut self, src: Crs, dst: Crs) {
        if src == dst {
            return;
        }
        for p in &mut self.points {
            if let Some((x, y)) = src.transform_point(&dst, p.point.x, p.point.y) {
                p.point.x = x;
                p.point.y = y;
            }
        }
    }

    /// Performs a least squares adjustment on the XY coordinates using the
    /// provided fixed point indices and observations.
    pub fn adjust(&mut self, fixed: &[usize], observations: &[Observation]) -> AdjustResult {
        let pts: Vec<Point> = self
            .points
            .iter()
            .map(|p| Point::new(p.point.x, p.point.y))
            .collect();
        let result = adjust_network(&pts, fixed, observations);
        for (sp, adj) in self.points.iter_mut().zip(result.points.iter()) {
            sp.point.x = adj.x;
            sp.point.y = adj.y;
        }
        result
    }

    /// Generates simple polylines from points sharing the same code. Points are
    /// ordered according to their position in the database.
    pub fn generate_linework(&self) -> Vec<Polyline> {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<String, Vec<Point>> = BTreeMap::new();
        for p in &self.points {
            for code in &p.codes {
                map.entry(code.clone())
                    .or_default()
                    .push(Point::new(p.point.x, p.point.y));
            }
        }
        map.into_iter().map(|(_, pts)| Polyline::new(pts)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surveying::Observation;

    #[test]
    fn transform_points() {
        let mut db = PointDatabase::new();
        db.add_point(SurveyPoint::new(
            None,
            Point3::new(0.0, 0.0, 0.0),
            None,
            Vec::new(),
        ));
        let wgs84 = Crs::from_epsg(4326);
        let webm = Crs::from_epsg(3857);
        db.transform(wgs84, webm);
        // 0,0 should map close to 0,0 in Web Mercator
        assert!(db.points[0].point.x.abs() < 1e-6);
        assert!(db.points[0].point.y.abs() < 1e-6);
    }

    #[test]
    fn adjust_database() {
        let mut db = PointDatabase::new();
        db.add_point(SurveyPoint::new(
            None,
            Point3::new(0.0, 0.0, 0.0),
            None,
            Vec::new(),
        ));
        db.add_point(SurveyPoint::new(
            None,
            Point3::new(100.0, 0.0, 0.0),
            None,
            Vec::new(),
        ));
        db.add_point(SurveyPoint::new(
            None,
            Point3::new(40.0, 40.0, 0.0),
            None,
            Vec::new(),
        ));
        let obs = vec![
            Observation::Distance {
                from: 0,
                to: 2,
                value: (50f64.powi(2) + 40f64.powi(2)).sqrt(),
                weight: 1.0,
            },
            Observation::Distance {
                from: 1,
                to: 2,
                value: (50f64.powi(2) + 40f64.powi(2)).sqrt(),
                weight: 1.0,
            },
        ];
        let result = db.adjust(&[0, 1], &obs);
        let p = &db.points[2].point;
        assert!((p.x - 50.0).abs() < 1e-2);
        assert!((p.y - 40.0).abs() < 1e-2);
        assert!(result.residuals.iter().all(|v| v.abs() < 1e-6));
    }

    #[test]
    fn generate_linework_groups_by_code() {
        let mut db = PointDatabase::new();
        db.add_point(SurveyPoint::new(
            Some(1),
            Point3::new(0.0, 0.0, 0.0),
            None,
            vec!["A1".into()],
        ));
        db.add_point(SurveyPoint::new(
            Some(2),
            Point3::new(1.0, 0.0, 0.0),
            None,
            vec!["A1".into()],
        ));
        db.add_point(SurveyPoint::new(
            Some(3),
            Point3::new(0.0, 1.0, 0.0),
            None,
            vec!["B1".into()],
        ));
        let lines = db.generate_linework();
        assert_eq!(lines.len(), 2);
        let mut lens: Vec<usize> = lines.iter().map(|l| l.vertices.len()).collect();
        lens.sort();
        assert_eq!(lens, vec![1, 2]);
    }
}
