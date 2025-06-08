use crate::alignment::{HorizontalAlignment, HorizontalElement};
use crate::geometry::{Arc, Point};

/// Computes the stakeout position at a given station and offset along a
/// horizontal alignment. Tangent segments use a perpendicular offset while
/// curves apply a radial offset.
pub fn stakeout_position(
    alignment: &HorizontalAlignment,
    station: f64,
    offset: f64,
) -> Option<Point> {
    if station < 0.0 || station > alignment.length() {
        return None;
    }
    let mut remaining = station;
    for elem in &alignment.elements {
        let len = elem.length();
        if remaining <= len {
            return Some(match elem {
                HorizontalElement::Tangent { start, end } => {
                    tangent_point(*start, *end, remaining, offset)
                }
                HorizontalElement::Curve { arc } => curve_point(arc, remaining, offset),
                HorizontalElement::Spiral { spiral } => {
                    let base = spiral.point_at(remaining);
                    let dir = spiral.direction_at(remaining);
                    let norm = (-dir.1, dir.0);
                    let nlen = (norm.0 * norm.0 + norm.1 * norm.1).sqrt();
                    if nlen.abs() < f64::EPSILON {
                        base
                    } else {
                        Point::new(
                            base.x + offset * norm.0 / nlen,
                            base.y + offset * norm.1 / nlen,
                        )
                    }
                }
            });
        }
        remaining -= len;
    }
    None
}

fn tangent_point(start: Point, end: Point, distance: f64, offset: f64) -> Point {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len.abs() < f64::EPSILON {
        return start;
    }
    let t = distance / len;
    let x = start.x + t * dx;
    let y = start.y + t * dy;
    let nx = -dy / len;
    let ny = dx / len;
    Point::new(x + offset * nx, y + offset * ny)
}

fn curve_point(arc: &Arc, distance: f64, offset: f64) -> Point {
    let dir = if arc.end_angle >= arc.start_angle {
        1.0
    } else {
        -1.0
    };
    let ang = arc.start_angle + distance / arc.radius * dir;
    let r = arc.radius + offset;
    Point::new(arc.center.x + r * ang.cos(), arc.center.y + r * ang.sin())
}

/// Generates a sorted list of station values that include all alignment element boundaries
/// and evenly spaced stations at the provided interval. Duplicate stations are removed.
pub fn optimal_stationing(alignment: &HorizontalAlignment, interval: f64) -> Vec<f64> {
    let mut stations = alignment.stations();
    if interval > 0.0 {
        let mut s = 0.0;
        let len = alignment.length();
        while s <= len {
            stations.push(s);
            s += interval;
        }
    }
    stations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    stations.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    stations
}

/// Returns stakeout points for a rectangular grid defined by its minimum and maximum corners
/// using the given spacing. Points are ordered row by row starting at the minimum corner.
pub fn grid_stakeout_points(min: Point, max: Point, spacing: f64) -> Vec<Point> {
    if spacing <= 0.0 || max.x <= min.x || max.y <= min.y {
        return Vec::new();
    }
    let nx = ((max.x - min.x) / spacing).floor() as usize;
    let ny = ((max.y - min.y) / spacing).floor() as usize;
    let mut pts = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            pts.push(Point::new(
                min.x + i as f64 * spacing,
                min.y + j as f64 * spacing,
            ));
        }
    }
    pts
}
