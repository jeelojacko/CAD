//! Simple sheet/layout generation utilities.
//!
//! These helpers output very basic SVG files representing plan and profile
//! views or cross-section sheets. The implementation is intentionally
//! lightweight and meant only as a placeholder for more advanced drawing
//! capabilities.

use std::fs::File;
use std::io::{self, Write};

use crate::alignment::{HorizontalAlignment, VerticalAlignment, Alignment};
use crate::corridor::CrossSection;
use crate::geometry::{Point, Point3};

fn sample_horizontal(halign: &HorizontalAlignment, step: f64) -> Vec<Point> {
    let len = halign.length();
    let mut pts = Vec::new();
    let mut s = 0.0;
    while s <= len {
        if let Some(p) = halign.point_at(s) {
            pts.push(p);
        }
        s += step;
    }
    if let Some(p) = halign.point_at(len) {
        if pts.last() != Some(&p) {
            pts.push(p);
        }
    }
    pts
}

fn bbox(points: &[Point]) -> Option<(f64, f64, f64, f64)> {
    if points.is_empty() {
        return None;
    }
    let mut min_x = points[0].x;
    let mut max_x = points[0].x;
    let mut min_y = points[0].y;
    let mut max_y = points[0].y;
    for p in points.iter().skip(1) {
        if p.x < min_x { min_x = p.x; }
        if p.x > max_x { max_x = p.x; }
        if p.y < min_y { min_y = p.y; }
        if p.y > max_y { max_y = p.y; }
    }
    Some((min_x, min_y, max_x, max_y))
}

fn write_svg_header(file: &mut File, width: f64, height: f64) -> io::Result<()> {
    writeln!(file, "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}'>")
}

fn write_svg_footer(file: &mut File) -> io::Result<()> {
    writeln!(file, "</svg>")
}

fn write_polyline(file: &mut File, pts: &[Point], stroke: &str) -> io::Result<()> {
    write!(file, "<polyline points='")?;
    for p in pts {
        write!(file, "{:.2},{:.2} ", p.x, p.y)?;
    }
    writeln!(file, "' fill='none' stroke='{stroke}' stroke-width='1' />")
}

/// Writes a very simple plan and profile sheet to `path` in SVG format.
///
/// The horizontal alignment is sampled every `step` units to generate the plan
/// view polyline. Elevations from the vertical alignment are plotted against
/// station for the profile view. The output is not to scale and meant only for
/// quick visualization.
pub fn write_plan_profile_svg(
    path: &str,
    halign: &HorizontalAlignment,
    valign: &VerticalAlignment,
    step: f64,
) -> io::Result<()> {
    let plan = sample_horizontal(halign, step);
    let profile_len = halign.length();
    let mut profile = Vec::new();
    let mut s = 0.0;
    while s <= profile_len {
        if let Some(z) = valign.elevation_at(s) {
            profile.push(Point::new(s, z));
        }
        s += step;
    }
    if let Some(z) = valign.elevation_at(profile_len) {
        profile.push(Point::new(profile_len, z));
    }

    let (min_x, min_y, max_x, max_y) = bbox(&plan).unwrap_or((0.0,0.0,0.0,0.0));
    let width = max_x - min_x;
    let height = max_y - min_y;
    let mut f = File::create(path)?;
    write_svg_header(&mut f, width + 40.0, height + 120.0)?;
    // translate plan
    writeln!(f, "<g transform='translate(20,20)'>")?;
    let plan_scaled: Vec<Point> = plan
        .iter()
        .map(|p| Point::new(p.x - min_x, max_y - p.y))
        .collect();
    write_polyline(&mut f, &plan_scaled, "blue")?;
    writeln!(f, "</g>")?;

    // profile below plan
    writeln!(f, "<g transform='translate(20,{})'>", height + 40.0)?;
    let prof_bbox = bbox(&profile).unwrap_or((0.0,0.0,0.0,0.0));
    let prof_height = prof_bbox.3 - prof_bbox.1;
    let profile_scaled: Vec<Point> = profile
        .iter()
        .map(|p| Point::new(p.x - prof_bbox.0, prof_height - (p.y - prof_bbox.1)))
        .collect();
    write_polyline(&mut f, &profile_scaled, "green")?;
    writeln!(f, "</g>")?;

    write_svg_footer(&mut f)
}

/// Writes cross-section sheets from the provided sections to an SVG file.
///
/// Each section is plotted using offsets and elevations relative to the
/// alignment at the same station. Sections are placed side by side with the
/// given `spacing` in SVG units.
pub fn write_cross_section_svg(
    path: &str,
    alignment: &Alignment,
    sections: &[CrossSection],
    spacing: f64,
) -> io::Result<()> {
    let mut all_lines: Vec<Vec<Point>> = Vec::new();
    let mut max_width = 0.0;
    let mut max_height = 0.0;
    for sec in sections {
        if let (Some(center), Some(dir), Some(grade)) = (
            alignment.horizontal.point_at(sec.station),
            alignment.horizontal.direction_at(sec.station),
            alignment.vertical.elevation_at(sec.station),
        ) {
            let normal = (-dir.1, dir.0);
            let mut pts = Vec::new();
            for p in &sec.points {
                let dx = p.x - center.x;
                let dy = p.y - center.y;
                let off = dx * normal.0 + dy * normal.1;
                let elev = p.z - grade;
                pts.push(Point::new(off, -elev));
            }
            if let Some((min_x, min_y, max_x, max_y)) = bbox(&pts) {
                max_width = max_width.max(max_x - min_x);
                max_height = max_height.max(max_y - min_y);
            }
            all_lines.push(pts);
        }
    }

    let mut f = File::create(path)?;
    let width = spacing * sections.len() as f64 + max_width;
    write_svg_header(&mut f, width + 40.0, max_height + 40.0)?;

    for (i, pts) in all_lines.iter().enumerate() {
        let tx = 20.0 + i as f64 * spacing;
        writeln!(f, "<g transform='translate({tx},20)'>")?;
        let bbox = bbox(pts).unwrap();
        let shifted: Vec<Point> = pts
            .iter()
            .map(|p| Point::new(p.x - bbox.0, p.y - bbox.1))
            .collect();
        write_polyline(&mut f, &shifted, "red")?;
        writeln!(f, "</g>")?;
    }

    write_svg_footer(&mut f)
}

