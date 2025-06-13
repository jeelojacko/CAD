use crate::geometry::{distance, Line, Point};
#[cfg(test)]
use crate::geometry::Arc;
use crate::io::DxfEntity;
use crate::surveying::line_intersection;

/// Snaps to the nearest endpoint of supported entities within `tol` units.
pub fn snap_to_endpoint(target: Point, entities: &[DxfEntity], tol: f64) -> Option<Point> {
    let mut best = None;
    let mut best_dist = tol;
    for e in entities {
        match e {
            DxfEntity::Point { point, .. } => {
                let d = distance(target, *point);
                if d < best_dist {
                    best_dist = d;
                    best = Some(*point);
                }
            }
            DxfEntity::Line { line, .. } => {
                for p in [line.start, line.end] {
                    let d = distance(target, p);
                    if d < best_dist {
                        best_dist = d;
                        best = Some(p);
                    }
                }
            }
            DxfEntity::Polyline { polyline, .. } => {
                for &p in &polyline.vertices {
                    let d = distance(target, p);
                    if d < best_dist {
                        best_dist = d;
                        best = Some(p);
                    }
                }
            }
            DxfEntity::Arc { arc, .. } => {
                for p in [arc.start_point(), arc.end_point()] {
                    let d = distance(target, p);
                    if d < best_dist {
                        best_dist = d;
                        best = Some(p);
                    }
                }
            }
            DxfEntity::Text { position, .. } => {
                let d = distance(target, *position);
                if d < best_dist {
                    best_dist = d;
                    best = Some(*position);
                }
            }
        }
    }
    best
}

/// Snaps to the centre of arc entities within `tol` units.
pub fn snap_to_centre(target: Point, entities: &[DxfEntity], tol: f64) -> Option<Point> {
    let mut best = None;
    let mut best_dist = tol;
    for e in entities {
        if let DxfEntity::Arc { arc, .. } = e {
            let d = distance(target, arc.center);
            if d < best_dist {
                best_dist = d;
                best = Some(arc.center);
            }
        }
    }
    best
}

/// Snaps to intersections of line and polyline segments within `tol` units.
pub fn snap_to_intersection(target: Point, entities: &[DxfEntity], tol: f64) -> Option<Point> {
    let mut lines: Vec<Line> = Vec::new();
    for e in entities {
        match e {
            DxfEntity::Line { line, .. } => lines.push(*line),
            DxfEntity::Polyline { polyline, .. } => {
                for seg in polyline.vertices.windows(2) {
                    lines.push(Line::new(seg[0], seg[1]));
                }
            }
            _ => {}
        }
    }

    let mut best = None;
    let mut best_dist = tol;
    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            if let Some(p) = line_intersection(lines[i].start, lines[i].end, lines[j].start, lines[j].end) {
                let d = distance(target, p);
                if d < best_dist {
                    best_dist = d;
                    best = Some(p);
                }
            }
        }
    }
    best
}

/// Snaps to the nearest point on lines, polylines or arcs within `tol` units.
pub fn snap_to_nearest(target: Point, entities: &[DxfEntity], tol: f64) -> Option<Point> {
    let mut best = None;
    let mut best_dist = tol;
    for e in entities {
        match e {
            DxfEntity::Line { line, .. } => {
                let p = line.nearest_point(target);
                let d = distance(target, p);
                if d < best_dist {
                    best_dist = d;
                    best = Some(p);
                }
            }
            DxfEntity::Polyline { polyline, .. } => {
                let p = polyline.nearest_point(target);
                let d = distance(target, p);
                if d < best_dist {
                    best_dist = d;
                    best = Some(p);
                }
            }
            DxfEntity::Arc { arc, .. } => {
                let p = arc.nearest_point(target);
                let d = distance(target, p);
                if d < best_dist {
                    best_dist = d;
                    best = Some(p);
                }
            }
            _ => {}
        }
    }
    best
}

