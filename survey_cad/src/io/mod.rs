//! File input and output helpers for project data.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};

use crate::geometry::Point;

/// Reads a file to string.
pub fn read_to_string(path: &str) -> io::Result<String> {
    let mut buffer = String::new();
    File::open(path)?.read_to_string(&mut buffer)?;
    Ok(buffer)
}

/// Writes the provided string slice to the given file path, overwriting any
/// existing contents.
pub fn write_string(path: &str, contents: &str) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents.as_bytes())
}

/// Reads a file and returns a vector of lines as `String`s.
pub fn read_lines(path: &str) -> io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    reader.lines().collect()
}

/// Reads a CSV file containing pairs of `x,y` coordinates into a list of
/// [`Point`]s.
///
/// Each line of the CSV file is expected to contain two floating point numbers
/// separated by a comma.
pub fn read_points_csv(path: &str) -> io::Result<Vec<Point>> {
    let lines = read_lines(path)?;
    lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "expected two comma-separated values, got {} on line: '{}'",
                        parts.len(),
                        line
                    ),
                ));
            }
            let x = parts[0]
                .trim()
                .parse::<f64>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let y = parts[1]
                .trim()
                .parse::<f64>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Point::new(x, y))
        })
        .collect()
}

/// Writes a slice of [`Point`]s to a CSV file with each line in the form
/// `x,y`.
pub fn write_points_csv(path: &str, points: &[Point]) -> io::Result<()> {
    let mut file = File::create(path)?;
    for p in points {
        writeln!(file, "{},{}", p.x, p.y)?;
    }
    Ok(())
}

/// Reads a GeoJSON file containing Point features into a list of [`Point`]s.
pub fn read_points_geojson(path: &str) -> io::Result<Vec<Point>> {
    let contents = read_to_string(path)?;
    let geojson: geojson::GeoJson = contents
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    match geojson {
        geojson::GeoJson::FeatureCollection(fc) => {
            let mut pts = Vec::new();
            for feature in fc.features {
                if let Some(geojson::Geometry {
                    value: geojson::Value::Point(coord),
                    ..
                }) = feature.geometry
                {
                    if coord.len() >= 2 {
                        pts.push(Point::new(coord[0], coord[1]));
                    }
                }
            }
            Ok(pts)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected FeatureCollection",
        )),
    }
}

/// Writes a slice of [`Point`]s to a GeoJSON file as Point features.
pub fn write_points_geojson(path: &str, points: &[Point]) -> io::Result<()> {
    use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
    let features: Vec<Feature> = points
        .iter()
        .map(|p| Feature {
            bbox: None,
            geometry: Some(Geometry::new(Value::Point(vec![p.x, p.y]))),
            id: None,
            properties: None,
            foreign_members: None,
        })
        .collect();
    let fc = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };
    write_string(path, &GeoJson::FeatureCollection(fc).to_string())
}

/// Writes a slice of [`Point`]s to a very simple ASCII DXF file containing
/// `POINT` entities. Only x and y coordinates are written.
pub fn write_points_dxf(path: &str, points: &[Point]) -> io::Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "0")?;
    writeln!(file, "SECTION")?;
    writeln!(file, "2")?;
    writeln!(file, "ENTITIES")?;
    for p in points {
        writeln!(file, "0")?;
        writeln!(file, "POINT")?;
        writeln!(file, "10")?;
        writeln!(file, "{}", p.x)?;
        writeln!(file, "20")?;
        writeln!(file, "{}", p.y)?;
        writeln!(file, "30")?;
        writeln!(file, "0.0")?;
    }
    writeln!(file, "0")?;
    writeln!(file, "ENDSEC")?;
    writeln!(file, "0")?;
    writeln!(file, "EOF")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_read_string() {
        let path = std::env::temp_dir().join("cad_io_test.txt");
        let path_str = path.to_str().unwrap();
        write_string(path_str, "hello world").unwrap();
        let contents = read_to_string(path_str).unwrap();
        assert_eq!(contents, "hello world");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_points() {
        let path = std::env::temp_dir().join("cad_points.csv");
        let path_str = path.to_str().unwrap();
        let pts = vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)];
        write_points_csv(path_str, &pts).unwrap();
        let read_pts = read_points_csv(path_str).unwrap();
        assert_eq!(read_pts, pts);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_geojson() {
        let path = std::env::temp_dir().join("cad_points.geojson");
        let path_str = path.to_str().unwrap();
        let pts = vec![Point::new(5.0, 6.0), Point::new(7.0, 8.0)];
        write_points_geojson(path_str, &pts).unwrap();
        let read_pts = read_points_geojson(path_str).unwrap();
        assert_eq!(read_pts, pts);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_dxf() {
        let path = std::env::temp_dir().join("cad_points.dxf");
        let path_str = path.to_str().unwrap();
        let pts = vec![Point::new(1.0, 1.0), Point::new(2.0, 2.0)];
        write_points_dxf(path_str, &pts).unwrap();
        assert!(std::fs::metadata(path_str).is_ok());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn read_points_csv_skips_empty_lines() {
        let path = std::env::temp_dir().join("cad_points_blank.csv");
        let path_str = path.to_str().unwrap();
        let contents = "1.0,2.0\n\n3.0,4.0\n";
        write_string(path_str, contents).unwrap();
        let pts = read_points_csv(path_str).unwrap();
        assert_eq!(pts, vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)]);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn read_points_csv_bad_field_count() {
        let path = std::env::temp_dir().join("cad_points_bad.csv");
        let path_str = path.to_str().unwrap();
        let contents = "1.0\n1.0,2.0,3.0\n";
        write_string(path_str, contents).unwrap();
        let err = read_points_csv(path_str).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        std::fs::remove_file(path).ok();
    }
}
