use super::SurveyPoint;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use survey_cad::geometry::Point3;

/// Parses a simple comma or whitespace separated raw file into survey points.
/// The expected order is point number, northing, easting, elevation, optional description.
fn parse_simple_raw(path: &str) -> io::Result<Vec<SurveyPoint>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut pts = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = if line.contains(',') {
            line.split(',').collect()
        } else {
            line.split_whitespace().collect()
        };
        if fields.len() < 4 {
            continue;
        }
        let number = fields[0].parse::<u32>().ok();
        let n: f64 = fields[1]
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let e: f64 = fields[2]
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let z: f64 = fields[3]
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let desc = if fields.len() > 4 {
            Some(fields[4..].join(" "))
        } else {
            None
        };
        pts.push(SurveyPoint {
            number,
            point: Point3::new(e, n, z),
            description: desc,
            codes: Vec::new(),
        });
    }
    Ok(pts)
}

/// Reads a Leica RAW file into survey points.
pub fn read_leica_raw(path: &str) -> io::Result<Vec<SurveyPoint>> {
    parse_simple_raw(path)
}

/// Reads a Trimble RAW file into survey points.
pub fn read_trimble_raw(path: &str) -> io::Result<Vec<SurveyPoint>> {
    parse_simple_raw(path)
}

/// Reads a Topcon RAW file into survey points.
pub fn read_topcon_raw(path: &str) -> io::Result<Vec<SurveyPoint>> {
    parse_simple_raw(path)
}

/// Reads a Sokkia RAW file into survey points.
pub fn read_sokkia_raw(path: &str) -> io::Result<Vec<SurveyPoint>> {
    parse_simple_raw(path)
}
