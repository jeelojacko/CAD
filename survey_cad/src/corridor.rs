use crate::alignment::Alignment;
use crate::dtm::Tin;
use crate::geometry::{Point, Point3};

/// 3D cross-section sampled at a station along a corridor.
#[derive(Debug, Clone)]
pub struct CrossSection {
    pub station: f64,
    pub points: Vec<Point3>,
}

impl CrossSection {
    pub fn new(station: f64, points: Vec<Point3>) -> Self {
        Self { station, points }
    }
}

impl Tin {
    /// Returns the interpolated elevation at (x,y) if the point lies within the TIN.
    pub fn elevation_at(&self, x: f64, y: f64) -> Option<f64> {
        for tri in &self.triangles {
            let a = self.vertices[tri[0]];
            let b = self.vertices[tri[1]];
            let c = self.vertices[tri[2]];
            if let Some((u, v, w)) = barycentric(Point::new(x, y), a, b, c) {
                if u >= 0.0 && v >= 0.0 && w >= 0.0 {
                    return Some(u * a.z + v * b.z + w * c.z);
                }
            }
        }
        None
    }
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

/// Generates cross-sections along an alignment using a ground TIN.
pub fn extract_cross_sections(
    tin: &Tin,
    alignment: &Alignment,
    width: f64,
    interval: f64,
    offset_step: f64,
) -> Vec<CrossSection> {
    let mut sections = Vec::new();
    let length = alignment.horizontal.length();
    let mut station = 0.0;
    while station <= length {
        if let Some(center) = alignment.horizontal.point_at(station) {
            if let Some(dir) = alignment.horizontal.direction_at(station) {
                let normal = (-dir.1, dir.0);
                let mut pts = Vec::new();
                let mut offset = -width;
                while offset <= width {
                    let x = center.x + offset * normal.0;
                    let y = center.y + offset * normal.1;
                    if let Some(z) = tin.elevation_at(x, y) {
                        pts.push(Point3::new(x, y, z));
                    }
                    offset += offset_step;
                }
                sections.push(CrossSection::new(station, pts));
            }
        }
        station += interval;
    }
    sections
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::alignment::{HorizontalAlignment, VerticalAlignment, Alignment};
    use crate::geometry::{Point, Point3};

    #[test]
    fn flat_cross_sections() {
        // flat TIN at elevation 0
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
        ];
        let tin = Tin::from_points(pts);
        let halign = HorizontalAlignment::new(vec![Point::new(0.0, 5.0), Point::new(10.0, 5.0)]);
        let valign = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
        let align = Alignment::new(halign, valign);
        let sections = extract_cross_sections(&tin, &align, 5.0, 5.0, 2.5);
        assert_eq!(sections.len(), 3);
        for sec in sections {
            assert_eq!(sec.points.len(), 5);
            for p in sec.points {
                assert!((p.z - 0.0).abs() < 1e-6);
            }
        }
    }
}
