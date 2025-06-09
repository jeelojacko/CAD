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

/// Extract linear breaklines from a point cloud based on slope between
/// nearby points.
///
/// Points within `radius` are connected when the vertical slope between
/// them exceeds `slope_threshold` (rise over run). The result is a list of
/// index pairs into the input array representing detected breaklines.
pub fn extract_breaklines(
    points: &[Point3],
    radius: f64,
    slope_threshold: f64,
) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    for (i, &a) in points.iter().enumerate() {
        for (j, &b) in points.iter().enumerate().skip(i + 1) {
            let dx = a.x - b.x;
            let dy = a.y - b.y;
            let horiz = (dx * dx + dy * dy).sqrt();
            if horiz <= radius && horiz > f64::EPSILON {
                let dz = (a.z - b.z).abs();
                if dz / horiz >= slope_threshold {
                    lines.push((i, j));
                }
            }
        }
    }
    lines
}

#[cfg(feature = "render")]
use bevy::asset::RenderAssetUsages;
#[cfg(feature = "render")]
use bevy::prelude::Mesh;
#[cfg(feature = "render")]
use bevy::render::mesh::{Indices, PrimitiveTopology};

/// Builds a textured mesh from a point cloud using Delaunay triangulation.
///
/// The mesh is suitable for basic visualization of the point cloud surface.
#[cfg(feature = "render")]
pub fn point_cloud_to_mesh(points: &[Point3]) -> Mesh {
    use crate::dtm::Tin;
    let tin = Tin::from_points(points.to_vec());
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let positions: Vec<[f32; 3]> = tin
        .vertices
        .iter()
        .map(|p| [p.x as f32, p.y as f32, p.z as f32])
        .collect();
    let min_x = positions.iter().fold(f32::INFINITY, |m, v| m.min(v[0]));
    let max_x = positions.iter().fold(f32::NEG_INFINITY, |m, v| m.max(v[0]));
    let min_y = positions.iter().fold(f32::INFINITY, |m, v| m.min(v[1]));
    let max_y = positions.iter().fold(f32::NEG_INFINITY, |m, v| m.max(v[1]));
    let width = (max_x - min_x).max(f32::EPSILON);
    let height = (max_y - min_y).max(f32::EPSILON);
    let uvs: Vec<[f32; 2]> = positions
        .iter()
        .map(|p| [(p[0] - min_x) / width, (p[1] - min_y) / height])
        .collect();
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let indices: Vec<u32> = tin
        .triangles
        .iter()
        .flat_map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakline_detection_basic() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
        ];
        let lines = extract_breaklines(&pts, 1.1, 0.5);
        assert!(lines.contains(&(0, 2)) || lines.contains(&(2, 0)));
        assert!(lines.contains(&(1, 3)) || lines.contains(&(3, 1)));
    }
}
