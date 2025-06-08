use crate::geometry::Point3;
use std::collections::HashMap;

/// Classification types for point cloud filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    Ground,
    Vegetation,
    Building,
    Noise,
}

/// Simple statistical outlier removal based on neighbor counts.
///
/// Points with fewer than `min_neighbors` neighbours inside `radius`
/// are considered noise and removed.
pub fn filter_noise(points: &[Point3], radius: f64, min_neighbors: usize) -> Vec<Point3> {
    let mut filtered = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let mut count = 0;
        for (j, q) in points.iter().enumerate() {
            if i == j {
                continue;
            }
            let dx = p.x - q.x;
            let dy = p.y - q.y;
            let dz = p.z - q.z;
            if (dx * dx + dy * dy + dz * dz).sqrt() <= radius {
                count += 1;
                if count >= min_neighbors {
                    break;
                }
            }
        }
        if count >= min_neighbors {
            filtered.push(*p);
        }
    }
    filtered
}

/// Classify points into ground, vegetation and buildings using a simple
/// grid-based minimum elevation approach.
///
/// * `cell_size` - size of the XY grid cell used to determine ground elevation
/// * `ground_threshold` - maximum height difference from the cell minimum to be
///   considered ground
/// * `veg_threshold` - height difference above ground classified as vegetation;
///   anything higher is considered building.
pub fn classify_points(
    points: &[Point3],
    cell_size: f64,
    ground_threshold: f64,
    veg_threshold: f64,
) -> Vec<Classification> {
    let mut grid_min: HashMap<(i64, i64), f64> = HashMap::new();
    for p in points {
        let key = (
            (p.x / cell_size).floor() as i64,
            (p.y / cell_size).floor() as i64,
        );
        grid_min
            .entry(key)
            .and_modify(|z| {
                if p.z < *z {
                    *z = p.z
                }
            })
            .or_insert(p.z);
    }
    let mut classes = Vec::with_capacity(points.len());
    for p in points {
        let key = (
            (p.x / cell_size).floor() as i64,
            (p.y / cell_size).floor() as i64,
        );
        let base_z = *grid_min.get(&key).unwrap_or(&p.z);
        let dz = p.z - base_z;
        if dz.abs() <= ground_threshold {
            classes.push(Classification::Ground);
        } else if dz <= veg_threshold {
            classes.push(Classification::Vegetation);
        } else {
            classes.push(Classification::Building);
        }
    }
    classes
}
