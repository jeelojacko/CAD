//! Simple sheet/layout generation utilities.
//!
//! These helpers output very basic SVG files representing plan and profile
//! views or cross-section sheets. The implementation is intentionally
//! lightweight and meant only as a placeholder for more advanced drawing
//! capabilities.

use std::fs::File;
use std::io::{self, Write};

use crate::alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
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
        if p.x < min_x {
            min_x = p.x;
        }
        if p.x > max_x {
            max_x = p.x;
        }
        if p.y < min_y {
            min_y = p.y;
        }
        if p.y > max_y {
            max_y = p.y;
        }
    }
    Some((min_x, min_y, max_x, max_y))
}

fn write_svg_header(file: &mut File, width: f64, height: f64) -> io::Result<()> {
    writeln!(
        file,
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}'>"
    )
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

fn write_line(file: &mut File, x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str) -> io::Result<()> {
    writeln!(
        file,
        "<line x1='{x1:.2}' y1='{y1:.2}' x2='{x2:.2}' y2='{y2:.2}' stroke='{stroke}' stroke-width='0.5' />"
    )
}

fn write_text(file: &mut File, x: f64, y: f64, text: &str) -> io::Result<()> {
    writeln!(
        file,
        "<text x='{x:.2}' y='{y:.2}' font-size='8' font-family='sans-serif'>{text}</text>"
    )
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

    let (min_x, min_y, max_x, max_y) = bbox(&plan).unwrap_or((0.0, 0.0, 0.0, 0.0));
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
    let prof_bbox = bbox(&profile).unwrap_or((0.0, 0.0, 0.0, 0.0));
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
    let mut max_width: f64 = 0.0;
    let mut max_height: f64 = 0.0;
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

/// Writes a scaled plan/profile sheet with basic grid lines and labels.
pub fn write_plan_profile_scaled_svg(
    path: &str,
    halign: &HorizontalAlignment,
    valign: &VerticalAlignment,
    step: f64,
    plan_scale: f64,
    profile_hscale: f64,
    profile_vscale: f64,
    grid: f64,
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

    let plan_bbox = bbox(&plan).unwrap_or((0.0, 0.0, 0.0, 0.0));
    let plan_width = (plan_bbox.2 - plan_bbox.0) / plan_scale;
    let plan_height = (plan_bbox.3 - plan_bbox.1) / plan_scale;

    let prof_bbox = bbox(&profile).unwrap_or((0.0, 0.0, 0.0, 0.0));
    let prof_width = (prof_bbox.2 - prof_bbox.0) / profile_hscale;
    let prof_height = (prof_bbox.3 - prof_bbox.1) / profile_vscale;

    let width = plan_width.max(prof_width) + 40.0;
    let height = plan_height + prof_height + 60.0;
    let mut f = File::create(path)?;
    write_svg_header(&mut f, width, height)?;

    // plan view with grid
    writeln!(f, "<g transform='translate(20,20)'>")?;
    let mut y = 0.0;
    while y <= plan_height {
        write_line(&mut f, 0.0, y, plan_width, y, "#ccc")?;
        y += grid;
    }
    let mut x = 0.0;
    while x <= plan_width {
        write_line(&mut f, x, 0.0, x, plan_height, "#ccc")?;
        x += grid;
    }
    let plan_scaled: Vec<Point> = plan
        .iter()
        .map(|p| {
            Point::new(
                (p.x - plan_bbox.0) / plan_scale,
                plan_height - (p.y - plan_bbox.1) / plan_scale,
            )
        })
        .collect();
    write_polyline(&mut f, &plan_scaled, "blue")?;
    writeln!(f, "</g>")?;

    // profile view below plan
    writeln!(f, "<g transform='translate(20,{})'>", plan_height + 40.0)?;
    let mut y = 0.0;
    while y <= prof_height {
        write_line(&mut f, 0.0, y, prof_width, y, "#ccc")?;
        y += grid;
    }
    let mut x = 0.0;
    while x <= prof_width {
        write_line(&mut f, x, 0.0, x, prof_height, "#ccc")?;
        x += grid;
    }
    let profile_scaled: Vec<Point> = profile
        .iter()
        .map(|p| {
            Point::new(
                (p.x - prof_bbox.0) / profile_hscale,
                prof_height - (p.y - prof_bbox.1) / profile_vscale,
            )
        })
        .collect();
    write_polyline(&mut f, &profile_scaled, "green")?;
    writeln!(f, "</g>")?;

    write_svg_footer(&mut f)
}

/// Writes scaled cross-section sheets with grid lines and station labels.
pub fn write_cross_section_scaled_svg(
    path: &str,
    alignment: &Alignment,
    sections: &[CrossSection],
    spacing: f64,
    hscale: f64,
    vscale: f64,
    grid: f64,
) -> io::Result<()> {
    let mut all_lines: Vec<(f64, f64, Vec<Point>, f64)> = Vec::new();
    let mut max_width: f64 = 0.0;
    let mut max_height: f64 = 0.0;
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
            if let Some(b) = bbox(&pts) {
                let width = (b.2 - b.0) / hscale;
                let height = (b.3 - b.1) / vscale;
                max_width = max_width.max(width);
                max_height = max_height.max(height);
            }
            all_lines.push((sec.station, grade, pts, grade));
        }
    }

    let mut f = File::create(path)?;
    let width = spacing * sections.len() as f64 + max_width + 40.0;
    let height = max_height + 40.0;
    write_svg_header(&mut f, width, height)?;

    for (i, (station, _grade, pts, _)) in all_lines.iter().enumerate() {
        let tx = 20.0 + i as f64 * spacing;
        writeln!(f, "<g transform='translate({tx},20)'>")?;
        if let Some(b) = bbox(pts) {
            let sec_width = (b.2 - b.0) / hscale;
            let sec_height = (b.3 - b.1) / vscale;
            let mut y = 0.0;
            while y <= sec_height {
                write_line(&mut f, 0.0, y, sec_width, y, "#ccc")?;
                y += grid;
            }
            let mut x = 0.0;
            while x <= sec_width {
                write_line(&mut f, x, 0.0, x, sec_height, "#ccc")?;
                x += grid;
            }
            let shifted: Vec<Point> = pts
                .iter()
                .map(|p| Point::new((p.x - b.0) / hscale, (p.y - b.1) / vscale))
                .collect();
            write_polyline(&mut f, &shifted, "red")?;
            write_text(
                &mut f,
                sec_width / 2.0 - 10.0,
                sec_height + 12.0,
                &format!("Sta {:.2}", station),
            )?;
        }
        writeln!(f, "</g>")?;
    }

    write_svg_footer(&mut f)
}
