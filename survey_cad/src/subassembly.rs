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

/// Mirrors a subassembly about the alignment, returning a new one suitable for
/// the opposite side of the road.
pub fn mirror(sub: &Subassembly) -> Subassembly {
    let mut profile: Vec<(f64, f64)> = sub
        .profile
        .iter()
        .rev()
        .map(|(o, e)| (-o, *e))
        .collect();
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
        assert_eq!(section.profile.len(), 3);
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
}
