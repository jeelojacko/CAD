use crate::alignment::Alignment;
use crate::dtm::Tin;
use crate::geometry::{Point, Point3};
use crate::superelevation::{slopes_at, SuperelevationTable};
use crate::variable_offset::offset_at;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProfilePoint {
    pub station: f64,
    pub profile: Vec<(f64, f64)>,
}

pub type ProfileTable = Vec<ProfilePoint>;

fn profile_at(table: &ProfileTable, station: f64) -> Vec<(f64, f64)> {
    if table.is_empty() {
        return Vec::new();
    }
    if station <= table[0].station {
        return table[0].profile.clone();
    }
    for pair in table.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        if station >= a.station && station <= b.station {
            let t = if (b.station - a.station).abs() < f64::EPSILON {
                0.0
            } else {
                (station - a.station) / (b.station - a.station)
            };
            return a
                .profile
                .iter()
                .zip(&b.profile)
                .map(|(pa, pb)| {
                    let off = pa.0 + t * (pb.0 - pa.0);
                    let elev = pa.1 + t * (pb.1 - pa.1);
                    (off, elev)
                })
                .collect();
        }
    }
    table.last().unwrap().profile.clone()
}

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

/// Representation of a cross-section shape relative to an alignment centerline.
/// Each tuple in `profile` contains `(offset, elevation)` values where `offset`
/// is measured perpendicular to the alignment and `elevation` is relative to
/// the alignment grade line.
#[derive(Debug, Clone)]
pub struct Subassembly {
    pub profile: Vec<(f64, f64)>,
    pub offsets: Option<crate::variable_offset::OffsetTable>,
    pub superelevation: Option<SuperelevationTable>,
    pub profile_table: Option<ProfileTable>,
}

