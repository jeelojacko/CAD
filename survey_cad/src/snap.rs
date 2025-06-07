use crate::geometry::{distance, Arc, Line, Point, Polyline};
use crate::io::DxfEntity;
use crate::surveying::line_intersection;

/// Attempts to snap `target` to nearby geometry within `tol` units.
///
/// The function checks endpoints, midpoints, line intersections and
/// nearest points on line or arc entities.
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
    fn snap_to_intersection() {
        let line1 = DxfEntity::Line {
            line: Line::new(Point::new(-1.0, 0.0), Point::new(1.0, 0.0)),
            layer: None,
        };
        let line2 = DxfEntity::Line {
            line: Line::new(Point::new(0.0, -1.0), Point::new(0.0, 1.0)),
            layer: None,
        };
        let snapped = snap_point(Point::new(0.1, 0.1), &[line1, line2], 0.5).unwrap();
        assert!(distance(snapped, Point::new(0.0, 0.0)) < 0.2);
    }

    #[test]
    fn snap_to_nearest_point() {
        let line = DxfEntity::Line {
            line: Line::new(Point::new(0.0, 0.0), Point::new(2.0, 0.0)),
            layer: None,
        };
        let snapped = snap_point(Point::new(1.0, 2.0), &[line], 5.0).unwrap();
        assert!((snapped.x - 1.0).abs() < 1e-6 && snapped.y.abs() < 1e-6);
    }
}
