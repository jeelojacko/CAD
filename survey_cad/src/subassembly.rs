//! Library of reusable roadway subassemblies.

use crate::corridor::Subassembly;

/// Creates a simple travel lane with the given `width` and cross slope.
/// Positive `slope` values fall to the right, negative to the left.
pub fn lane(width: f64, slope: f64) -> Subassembly {
    Subassembly::new(vec![(0.0, 0.0), (width, width * slope)])
}

/// Creates a paved shoulder with the specified `width` and cross slope.
pub fn shoulder(width: f64, slope: f64) -> Subassembly {
    Subassembly::new(vec![(0.0, 0.0), (width, width * slope)])
}

/// Creates a curb represented by a vertical face of `height` and top width.
pub fn curb(height: f64, width: f64) -> Subassembly {
    Subassembly::new(vec![(0.0, 0.0), (0.0, height), (width, height)])
}

/// Creates a sidewalk with `width` and cross slope.
pub fn sidewalk(width: f64, slope: f64) -> Subassembly {
    Subassembly::new(vec![(0.0, 0.0), (width, width * slope)])
}

/// Creates a combined curb and gutter section. The curb has a vertical face of
/// `height` and `curb_width`. The gutter extends an additional `gutter_width`
/// at a constant `gutter_slope`.
pub fn curb_and_gutter(
    height: f64,
    curb_width: f64,
    gutter_width: f64,
    gutter_slope: f64,
) -> Subassembly {
    let mut profile = vec![(0.0, 0.0), (0.0, height), (curb_width, height)];
    profile.push((
        curb_width + gutter_width,
        height + gutter_width * gutter_slope,
    ));
    Subassembly::new(profile)
}

/// Creates a raised median with vertical faces given `width` and `height`.
/// The profile starts at the pavement edge and returns to grade at the end
/// of the median.
pub fn median(width: f64, height: f64) -> Subassembly {
    Subassembly::new(vec![
        (0.0, 0.0),
        (0.0, height),
        (width, height),
        (width, 0.0),
    ])
}

/// Creates a ditch with `depth`, `bottom_width` and `side_slope` (horizontal to
/// vertical). The profile begins at existing grade, transitions down to the
/// bottom and then back up to grade.
pub fn ditch(depth: f64, bottom_width: f64, side_slope: f64) -> Subassembly {
    let run = depth * side_slope.abs();
    let mut pts = vec![(0.0, 0.0), (run, -depth)];
    if bottom_width > 0.0 {
        pts.push((run + bottom_width, -depth));
    }
    pts.push((run + bottom_width + run, 0.0));
    Subassembly::new(pts)
}

/// Creates a daylight slope that projects from the origin for `width` at the
/// provided `slope`.
pub fn daylight(width: f64, slope: f64) -> Subassembly {
    Subassembly::new(vec![(0.0, 0.0), (width, width * slope)])
}

/// Generates a daylight profile table that targets an existing surface. The
/// returned subassembly contains a profile table mapping station along the
/// `alignment` to the daylight intercept computed from `slope`.
pub fn daylight_to_surface(
    surface: &crate::dtm::Tin,
    alignment: &crate::alignment::Alignment,
    slope: f64,
    interval: f64,
    step: f64,
    max_dist: f64,
) -> Subassembly {
    use crate::corridor::ProfilePoint;

    let mut table = Vec::new();
    let length = alignment.horizontal.length();
    let mut station = 0.0;
    while station <= length {
        if let (Some(center), Some(dir), Some(grade)) = (
            alignment.horizontal.point_at(station),
            alignment.horizontal.direction_at(station),
            alignment.vertical.elevation_at(station),
        ) {
            let normal = (-dir.1, dir.0);
            let side = if slope <= 0.0 {
                normal
            } else {
                (-normal.0, -normal.1)
            };
            if let Some(p) = surface.slope_projection(
                crate::geometry::Point3::new(center.x, center.y, grade),
                side,
                slope,
                step,
                max_dist,
            ) {
                let dist = (p.x - center.x) * side.0 + (p.y - center.y) * side.1;
                let offset = if slope <= 0.0 { dist } else { -dist };
                let profile = vec![(0.0, 0.0), (offset, slope * dist)];
                table.push(ProfilePoint { station, profile });
            }
        }
        station += interval;
    }

    let profile = table
        .first()
        .map(|p| p.profile.clone())
        .unwrap_or_else(|| vec![(0.0, 0.0)]);
    let mut sub = Subassembly::new(profile);
    sub.profile_table = Some(table);
    sub
}

/// Creates a simple retaining wall with a vertical face of `height` and a
/// footing `width`.
pub fn retaining_wall(height: f64, width: f64) -> Subassembly {
    Subassembly::new(vec![(0.0, 0.0), (0.0, -height), (width, -height)])
}

/// Generates a subassembly that linearly transitions from `start` to `end`
/// over the provided `length`. The returned subassembly contains a profile
/// table used during corridor extraction to interpolate the shape.
pub fn transition(start: &Subassembly, end: &Subassembly, length: f64) -> Subassembly {
    use crate::corridor::ProfilePoint;

    let table = vec![
        ProfilePoint {
            station: 0.0,
            profile: start.profile.clone(),
        },
        ProfilePoint {
            station: length,
            profile: end.profile.clone(),
        },
    ];

    let mut sub = Subassembly::new(start.profile.clone());
    sub.profile_table = Some(table);
    sub
}

