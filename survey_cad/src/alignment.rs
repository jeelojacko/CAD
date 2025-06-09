use crate::geometry::{distance, Arc, Point, Point3};

/// Euler spiral segment described analytically.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Spiral {
    pub start: Point,
    pub orientation: f64,
    pub length: f64,
    pub start_radius: f64,
    pub end_radius: f64,
}

impl Spiral {
    /// Returns the starting point of the spiral.
    pub fn start_point(&self) -> Point {
        self.start
    }

    /// Returns the end point of the spiral.
    pub fn end_point(&self) -> Point {
        self.point_at(self.length)
    }

    pub(crate) fn point_at(&self, s: f64) -> Point {
        let k0 = if self.start_radius.is_infinite() {
            0.0
        } else {
            1.0 / self.start_radius
        };
        let k1 = if self.end_radius.is_infinite() {
            0.0
        } else {
            1.0 / self.end_radius
        };
        let kp = (k1 - k0) / self.length;

        if kp.abs() < f64::EPSILON {
            if k0.abs() < f64::EPSILON {
                return Point::new(
                    self.start.x + s * self.orientation.cos(),
                    self.start.y + s * self.orientation.sin(),
                );
            }
            let r = 1.0 / k0;
            let cx = self.start.x - r * self.orientation.sin();
            let cy = self.start.y + r * self.orientation.cos();
            let ang = self.orientation + k0 * s;
            return Point::new(cx + r * ang.sin(), cy - r * ang.cos());
        }

        let alpha = kp / 2.0;
        let beta = k0;
        let delta = self.orientation - beta * beta / (4.0 * alpha);
        let sign = alpha.signum();
        let z = |x: f64| -> f64 {
            sign * (2.0 * alpha.abs() / std::f64::consts::PI).sqrt() * (x + beta / (2.0 * alpha))
        };
        let (s0, c0) = fresnel::fresnl(z(0.0));
        let (s1, c1) = fresnel::fresnl(z(s));
        let fac = (std::f64::consts::PI / (2.0 * alpha.abs())).sqrt();
        let dx = fac * ((c1 - c0) * delta.cos() - sign * (s1 - s0) * delta.sin());
        let dy = fac * ((s1 - s0) * delta.cos() + sign * (c1 - c0) * delta.sin());
        Point::new(self.start.x + dx, self.start.y + dy)
    }

    pub(crate) fn direction_at(&self, s: f64) -> (f64, f64) {
        let k0 = if self.start_radius.is_infinite() {
            0.0
        } else {
            1.0 / self.start_radius
        };
        let k1 = if self.end_radius.is_infinite() {
            0.0
        } else {
            1.0 / self.end_radius
        };
        let kp = (k1 - k0) / self.length;
        let theta = self.orientation + k0 * s + 0.5 * kp * s * s;
        (theta.cos(), theta.sin())
    }

    pub(crate) fn length(&self) -> f64 {
        self.length
    }
}

/// Individual elements of a horizontal alignment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HorizontalElement {
    /// Straight tangent between two points.
    Tangent { start: Point, end: Point },
    /// Circular curve described by an [`Arc`].
    Curve { arc: Arc },
    /// Transition spiral described analytically.
    Spiral { spiral: Spiral },
}

impl HorizontalElement {
    pub(crate) fn length(&self) -> f64 {
        match self {
            HorizontalElement::Tangent { start, end } => distance(*start, *end),
            HorizontalElement::Curve { arc } => arc.length(),
            HorizontalElement::Spiral { spiral } => spiral.length(),
        }
    }