/// Attempts to snap `target` to nearby geometry within `tol` units.
///
/// The function checks endpoints, midpoints, arc centres, line
/// intersections and nearest points on line or arc entities.
pub fn snap_point(target: Point, entities: &[DxfEntity], tol: f64) -> Option<Point> {
    let mut candidates: Vec<Point> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();

    for e in entities {
        match e {
            DxfEntity::Point { point, .. } => candidates.push(*point),
            DxfEntity::Line { line, .. } => {
                candidates.push(line.start);
                candidates.push(line.end);
                candidates.push(line.midpoint());
                lines.push(*line);
            }
            DxfEntity::Polyline { polyline, .. } => {
                for seg in polyline.vertices.windows(2) {
                    let l = Line::new(seg[0], seg[1]);
                    candidates.push(l.start);
                    candidates.push(l.end);
                    candidates.push(l.midpoint());
                    lines.push(l);
                }
            }
            DxfEntity::Arc { arc, .. } => {
                candidates.push(arc.start_point());
                candidates.push(arc.end_point());
                candidates.push(arc.midpoint());
                candidates.push(arc.center);
            }
            DxfEntity::Text { position, .. } => candidates.push(*position),
        }
    }

    // line-line intersections
    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            if let Some(p) =
                line_intersection(lines[i].start, lines[i].end, lines[j].start, lines[j].end)
            {
                candidates.push(p);
            }
        }
    }

    let mut best = None;
    let mut best_dist = tol;

    for c in &candidates {
        let d = distance(target, *c);
        if d < best_dist {
            best_dist = d;
            best = Some(*c);
        }
    }

    // nearest on segments and arcs if nothing else
    for l in &lines {
        let p = l.nearest_point(target);
        let d = distance(target, p);
        if d < best_dist {
            best_dist = d;
            best = Some(p);
        }
    }
    for e in entities {
        if let DxfEntity::Arc { arc, .. } = e {
            let p = arc.nearest_point(target);
            let d = distance(target, p);
            if d < best_dist {
                best_dist = d;
                best = Some(p);
            }
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_snapping() {
        let line = DxfEntity::Line {
            line: Line::new(Point::new(0.0, 0.0), Point::new(2.0, 0.0)),
            layer: None,
        };
        let snapped = snap_to_endpoint(Point::new(0.1, 0.1), &[line], 0.5).unwrap();
        assert!(distance(snapped, Point::new(0.0, 0.0)) < 0.2);
    }

    #[test]
    fn centre_snapping() {
        let arc = DxfEntity::Arc {
            arc: Arc::new(Point::new(1.0, 1.0), 1.0, 0.0, std::f64::consts::PI),
            layer: None,
        };
        let snapped = snap_to_centre(Point::new(1.2, 1.0), &[arc], 0.5).unwrap();
        assert!(distance(snapped, Point::new(1.0, 1.0)) < 0.2);
    }

    #[test]
    fn intersection_snapping() {
        let line1 = DxfEntity::Line {
            line: Line::new(Point::new(-1.0, 0.0), Point::new(1.0, 0.0)),
            layer: None,
        };
        let line2 = DxfEntity::Line {
            line: Line::new(Point::new(0.0, -1.0), Point::new(0.0, 1.0)),
            layer: None,
        };
        let snapped = snap_point(Point::new(0.1, 0.1), &[line1.clone(), line2.clone()], 0.5).unwrap();
        assert!(distance(snapped, Point::new(0.0, 0.0)) < 0.2);

        let snapped2 = super::snap_to_intersection(Point::new(0.1, 0.1), &[line1, line2], 0.5).unwrap();
        assert!(distance(snapped2, Point::new(0.0, 0.0)) < 0.2);
    }

    #[test]
    fn nearest_snapping() {
        let line = DxfEntity::Line {
            line: Line::new(Point::new(0.0, 0.0), Point::new(2.0, 0.0)),
            layer: None,
        };
        let snapped = snap_point(Point::new(1.0, 2.0), &[line.clone()], 5.0).unwrap();
        assert!((snapped.x - 1.0).abs() < 1e-6 && snapped.y.abs() < 1e-6);

        let snapped2 = super::snap_to_nearest(Point::new(1.0, 2.0), &[line], 5.0).unwrap();
        assert!((snapped2.x - 1.0).abs() < 1e-6 && snapped2.y.abs() < 1e-6);
    }
}