impl Subassembly {
    pub fn new(profile: Vec<(f64, f64)>) -> Self {
        Self {
            profile,
            offsets: None,
            superelevation: None,
            profile_table: None,
        }
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

/// Generates cross-sections along a 2D polyline using a ground TIN.
pub fn extract_polyline_cross_sections(
    tin: &Tin,
    polyline: &crate::geometry::Polyline,
    width: f64,
    interval: f64,
    offset_step: f64,
) -> Vec<CrossSection> {
    let mut sections = Vec::new();
    let length = polyline.length();
    let mut station = 0.0;
    while station <= length {
        if let (Some(center), Some(dir)) =
            (polyline.point_at(station), polyline.direction_at(station))
        {
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
        station += interval;
    }
    sections
}

/// Generates design cross-sections from subassemblies with optional
/// superelevation and variable offsets.
pub fn extract_design_cross_sections(
    alignment: &Alignment,
    subs: &[Subassembly],
    superelevation: Option<&SuperelevationTable>,
    interval: f64,
) -> Vec<CrossSection> {
    let mut sections = Vec::new();
    let length = alignment.horizontal.length();
    let mut station = 0.0;
    while station <= length {
        if let (Some(center), Some(dir), Some(grade)) = (
            alignment.horizontal.point_at(station),
            alignment.horizontal.direction_at(station),
            alignment.vertical.elevation_at(station),
        ) {
            let global_slopes = superelevation
                .map(|t| slopes_at(t, station))
                .unwrap_or((0.0, 0.0));
            let normal = (-dir.1, dir.0);
            let mut pts = Vec::new();
            for sub in subs {
                let var_off = sub
                    .offsets
                    .as_ref()
                    .map(|t| offset_at(t, station))
                    .unwrap_or(0.0);
                let profile = sub
                    .profile_table
                    .as_ref()
                    .map(|t| profile_at(t, station))
                    .unwrap_or_else(|| sub.profile.clone());
                let slopes = sub
                    .superelevation
                    .as_ref()
                    .map(|t| slopes_at(t, station))
                    .unwrap_or(global_slopes);
                for (offset, elev) in profile {
                    let o = offset + var_off;
                    let slope = if o < 0.0 { slopes.0 } else { slopes.1 };
                    let x = center.x + o * normal.0;
                    let y = center.y + o * normal.1;
                    let z = grade + elev + o * slope;
                    pts.push(Point3::new(x, y, z));
                }
            }
            sections.push(CrossSection::new(station, pts));
        }
        station += interval;
    }
    sections
}

/// Builds a design surface by applying cross-section subassemblies along an
/// alignment at the specified station `interval`.
pub fn build_design_surface(alignment: &Alignment, subs: &[Subassembly], interval: f64) -> Tin {
    let mut pts = Vec::new();
    let length = alignment.horizontal.length();
    let mut station = 0.0;
    while station <= length {
        if let (Some(center), Some(dir), Some(grade)) = (
            alignment.horizontal.point_at(station),
            alignment.horizontal.direction_at(station),
            alignment.vertical.elevation_at(station),
        ) {
            let normal = (-dir.1, dir.0);
            for sub in subs {
                for (offset, elev) in &sub.profile {
                    let x = center.x + offset * normal.0;
                    let y = center.y + offset * normal.1;
                    let z = grade + elev;
                    pts.push(Point3::new(x, y, z));
                }
            }
        }
        station += interval;
    }
    Tin::from_points(pts)
}

/// Corridor model that automatically rebuilds its design surface when modified.
#[derive(Debug, Clone)]
pub struct Corridor {
    /// Alignment centerline and profile.
    pub alignment: Alignment,
    /// Collection of subassemblies defining the corridor section.
    pub subassemblies: Vec<Subassembly>,
    /// Optional global superelevation table.
    pub superelevation: Option<SuperelevationTable>,
    /// Spacing used when sampling the corridor.
    pub interval: f64,
    /// Current design surface generated from the above parameters.
    pub design_surface: Tin,
}

impl Corridor {
    /// Creates a new corridor and immediately builds its design surface.
    pub fn new(
        alignment: Alignment,
        subassemblies: Vec<Subassembly>,
        superelevation: Option<SuperelevationTable>,
        interval: f64,
    ) -> Self {
        let design_surface = build_design_surface_dynamic(
            &alignment,
            &subassemblies,
            superelevation.as_ref(),
            interval,
        );
        Self {
            alignment,
            subassemblies,
            superelevation,
            interval,
            design_surface,
        }
    }

    /// Rebuilds the design surface using current parameters.
    pub fn update_design_surface(&mut self) {
        self.design_surface = build_design_surface_dynamic(
            &self.alignment,
            &self.subassemblies,
            self.superelevation.as_ref(),
            self.interval,
        );
    }

    /// Sets a new superelevation table and rebuilds the surface.
    pub fn set_superelevation(&mut self, table: Option<SuperelevationTable>) {
        self.superelevation = table;
        self.update_design_surface();
    }

    /// Replaces the corridor subassemblies and rebuilds the surface.
    pub fn set_subassemblies(&mut self, subs: Vec<Subassembly>) {
        self.subassemblies = subs;
        self.update_design_surface();
    }

    /// Replaces the corridor alignment and rebuilds the surface.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
        self.update_design_surface();
    }

    /// Sets the sampling interval and rebuilds the surface.
    pub fn set_interval(&mut self, interval: f64) {
        self.interval = interval;
        self.update_design_surface();
    }

    /// Returns a reference to the current design surface.
    pub fn design_surface(&self) -> &Tin {
        &self.design_surface
    }
}

/// Builds a design surface using superelevation and variable offsets.
pub fn build_design_surface_dynamic(
    alignment: &Alignment,
    subs: &[Subassembly],
    superelevation: Option<&SuperelevationTable>,
    interval: f64,
) -> Tin {
    let mut pts = Vec::new();
    let length = alignment.horizontal.length();
    let mut station = 0.0;
    while station <= length {
        if let (Some(center), Some(dir), Some(grade)) = (
            alignment.horizontal.point_at(station),
            alignment.horizontal.direction_at(station),
            alignment.vertical.elevation_at(station),
        ) {
            let global_slopes = superelevation
                .map(|t| slopes_at(t, station))
                .unwrap_or((0.0, 0.0));
            let normal = (-dir.1, dir.0);
            for sub in subs {
                let var_off = sub
                    .offsets
                    .as_ref()
                    .map(|t| offset_at(t, station))
                    .unwrap_or(0.0);
                let profile = sub
                    .profile_table
                    .as_ref()
                    .map(|t| profile_at(t, station))
                    .unwrap_or_else(|| sub.profile.clone());
                let slopes = sub
                    .superelevation
                    .as_ref()
                    .map(|t| slopes_at(t, station))
                    .unwrap_or(global_slopes);
                for (offset, elev) in profile {
                    let o = offset + var_off;
                    let slope = if o < 0.0 { slopes.0 } else { slopes.1 };
                    let x = center.x + o * normal.0;
                    let y = center.y + o * normal.1;
                    let z = grade + elev + o * slope;
                    pts.push(Point3::new(x, y, z));
                }
            }
        }
        station += interval;
    }
    Tin::from_points(pts)
}

/// Calculates the volume between a design and ground surface along an alignment
/// using the average end area method.
pub fn corridor_volume(
    design: &Tin,
    ground: &Tin,
    alignment: &Alignment,
    width: f64,
    station_interval: f64,
    offset_step: f64,
) -> f64 {
    let design_sections =
        extract_cross_sections(design, alignment, width, station_interval, offset_step);
    let ground_sections =
        extract_cross_sections(ground, alignment, width, station_interval, offset_step);
    let count = design_sections.len().min(ground_sections.len());
    if count < 2 {
        return 0.0;
    }

    let mut areas = Vec::new();
    for i in 0..count {
        let d = &design_sections[i];
        let g = &ground_sections[i];
        let n = d.points.len().min(g.points.len());
        if n < 2 {
            areas.push(0.0);
            continue;
        }
        let mut area = 0.0;
        for j in 0..(n - 1) {
            let dz1 = d.points[j].z - g.points[j].z;
            let dz2 = d.points[j + 1].z - g.points[j + 1].z;
            area += (dz1 + dz2) * 0.5 * offset_step;
        }
        areas.push(area);
    }

    let mut volume = 0.0;
    for i in 0..(areas.len() - 1) {
        volume += (areas[i] + areas[i + 1]) * 0.5 * station_interval;
    }
    volume
}

/// Calculates separate cut and fill volumes along an alignment.
/// Returns `(cut, fill)` using the average end area method.
pub fn corridor_cut_fill(
    design: &Tin,
    ground: &Tin,
    alignment: &Alignment,
    width: f64,
    station_interval: f64,
    offset_step: f64,
) -> (f64, f64) {
    let design_sections =
        extract_cross_sections(design, alignment, width, station_interval, offset_step);
    let ground_sections =
        extract_cross_sections(ground, alignment, width, station_interval, offset_step);
    let count = design_sections.len().min(ground_sections.len());
    if count < 2 {
        return (0.0, 0.0);
    }

    let mut cut_areas = vec![0.0; count];
    let mut fill_areas = vec![0.0; count];
    for i in 0..count {
        let d = &design_sections[i];
        let g = &ground_sections[i];
        let n = d.points.len().min(g.points.len());
        if n < 2 {
            continue;
        }
        for j in 0..(n - 1) {
            let dz1 = d.points[j].z - g.points[j].z;
            let dz2 = d.points[j + 1].z - g.points[j + 1].z;
            let area = (dz1 + dz2) * 0.5 * offset_step;
            if area > 0.0 {
                fill_areas[i] += area;
            } else {
                cut_areas[i] += -area;
            }
        }
    }

    let mut cut = 0.0;
    let mut fill = 0.0;
    for i in 0..(count - 1) {
        cut += (cut_areas[i] + cut_areas[i + 1]) * 0.5 * station_interval;
        fill += (fill_areas[i] + fill_areas[i + 1]) * 0.5 * station_interval;
    }
    (cut, fill)
}

/// Computes a mass haul diagram along an alignment. The returned vector
/// contains `(station, cumulative_volume)` pairs where positive values
/// represent fill and negative values represent cut.
pub fn corridor_mass_haul(
    design: &Tin,
    ground: &Tin,
    alignment: &Alignment,
    width: f64,
    station_interval: f64,
    offset_step: f64,
) -> Vec<(f64, f64)> {
    let design_sections =
        extract_cross_sections(design, alignment, width, station_interval, offset_step);
    let ground_sections =
        extract_cross_sections(ground, alignment, width, station_interval, offset_step);
    let count = design_sections.len().min(ground_sections.len());
    if count == 0 {
        return Vec::new();
    }

    let mut areas = vec![0.0; count];
    for i in 0..count {
        let d = &design_sections[i];
        let g = &ground_sections[i];
        let n = d.points.len().min(g.points.len());
        if n < 2 {
            continue;
        }
        for j in 0..(n - 1) {
            let dz1 = d.points[j].z - g.points[j].z;
            let dz2 = d.points[j + 1].z - g.points[j + 1].z;
            areas[i] += (dz1 + dz2) * 0.5 * offset_step;
        }
    }

    let mut haul = Vec::new();
    let mut cumulative = 0.0;
    haul.push((design_sections[0].station, 0.0));
    for i in 1..count {
        let vol = (areas[i - 1] + areas[i]) * 0.5 * station_interval;
        cumulative += vol;
        haul.push((design_sections[i].station, cumulative));
    }
    haul
}

/// Maintains cross-sections that update automatically when the alignment or
/// surface changes.
#[derive(Debug, Clone)]
pub struct DynamicCrossSections {
    alignment: Alignment,
    surface: Tin,
    width: f64,
    interval: f64,
    offset_step: f64,
    pub sections: Vec<CrossSection>,
}

impl DynamicCrossSections {
    pub fn new(
        alignment: Alignment,
        surface: Tin,
        width: f64,
        interval: f64,
        offset_step: f64,
    ) -> Self {
        let sections = extract_cross_sections(&surface, &alignment, width, interval, offset_step);
        Self {
            alignment,
            surface,
            width,
            interval,
            offset_step,
            sections,
        }
    }

    /// Replaces the alignment and recomputes cross-sections.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
        self.sections = extract_cross_sections(
            &self.surface,
            &self.alignment,
            self.width,
            self.interval,
            self.offset_step,
        );
    }

    /// Replaces the surface and recomputes cross-sections.
    pub fn set_surface(&mut self, surface: Tin) {
        self.surface = surface;
        self.sections = extract_cross_sections(
            &self.surface,
            &self.alignment,
            self.width,
            self.interval,
            self.offset_step,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::DynamicCrossSections;
    use super::*;
    use crate::alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
    use crate::geometry::Polyline;
    use crate::geometry::{Point, Point3};
    use crate::superelevation::SuperelevationPoint;

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

    #[test]
    fn polyline_cross_sections_flat() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
        ];
        let tin = Tin::from_points(pts);
        let pl = Polyline::new(vec![Point::new(0.0, 5.0), Point::new(10.0, 5.0)]);
        let sections = extract_polyline_cross_sections(&tin, &pl, 5.0, 5.0, 2.5);
        assert_eq!(sections.len(), 3);
        for sec in sections {
            assert_eq!(sec.points.len(), 5);
            for p in sec.points {
                assert!((p.z - 0.0).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn build_design_surface_simple() {
        let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
        let valign = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
        let align = Alignment::new(halign, valign);
        let sub = Subassembly::new(vec![(-1.0, 1.0), (1.0, 1.0)]);
        let tin = build_design_surface(&align, &[sub], 10.0);
        assert_eq!(tin.vertices.len(), 4);
    }

    #[test]
    fn dynamic_profile_sections() {
        let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
        let valign = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
        let align = Alignment::new(halign, valign);
        let table = vec![
            ProfilePoint {
                station: 0.0,
                profile: vec![(-1.0, 0.0), (1.0, 0.0)],
            },
            ProfilePoint {
                station: 10.0,
                profile: vec![(-2.0, 0.0), (2.0, 0.0)],
            },
        ];
        let mut sub = Subassembly::new(Vec::new());
        sub.profile_table = Some(table);
        let sections = extract_design_cross_sections(&align, &[sub], None, 10.0);
        assert_eq!(sections.len(), 2);
        assert!((sections[0].points.first().unwrap().y + 1.0).abs() < 1e-6);
        assert!((sections[1].points.first().unwrap().y + 2.0).abs() < 1e-6);
    }

    #[test]
    fn corridor_surface_updates() {
        let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
        let valign = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
        let align = Alignment::new(halign, valign);
        let sub = Subassembly::new(vec![(0.0, 0.0), (1.0, 0.0)]);
        let sup = vec![
            SuperelevationPoint {
                station: 0.0,
                left_slope: 0.0,
                right_slope: 0.0,
            },
            SuperelevationPoint {
                station: 10.0,
                left_slope: 0.0,
                right_slope: 0.0,
            },
        ];
        let mut cor = Corridor::new(align.clone(), vec![sub.clone()], Some(sup), 10.0);
        let initial_z = cor
            .design_surface
            .vertices
            .iter()
            .map(|p| p.z)
            .fold(f64::INFINITY, f64::min);

        let sup2 = vec![
            SuperelevationPoint {
                station: 0.0,
                left_slope: 0.0,
                right_slope: -0.1,
            },
            SuperelevationPoint {
                station: 10.0,
                left_slope: 0.0,
                right_slope: -0.1,
            },
        ];
        cor.set_superelevation(Some(sup2));
        let new_min = cor
            .design_surface
            .vertices
            .iter()
            .map(|p| p.z)
            .fold(f64::INFINITY, f64::min);
        assert!(new_min < initial_z);
    }

    #[test]
    fn dynamic_cross_section_updates() {
        let tin = Tin::from_points(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
        ]);
        let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
        let valign = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
        let align = Alignment::new(halign.clone(), valign);
        let mut secs = DynamicCrossSections::new(align, tin.clone(), 5.0, 5.0, 5.0);
        let count_initial = secs.sections.len();
        secs.set_alignment(Alignment::new(
            HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(5.0, 0.0)]),
            VerticalAlignment::new(vec![(0.0, 0.0), (5.0, 0.0)]),
        ));
        assert!(secs.sections.len() != count_initial);
        secs.set_surface(tin);
        assert!(!secs.sections.is_empty());
    }
}
