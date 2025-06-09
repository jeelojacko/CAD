use crate::geometry::{polygon_area, Point, Point3, Polyline};

/// Classification for breaklines when building constrained TINs.
#[derive(Debug, Clone, Copy)]
pub enum BreaklineKind {
    /// Hard breaklines enforce triangle edges exactly.
    Hard,
    /// Soft breaklines are used only for smoothing.
    Soft,
}

/// Breakline with an associated classification.
#[derive(Debug, Clone, Copy)]
pub struct ClassifiedBreakline {
    pub start: usize,
    pub end: usize,
    pub kind: BreaklineKind,
}

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

fn subtract(a: Point3, b: Point3) -> Point3 {
    Point3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn cross(a: Point3, b: Point3) -> Point3 {
    Point3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}

fn point_on_segment(a: Point3, b: Point3, p: Point3, tol: f64) -> bool {
    let ab = subtract(b, a);
    let ap = subtract(p, a);
    let cross = cross(ab, ap);
    if cross.x.abs() > tol || cross.y.abs() > tol || cross.z.abs() > tol {
        return false;
    }
    let dot = (ap.x * ab.x + ap.y * ab.y + ap.z * ab.z) / (ab.x * ab.x + ab.y * ab.y + ab.z * ab.z);
    dot >= 0.0 - tol && dot <= 1.0 + tol
}

fn refine_edges_for_points(points: &[Point3], edges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut refined = Vec::new();
    for &(a, b) in edges {
        let pa = points[a];
        let pb = points[b];
        let mut mids: Vec<(usize, f64)> = Vec::new();
        for (i, &p) in points.iter().enumerate() {
            if i == a || i == b {
                continue;
            }
            if point_on_segment(pa, pb, p, 1e-6) {
                let t = ((p.x - pa.x).powi(2) + (p.y - pa.y).powi(2) + (p.z - pa.z).powi(2)).sqrt();
                mids.push((i, t));
            }
        }
        mids.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
        let mut last = a;
        for (idx, _) in mids {
            refined.push((last, idx));
            last = idx;
        }
        refined.push((last, b));
        if !mids.is_empty() {
            refined.push((a, b));
        }
    }
    refined.sort_unstable();
    refined.dedup();
    refined
}

fn edge_slope(p: Point3, q: Point3) -> f64 {
    let dx = p.x - q.x;
    let dy = p.y - q.y;
    let horiz = (dx * dx + dy * dy).sqrt();
    if horiz <= f64::EPSILON {
        90.0
    } else {
        ((p.z - q.z).abs() / horiz).atan().to_degrees()
    }
}

fn triangle_slope_deg(a: Point3, b: Point3, c: Point3) -> f64 {
    edge_slope(a, b).max(edge_slope(a, c)).max(edge_slope(b, c))
}

fn barycentric(p: Point, a: Point3, b: Point3, c: Point3) -> Option<(f64, f64, f64)> {
    let det = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if det.abs() < f64::EPSILON {
        return None;
    }
    let u = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / det;
    let v = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / det;
    let w = 1.0 - u - v;
    Some((u, v, w))
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

        if !edges.is_empty() {
            edges = refine_edges_for_points(&points, &edges);
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

    /// Builds a constrained TIN with optional breaklines, outer boundary and
    /// interior hole boundaries. Holes are provided as a slice of index
    /// polygons. Each hole polygon should be closed (first and last index may
    /// repeat or will be closed automatically).
    pub fn from_points_constrained_with_holes(
        points: Vec<Point3>,
        breaklines: Option<&[(usize, usize)]>,
        outer_boundary: Option<&[usize]>,
        holes: &[Vec<usize>],
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
        for hole in holes {
            if hole.len() > 1 {
                for w in hole.windows(2) {
                    edges.push((w[0], w[1]));
                }
                edges.push((*hole.last().unwrap(), hole[0]));
            }
        }

        if !edges.is_empty() {
            edges = refine_edges_for_points(&points, &edges);
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

    /// Returns a new TIN with the same vertices but enforcing the provided
    /// breaklines.
    pub fn with_breaklines(&self, breaklines: &[(usize, usize)]) -> Self {
        Tin::from_points_constrained(self.vertices.clone(), Some(breaklines), None)
    }

    /// Builds a constrained TIN using classified breaklines. Only hard
    /// breaklines are enforced; soft breaklines are ignored when
    /// constructing the triangulation.
    pub fn from_points_classified(
        points: Vec<Point3>,
        breaklines: &[ClassifiedBreakline],
        outer_boundary: Option<&[usize]>,
        holes: &[Vec<usize>],
    ) -> Self {
        let hard: Vec<(usize, usize)> = breaklines
            .iter()
            .filter(|b| matches!(b.kind, BreaklineKind::Hard))
            .map(|b| (b.start, b.end))
            .collect();
        Tin::from_points_constrained_with_holes(points, Some(&hard), outer_boundary, holes)
    }

    /// Returns a new TIN with an updated outer boundary.
    pub fn with_boundary(&self, boundary: &[usize]) -> Self {
        Tin::from_points_constrained(self.vertices.clone(), None, Some(boundary))
    }

    /// Returns a new TIN with interior hole boundaries applied.
    pub fn with_holes(&self, holes: &[Vec<usize>]) -> Self {
        Tin::from_points_constrained_with_holes(self.vertices.clone(), None, None, holes)
    }

    /// Merges this TIN with `other` using the provided tolerance. Vertices
    /// from `other` that are within `tolerance` of existing vertices are
    /// discarded. The resulting surface is rebuilt from all kept points.
    pub fn merge_with(&self, other: &Tin, tolerance: f64) -> Self {
        let mut points = self.vertices.clone();
        for v in &other.vertices {
            if !points.iter().any(|p| {
                (p.x - v.x).hypot(p.y - v.y) <= tolerance && (p.z - v.z).abs() <= tolerance
            }) {
                points.push(*v);
            }
        }
        Tin::from_points(points)
    }

    /// Smooths the surface elevations using simple Laplacian smoothing.
    /// Only vertex Z values are modified. Boundaries are not preserved.
    pub fn smooth(&self, iterations: usize) -> Self {
        if iterations == 0 {
            return self.clone();
        }
        let mut verts = self.vertices.clone();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); verts.len()];
        for tri in &self.triangles {
            for &a in tri.iter() {
                for &b in tri.iter() {
                    if a != b && !adj[a].contains(&b) {
                        adj[a].push(b);
                    }
                }
            }
        }
        for _ in 0..iterations {
            let mut new_z: Vec<f64> = verts.iter().map(|v| v.z).collect();
            for i in 0..verts.len() {
                if adj[i].is_empty() {
                    continue;
                }
                let sum: f64 = adj[i].iter().map(|&j| verts[j].z).sum();
                new_z[i] = sum / adj[i].len() as f64;
            }
            for (v, z) in verts.iter_mut().zip(new_z) {
                v.z = z;
            }
        }
        Self {
            vertices: verts,
            triangles: self.triangles.clone(),
        }
    }

    /// Returns the slope in degrees for each triangle in the TIN.
    pub fn triangle_slopes(&self) -> Vec<f64> {
        self.triangles
            .iter()
            .map(|t| {
                triangle_slope_deg(
                    self.vertices[t[0]],
                    self.vertices[t[1]],
                    self.vertices[t[2]],
                )
            })
            .collect()
    }

    /// Returns the slope at (x, y) if the point lies within the TIN.
    pub fn slope_at(&self, x: f64, y: f64) -> Option<f64> {
        for tri in &self.triangles {
            let a = self.vertices[tri[0]];
            let b = self.vertices[tri[1]];
            let c = self.vertices[tri[2]];
            if let Some((u, v, w)) = barycentric(Point::new(x, y), a, b, c) {
                if u >= 0.0 && v >= 0.0 && w >= 0.0 {
                    return Some(triangle_slope_deg(a, b, c));
                }
            }
        }
        None
    }

    /// Returns the elevation difference between this surface and `other` at
    /// the provided XY location if both surfaces contain the point.
    pub fn elevation_difference_at(&self, other: &Tin, x: f64, y: f64) -> Option<f64> {
        let a = self.elevation_at(x, y)?;
        let b = other.elevation_at(x, y)?;
        Some(a - b)
    }

    /// Projects a constant slope from `start` along `dir` onto the surface.
    /// `slope` is vertical change per unit horizontal distance. The search
    /// progresses in `step` increments up to `max_dist` and returns the
    /// daylight point when the projected grade intersects the surface.
    pub fn slope_projection(
        &self,
        start: Point3,
        dir: (f64, f64),
        slope: f64,
        step: f64,
        max_dist: f64,
    ) -> Option<Point3> {
        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
        if len <= f64::EPSILON || step <= 0.0 {
            return None;
        }
        let dir = (dir.0 / len, dir.1 / len);
        let mut dist = 0.0;
        let mut prev = start.z - self.elevation_at(start.x, start.y)?;
        while dist <= max_dist {
            let x = start.x + dir.0 * dist;
            let y = start.y + dir.1 * dist;
            if let Some(ground) = self.elevation_at(x, y) {
                let design = start.z + slope * dist;
                let diff = design - ground;
                if diff.abs() < 1e-3 {
                    return Some(Point3::new(x, y, ground));
                }
                if diff.signum() != prev.signum() {
                    let t = prev / (prev - diff);
                    let xi = x - dir.0 * step * (1.0 - t);
                    let yi = y - dir.1 * step * (1.0 - t);
                    if let Some(z) = self.elevation_at(xi, yi) {
                        return Some(Point3::new(xi, yi, z));
                    }
                    return None;
                }
                prev = diff;
            } else {
                return None;
            }
            dist += step;
        }
        None
    }

    /// Generates a daylight line polyline from `start` along `dir` with the
    /// provided `slope`. Points are spaced at `step` intervals until the
    /// projected grade meets the surface or `max_dist` is exceeded.
    pub fn daylight_line(
        &self,
        start: Point3,
        dir: (f64, f64),
        slope: f64,
        step: f64,
        max_dist: f64,
    ) -> Vec<Point3> {
        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
        if len <= f64::EPSILON || step <= 0.0 {
            return vec![start];
        }
        let dir = (dir.0 / len, dir.1 / len);
        let mut pts = vec![start];
        let mut dist = step;
        let mut prev = start.z - self.elevation_at(start.x, start.y).unwrap_or(start.z);
        while dist <= max_dist {
            let x = start.x + dir.0 * dist;
            let y = start.y + dir.1 * dist;
            let z = start.z + slope * dist;
            pts.push(Point3::new(x, y, z));
            if let Some(ground) = self.elevation_at(x, y) {
                let diff = z - ground;
                if diff.signum() != prev.signum() {
                    if let Some(p) = self.slope_projection(start, dir, slope, step, dist) {
                        pts.push(p);
                    }
                    break;
                }
                prev = diff;
            } else {
                break;
            }
            dist += step;
        }
        pts
    }

    /// Generates contour line segments at the specified interval. Optional
    /// `include` and `exclude` polygons can limit where contours are created.
    pub fn contour_segments(&self, interval: f64) -> Vec<(Point3, Point3)> {
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

    /// Generates contour polylines at the specified interval. `smooth` controls
    /// the number of Chaikin smoothing iterations applied to each contour.
    pub fn contour_polylines(&self, interval: f64, smooth: usize) -> (Vec<Polyline>, Vec<Vec<Point3>>) {
        let segs = self.contour_segments(interval);
        let lines3 = segments_to_polylines(&segs, 1e-8);
        let mut lines2d = Vec::new();
        for pts3 in &lines3 {
            let pts: Vec<Point> = pts3.iter().map(|p| Point::new(p.x, p.y)).collect();
            let pl = Polyline::new(pts).smooth(smooth);
            lines2d.push(pl);
        }
        (lines2d, lines3)
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

    /// Calculates the volume between two TIN surfaces using the prismoidal
    /// method. The calculation is symmetric with respect to the two surfaces
    /// to reduce bias from differing triangulations. Only areas where both
    /// surfaces contain data contribute to the result. Positive values indicate
    /// that `self` lies above `other` on average.
    pub fn prismoidal_volume_between(&self, other: &Tin) -> f64 {
        fn volume_from(a: &Tin, b: &Tin) -> f64 {
            let mut vol = 0.0;
            for tri in &a.triangles {
                let a0 = a.vertices[tri[0]];
                let a1 = a.vertices[tri[1]];
                let a2 = a.vertices[tri[2]];
                if let (Some(b0), Some(b1), Some(b2)) = (
                    b.elevation_at(a0.x, a0.y),
                    b.elevation_at(a1.x, a1.y),
                    b.elevation_at(a2.x, a2.y),
                ) {
                    let dz0 = a0.z - b0;
                    let dz1 = a1.z - b1;
                    let dz2 = a2.z - b2;
                    let area = polygon_area(&[
                        Point::new(a0.x, a0.y),
                        Point::new(a1.x, a1.y),
                        Point::new(a2.x, a2.y),
                    ]);
                    vol += area * (dz0 + dz1 + dz2) / 3.0;
                }
            }
            vol
        }

        let v_ab = volume_from(self, other);
        let v_ba = volume_from(other, self);
        (v_ab - v_ba) / 2.0
    }

    /// Returns the cut and fill volumes between two TIN surfaces using a
    /// symmetric prismoidal calculation. The result is a tuple `(cut, fill)`
    /// where `cut` is the volume where `self` lies below `other` and `fill`
    /// is where `self` is above `other`.
    pub fn cut_fill_between(&self, other: &Tin) -> (f64, f64) {
        fn cut_fill_from(a: &Tin, b: &Tin) -> (f64, f64) {
            let mut cut = 0.0;
            let mut fill = 0.0;
            for tri in &a.triangles {
                let a0 = a.vertices[tri[0]];
                let a1 = a.vertices[tri[1]];
                let a2 = a.vertices[tri[2]];
                if let (Some(b0), Some(b1), Some(b2)) = (
                    b.elevation_at(a0.x, a0.y),
                    b.elevation_at(a1.x, a1.y),
                    b.elevation_at(a2.x, a2.y),
                ) {
                    let dz0 = a0.z - b0;
                    let dz1 = a1.z - b1;
                    let dz2 = a2.z - b2;
                    let area = polygon_area(&[
                        Point::new(a0.x, a0.y),
                        Point::new(a1.x, a1.y),
                        Point::new(a2.x, a2.y),
                    ]);
                    let avg = (dz0 + dz1 + dz2) / 3.0;
                    if avg > 0.0 {
                        fill += area * avg;
                    } else {
                        cut += area * -avg;
                    }
                }
            }
            (cut, fill)
        }

        let (cut_ab, fill_ab) = cut_fill_from(self, other);
        let (cut_ba, fill_ba) = cut_fill_from(other, self);
        let cut = (cut_ab + fill_ba) / 2.0;
        let fill = (fill_ab + cut_ba) / 2.0;
        (cut, fill)
    }
}

/// Surface that automatically rebuilds when its points are modified.
#[derive(Debug, Clone)]
pub struct DynamicTin {
    pub points: Vec<Point3>,
    pub breaklines: Vec<(usize, usize)>,
    pub boundary: Option<Vec<usize>>,
    pub holes: Vec<Vec<usize>>,
    pub tin: Tin,
}

/// Container for working with multiple TIN surfaces.
#[derive(Debug, Default, Clone)]
pub struct TinManager {
    pub tins: Vec<Tin>,
}

impl TinManager {
    /// Adds a new TIN to the manager.
    pub fn add(&mut self, tin: Tin) {
        self.tins.push(tin);
    }

    /// Removes a TIN by index if it exists.
    pub fn remove(&mut self, index: usize) {
        if index < self.tins.len() {
            self.tins.remove(index);
        }
    }

    /// Returns a reference to a TIN by index.
    pub fn get(&self, index: usize) -> Option<&Tin> {
        self.tins.get(index)
    }

    /// Returns the number of managed TINs.
    pub fn len(&self) -> usize {
        self.tins.len()
    }

    /// Returns `true` if no TINs are managed.
    pub fn is_empty(&self) -> bool {
        self.tins.is_empty()
    }
}

impl DynamicTin {
    /// Creates a new dynamic surface from points.
    pub fn new(points: Vec<Point3>) -> Self {
        let tin = Tin::from_points(points.clone());
        Self {
            points,
            breaklines: Vec::new(),
            boundary: None,
            holes: Vec::new(),
            tin,
        }
    }

    /// Rebuilds the internal TIN using current points and constraints.
    pub fn rebuild(&mut self) {
        self.tin = Tin::from_points_constrained_with_holes(
            self.points.clone(),
            Some(&self.breaklines),
            self.boundary.as_deref(),
            &self.holes,
        );
    }

    /// Updates a single point and rebuilds the surface.
    pub fn update_point(&mut self, index: usize, point: Point3) {
        if let Some(p) = self.points.get_mut(index) {
            *p = point;
            self.rebuild();
        }
    }

    /// Adds a breakline and rebuilds the surface.
    pub fn add_breakline(&mut self, start: usize, end: usize) {
        if !self
            .breaklines
            .iter()
            .any(|&(a, b)| (a == start && b == end) || (a == end && b == start))
        {
            self.breaklines.push((start, end));
            self.rebuild();
        }
    }

    /// Returns a reference to the underlying TIN.
    pub fn tin(&self) -> &Tin {
        &self.tin
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

fn points_close(a: Point3, b: Point3, tol: f64) -> bool {
    (a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol && (a.z - b.z).abs() <= tol
}

fn segments_to_polylines(segs: &[(Point3, Point3)], tol: f64) -> Vec<Vec<Point3>> {
    let mut remaining: Vec<(Point3, Point3)> = segs.to_vec();
    let mut out = Vec::new();
    while let Some((a, b)) = remaining.pop() {
        let mut line = vec![a, b];
        let mut extended = true;
        while extended {
            extended = false;
            let last = *line.last().unwrap();
            for i in 0..remaining.len() {
                let seg = remaining[i];
                if points_close(seg.0, last, tol) {
                    line.push(seg.1);
                    remaining.swap_remove(i);
                    extended = true;
                    break;
                } else if points_close(seg.1, last, tol) {
                    line.push(seg.0);
                    remaining.swap_remove(i);
                    extended = true;
                    break;
                }
            }
        }
        out.push(line);
    }
    out
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
        assert!(tin
            .triangles
            .iter()
            .any(|t| t.contains(&0) && t.contains(&2)));
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

    #[test]
    fn prismoidal_volume_between_flat() {
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
        let vol = design.prismoidal_volume_between(&ground);
        assert!((vol - 20.0).abs() < 1e-6);
    }

    #[test]
    fn prismoidal_volume_identical_zero() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let a = Tin::from_points(pts.clone());
        let b = Tin::from_points(pts);
        let vol = a.prismoidal_volume_between(&b);
        assert!(vol.abs() < 1e-6);
    }

    #[test]
    fn slope_analysis_basic() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let tin = Tin::from_points(pts);
        let slopes = tin.triangle_slopes();
        assert_eq!(slopes.len(), 1);
        assert!((slopes[0] - 45.0).abs() < 1e-6);
        let s = tin.slope_at(0.25, 0.25).unwrap();
        assert!((s - 45.0).abs() < 1e-6);
    }

    #[test]
    fn surface_difference_simple() {
        let a = Tin::from_points(vec![
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ]);
        let b = Tin::from_points(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ]);
        let diff = a.elevation_difference_at(&b, 0.1, 0.1).unwrap();
        assert!((diff - 1.0).abs() < 1e-6);
    }

    #[test]
    fn breakline_editing() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.5, 0.5, 0.0),
        ];
        let tin = Tin::from_points(pts.clone());
        let edited = tin.with_breaklines(&[(0usize, 2usize)]);
        assert!(edited
            .triangles
            .iter()
            .any(|t| t.contains(&0) && t.contains(&2)));
    }

    #[test]
    fn slope_projection_simple() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
        ];
        let tin = Tin::from_points(pts);
        let start = Point3::new(5.0, 5.0, 5.0);
        let p = tin
            .slope_projection(start, (1.0, 0.0), -1.0, 1.0, 10.0)
            .unwrap();
        assert!((p.x - 10.0).abs() < 1.0e-6);
    }

    #[test]
    fn tin_smoothing() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 10.0),
        ];
        let tin = Tin::from_points(pts);
        let smoothed = tin.smooth(1);
        assert!(smoothed.vertices[3].z < 10.0);
    }

    #[test]
    fn classified_breaklines_ignore_soft() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.5, 0.5, 0.0),
        ];
        let breaklines = vec![
            ClassifiedBreakline {
                start: 0,
                end: 2,
                kind: BreaklineKind::Hard,
            },
            ClassifiedBreakline {
                start: 1,
                end: 3,
                kind: BreaklineKind::Soft,
            },
        ];
        let tin = Tin::from_points_classified(pts, &breaklines, None, &[]);
        assert!(tin
            .triangles
            .iter()
            .any(|t| t.contains(&0) && t.contains(&2)));
    }

    #[test]
    fn dynamic_tin_updates() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let mut dtin = DynamicTin::new(pts);
        let z_before = dtin.tin.vertices[0].z;
        dtin.update_point(0, Point3::new(0.0, 0.0, 5.0));
        let z_after = dtin.tin.vertices[0].z;
        assert!(z_after - z_before > 4.9);
    }

    #[test]
    fn merge_tins_with_tolerance() {
        let a = Tin::from_points(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ]);
        let b = Tin::from_points(vec![
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ]);
        let merged = a.merge_with(&b, 0.01);
        assert!(merged.vertices.len() < a.vertices.len() + b.vertices.len());
    }

    #[test]
    fn contour_polylines_basic() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let tin = Tin::from_points(pts);
        let (lines, _z) = tin.contour_polylines(0.5, 0);
        assert!(!lines.is_empty());
    }

    #[test]
    fn cut_fill_between_flat() {
        let design_pts = vec![
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let ground_pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let design = Tin::from_points(design_pts);
        let ground = Tin::from_points(ground_pts);
        let (cut, fill) = design.cut_fill_between(&ground);
        assert!(cut.abs() < 1e-6);
        assert!((fill - 1.0).abs() < 1e-6);
    }
}
