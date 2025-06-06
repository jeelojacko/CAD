use crate::geometry::{distance, Arc, Point, Point3, Polyline};

/// Individual elements of a horizontal alignment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HorizontalElement {
    /// Straight tangent between two points.
    Tangent { start: Point, end: Point },
    /// Circular curve described by an [`Arc`].
    Curve { arc: Arc },
    /// Spiral approximated by a polyline.
    Spiral { polyline: Polyline },
}

impl HorizontalElement {
    fn length(&self) -> f64 {
        match self {
            HorizontalElement::Tangent { start, end } => distance(*start, *end),
            HorizontalElement::Curve { arc } => arc.length(),
            HorizontalElement::Spiral { polyline } => polyline.length(),
        }
    }

    fn start_point(&self) -> Point {
        match self {
            HorizontalElement::Tangent { start, .. } => *start,
            HorizontalElement::Curve { arc } => Point::new(
                arc.center.x + arc.radius * arc.start_angle.cos(),
                arc.center.y + arc.radius * arc.start_angle.sin(),
            ),
            HorizontalElement::Spiral { polyline } => polyline.vertices.first().copied().unwrap(),
        }
    }

    fn end_point(&self) -> Point {
        match self {
            HorizontalElement::Tangent { end, .. } => *end,
            HorizontalElement::Curve { arc } => Point::new(
                arc.center.x + arc.radius * arc.end_angle.cos(),
                arc.center.y + arc.radius * arc.end_angle.sin(),
            ),
            HorizontalElement::Spiral { polyline } => polyline.vertices.last().copied().unwrap(),
        }
    }

    fn point_at(&self, s: f64) -> Point {
        match self {
            HorizontalElement::Tangent { start, end } => {
                let len = distance(*start, *end);
                let t = if len.abs() < f64::EPSILON {
                    0.0
                } else {
                    s / len
                };
                Point::new(
                    start.x + t * (end.x - start.x),
                    start.y + t * (end.y - start.y),
                )
            }
            HorizontalElement::Curve { arc } => {
                let dir = if arc.end_angle >= arc.start_angle {
                    1.0
                } else {
                    -1.0
                };
                let sweep = s / arc.radius * dir;
                let ang = arc.start_angle + sweep;
                Point::new(
                    arc.center.x + arc.radius * ang.cos(),
                    arc.center.y + arc.radius * ang.sin(),
                )
            }
            HorizontalElement::Spiral { polyline } => {
                // treat as polyline
                let verts = &polyline.vertices;
                let mut rem = s;
                for seg in verts.windows(2) {
                    let seg_len = distance(seg[0], seg[1]);
                    if rem <= seg_len {
                        let t = if seg_len.abs() < f64::EPSILON {
                            0.0
                        } else {
                            rem / seg_len
                        };
                        return Point::new(
                            seg[0].x + t * (seg[1].x - seg[0].x),
                            seg[0].y + t * (seg[1].y - seg[0].y),
                        );
                    }
                    rem -= seg_len;
                }
                *verts.last().unwrap()
            }
        }
    }

    fn direction_at(&self, s: f64) -> (f64, f64) {
        match self {
            HorizontalElement::Tangent { start, end } => {
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len.abs() < f64::EPSILON {
                    (0.0, 0.0)
                } else {
                    (dx / len, dy / len)
                }
            }
            HorizontalElement::Curve { arc } => {
                let dir = if arc.end_angle >= arc.start_angle {
                    1.0
                } else {
                    -1.0
                };
                let ang = arc.start_angle + s / arc.radius * dir;
                let tangent = ang + dir * std::f64::consts::FRAC_PI_2;
                (tangent.cos(), tangent.sin())
            }
            HorizontalElement::Spiral { polyline } => {
                // approximate using local segment
                let verts = &polyline.vertices;
                let mut rem = s;
                for seg in verts.windows(2) {
                    let seg_len = distance(seg[0], seg[1]);
                    if rem <= seg_len {
                        let dx = seg[1].x - seg[0].x;
                        let dy = seg[1].y - seg[0].y;
                        let len = (dx * dx + dy * dy).sqrt();
                        return if len.abs() < f64::EPSILON {
                            (0.0, 0.0)
                        } else {
                            (dx / len, dy / len)
                        };
                    }
                    rem -= seg_len;
                }
                // fallback to last segment direction
                if let Some(seg) = verts.windows(2).last() {
                    let dx = seg[1].x - seg[0].x;
                    let dy = seg[1].y - seg[0].y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len.abs() < f64::EPSILON {
                        (0.0, 0.0)
                    } else {
                        (dx / len, dy / len)
                    }
                } else {
                    (0.0, 0.0)
                }
            }
        }
    }
}

