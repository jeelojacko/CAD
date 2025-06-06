use crate::geometry::{polygon_area, Point, Point3};

/// Returns `true` if point `p` is inside the polygon defined by `poly` using
/// the ray casting algorithm.
fn point_in_polygon(p: Point, poly: &[Point]) -> bool {
    let mut inside = false;
    if poly.is_empty() {
        return inside;
    }
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let pi = poly[i];
        let pj = poly[j];
        if ((pi.y > p.y) != (pj.y > p.y))
            && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Triangulated Irregular Network constructed from 3D points.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

    /// Builds a constrained TIN using optional breaklines and an optional outer
    /// boundary. The `breaklines` slice contains index pairs into `points`
    /// representing fixed edges. When `outer_boundary` is provided it should be
    /// a closed polygon (first and last index may be equal or will be closed
    /// automatically).
    pub fn from_points_constrained(
        points: Vec<Point3>,
        breaklines: Option<&[(usize, usize)]>,
        outer_boundary: Option<&[usize]>,
    ) -> Self {
        let coords: Vec<(f64, f64)> = points.iter().map(|p| (p.x, p.y)).collect();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        if let Some(bl) = breaklines {
            edges.extend_from_slice(bl);
        }
        if let Some(bound) = outer_boundary {
            if bound.len() > 1 {
                for w in bound.windows(2) {
                    edges.push((w[0], w[1]));
                }
                edges.push((*bound.last().unwrap(), bound[0]));
            }
        }

        let tris = if edges.is_empty() {
            cdt::triangulate_points(&coords).unwrap()
        } else {
            cdt::triangulate_with_edges(&coords, &edges).unwrap()
        };
        let triangles = tris.into_iter().map(|t| [t.0, t.1, t.2]).collect();
        Self {
            vertices: points,
            triangles,
        }
    }

    /// Generates contour line segments at the specified interval. Optional
    /// `include` and `exclude` polygons can limit where contours are created.
    pub fn contour_segments(
        &self,
        interval: f64,
    ) -> Vec<(Point3, Point3)> {
        self.contour_segments_bounded(interval, None, &[])
    }

    /// Contour generation with inclusion/exclusion boundaries.
    pub fn contour_segments_bounded(
        &self,
        interval: f64,
        include: Option<&[Point]>,
        exclude: &[Vec<Point>],
    ) -> Vec<(Point3, Point3)> {
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
                let centroid = Point::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0);
                if let Some(poly) = include {
                    if !point_in_polygon(centroid, poly) {
                        continue;
                    }
                }
                if exclude.iter().any(|ex| point_in_polygon(centroid, ex)) {
                    continue;
                }
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
        self.volume_to_elevation_bounded(base_elev, None, &[])
    }

    /// Calculates volume with optional inclusion/exclusion boundaries.
    pub fn volume_to_elevation_bounded(
        &self,
        base_elev: f64,
        include: Option<&[Point]>,
        exclude: &[Vec<Point>],
    ) -> f64 {
        let mut volume = 0.0;
        for tri in &self.triangles {
            let a = self.vertices[tri[0]];
            let b = self.vertices[tri[1]];
            let c = self.vertices[tri[2]];
            let centroid = Point::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0);
            if let Some(poly) = include {
                if !point_in_polygon(centroid, poly) {
                    continue;
                }
            }
            if exclude.iter().any(|ex| point_in_polygon(centroid, ex)) {
                continue;
            }
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

    /// Calculates the net volume difference between two TIN surfaces using the
    /// lowest elevation of both as the base plane. Positive values indicate the
    /// `self` surface lies above `other` on average.
    pub fn volume_between(&self, other: &Tin) -> f64 {
        let min_self = self
            .vertices
            .iter()
            .map(|p| p.z)
            .fold(f64::INFINITY, f64::min);
        let min_other = other
            .vertices
            .iter()
            .map(|p| p.z)
            .fold(f64::INFINITY, f64::min);
        let base = min_self.min(min_other);
        self.volume_to_elevation(base) - other.volume_to_elevation(base)
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

    #[test]
    #[ignore]
    fn tin_from_points_constrained_breakline() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.5, 0.5, 0.0),
        ];
        let boundary = vec![0usize, 1, 2, 3];
        let breaklines = vec![(0usize, 2usize)];
        let tin = Tin::from_points_constrained(pts, Some(&breaklines), Some(&boundary));
        assert!(tin.triangles.iter().any(|t| t.contains(&0) && t.contains(&2)));
    }

    #[test]
    #[ignore]
    fn volume_with_bounds() {
        let pts = vec![
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let tin = Tin::from_points(pts);
        let include = vec![
            Point::new(0.0, 0.0),
            Point::new(0.5, 0.0),
            Point::new(0.5, 0.5),
            Point::new(0.0, 0.5),
        ];
        let vol = tin.volume_to_elevation_bounded(0.0, Some(&include), &[]);
        assert!((vol - 0.25).abs() < 1e-6);
    }

    #[test]
    fn volume_between_surfaces_flat() {
        let design_pts = vec![
            Point3::new(0.0, -1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
            Point3::new(10.0, -1.0, 1.0),
            Point3::new(10.0, 1.0, 1.0),
        ];
        let ground_pts = vec![
            Point3::new(0.0, -1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(10.0, -1.0, 0.0),
            Point3::new(10.0, 1.0, 0.0),
        ];
        let design = Tin::from_points(design_pts);
        let ground = Tin::from_points(ground_pts);
        let vol = design.volume_between(&ground);
        assert!((vol - 20.0).abs() < 1e-6);
    }
}
