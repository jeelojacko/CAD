use crate::geometry::{polygon_area, Point, Point3};

/// Triangulated Irregular Network constructed from 3D points.
#[derive(Debug, Clone)]
pub struct Tin {
    /// Vertices of the TIN.
    pub vertices: Vec<Point3>,
    /// Indices into `vertices` forming triangles.
    pub triangles: Vec<[usize; 3]>,
}

impl Tin {
    /// Builds a TIN from the provided vertices using Delaunay triangulation on the XY plane.
    pub fn from_points(points: Vec<Point3>) -> Self {
        let coords: Vec<delaunator::Point> = points
            .iter()
            .map(|p| delaunator::Point { x: p.x, y: p.y })
            .collect();
        let triangulation = delaunator::triangulate(&coords);
        let triangles = triangulation
            .triangles
            .chunks(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        Self {
            vertices: points,
            triangles,
        }
    }

    /// Generates contour line segments at the specified interval.
    pub fn contour_segments(&self, interval: f64) -> Vec<(Point3, Point3)> {
        if interval <= 0.0 || self.vertices.is_empty() {
            return Vec::new();
        }
        let min_z = self
            .vertices
            .iter()
            .map(|p| p.z)
            .fold(f64::INFINITY, f64::min);
        let max_z = self
            .vertices
            .iter()
            .map(|p| p.z)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut segments = Vec::new();
        let mut level = (min_z / interval).ceil() * interval;
        while level <= max_z {
            for tri in &self.triangles {
                let a = self.vertices[tri[0]];
                let b = self.vertices[tri[1]];
                let c = self.vertices[tri[2]];
                let tmin = a.z.min(b.z).min(c.z);
                let tmax = a.z.max(b.z).max(c.z);
                if level < tmin || level > tmax {
                    continue;
                }
                let mut pts = Vec::new();
                if let Some(p) = intersect_edge(a, b, level) {
                    pts.push(p);
                }
                if let Some(p) = intersect_edge(b, c, level) {
                    pts.push(p);
                }
                if let Some(p) = intersect_edge(c, a, level) {
                    pts.push(p);
                }
                if pts.len() == 2 {
                    segments.push((pts[0], pts[1]));
                }
            }
            level += interval;
        }
        segments
    }

    /// Calculates the volume between the TIN surface and a horizontal plane at `base_elev`.
    pub fn volume_to_elevation(&self, base_elev: f64) -> f64 {
        let mut volume = 0.0;
        for tri in &self.triangles {
            let a = self.vertices[tri[0]];
            let b = self.vertices[tri[1]];
            let c = self.vertices[tri[2]];
            let area = polygon_area(&[
                Point::new(a.x, a.y),
                Point::new(b.x, b.y),
                Point::new(c.x, c.y),
            ])
            .abs();
            let avg_z = (a.z + b.z + c.z) / 3.0;
            volume += area * (avg_z - base_elev);
        }
        volume
    }
}

fn intersect_edge(a: Point3, b: Point3, level: f64) -> Option<Point3> {
    let da = a.z - level;
    let db = b.z - level;
    if da * db > 0.0 || (da - db).abs() < f64::EPSILON {
        None
    } else {
        let t = da / (da - db);
        Some(Point3::new(
            a.x + t * (b.x - a.x),
            a.y + t * (b.y - a.y),
            level,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tin_volume_flat_square() {
        let pts = vec![
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let tin = Tin::from_points(pts);
        let volume = tin.volume_to_elevation(0.0);
        assert!((volume - 1.0).abs() < 1e-6);
    }
}
