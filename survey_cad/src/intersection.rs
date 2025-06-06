use crate::alignment::{HorizontalAlignment, HorizontalElement, VerticalAlignment};
use crate::geometry::{Arc, Line, Point};
use crate::surveying::line_intersection;

/// Description of a curb return connecting two approach tangents.
#[derive(Debug, Clone)]
pub struct CurbReturn {
    pub start: Point,
    pub end: Point,
    pub arc: Arc,
}

fn unit(v: (f64, f64)) -> (f64, f64) {
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();
    if len.abs() < f64::EPSILON {
        (0.0, 0.0)
    } else {
        (v.0 / len, v.1 / len)
    }
}

fn rotate90(v: (f64, f64), sign: f64) -> (f64, f64) {
    if sign >= 0.0 {
        (-v.1, v.0)
    } else {
        (v.1, -v.0)
    }
}

/// Builds a curb return arc between two lines.
pub fn build_curb_return_arc(line_in: &Line, line_out: &Line, radius: f64) -> Option<CurbReturn> {
    let pi = line_intersection(line_in.start, line_in.end, line_out.start, line_out.end)?;
    let t1 = unit((
        line_in.end.x - line_in.start.x,
        line_in.end.y - line_in.start.y,
    ));
    let t2 = unit((
        line_out.end.x - line_out.start.x,
        line_out.end.y - line_out.start.y,
    ));

    let ang1 = t1.1.atan2(t1.0);
    let ang2 = t2.1.atan2(t2.0);
    let mut phi = ang2 - ang1;
    while phi <= -std::f64::consts::PI {
        phi += 2.0 * std::f64::consts::PI;
    }
    while phi > std::f64::consts::PI {
        phi -= 2.0 * std::f64::consts::PI;
    }
    let phi = phi.abs();
    if phi.abs() < f64::EPSILON || phi >= std::f64::consts::PI {
        return None;
    }
    let tangent = radius * (phi / 2.0).tan();

    let pc = Point::new(pi.x - t1.0 * tangent, pi.y - t1.1 * tangent);
    let pt = Point::new(pi.x + t2.0 * tangent, pi.y + t2.1 * tangent);

    let sign = (t1.0 * t2.1 - t1.1 * t2.0).signum();
    let n1 = unit(rotate90(t1, sign));
    let n2 = unit(rotate90(t2, sign));

    let center = line_intersection(
        pc,
        Point::new(pc.x + n1.0 * radius * 2.0, pc.y + n1.1 * radius * 2.0),
        pt,
        Point::new(pt.x + n2.0 * radius * 2.0, pt.y + n2.1 * radius * 2.0),
    )?;

    let start_angle = (pc.y - center.y).atan2(pc.x - center.x);
    let end_angle = (pt.y - center.y).atan2(pt.x - center.x);
    let arc = if sign >= 0.0 {
        Arc::new(center, radius, start_angle, end_angle)
    } else {
        Arc::new(center, radius, end_angle, start_angle)
    };

    Some(CurbReturn {
        start: pc,
        end: pt,
        arc,
    })
}

/// Creates a curb return between two alignments using their last and first tangent segments.
pub fn curb_return_between_alignments(
    a: &HorizontalAlignment,
    b: &HorizontalAlignment,
    radius: f64,
) -> Option<CurbReturn> {
    let line_a = match a.elements.last()? {
        HorizontalElement::Tangent { start, end } => Line::new(*start, *end),
        _ => return None,
    };
    let line_b = match b.elements.first()? {
        HorizontalElement::Tangent { start, end } => Line::new(*start, *end),
        _ => return None,
    };
    build_curb_return_arc(&line_a, &line_b, radius)
}

/// Applies a constant grade adjustment after the given station.
pub fn apply_grade_adjustment(alignment: &mut VerticalAlignment, station: f64, delta: f64) {
    for elem in &mut alignment.elements {
        match elem {
            crate::alignment::VerticalElement::Grade {
                start_station,
                start_elev,
                end_elev,
                ..
            } => {
                if *start_station >= station {
                    *start_elev += delta;
                    *end_elev += delta;
                }
            }
            crate::alignment::VerticalElement::Parabola {
                start_station,
                start_elev,
                ..
            } => {
                if *start_station >= station {
                    *start_elev += delta;
                }
            }
        }
    }
}
