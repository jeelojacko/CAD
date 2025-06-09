use crate::alignment::{
    Alignment, HorizontalAlignment, HorizontalElement, VerticalAlignment, VerticalElement,
};
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

/// Result information for a connecting vertical curve.
#[derive(Debug, Clone, Copy)]
pub struct VerticalCurveInfo {
    /// Total length of the curve.
    pub length: f64,
    /// Station of the high/low point.
    pub high_low_station: f64,
    /// Elevation of the high/low point.
    pub high_low_elev: f64,
    /// Elevation adjustment required for the outgoing alignment.
    pub grade_adjustment: f64,
}

fn build_vertical_curve(
    a: &VerticalAlignment,
    b: &VerticalAlignment,
    station: f64,
    grade_in: f64,
    grade_out: f64,
) -> Option<VerticalCurveInfo> {
    let start_elem = a.elements.last()?;
    let end_elem = b.elements.first()?;

    let start_station = match start_elem {
        crate::alignment::VerticalElement::Grade { start_station, .. } => *start_station,
        crate::alignment::VerticalElement::Parabola { start_station, .. } => *start_station,
    };

    let end_station = match end_elem {
        crate::alignment::VerticalElement::Grade { end_station, .. } => *end_station,
        crate::alignment::VerticalElement::Parabola { end_station, .. } => *end_station,
    };

    let l1 = station - start_station;
    let l2 = end_station - station;
    let length = l1 + l2;
    if length <= 0.0 {
        return None;
    }

    let start_elev = a.elevation_at(start_station)?;

    let x_high = if (grade_out - grade_in).abs() < f64::EPSILON {
        0.0
    } else {
        (-grade_in * length) / (grade_out - grade_in)
    };

    let x_high = x_high.clamp(0.0, length);
    let high_station = start_station + x_high;
    let high_elev_raw =
        start_elev + grade_in * x_high + 0.5 * (grade_out - grade_in) / length * x_high * x_high;

    let curve_at_intersection =
        start_elev + grade_in * l1 + 0.5 * (grade_out - grade_in) / length * l1 * l1;
    let b_elev = b.elevation_at(station)?;
    let offset = curve_at_intersection - b_elev;
    let high_elev = high_elev_raw - offset;
    let grade_adjustment = 0.0;

    Some(VerticalCurveInfo {
        length,
        high_low_station: high_station,
        high_low_elev: high_elev,
        grade_adjustment,
    })
}

/// Creates a crest vertical curve connecting two alignments.
pub fn crest_curve_between_alignments(
    a: &VerticalAlignment,
    b: &VerticalAlignment,
    station: f64,
    grade_in: f64,
    grade_out: f64,
) -> Option<VerticalCurveInfo> {
    build_vertical_curve(a, b, station, grade_in, grade_out)
}

/// Creates a sag vertical curve connecting two alignments.
pub fn sag_curve_between_alignments(
    a: &VerticalAlignment,
    b: &VerticalAlignment,
    station: f64,
    grade_in: f64,
    grade_out: f64,
) -> Option<VerticalCurveInfo> {
    build_vertical_curve(a, b, station, grade_in, grade_out)
}

fn grade_at_start(elem: &VerticalElement) -> f64 {
    match elem {
        VerticalElement::Grade {
            start_station,
            end_station,
            start_elev,
            end_elev,
        } => {
            if (*end_station - *start_station).abs() < f64::EPSILON {
                0.0
            } else {
                (end_elev - start_elev) / (end_station - start_station)
            }
        }
        VerticalElement::Parabola { start_grade, .. } => *start_grade,
    }
}

fn grade_at_end(elem: &VerticalElement) -> f64 {
    match elem {
        VerticalElement::Grade {
            start_station,
            end_station,
            start_elev,
            end_elev,
        } => {
            if (*end_station - *start_station).abs() < f64::EPSILON {
                0.0
            } else {
                (end_elev - start_elev) / (end_station - start_station)
            }
        }
        VerticalElement::Parabola { end_grade, .. } => *end_grade,
    }
}

/// Creates a full intersection alignment between two approach alignments.
/// The incoming alignment `a` must end with a tangent and `b` must begin with
/// a tangent. The vertical alignments are connected with a parabolic curve and
/// the outgoing alignment is adjusted so grades tie together smoothly.
pub fn intersection_alignment(a: &Alignment, b: &Alignment, radius: f64) -> Option<Alignment> {
    let curb = curb_return_between_alignments(&a.horizontal, &b.horizontal, radius)?;

    if a.horizontal.elements.is_empty() || b.horizontal.elements.is_empty() {
        return None;
    }

    let mut h_elems = Vec::new();
    for elem in a
        .horizontal
        .elements
        .iter()
        .take(a.horizontal.elements.len() - 1)
    {
        h_elems.push(elem.clone());
    }
    match a.horizontal.elements.last().unwrap() {
        HorizontalElement::Tangent { start, .. } => {
            h_elems.push(HorizontalElement::Tangent {
                start: *start,
                end: curb.start,
            });
        }
        _ => return None,
    }

    h_elems.push(HorizontalElement::Curve { arc: curb.arc });

    match b.horizontal.elements.first().unwrap() {
        HorizontalElement::Tangent { end, .. } => {
            h_elems.push(HorizontalElement::Tangent {
                start: curb.end,
                end: *end,
            });
        }
        _ => return None,
    }
    for elem in b.horizontal.elements.iter().skip(1) {
        h_elems.push(elem.clone());
    }
    let horizontal = HorizontalAlignment { elements: h_elems };

    if a.vertical.elements.is_empty() || b.vertical.elements.is_empty() {
        return None;
    }

    let start_elem = a.vertical.elements.last().unwrap();
    let end_elem = b.vertical.elements.first().unwrap();

    let station = match end_elem {
        VerticalElement::Grade { start_station, .. } => *start_station,
        VerticalElement::Parabola { start_station, .. } => *start_station,
    };

    let grade_in = grade_at_end(start_elem);
    let grade_out = grade_at_start(end_elem);

    let info = if grade_out > grade_in {
        sag_curve_between_alignments(&a.vertical, &b.vertical, station, grade_in, grade_out)?
    } else {
        crest_curve_between_alignments(&a.vertical, &b.vertical, station, grade_in, grade_out)?
    };

    let start_station = match start_elem {
        VerticalElement::Grade { start_station, .. } => *start_station,
        VerticalElement::Parabola { start_station, .. } => *start_station,
    };
    let end_station = match end_elem {
        VerticalElement::Grade { end_station, .. } => *end_station,
        VerticalElement::Parabola { end_station, .. } => *end_station,
    };
    let start_elev = a.vertical.elevation_at(start_station)?;

    let parabola = VerticalElement::Parabola {
        start_station,
        end_station,
        start_elev,
        start_grade: grade_in,
        end_grade: grade_out,
    };

    let mut v_elems = Vec::new();
    for elem in a
        .vertical
        .elements
        .iter()
        .take(a.vertical.elements.len() - 1)
    {
        v_elems.push(elem.clone());
    }
    v_elems.push(parabola);

    let mut b_adj = b.vertical.clone();
    apply_grade_adjustment(&mut b_adj, station, info.grade_adjustment);
    for elem in b_adj.elements.into_iter().skip(1) {
        v_elems.push(elem);
    }
    let vertical = VerticalAlignment { elements: v_elems };

    Some(Alignment::new(horizontal, vertical))
}
