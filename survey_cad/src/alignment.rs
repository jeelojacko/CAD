use crate::geometry::{Point, Point3, Polyline};

/// Horizontal alignment represented by a polyline.
#[derive(Debug, Clone)]
pub struct HorizontalAlignment {
    pub centerline: Polyline,
}

impl HorizontalAlignment {
    /// Creates a new horizontal alignment from vertices.
    pub fn new(vertices: Vec<Point>) -> Self {
        Self {
            centerline: Polyline::new(vertices),
        }
    }

    /// Total length of the alignment.
    pub fn length(&self) -> f64 {
        self.centerline.length()
    }

    /// Returns the position at the given station along the alignment.
    pub fn point_at(&self, station: f64) -> Option<Point> {
        if station < 0.0 || station > self.length() {
            return None;
        }
        let mut remaining = station;
        let verts = &self.centerline.vertices;
        for seg in verts.windows(2) {
            let seg_len = crate::geometry::distance(seg[0], seg[1]);
            if remaining <= seg_len {
                let t = remaining / seg_len;
                return Some(Point::new(
                    seg[0].x + t * (seg[1].x - seg[0].x),
                    seg[0].y + t * (seg[1].y - seg[0].y),
                ));
            }
            remaining -= seg_len;
        }
        verts.last().copied()
    }

    /// Returns a unit tangent vector at the given station.
    pub fn direction_at(&self, station: f64) -> Option<(f64, f64)> {
        if station < 0.0 || station > self.length() {
            return None;
        }
        let mut remaining = station;
        let verts = &self.centerline.vertices;
        for seg in verts.windows(2) {
            let seg_len = crate::geometry::distance(seg[0], seg[1]);
            if remaining <= seg_len {
                let dx = seg[1].x - seg[0].x;
                let dy = seg[1].y - seg[0].y;
                let len = (dx * dx + dy * dy).sqrt();
                return Some((dx / len, dy / len));
            }
            remaining -= seg_len;
        }
        None
    }
}

/// Vertical alignment defined by station/elevation pairs.
#[derive(Debug, Clone)]
pub struct VerticalAlignment {
    pub points: Vec<(f64, f64)>,
}

impl VerticalAlignment {
    /// Creates a new vertical alignment.
    pub fn new(points: Vec<(f64, f64)>) -> Self {
        Self { points }
    }

    /// Elevation at the given station using linear interpolation.
    pub fn elevation_at(&self, station: f64) -> Option<f64> {
        if self.points.is_empty() {
            return None;
        }
        if station <= self.points[0].0 {
            return Some(self.points[0].1);
        }
        for pair in self.points.windows(2) {
            if station >= pair[0].0 && station <= pair[1].0 {
                let t = (station - pair[0].0) / (pair[1].0 - pair[0].0);
                return Some(pair[0].1 + t * (pair[1].1 - pair[0].1));
            }
        }
        self.points.last().map(|p| p.1)
    }
}

/// Combined horizontal and vertical alignment.
#[derive(Debug, Clone)]
pub struct Alignment {
    pub horizontal: HorizontalAlignment,
    pub vertical: VerticalAlignment,
}

impl Alignment {
    pub fn new(horizontal: HorizontalAlignment, vertical: VerticalAlignment) -> Self {
        Self { horizontal, vertical }
    }

    /// Returns the 3D point on the alignment at the specified station.
    pub fn point3_at(&self, station: f64) -> Option<Point3> {
        let p = self.horizontal.point_at(station)?;
        let z = self.vertical.elevation_at(station)?;
        Some(Point3::new(p.x, p.y, z))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    #[test]
    fn point_and_elevation() {
        let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
        let valign = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 5.0)]);
        let align = Alignment::new(halign, valign);
        let p = align.point3_at(5.0).unwrap();
        assert!((p.x - 5.0).abs() < 1e-6);
        assert!((p.y - 0.0).abs() < 1e-6);
        assert!((p.z - 2.5).abs() < 1e-6);
    }
}
