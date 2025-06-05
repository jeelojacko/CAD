use std::io;

use survey_cad::{
    geometry::Point,
    geometry::Point3,
    io::{read_lines, read_points_csv as sc_read_csv, read_points_geojson as sc_read_geojson},
};

/// Reads a CSV file of `x,y` pairs into [`Point`]s.
pub fn read_points_csv(path: &str) -> io::Result<Vec<Point>> {
    sc_read_csv(path)
}

/// Reads a GeoJSON file of Point features into [`Point`]s.
pub fn read_points_geojson(path: &str) -> io::Result<Vec<Point>> {
    sc_read_geojson(path)
}

/// Reads a very simple ASCII DXF file containing only `POINT` entities and
/// returns their coordinates as [`Point`]s.
pub fn read_points_dxf(path: &str) -> io::Result<Vec<Point>> {
    let lines = read_lines(path)?;
    let mut pts = Vec::new();
    let mut iter = lines.iter();
    while let (Some(code), Some(value)) = (iter.next(), iter.next()) {
        if code.trim() == "0" && value.trim() == "POINT" {
            let mut x = None;
            let mut y = None;
            while let (Some(c), Some(v)) = (iter.next(), iter.next()) {
                match c.trim() {
                    "10" => x = v.trim().parse().ok(),
                    "20" => y = v.trim().parse().ok(),
                    "30" => break,
                    _ => {}
                }
            }
            if let (Some(x), Some(y)) = (x, y) {
                pts.push(Point::new(x, y));
            }
        }
    }
    Ok(pts)
}

/// Representation of a survey point with optional point number and description.
#[derive(Debug, Clone, PartialEq)]
pub struct SurveyPoint {
    pub number: Option<u32>,
    pub point: Point3,
    pub description: Option<String>,
}

/// Common point file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointFileFormat {
    PNEZD,
    PENZD,
    PNEZ,
    PENZ,
    NEZ,
    ENZ,
    NEZD,
    ENZD,
}

impl PointFileFormat {
    /// Parses a string to a [`PointFileFormat`]. Case insensitive.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "pnezd" => Some(Self::PNEZD),
            "penzd" => Some(Self::PENZD),
            "pnez" => Some(Self::PNEZ),
            "penz" => Some(Self::PENZ),
            "nez" => Some(Self::NEZ),
            "enz" => Some(Self::ENZ),
            "nezd" => Some(Self::NEZD),
            "enzd" => Some(Self::ENZD),
            _ => None,
        }
    }
}

/// Reads a survey point file using the specified [`PointFileFormat`].
pub fn read_point_file(path: &str, format: PointFileFormat) -> io::Result<Vec<SurveyPoint>> {
    let lines = read_lines(path)?;
    let mut pts = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = if line.contains(',') {
            line.split(',').collect()
        } else {
            line.split_whitespace().collect()
        };
        let parse_f64 = |s: &str| s.trim().parse::<f64>().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        let parse_u32 = |s: &str| s.trim().parse::<u32>().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        let p = match format {
            PointFileFormat::PNEZD => {
                if fields.len() < 4 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "expected at least 4 fields"));
                }
                let number = parse_u32(fields[0]).ok();
                let n = parse_f64(fields[1])?;
                let e = parse_f64(fields[2])?;
                let z = parse_f64(fields[3])?;
                let desc = if fields.len() > 4 {
                    Some(fields[4..].join(" "))
                } else {
                    None
                };
                SurveyPoint { number, point: Point3::new(e, n, z), description: desc }
            }
            PointFileFormat::PENZD => {
                if fields.len() < 4 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "expected at least 4 fields"));
                }
                let number = parse_u32(fields[0]).ok();
                let e = parse_f64(fields[1])?;
                let n = parse_f64(fields[2])?;
                let z = parse_f64(fields[3])?;
                let desc = if fields.len() > 4 {
                    Some(fields[4..].join(" "))
                } else {
                    None
                };
                SurveyPoint { number, point: Point3::new(e, n, z), description: desc }
            }
            PointFileFormat::PNEZ => {
                if fields.len() < 4 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "expected 4 fields"));
                }
                let number = parse_u32(fields[0]).ok();
                let n = parse_f64(fields[1])?;
                let e = parse_f64(fields[2])?;
                let z = parse_f64(fields[3])?;
                SurveyPoint { number, point: Point3::new(e, n, z), description: None }
            }
            PointFileFormat::PENZ => {
                if fields.len() < 4 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "expected 4 fields"));
                }
                let number = parse_u32(fields[0]).ok();
                let e = parse_f64(fields[1])?;
                let n = parse_f64(fields[2])?;
                let z = parse_f64(fields[3])?;
                SurveyPoint { number, point: Point3::new(e, n, z), description: None }
            }
            PointFileFormat::NEZ => {
                if fields.len() < 3 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "expected 3 fields"));
                }
                let n = parse_f64(fields[0])?;
                let e = parse_f64(fields[1])?;
                let z = parse_f64(fields[2])?;
                SurveyPoint { number: None, point: Point3::new(e, n, z), description: None }
            }
            PointFileFormat::ENZ => {
                if fields.len() < 3 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "expected 3 fields"));
                }
                let e = parse_f64(fields[0])?;
                let n = parse_f64(fields[1])?;
                let z = parse_f64(fields[2])?;
                SurveyPoint { number: None, point: Point3::new(e, n, z), description: None }
            }
            PointFileFormat::NEZD => {
                if fields.len() < 4 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "expected at least 4 fields"));
                }
                let n = parse_f64(fields[0])?;
                let e = parse_f64(fields[1])?;
                let z = parse_f64(fields[2])?;
                let desc = Some(fields[3..].join(" "));
                SurveyPoint { number: None, point: Point3::new(e, n, z), description: desc }
            }
            PointFileFormat::ENZD => {
                if fields.len() < 4 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "expected at least 4 fields"));
                }
                let e = parse_f64(fields[0])?;
                let n = parse_f64(fields[1])?;
                let z = parse_f64(fields[2])?;
                let desc = Some(fields[3..].join(" "));
                SurveyPoint { number: None, point: Point3::new(e, n, z), description: desc }
            }
        };
        pts.push(p);
    }
    Ok(pts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use survey_cad::io::write_points_dxf;

    #[test]
    fn read_written_dxf_points() {
        let path = std::env::temp_dir().join("import_pts.dxf");
        let pts = vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)];
        write_points_dxf(path.to_str().unwrap(), &pts).unwrap();
        let read = read_points_dxf(path.to_str().unwrap()).unwrap();
        assert_eq!(read, pts);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn read_pnezd_point_file() {
        let path = std::env::temp_dir().join("pnezd.txt");
        std::fs::write(&path, "1,100.0,200.0,50.0,TEST\n").unwrap();
        let pts = read_point_file(path.to_str().unwrap(), PointFileFormat::PNEZD).unwrap();
        assert_eq!(pts.len(), 1);
        let p = &pts[0];
        assert_eq!(p.number, Some(1));
        assert_eq!(p.point, Point3::new(200.0, 100.0, 50.0));
        assert_eq!(p.description.as_deref(), Some("TEST"));
        std::fs::remove_file(path).ok();
    }
}
