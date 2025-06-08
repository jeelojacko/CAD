use regex::Regex;
use crate::geometry::Point;
use crate::layers::{LayerManager};
use crate::geometry;

/// Returns layer names that do not conform to `^[A-Z0-9_]+$`.
pub fn check_layer_naming(mgr: &LayerManager) -> Vec<String> {
    let re = Regex::new("^[A-Z0-9_]+$").unwrap();
    mgr.names()
        .filter(|name| !re.is_match(name))
        .map(|s| s.to_string())
        .collect()
}

/// Returns names of used layers that are missing from the manager.
pub fn check_layer_usage(mgr: &LayerManager, used: &[String]) -> Vec<String> {
    used.iter()
        .filter(|name| mgr.layer(name.as_str()).is_none())
        .cloned()
        .collect()
}

/// Finds point indices that are farther than `threshold` from any control point.
pub fn coordinate_outliers(points: &[Point], control: &[Point], threshold: f64) -> Vec<(usize, f64)> {
    points
        .iter()
        .enumerate()
        .filter_map(|(idx, p)| {
            let min_dist = control
                .iter()
                .map(|c| geometry::distance(*p, *c))
                .fold(f64::INFINITY, f64::min);
            if min_dist > threshold {
                Some((idx, min_dist))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::Layer;

    #[test]
    fn test_check_layer_naming() {
        let mut mgr = LayerManager::new();
        mgr.add_layer(Layer::new("GOOD_LAYER"));
        mgr.add_layer(Layer::new("badLayer"));
        let bad = check_layer_naming(&mgr);
        assert_eq!(bad, vec!["badLayer".to_string()]);
    }

    #[test]
    fn test_coordinate_outliers() {
        let pts = vec![Point::new(0.0,0.0), Point::new(10.0,10.0)];
        let ctl = vec![Point::new(0.1,0.1)];
        let res = coordinate_outliers(&pts, &ctl, 1.0);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].0, 1);
    }
}
