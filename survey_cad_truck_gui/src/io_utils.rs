use survey_cad::geometry::{Arc, Point};

/// Type alias for a single line represented by its start and end points.
type Line = (Point, Point);

/// Return type for [`read_field_book_csv`].
type PointsAndLines = (Vec<Point>, Vec<Line>);

pub fn read_line_csv(path: &str, dst_epsg: u32) -> std::io::Result<(Point, Point)> {
    let pts = survey_cad::io::read_points_csv(path, Some(4326), Some(dst_epsg))?;
    if pts.len() != 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected exactly two points",
        ));
    }
    Ok((pts[0], pts[1]))
}

pub fn read_points_list(path: &str, dst_epsg: u32) -> std::io::Result<Vec<Point>> {
    survey_cad::io::read_points_csv(path, Some(4326), Some(dst_epsg))
}

pub fn read_arc_csv(path: &str) -> std::io::Result<Arc> {
    let lines = survey_cad::io::read_lines(path)?;
    if lines.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "empty file",
        ));
    }
    let parts: Vec<&str> = lines[0].split(',').collect();
    if parts.len() != 5 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected cx,cy,radius,start,end",
        ));
    }
    let cx: f64 = parts[0]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let cy: f64 = parts[1]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let r: f64 = parts[2]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let sa: f64 = parts[3]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let ea: f64 = parts[4]
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Arc::new(Point::new(cx, cy), r, sa, ea))
}

use std::collections::BTreeMap;
use survey_cad::crs::Crs;
use survey_cad::surveying::field_code::{CodeAction, FieldCode};

pub fn read_field_book_csv(
    path: &str,
    dst_epsg: u32,
) -> std::io::Result<PointsAndLines> {
    let lines = survey_cad::io::read_lines(path)?;
    if lines.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let header: Vec<String> = lines[0].split(',').map(|s| s.trim().to_string()).collect();
    let mut start = 0usize;
    let mut x_idx = 0usize;
    let mut y_idx = 1usize;
    let mut code_idx: Option<usize> = None;
    let lower: Vec<String> = header.iter().map(|s| s.to_lowercase()).collect();
    if lower
        .iter()
        .any(|h| h.contains('e') || h.contains("north") || h.contains("code"))
        && lower.iter().any(|h| h.chars().any(|c| c.is_alphabetic()))
    {
        start = 1;
        for (i, h) in lower.iter().enumerate() {
            if h.contains("east") || h == "e" || h == "x" {
                x_idx = i;
            } else if h.contains("north") || h == "n" || h == "y" {
                y_idx = i;
            } else if h.contains("code") || h.contains("desc") {
                code_idx = Some(i);
            }
        }
    }
    let mut pts_codes: Vec<(Point, String)> = Vec::new();
    for line in lines.iter().skip(start) {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() <= x_idx || parts.len() <= y_idx {
            continue;
        }
        let x: f64 = parts[x_idx]
            .trim()
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let y: f64 = parts[y_idx]
            .trim()
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let code = code_idx
            .and_then(|i| parts.get(i))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        pts_codes.push((Point::new(x, y), code));
    }
    let src = Crs::from_epsg(4326);
    let dst = Crs::from_epsg(dst_epsg);
    for (p, _) in &mut pts_codes {
        if let Some((x, y)) = src.transform_point(&dst, p.x, p.y) {
            p.x = x;
            p.y = y;
        }
    }
    let mut active: BTreeMap<String, Point> = BTreeMap::new();
    let mut out_lines = Vec::new();
    for (pt, codes) in &pts_codes {
        for raw in codes
            .split([' ', ';'])
            .filter(|s| !s.is_empty())
        {
            let fc = FieldCode::parse(raw);
            if fc.code.is_empty() {
                continue;
            }
            match fc.action {
                CodeAction::Begin => {
                    active.insert(fc.code, *pt);
                }
                CodeAction::Continue => {
                    if let Some(prev) = active.get_mut(&fc.code) {
                        out_lines.push((*prev, *pt));
                        *prev = *pt;
                    } else {
                        active.insert(fc.code, *pt);
                    }
                }
                CodeAction::End => {
                    if let Some(prev) = active.remove(&fc.code) {
                        out_lines.push((prev, *pt));
                    }
                }
                CodeAction::None => {}
            }
        }
    }
    let pts: Vec<Point> = pts_codes.into_iter().map(|(p, _)| p).collect();
    Ok((pts, out_lines))
}