/// Mirrors a subassembly about the alignment, returning a new one suitable for
/// the opposite side of the road.
pub fn mirror(sub: &Subassembly) -> Subassembly {
    let mut profile: Vec<(f64, f64)> = sub.profile.iter().rev().map(|(o, e)| (-o, *e)).collect();
    // ensure the first point is exactly mirrored
    if let Some(first) = profile.first_mut() {
        first.0 = 0.0;
    }
    Subassembly::new(profile)
}

/// Sequentially joins multiple subassemblies together into a single profile.
/// Each subassembly should start at `(0.0, 0.0)` and is appended to the
/// previous one's end.
pub fn compose(parts: &[Subassembly]) -> Subassembly {
    let mut profile = Vec::new();
    let mut off = 0.0;
    let mut elev = 0.0;
    for part in parts {
        if part.profile.is_empty() {
            continue;
        }
        for (i, (o, e)) in part.profile.iter().enumerate() {
            let p = (off + o, elev + e);
            if i == 0 && !profile.is_empty() {
                // skip repeating the connecting vertex
                continue;
            }
            profile.push(p);
        }
        off += part.profile.last().unwrap().0;
        elev += part.profile.last().unwrap().1;
    }
    Subassembly::new(profile)
}

/// Builds a symmetric cross section from a collection of right-side
/// subassemblies. The returned vector contains the left and right profiles ready
/// to be passed to [`extract_design_cross_sections`](crate::corridor::extract_design_cross_sections).
pub fn symmetric_section(parts_right: &[Subassembly]) -> Vec<Subassembly> {
    let right = compose(parts_right);
    let left = mirror(&right);
    vec![left, right]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_two_lanes() {
        let right = lane(3.0, -0.02);
        let curb = curb(0.15, 0.3);
        let section = compose(&[right, curb]);
        assert_eq!(section.profile.len(), 4);
        let last = *section.profile.last().unwrap();
        assert!((last.0 - 3.3).abs() < 1e-6);
        assert!((last.1 - (-0.06 + 0.15)).abs() < 1e-6);
    }

    #[test]
    fn symmetric_builder() {
        let right = lane(3.0, -0.02);
        let sections = symmetric_section(&[right.clone()]);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].profile.first().unwrap().0, 0.0);
        assert_eq!(sections[1].profile.first().unwrap().0, 0.0);
        assert_eq!(sections[0].profile.len(), sections[1].profile.len());
    }

    #[test]
    fn median_profile() {
        let m = median(2.0, 0.5);
        assert_eq!(m.profile.len(), 4);
        assert_eq!(m.profile[1], (0.0, 0.5));
        assert_eq!(m.profile.last().unwrap(), &(2.0, 0.0));
    }

    #[test]
    fn ditch_profile() {
        let d = ditch(1.0, 2.0, 3.0);
        assert_eq!(d.profile.first().unwrap(), &(0.0, 0.0));
        assert_eq!(d.profile[1], (3.0, -1.0));
        assert_eq!(d.profile.last().unwrap(), &(8.0, 0.0));
    }

    #[test]
    fn wall_profile() {
        let w = retaining_wall(2.0, 0.5);
        assert_eq!(w.profile, vec![(0.0, 0.0), (0.0, -2.0), (0.5, -2.0)]);
    }

    #[test]
    fn curb_and_gutter_profile() {
        let cg = curb_and_gutter(0.15, 0.3, 0.5, -0.05);
        assert_eq!(cg.profile.len(), 4);
        assert_eq!(cg.profile[1], (0.0, 0.15));
        let last = cg.profile.last().unwrap();
        assert!((last.0 - 0.8).abs() < 1e-6);
        assert!((last.1 - (0.15 + 0.5 * -0.05)).abs() < 1e-6);
    }

    #[test]
    fn transition_table() {
        let a = lane(3.0, -0.02);
        let b = shoulder(3.5, -0.02);
        let t = transition(&a, &b, 10.0);
        assert!(t.profile_table.is_some());
        let table = t.profile_table.as_ref().unwrap();
        assert_eq!(table.len(), 2);
        assert_eq!(table[0].profile, a.profile);
        assert_eq!(table[1].profile, b.profile);
    }

    #[test]
    fn daylight_surface_table() {
        use crate::alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
        use crate::dtm::Tin;
        use crate::geometry::{Point, Point3};

        let ground = Tin::from_points(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, -5.0),
            Point3::new(0.0, 10.0, 0.0),
        ]);
        let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
        let valign = VerticalAlignment::new(vec![(0.0, 1.0), (10.0, 1.0)]);
        let align = Alignment::new(halign, valign);

        let sub = daylight_to_surface(&ground, &align, -0.5, 5.0, 1.0, 20.0);
        assert!(sub.profile_table.is_some());
        let table = sub.profile_table.unwrap();
        assert!(!table.is_empty());
    }
}