    fn end_point(&self) -> Point {
        match self {
            HorizontalElement::Tangent { end, .. } => *end,
            HorizontalElement::Curve { arc } => Point::new(
                arc.center.x + arc.radius * arc.end_angle.cos(),
                arc.center.y + arc.radius * arc.end_angle.sin(),
            ),
            HorizontalElement::Spiral { spiral } => spiral.end_point(),
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
            HorizontalElement::Spiral { spiral } => spiral.point_at(s),
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
            HorizontalElement::Spiral { spiral } => spiral.direction_at(s),
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

    /// Creates a new empty alignment. Elements can be added with
    /// [`HorizontalAlignmentBuilder`].
    pub fn empty() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    /// Returns the station value at the start of each element.
    pub fn stations(&self) -> Vec<f64> {
        let mut out = Vec::new();
        let mut sta = 0.0;
        out.push(sta);
        for elem in &self.elements {
            sta += elem.length();
            out.push(sta);
        }
        out
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

/// Builder for [`HorizontalAlignment`].
#[derive(Debug, Default)]
pub struct HorizontalAlignmentBuilder {
    elements: Vec<HorizontalElement>,
}

impl HorizontalAlignmentBuilder {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    pub fn add_tangent(mut self, start: Point, end: Point) -> Self {
        self.elements
            .push(HorizontalElement::Tangent { start, end });
        self
    }

    pub fn add_curve(mut self, arc: Arc) -> Self {
        self.elements.push(HorizontalElement::Curve { arc });
        self
    }

    pub fn add_spiral(mut self, spiral: Spiral) -> Self {
        self.elements.push(HorizontalElement::Spiral { spiral });
        self
    }

    pub fn build(self) -> HorizontalAlignment {
        HorizontalAlignment {
            elements: self.elements,
        }
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

    /// Appends a parabolic vertical curve to the alignment.
    pub fn add_parabola(
        &mut self,
        start_station: f64,
        end_station: f64,
        start_elev: f64,
        start_grade: f64,
        end_grade: f64,
    ) {
        self.elements.push(VerticalElement::Parabola {
            start_station,
            end_station,
            start_elev,
            start_grade,
            end_grade,
        });
    }

    /// Returns the elevation contributed by a single vertical element at the
    /// given station if the element spans the station.
    fn element_elevation(elem: &VerticalElement, station: f64) -> Option<f64> {
        match *elem {
            VerticalElement::Grade {
                start_station,
                end_station,
                start_elev,
                end_elev,
            } => {
                if station >= start_station && station <= end_station {
                    let t = (station - start_station) / (end_station - start_station);
                    Some(start_elev + t * (end_elev - start_elev))
                } else {
                    None
                }
            }
            VerticalElement::Parabola {
                start_station,
                end_station,
                start_elev,
                start_grade,
                end_grade,
            } => {
                if station >= start_station && station <= end_station {
                    let l = end_station - start_station;
                    let x = station - start_station;
                    let g1 = start_grade;
                    let g2 = end_grade;
                    let dz = g1 * x + 0.5 * (g2 - g1) / l * x * x;
                    Some(start_elev + dz)
                } else {
                    None
                }
            }
        }
    }

    /// Returns the lowest elevation of all elements at the given station.
    fn min_elevation_at(&self, station: f64) -> Option<f64> {
        let mut min_val: Option<f64> = None;
        for e in &self.elements {
            if let Some(z) = Self::element_elevation(e, station) {
                min_val = Some(min_val.map_or(z, |m| m.min(z)));
            }
        }
        if min_val.is_none() {
            self.elevation_at(station)
        } else {
            min_val
        }
    }

    /// Checks vertical clearance against a ground surface along a horizontal
    /// alignment. Returns `true` if all stations satisfy the minimum
    /// clearance.
    pub fn check_clearance(
        &self,
        halign: &HorizontalAlignment,
        ground: &crate::dtm::Tin,
        min_clearance: f64,
        interval: f64,
    ) -> bool {
        let length = halign.length();
        let mut station = 0.0;
        while station <= length {
            if let (Some(pt), Some(grade)) = (halign.point_at(station), self.min_elevation_at(station))
            {
                if let Some(gz) = ground.elevation_at(pt.x, pt.y) {
                    if grade - gz < min_clearance {
                        return false;
                    }
                }
            }
            station += interval;
        }
        true
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
    use crate::dtm::Tin;
    use crate::geometry::Point;
    use crate::geometry::Point3;

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

    #[test]
    fn spiral_geometry() {
        let spiral = Spiral {
            start: Point::new(0.0, 0.0),
            orientation: 0.0,
            length: 50.0,
            start_radius: f64::INFINITY,
            end_radius: 100.0,
        };
        let end = spiral.end_point();
        assert!((end.x - 49.6884029).abs() < 1e-6);
        assert!((end.y - 4.1481024).abs() < 1e-6);
        let dir = spiral.direction_at(50.0);
        assert!((dir.0 - 0.9689124).abs() < 1e-6);
        assert!((dir.1 - 0.2474039).abs() < 1e-6);
    }

    #[test]
    fn spiral_in_alignment() {
        let spiral = Spiral {
            start: Point::new(0.0, 0.0),
            orientation: 0.0,
            length: 50.0,
            start_radius: f64::INFINITY,
            end_radius: 100.0,
        };
        let halign = HorizontalAlignment {
            elements: vec![HorizontalElement::Spiral { spiral }],
        };
        let valign = VerticalAlignment::new(vec![(0.0, 0.0), (50.0, 0.0)]);
        let align = Alignment::new(halign, valign);
        let p = align.point3_at(25.0).unwrap();
        let pt = spiral.point_at(25.0);
        assert!((p.x - pt.x).abs() < 1e-6);
        assert!((p.y - pt.y).abs() < 1e-6);
    }

    #[test]
    fn horizontal_stationing() {
        let halign = HorizontalAlignmentBuilder::new()
            .add_tangent(Point::new(0.0, 0.0), Point::new(10.0, 0.0))
            .add_tangent(Point::new(10.0, 0.0), Point::new(20.0, 0.0))
            .build();
        let stations = halign.stations();
        assert_eq!(stations.len(), 3);
        assert!((stations[2] - 20.0).abs() < 1e-6);
    }

    #[test]
    fn clearance_check() {
        let surface = Tin::from_points(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
        ]);
        let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
        let mut valign = VerticalAlignment::new(vec![(0.0, 1.0), (10.0, 1.0)]);
        assert!(valign.check_clearance(&halign, &surface, 0.5, 5.0));
        valign.add_parabola(0.0, 10.0, 0.0, 0.0, 0.0);
        assert!(!valign.check_clearance(&halign, &surface, 0.5, 5.0));
    }
}
