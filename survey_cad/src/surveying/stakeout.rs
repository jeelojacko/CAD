use crate::alignment::{HorizontalAlignment, HorizontalElement};
use crate::geometry::{Point, Arc};

/// Computes the stakeout position at a given station and offset along a
/// horizontal alignment. Tangent segments use a perpendicular offset while
/// curves apply a radial offset.
pub fn stakeout_position(alignment: &HorizontalAlignment, station: f64, offset: f64) -> Option<Point> {
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
                        Point::new(base.x + offset * norm.0 / nlen, base.y + offset * norm.1 / nlen)
                    }
                }
            });
        }
        remaining -= len;
    }
    None
}

/// Computes a stakeout point on a tangent segment.
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

/// Computes a stakeout point on a circular curve using a radial offset.
fn curve_point(arc: &Arc, distance: f64, offset: f64) -> Point {
    let dir = if arc.end_angle >= arc.start_angle { 1.0 } else { -1.0 };
    let ang = arc.start_angle + distance / arc.radius * dir;
    let r = arc.radius + offset;
    Point::new(arc.center.x + r * ang.cos(), arc.center.y + r * ang.sin())
}