/// Horizontal alignment consisting of tangent, curve and spiral elements.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HorizontalAlignment {
    pub elements: Vec<HorizontalElement>,
}

impl HorizontalAlignment {
    /// Creates a new horizontal alignment from vertices interpreted as tangent segments.
    pub fn new(vertices: Vec<Point>) -> Self {
        let mut elements = Vec::new();
        for pair in vertices.windows(2) {
            elements.push(HorizontalElement::Tangent {
                start: pair[0],
                end: pair[1],
            });
        }
        Self { elements }
    }

    /// Total length of the alignment.
    pub fn length(&self) -> f64 {
        self.elements.iter().map(|e| e.length()).sum()
    }

    /// Returns the position at the given station along the alignment.
    pub fn point_at(&self, station: f64) -> Option<Point> {
        if station < 0.0 || station > self.length() {
            return None;
        }
        let mut remaining = station;
        for elem in &self.elements {
            let len = elem.length();
            if remaining <= len {
                return Some(elem.point_at(remaining));
            }
            remaining -= len;
        }
        self.elements.last().map(|e| e.end_point())
    }

    /// Returns a unit tangent vector at the given station.
    pub fn direction_at(&self, station: f64) -> Option<(f64, f64)> {
        if station < 0.0 || station > self.length() {
            return None;
        }
        let mut remaining = station;
        for elem in &self.elements {
            let len = elem.length();
            if remaining <= len {
                return Some(elem.direction_at(remaining));
            }
            remaining -= len;
        }
        None
    }
}

/// Types of vertical alignment elements.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum VerticalElement {
    /// Straight grade between two stations.
    Grade {
        start_station: f64,
        end_station: f64,
        start_elev: f64,
        end_elev: f64,
    },
    /// Simple parabolic vertical curve.
    Parabola {
        start_station: f64,
        end_station: f64,
        start_elev: f64,
        start_grade: f64,
        end_grade: f64,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerticalAlignment {
    pub elements: Vec<VerticalElement>,
}

impl VerticalAlignment {
    /// Creates a new vertical alignment using linear grade segments defined by (station, elevation) pairs.
    pub fn new(points: Vec<(f64, f64)>) -> Self {
        let mut elements = Vec::new();
        for pair in points.windows(2) {
            elements.push(VerticalElement::Grade {
                start_station: pair[0].0,
                end_station: pair[1].0,
                start_elev: pair[0].1,
                end_elev: pair[1].1,
            });
        }
        Self { elements }
    }

    /// Elevation at the given station evaluating grades and parabolic curves.
    pub fn elevation_at(&self, station: f64) -> Option<f64> {
        if self.elements.is_empty() {
            return None;
        }
        if let Some(first) = self.elements.first() {
            match first {
                VerticalElement::Grade {
                    start_station,
                    start_elev,
                    ..
                }
                | VerticalElement::Parabola {
                    start_station,
                    start_elev,
                    ..
                } => {
                    if station <= *start_station {
                        return Some(*start_elev);
                    }
                }
            }
        }

        for elem in &self.elements {
            match elem {
                VerticalElement::Grade {
                    start_station,
                    end_station,
                    start_elev,
                    end_elev,
                } => {
                    if station >= *start_station && station <= *end_station {
                        let t = (station - start_station) / (end_station - start_station);
                        return Some(start_elev + t * (end_elev - start_elev));
                    }
                }
                VerticalElement::Parabola {
                    start_station,
                    end_station,
                    start_elev,
                    start_grade,
                    end_grade,
                } => {
                    if station >= *start_station && station <= *end_station {
                        let l = end_station - start_station;
                        let x = station - start_station;
                        let g1 = *start_grade;
                        let g2 = *end_grade;
                        let dz = g1 * x + 0.5 * (g2 - g1) / l * x * x;
                        return Some(start_elev + dz);
                    }
                }
            }
        }
        match self.elements.last() {
            Some(VerticalElement::Grade { end_elev, .. }) => Some(*end_elev),
            Some(VerticalElement::Parabola {
                start_elev,
                start_grade,
                end_grade,
                start_station,
                end_station,
            }) => {
                let l = end_station - start_station;
                Some(*start_elev + start_grade * l + 0.5 * (end_grade - start_grade) * l)
            }
            None => None,
        }
    }
}

/// Combined horizontal and vertical alignment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Alignment {
    pub horizontal: HorizontalAlignment,
    pub vertical: VerticalAlignment,
}

impl Alignment {
    pub fn new(horizontal: HorizontalAlignment, vertical: VerticalAlignment) -> Self {
        Self {
            horizontal,
            vertical,
        }
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
