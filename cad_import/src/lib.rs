use std::io;

pub mod instrument;

use survey_cad::{
    geometry::Point,
    geometry::Point3,
    io::{
        read_dxf, read_lines, read_points_csv as sc_read_csv,
        read_points_geojson as sc_read_geojson, DxfEntity,
    },
};

/// Reads a CSV file of `x,y` pairs into [`Point`]s.
pub fn read_points_csv(
    path: &str,
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<Vec<Point>> {
    sc_read_csv(path, src_epsg, dst_epsg)
}

/// Reads a GeoJSON file of Point features into [`Point`]s.
pub fn read_points_geojson(
    path: &str,
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<Vec<Point>> {
    sc_read_geojson(path, src_epsg, dst_epsg)
}

/// Reads a DXF file and extracts all `POINT` entities.
pub fn read_points_dxf(path: &str) -> io::Result<Vec<Point>> {
    let entities = read_dxf(path)?;
    Ok(entities
        .into_iter()
        .filter_map(|e| match e {
            DxfEntity::Point { point, .. } => Some(point),
            _ => None,
        })
        .collect())
}

/// Representation of a survey point with optional point number and description.
#[derive(Debug, Clone, PartialEq)]
pub struct SurveyPoint {
    pub number: Option<u32>,
    pub point: Point3,
    pub description: Option<String>,
    pub codes: Vec<String>,
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

impl std::str::FromStr for PointFileFormat {
    type Err = ();

    /// Parses a string to a [`PointFileFormat`]. Case insensitive.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fmt = match s.to_ascii_lowercase().as_str() {
            "pnezd" => Self::PNEZD,
            "penzd" => Self::PENZD,
            "pnez" => Self::PNEZ,
            "penz" => Self::PENZ,
            "nez" => Self::NEZ,
            "enz" => Self::ENZ,
            "nezd" => Self::NEZD,
            "enzd" => Self::ENZD,
            _ => return Err(()),
        };
        Ok(fmt)
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
        let parse_f64 = |s: &str| {
            s.trim()
                .parse::<f64>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        };
        let parse_u32 = |s: &str| {
            s.trim()
                .parse::<u32>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        };
        let p = match format {
            PointFileFormat::PNEZD => {
                if fields.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected at least 4 fields",
                    ));
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
                SurveyPoint {
                    number,
                    point: Point3::new(e, n, z),
                    description: desc,
                    codes: Vec::new(),
                }
            }
            PointFileFormat::PENZD => {
                if fields.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected at least 4 fields",
                    ));
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
                SurveyPoint {
                    number,
                    point: Point3::new(e, n, z),
                    description: desc,
                    codes: Vec::new(),
                }
            }
            PointFileFormat::PNEZ => {
                if fields.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected 4 fields",
                    ));
                }
                let number = parse_u32(fields[0]).ok();
                let n = parse_f64(fields[1])?;
                let e = parse_f64(fields[2])?;
                let z = parse_f64(fields[3])?;
                SurveyPoint {
                    number,
                    point: Point3::new(e, n, z),
                    description: None,
                    codes: Vec::new(),
                }
            }
            PointFileFormat::PENZ => {
                if fields.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected 4 fields",
                    ));
                }
                let number = parse_u32(fields[0]).ok();
                let e = parse_f64(fields[1])?;
                let n = parse_f64(fields[2])?;
                let z = parse_f64(fields[3])?;
                SurveyPoint {
                    number,
                    point: Point3::new(e, n, z),
                    description: None,
                    codes: Vec::new(),
                }
            }
            PointFileFormat::NEZ => {
                if fields.len() < 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected 3 fields",
                    ));
                }
                let n = parse_f64(fields[0])?;
                let e = parse_f64(fields[1])?;
                let z = parse_f64(fields[2])?;
                SurveyPoint {
                    number: None,
                    point: Point3::new(e, n, z),
                    description: None,
                    codes: Vec::new(),
                }
            }
            PointFileFormat::ENZ => {
                if fields.len() < 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected 3 fields",
                    ));
                }
                let e = parse_f64(fields[0])?;
                let n = parse_f64(fields[1])?;
                let z = parse_f64(fields[2])?;
                SurveyPoint {
                    number: None,
                    point: Point3::new(e, n, z),
                    description: None,
                    codes: Vec::new(),
                }
            }
            PointFileFormat::NEZD => {
                if fields.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected at least 4 fields",
                    ));
                }
                let n = parse_f64(fields[0])?;
                let e = parse_f64(fields[1])?;
                let z = parse_f64(fields[2])?;
                let desc = Some(fields[3..].join(" "));
                SurveyPoint {
                    number: None,
                    point: Point3::new(e, n, z),
                    description: desc,
                    codes: Vec::new(),
                }
            }
            PointFileFormat::ENZD => {
                if fields.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "expected at least 4 fields",
                    ));
                }
                let e = parse_f64(fields[0])?;
                let n = parse_f64(fields[1])?;
                let z = parse_f64(fields[2])?;
                let desc = Some(fields[3..].join(" "));
                SurveyPoint {
                    number: None,
                    point: Point3::new(e, n, z),
                    description: desc,
                    codes: Vec::new(),
                }
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

    type SurveyPointResult = io::Result<Vec<SurveyPoint>>;
    type SurveyPointReaderFn = fn(&str) -> SurveyPointResult;

    #[test]
    fn read_written_dxf_points() {
        let path = std::env::temp_dir().join("import_pts.dxf");
        let pts = vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)];
        write_points_dxf(path.to_str().unwrap(), &pts, None, None).unwrap();
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

    #[test]
    fn read_leica_trimble_topcon_sokkia_raw() {
        use std::env::temp_dir;
        use std::fs::write;

        let content = "1,100.0,200.0,50.0,TEST";
        let brands: [(&str, SurveyPointReaderFn); 4] = [
            ("leica.raw", instrument::read_leica_raw),
            ("trimble.raw", instrument::read_trimble_raw),
            ("topcon.raw", instrument::read_topcon_raw),
            ("sokkia.raw", instrument::read_sokkia_raw),
        ];
        for (name, func) in brands {
            let path = temp_dir().join(name);
            write(&path, content).unwrap();
            let pts = func(path.to_str().unwrap()).unwrap();
            assert_eq!(pts.len(), 1);
            let p = &pts[0];
            assert_eq!(p.number, Some(1));
            assert_eq!(p.point, Point3::new(200.0, 100.0, 50.0));
            assert_eq!(p.description.as_deref(), Some("TEST"));
            std::fs::remove_file(path).ok();
        }
    }
}
