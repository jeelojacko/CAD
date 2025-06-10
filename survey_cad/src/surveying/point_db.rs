use super::field_code::FieldCode;
use super::Traverse;
use super::{adjust_network, AdjustResult, Observation};
use crate::crs::{Crs, CrsTransformer};
use crate::geometry::{Point, Point3, Polyline};
use crate::parcel::Parcel;
use chrono::{DateTime, Utc};

/// Representation of a survey point with optional number and description.
#[derive(Debug, Clone, PartialEq)]
pub struct SurveyPoint {
    pub number: Option<u32>,
    pub point: Point3,
    pub description: Option<String>,
    pub codes: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum PointChange {
    Added(SurveyPoint),
    Modified { old: SurveyPoint, new: SurveyPoint },
}

#[derive(Debug, Clone)]
pub struct PointAuditEntry {
    pub index: usize,
    pub user: String,
    pub timestamp: DateTime<Utc>,
    pub comment: Option<String>,
    pub change: PointChange,
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

    /// Returns parsed field codes for this point.
    pub fn field_codes(&self) -> Vec<FieldCode> {
        self.codes.iter().map(|c| FieldCode::parse(c)).collect()
    }
}

/// Simple in-memory database of survey points.
#[derive(Debug, Default)]
pub struct PointDatabase {
    pub points: Vec<SurveyPoint>,
    pub history: Vec<PointAuditEntry>,
}

impl PointDatabase {
    /// Creates a new empty database.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            history: Vec::new(),
        }
    }

    /// Adds a survey point to the database.
    pub fn add_point(&mut self, point: SurveyPoint) {
        self.add_point_with_audit(point, "system", None);
    }

    pub fn add_point_with_audit(&mut self, point: SurveyPoint, user: &str, comment: Option<&str>) {
        let idx = self.points.len();
        self.history.push(PointAuditEntry {
            index: idx,
            user: user.to_string(),
            timestamp: Utc::now(),
            comment: comment.map(|s| s.to_string()),
            change: PointChange::Added(point.clone()),
        });
        self.points.push(point);
    }

    pub fn update_point(
        &mut self,
        index: usize,
        point: SurveyPoint,
        user: &str,
        comment: Option<&str>,
    ) {
        if let Some(old) = self.points.get_mut(index) {
            let old_clone = old.clone();
            *old = point.clone();
            self.history.push(PointAuditEntry {
                index,
                user: user.to_string(),
                timestamp: Utc::now(),
                comment: comment.map(|s| s.to_string()),
                change: PointChange::Modified {
                    old: old_clone,
                    new: point,
                },
            });
        }
    }

    pub fn history(&self) -> &[PointAuditEntry] {
        &self.history
    }

    /// Applies a coordinate transformation to all points in the database.
    pub fn transform(&mut self, src: Crs, dst: Crs) {
        if src == dst {
            return;
        }
        if let Some(trans) = crate::crs::CrsTransformer::new(&src, &dst) {
            for p in &mut self.points {
                if let Some((x, y, z)) = trans.transform(p.point.x, p.point.y, p.point.z) {
                    p.point.x = x;
                    p.point.y = y;
                    p.point.z = z;
                } else if let Some((x, y)) = src.transform_point(&dst, p.point.x, p.point.y) {
                    // Fallback to 2D if 3D transform fails
                    p.point.x = x;
                    p.point.y = y;
                }
            }
        } else {
            for p in &mut self.points {
                if let Some((x, y, z)) =
                    src.transform_point3d(&dst, p.point.x, p.point.y, p.point.z)
                {
                    p.point.x = x;
                    p.point.y = y;
                    p.point.z = z;
                } else if let Some((x, y)) = src.transform_point(&dst, p.point.x, p.point.y) {
                    // Fallback to 2D if 3D transform fails
                    p.point.x = x;
                    p.point.y = y;
                }
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

    /// Adjusts the network and returns a traverse constructed from the adjusted points.
    pub fn adjust_to_traverse(
        &mut self,
        fixed: &[usize],
        observations: &[Observation],
    ) -> (AdjustResult, Traverse) {
        let result = self.adjust(fixed, observations);
        let pts = self
            .points
            .iter()
            .map(|p| Point::new(p.point.x, p.point.y))
            .collect();
        (result, Traverse::new(pts))
    }

    /// Adjusts the network and builds a parcel from the resulting traverse.
    pub fn adjust_to_parcel(
        &mut self,
        fixed: &[usize],
        observations: &[Observation],
    ) -> (AdjustResult, Parcel) {
        let (res, trav) = self.adjust_to_traverse(fixed, observations);
        let parcel = Parcel::from_traverse(&trav);
        (res, parcel)
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
        map.into_values().map(Polyline::new).collect()
    }

    /// Generates linework by interpreting field codes using begin/continue/end
    /// semantics. Each figure is returned as a polyline in the order it was
    /// completed.
    pub fn generate_figures(&self) -> Vec<Polyline> {
        use super::field_code::CodeAction;
        use std::collections::BTreeMap;
        let mut active: BTreeMap<String, Vec<Point>> = BTreeMap::new();
        let mut result = Vec::new();
        for p in &self.points {
            let pt = Point::new(p.point.x, p.point.y);
            for fc in p.field_codes() {
                match fc.action {
                    CodeAction::Begin => {
                        if let Some(pts) = active.remove(&fc.code) {
                            if pts.len() >= 2 {
                                result.push(Polyline::new(pts));
                            }
                        }
                        active.insert(fc.code, vec![pt]);
                    }
                    CodeAction::Continue => {
                        active.entry(fc.code).or_default().push(pt);
                    }
                    CodeAction::End => {
                        if let Some(mut pts) = active.remove(&fc.code) {
                            pts.push(pt);
                            if pts.len() >= 2 {
                                result.push(Polyline::new(pts));
                            }
                        }
                    }
                    CodeAction::None => {}
                }
            }
        }
        for (_, pts) in active {
            if pts.len() >= 2 {
                result.push(Polyline::new(pts));
            }
        }
        result
    }

    /// Generates linework and block references using a [`CodeLibrary`].
    /// Codes marked with `linework` are connected in the order points appear.
    /// Codes with a `block` mapping create [`BlockRef`]s at the point location.
    pub fn field_to_finish(
        &self,
        library: &crate::surveying::code_library::CodeLibrary,
    ) -> (Vec<Polyline>, Vec<crate::surveying::code_library::BlockRef>) {
        use super::field_code::CodeAction;
        use crate::surveying::code_library::BlockRef;
        use std::collections::BTreeMap;

        let mut active: BTreeMap<String, Vec<Point>> = BTreeMap::new();
        let mut lines = Vec::new();
        let mut blocks = Vec::new();

        for p in &self.points {
            let pt2d = Point::new(p.point.x, p.point.y);

            for code in &p.codes {
                if let Some(entry) = library.get(code) {
                    if let Some(name) = &entry.block {
                        blocks.push(BlockRef {
                            location: p.point,
                            name: name.clone(),
                            attributes: entry.attributes.clone(),
                        });
                    }
                    if entry.linework {
                        active.entry(code.clone()).or_default().push(pt2d);
                    }
                }
            }

            for fc in p.field_codes() {
                let lw = library.get(&fc.code).map(|e| e.linework).unwrap_or(true);
                if !lw {
                    continue;
                }
                match fc.action {
                    CodeAction::Begin => {
                        if let Some(pts) = active.remove(&fc.code) {
                            if pts.len() >= 2 {
                                lines.push(Polyline::new(pts));
                            }
                        }
                        active.insert(fc.code, vec![pt2d]);
                    }
                    CodeAction::Continue => {
                        active.entry(fc.code).or_default().push(pt2d);
                    }
                    CodeAction::End => {
                        if let Some(mut pts) = active.remove(&fc.code) {
                            pts.push(pt2d);
                            if pts.len() >= 2 {
                                lines.push(Polyline::new(pts));
                            }
                        }
                    }
                    CodeAction::None => {}
                }
            }
        }

        for (_code, pts) in active {
            if pts.len() >= 2 {
                lines.push(Polyline::new(pts));
            }
        }

        (lines, blocks)
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
        // z should remain unchanged
        assert!(db.points[0].point.z.abs() < 1e-6);
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

    #[test]
    fn figures_from_field_codes() {
        let mut db = PointDatabase::new();
        db.add_point(SurveyPoint::new(
            Some(1),
            Point3::new(0.0, 0.0, 0.0),
            None,
            vec!["BCURB".into()],
        ));
        db.add_point(SurveyPoint::new(
            Some(2),
            Point3::new(1.0, 0.0, 0.0),
            None,
            vec!["CCURB".into()],
        ));
        db.add_point(SurveyPoint::new(
            Some(3),
            Point3::new(1.0, 1.0, 0.0),
            None,
            vec!["ECURB".into()],
        ));
        let figs = db.generate_figures();
        assert_eq!(figs.len(), 1);
        assert_eq!(figs[0].vertices.len(), 3);
    }

    #[test]
    fn adjust_to_parcel_builds_traverse_and_parcel() {
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
            Point3::new(45.0, 25.0, 0.0),
            None,
            Vec::new(),
        ));
        let dist = 53.85164807134504f64;
        let angle = 2.3805798993650633f64;
        let obs = vec![
            Observation::Distance {
                from: 0,
                to: 2,
                value: dist,
                weight: 1.0,
            },
            Observation::Distance {
                from: 1,
                to: 2,
                value: dist,
                weight: 1.0,
            },
            Observation::Angle {
                at: 2,
                from: 0,
                to: 1,
                value: angle,
                weight: 1.0,
            },
        ];
        let (res, parcel) = db.adjust_to_parcel(&[0, 1], &obs);
        let p = &db.points[2].point;
        assert!((p.x - 50.0).abs() < 1e-2);
        assert!((p.y - 20.0).abs() < 1e-2);
        assert!(res.residuals.iter().all(|v| v.abs() < 1e-6));
        assert_eq!(parcel.boundary.len(), 3);
        assert!((parcel.area() - 1000.0).abs() < 1.0);
    }

    #[test]
    fn field_to_finish_generates_blocks_and_lines() {
        use crate::surveying::code_library::CodeLibrary;
        let json = r#"{
            "codes": {
                "TREE": {"block": "tree", "attributes": {"type": "oak"}},
                "CURB": {"linework": true}
            }
        }"#;
        let path = std::env::temp_dir().join("codes.json");
        std::fs::write(&path, json).unwrap();
        let lib = CodeLibrary::from_json(path.to_str().unwrap()).unwrap();

        let mut db = PointDatabase::new();
        db.add_point(SurveyPoint::new(
            Some(1),
            Point3::new(0.0, 0.0, 0.0),
            None,
            vec!["TREE".into()],
        ));
        db.add_point(SurveyPoint::new(
            Some(2),
            Point3::new(1.0, 0.0, 0.0),
            None,
            vec!["BCURB".into()],
        ));
        db.add_point(SurveyPoint::new(
            Some(3),
            Point3::new(1.0, 1.0, 0.0),
            None,
            vec!["ECURB".into()],
        ));
        let (lines, blocks) = db.field_to_finish(&lib);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "tree");
        assert_eq!(blocks[0].attributes.get("type").unwrap(), "oak");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].vertices.len(), 2);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn audit_trail_records_changes() {
        let mut db = PointDatabase::new();
        db.add_point_with_audit(
            SurveyPoint::new(Some(1), Point3::new(0.0, 0.0, 0.0), None, Vec::new()),
            "tester",
            Some("initial"),
        );
        db.update_point(
            0,
            SurveyPoint::new(Some(1), Point3::new(1.0, 0.0, 0.0), None, Vec::new()),
            "tester",
            Some("move"),
        );
        assert_eq!(db.history.len(), 2);
        assert_eq!(db.history[0].user, "tester");
    }
}
