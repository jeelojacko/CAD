//! File input and output helpers for project data.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};

use crate::crs::Crs;

use crate::geometry::{Arc, Point, Point3, Polyline};

#[cfg(feature = "e57")]
pub mod e57;
#[cfg(feature = "fgdb")]
pub mod fgdb;
#[cfg(feature = "kml")]
pub mod kml;
pub mod landxml;
#[cfg(feature = "las")]
pub mod las;
#[cfg(feature = "shapefile")]
pub mod shp;
pub mod ifc;
pub mod project;

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
pub fn read_points_csv(
    path: &str,
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<Vec<Point>> {
    let lines = read_lines(path)?;
    let mut pts: Vec<Point> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(idx, line)| {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: expected two comma-separated values", idx + 1),
                ));
            }
            let x = parts[0].trim().parse::<f64>().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: {}", idx + 1, e),
                )
            })?;
            let y = parts[1].trim().parse::<f64>().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: {}", idx + 1, e),
                )
            })?;
            Ok(Point::new(x, y))
        })
        .collect::<Result<_, _>>()?;
    if let (Some(src), Some(dst)) = (src_epsg, dst_epsg) {
        if src != dst {
            let from = Crs::from_epsg(src);
            let to = Crs::from_epsg(dst);
            for p in &mut pts {
                if let Some((x, y)) = from.transform_point(&to, p.x, p.y) {
                    p.x = x;
                    p.y = y;
                }
            }
        }
    }
    Ok(pts)
}

/// Writes a slice of [`Point`]s to a CSV file with each line in the form
/// `x,y`.
pub fn write_points_csv(
    path: &str,
    points: &[Point],
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<()> {
    let mut file = File::create(path)?;
    let from = src_epsg.map(Crs::from_epsg);
    let to = dst_epsg.map(Crs::from_epsg);
    for p in points {
        let (x, y) = match (&from, &to) {
            (Some(f), Some(t)) if f != t => f.transform_point(t, p.x, p.y).unwrap_or((p.x, p.y)),
            _ => (p.x, p.y),
        };
        writeln!(file, "{},{}", x, y)?;
    }
    Ok(())
}

use crate::point_database::PointDatabase;

pub fn read_point_database_csv(
    path: &str,
    db: &mut PointDatabase,
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<()> {
    let pts = read_points_csv(path, src_epsg, dst_epsg)?;
    db.clear();
    db.extend(pts);
    Ok(())
}

pub fn write_point_database_csv(
    path: &str,
    db: &PointDatabase,
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<()> {
    write_points_csv(path, db.points(), src_epsg, dst_epsg)
}

/// Writes 3D points to a CSV file in `x,y,z` format commonly used for GNSS exports.
pub fn write_points_csv_gnss(path: &str, points: &[Point3]) -> io::Result<()> {
    let mut file = File::create(path)?;
    for p in points {
        writeln!(file, "{:.1},{:.1},{:.1}", p.x, p.y, p.z)?;
    }
    Ok(())
}

/// Writes a simple RAW file with point number, northing, easting and elevation.
/// This format is compatible with many total station controllers.
pub fn write_points_raw(path: &str, points: &[Point3]) -> io::Result<()> {
    let mut file = File::create(path)?;
    for (i, p) in points.iter().enumerate() {
        writeln!(file, "{},{:.1},{:.1},{:.1}", i + 1, p.y, p.x, p.z)?;
    }
    Ok(())
}

/// Reads a GeoJSON file containing Point features into a list of [`Point`]s.
pub fn read_points_geojson(
    path: &str,
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<Vec<Point>> {
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
            if let (Some(src), Some(dst)) = (src_epsg, dst_epsg) {
                if src != dst {
                    let from = Crs::from_epsg(src);
                    let to = Crs::from_epsg(dst);
                    for p in &mut pts {
                        if let Some((x, y)) = from.transform_point(&to, p.x, p.y) {
                            p.x = x;
                            p.y = y;
                        }
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
pub fn write_points_geojson(
    path: &str,
    points: &[Point],
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<()> {
    use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
    let from = src_epsg.map(Crs::from_epsg);
    let to = dst_epsg.map(Crs::from_epsg);
    let features: Vec<Feature> = points
        .iter()
        .map(|p| {
            let (x, y) = match (&from, &to) {
                (Some(f), Some(t)) if f != t => {
                    f.transform_point(t, p.x, p.y).unwrap_or((p.x, p.y))
                }
                _ => (p.x, p.y),
            };
            Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::Point(vec![x, y]))),
                id: None,
                properties: None,
                foreign_members: None,
            }
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
pub fn write_points_dxf(
    path: &str,
    points: &[Point],
    src_epsg: Option<u32>,
    dst_epsg: Option<u32>,
) -> io::Result<()> {
    let mut file = File::create(path)?;
    let from = src_epsg.map(Crs::from_epsg);
    let to = dst_epsg.map(Crs::from_epsg);
    writeln!(file, "0")?;
    writeln!(file, "SECTION")?;
    writeln!(file, "2")?;
    writeln!(file, "ENTITIES")?;
    for p in points {
        let (x, y) = match (&from, &to) {
            (Some(f), Some(t)) if f != t => f.transform_point(t, p.x, p.y).unwrap_or((p.x, p.y)),
            _ => (p.x, p.y),
        };
        writeln!(file, "0")?;
        writeln!(file, "POINT")?;
        writeln!(file, "10")?;
        writeln!(file, "{}", x)?;
        writeln!(file, "20")?;
        writeln!(file, "{}", y)?;
        writeln!(file, "30")?;
        writeln!(file, "0.0")?;
    }
    writeln!(file, "0")?;
    writeln!(file, "ENDSEC")?;
    writeln!(file, "0")?;
    writeln!(file, "EOF")?;
    Ok(())
}

/// Basic DXF entity types supported by the simple reader and writer.
#[derive(Debug, Clone, PartialEq)]
pub enum DxfEntity {
    Point {
        point: Point,
        layer: Option<String>,
    },
    Line {
        line: crate::geometry::Line,
        layer: Option<String>,
    },
    Polyline {
        polyline: Polyline,
        layer: Option<String>,
    },
    Arc {
        arc: Arc,
        layer: Option<String>,
    },
    Text {
        position: Point,
        height: f64,
        value: String,
        layer: Option<String>,
    },
}

/// Writes a collection of [`DxfEntity`] instances to a very simple DXF file.
pub fn write_dxf(path: &str, entities: &[DxfEntity]) -> io::Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "0")?;
    writeln!(file, "SECTION")?;
    writeln!(file, "2")?;
    writeln!(file, "ENTITIES")?;
    for e in entities {
        match e {
            DxfEntity::Point { point, layer } => {
                writeln!(file, "0")?;
                writeln!(file, "POINT")?;
                if let Some(l) = layer {
                    writeln!(file, "8")?;
                    writeln!(file, "{}", l)?;
                }
                writeln!(file, "10")?;
                writeln!(file, "{}", point.x)?;
                writeln!(file, "20")?;
                writeln!(file, "{}", point.y)?;
                writeln!(file, "30")?;
                writeln!(file, "0.0")?;
            }
            DxfEntity::Line { line, layer } => {
                writeln!(file, "0")?;
                writeln!(file, "LINE")?;
                if let Some(l) = layer {
                    writeln!(file, "8")?;
                    writeln!(file, "{}", l)?;
                }
                writeln!(file, "10")?;
                writeln!(file, "{}", line.start.x)?;
                writeln!(file, "20")?;
                writeln!(file, "{}", line.start.y)?;
                writeln!(file, "11")?;
                writeln!(file, "{}", line.end.x)?;
                writeln!(file, "21")?;
                writeln!(file, "{}", line.end.y)?;
            }
            DxfEntity::Polyline { polyline, layer } => {
                writeln!(file, "0")?;
                writeln!(file, "POLYLINE")?;
                if let Some(l) = layer {
                    writeln!(file, "8")?;
                    writeln!(file, "{}", l)?;
                }
                writeln!(file, "66")?;
                writeln!(file, "1")?;
                writeln!(file, "70")?;
                writeln!(file, "0")?;
                for v in &polyline.vertices {
                    writeln!(file, "0")?;
                    writeln!(file, "VERTEX")?;
                    if let Some(l) = layer {
                        writeln!(file, "8")?;
                        writeln!(file, "{}", l)?;
                    }
                    writeln!(file, "10")?;
                    writeln!(file, "{}", v.x)?;
                    writeln!(file, "20")?;
                    writeln!(file, "{}", v.y)?;
                    writeln!(file, "30")?;
                    writeln!(file, "0.0")?;
                }
                writeln!(file, "0")?;
                writeln!(file, "SEQEND")?;
            }
            DxfEntity::Arc { arc, layer } => {
                writeln!(file, "0")?;
                writeln!(file, "ARC")?;
                if let Some(l) = layer {
                    writeln!(file, "8")?;
                    writeln!(file, "{}", l)?;
                }
                writeln!(file, "10")?;
                writeln!(file, "{}", arc.center.x)?;
                writeln!(file, "20")?;
                writeln!(file, "{}", arc.center.y)?;
                writeln!(file, "40")?;
                writeln!(file, "{}", arc.radius)?;
                writeln!(file, "50")?;
                writeln!(file, "{}", arc.start_angle.to_degrees())?;
                writeln!(file, "51")?;
                writeln!(file, "{}", arc.end_angle.to_degrees())?;
            }
            DxfEntity::Text {
                position,
                height,
                value,
                layer,
            } => {
                writeln!(file, "0")?;
                writeln!(file, "TEXT")?;
                if let Some(l) = layer {
                    writeln!(file, "8")?;
                    writeln!(file, "{}", l)?;
                }
                writeln!(file, "10")?;
                writeln!(file, "{}", position.x)?;
                writeln!(file, "20")?;
                writeln!(file, "{}", position.y)?;
                writeln!(file, "40")?;
                writeln!(file, "{}", height)?;
                writeln!(file, "1")?;
                writeln!(file, "{}", value)?;
            }
        }
    }
    writeln!(file, "0")?;
    writeln!(file, "ENDSEC")?;
    writeln!(file, "0")?;
    writeln!(file, "EOF")?;
    Ok(())
}

/// Reads a very simple DXF file and returns any supported [`DxfEntity`] values.
pub fn read_dxf(path: &str) -> io::Result<Vec<DxfEntity>> {
    let lines = read_lines(path)?;
    let mut iter = lines.iter();
    let mut entities = Vec::new();
    while let (Some(code), Some(value)) = (iter.next(), iter.next()) {
        if code.trim() != "0" {
            continue;
        }
        match value.trim() {
            "POINT" => {
                let mut x = None;
                let mut y = None;
                let mut layer = None;
                while let (Some(c), Some(v)) = (iter.next(), iter.next()) {
                    match c.trim() {
                        "8" => layer = Some(v.trim().to_string()),
                        "10" => x = v.trim().parse().ok(),
                        "20" => y = v.trim().parse().ok(),
                        "30" => break,
                        _ => {}
                    }
                }
                if let (Some(x), Some(y)) = (x, y) {
                    entities.push(DxfEntity::Point {
                        point: Point::new(x, y),
                        layer,
                    });
                }
            }
            "LINE" => {
                let mut sx = None;
                let mut sy = None;
                let mut ex = None;
                let mut ey = None;
                let mut layer = None;
                while let (Some(c), Some(v)) = (iter.next(), iter.next()) {
                    match c.trim() {
                        "8" => layer = Some(v.trim().to_string()),
                        "10" => sx = v.trim().parse().ok(),
                        "20" => sy = v.trim().parse().ok(),
                        "11" => ex = v.trim().parse().ok(),
                        "21" => {
                            ey = v.trim().parse().ok();
                            break;
                        }
                        _ => {}
                    }
                }
                if let (Some(sx), Some(sy), Some(ex), Some(ey)) = (sx, sy, ex, ey) {
                    entities.push(DxfEntity::Line {
                        line: crate::geometry::Line::new(Point::new(sx, sy), Point::new(ex, ey)),
                        layer,
                    });
                }
            }
            "POLYLINE" => {
                let mut verts = Vec::new();
                let mut layer = None;
                while let (Some(c), Some(v)) = (iter.next(), iter.next()) {
                    match c.trim() {
                        "8" => layer = Some(v.trim().to_string()),
                        "0" if v.trim() == "VERTEX" => {
                            let mut vx = None;
                            let mut vy = None;
                            while let (Some(c2), Some(v2)) = (iter.next(), iter.next()) {
                                match c2.trim() {
                                    "10" => vx = v2.trim().parse().ok(),
                                    "20" => vy = v2.trim().parse().ok(),
                                    "30" => break,
                                    _ => {}
                                }
                            }
                            if let (Some(x), Some(y)) = (vx, vy) {
                                verts.push(Point::new(x, y));
                            }
                        }
                        "0" if v.trim() == "SEQEND" => break,
                        _ => {}
                    }
                }
                if !verts.is_empty() {
                    entities.push(DxfEntity::Polyline {
                        polyline: Polyline::new(verts),
                        layer,
                    });
                }
            }
            "ARC" => {
                let mut cx = None;
                let mut cy = None;
                let mut radius = None;
                let mut start = None;
                let mut end = None;
                let mut layer = None;
                while let (Some(c), Some(v)) = (iter.next(), iter.next()) {
                    match c.trim() {
                        "8" => layer = Some(v.trim().to_string()),
                        "10" => cx = v.trim().parse().ok(),
                        "20" => cy = v.trim().parse().ok(),
                        "40" => radius = v.trim().parse().ok(),
                        "50" => start = v.trim().parse::<f64>().ok().map(|d| d.to_radians()),
                        "51" => {
                            end = v.trim().parse::<f64>().ok().map(|d| d.to_radians());
                            break;
                        }
                        _ => {}
                    }
                }
                if let (Some(cx), Some(cy), Some(r), Some(sa), Some(ea)) =
                    (cx, cy, radius, start, end)
                {
                    let arc = Arc::new(Point::new(cx, cy), r, sa, ea);
                    entities.push(DxfEntity::Arc { arc, layer });
                }
            }
            "TEXT" => {
                let mut x = None;
                let mut y = None;
                let mut h = None;
                let mut val = None;
                let mut layer = None;
                while let (Some(c), Some(v)) = (iter.next(), iter.next()) {
                    match c.trim() {
                        "8" => layer = Some(v.trim().to_string()),
                        "10" => x = v.trim().parse().ok(),
                        "20" => y = v.trim().parse().ok(),
                        "40" => h = v.trim().parse().ok(),
                        "1" => {
                            val = Some(v.trim().to_string());
                            break;
                        }
                        _ => {}
                    }
                }
                if let (Some(x), Some(y), Some(h), Some(val)) = (x, y, h, val) {
                    entities.push(DxfEntity::Text {
                        position: Point::new(x, y),
                        height: h,
                        value: val,
                        layer,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(entities)
}

/// Writes supported [`DxfEntity`] values to a DWG file using the external
/// `dxf2dwg` command from the LibreDWG project. The entities are first written
/// to a temporary DXF file and then converted to DWG. An error is returned if
/// the command fails or is not available.
pub fn write_dwg(path: &str, entities: &[DxfEntity]) -> io::Result<()> {
    use std::process::Command;
    use tempfile::NamedTempFile;

    let tmp = NamedTempFile::new()?;
    write_dxf(tmp.path().to_str().unwrap(), entities)?;
    let status = Command::new("dxf2dwg")
        .arg(tmp.path())
        .arg(path)
        .status()
        .map_err(|e| io::Error::other(format!("failed to spawn dxf2dwg: {e}")))?;
    if !status.success() {
        return Err(io::Error::other("dxf2dwg failed"));
    }
    Ok(())
}

/// Reads supported [`DxfEntity`] values from a DWG file using the external
/// `dwg2dxf` command from the LibreDWG project. The DWG is converted to a
/// temporary DXF file which is then parsed using [`read_dxf`]. An error is
/// returned if the command fails or is not available.
pub fn read_dwg(path: &str) -> io::Result<Vec<DxfEntity>> {
    use std::process::Command;
    use tempfile::NamedTempFile;

    let tmp = NamedTempFile::new()?;
    let status = Command::new("dwg2dxf")
        .arg(path)
        .arg(tmp.path())
        .status()
        .map_err(|e| io::Error::other(format!("failed to spawn dwg2dxf: {e}")))?;
    if !status.success() {
        return Err(io::Error::other("dwg2dxf failed"));
    }
    read_dxf(tmp.path().to_str().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alignment::{
        HorizontalAlignment, HorizontalElement, VerticalAlignment, VerticalElement,
    };
    use crate::corridor::CrossSection;
    use crate::dtm::Tin;
    use crate::geometry::Point3;
    use crate::superelevation::SuperelevationPoint;

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
        write_points_csv(path_str, &pts, None, None).unwrap();
        let read_pts = read_points_csv(path_str, None, None).unwrap();
        assert_eq!(read_pts, pts);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_geojson() {
        let path = std::env::temp_dir().join("cad_points.geojson");
        let path_str = path.to_str().unwrap();
        let pts = vec![Point::new(5.0, 6.0), Point::new(7.0, 8.0)];
        write_points_geojson(path_str, &pts, None, None).unwrap();
        let read_pts = read_points_geojson(path_str, None, None).unwrap();
        assert_eq!(read_pts, pts);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_points_dxf_test() {
        let path = std::env::temp_dir().join("cad_points.dxf");
        let path_str = path.to_str().unwrap();
        let pts = vec![Point::new(1.0, 1.0), Point::new(2.0, 2.0)];
        write_points_dxf(path_str, &pts, None, None).unwrap();
        assert!(std::fs::metadata(path_str).is_ok());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_dxf_entities() {
        let path = std::env::temp_dir().join("entities.dxf");
        let path_str = path.to_str().unwrap();
        let entities = vec![
            DxfEntity::Point {
                point: Point::new(0.0, 0.0),
                layer: Some("P".into()),
            },
            DxfEntity::Line {
                line: crate::geometry::Line::new(Point::new(0.0, 0.0), Point::new(1.0, 0.0)),
                layer: Some("L1".into()),
            },
            DxfEntity::Polyline {
                polyline: Polyline::new(vec![Point::new(1.0, 1.0), Point::new(2.0, 2.0)]),
                layer: Some("L".into()),
            },
            DxfEntity::Arc {
                arc: Arc::new(Point::new(3.0, 3.0), 1.0, 0.0, std::f64::consts::FRAC_PI_2),
                layer: None,
            },
            DxfEntity::Text {
                position: Point::new(5.0, 5.0),
                height: 2.5,
                value: "Hello".into(),
                layer: None,
            },
        ];
        write_dxf(path_str, &entities).unwrap();
        let read = read_dxf(path_str).unwrap();
        assert_eq!(read.len(), 5);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn dwg_functions_return_error_without_tools() {
        let path = std::env::temp_dir().join("dummy.dwg");
        let path_str = path.to_str().unwrap();
        let entities = vec![DxfEntity::Point {
            point: Point::new(0.0, 0.0),
            layer: None,
        }];
        let err = write_dwg(path_str, &entities).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        let err2 = read_dwg(path_str).unwrap_err();
        assert_eq!(err2.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn read_points_csv_skips_empty_lines() {
        let path = std::env::temp_dir().join("cad_points_blank.csv");
        let path_str = path.to_str().unwrap();
        let contents = "1.0,2.0\n\n3.0,4.0\n";
        write_string(path_str, contents).unwrap();
        let pts = read_points_csv(path_str, None, None).unwrap();
        assert_eq!(pts, vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)]);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn read_points_csv_bad_field_count() {
        let path = std::env::temp_dir().join("cad_points_bad.csv");
        let path_str = path.to_str().unwrap();
        let contents = "1.0\n1.0,2.0,3.0\n";
        write_string(path_str, contents).unwrap();
        let err = read_points_csv(path_str, None, None).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("line 1"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn read_points_csv_parse_error_reports_line() {
        let path = std::env::temp_dir().join("cad_points_parse.csv");
        let path_str = path.to_str().unwrap();
        let contents = "1.0,2.0\nabc,3.0\n";
        write_string(path_str, contents).unwrap();
        let err = read_points_csv(path_str, None, None).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("line 2"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_landxml_surface() {
        let path = std::env::temp_dir().join("surf.xml");
        let tin = Tin {
            vertices: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
        };
        landxml::write_landxml_surface(path.to_str().unwrap(), &tin).unwrap();
        let read = landxml::read_landxml_surface(path.to_str().unwrap()).unwrap();
        assert_eq!(read.vertices.len(), 3);
        assert_eq!(read.triangles.len(), 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_landxml_alignment() {
        let path = std::env::temp_dir().join("align.xml");
        let hal = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)]);
        landxml::write_landxml_alignment(path.to_str().unwrap(), &hal).unwrap();
        let read = landxml::read_landxml_alignment(path.to_str().unwrap()).unwrap();
        assert_eq!(read.elements.len(), 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_landxml_alignment_with_curve() {
        use std::f64::consts::PI;
        let path = std::env::temp_dir().join("align_curve.xml");
        let mut elements = Vec::new();
        elements.push(HorizontalElement::Tangent {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
        });
        let arc = Arc::new(Point::new(10.0, 5.0), 5.0, -PI / 2.0, 0.0);
        elements.push(HorizontalElement::Curve { arc });
        let hal = HorizontalAlignment { elements };
        landxml::write_landxml_alignment(path.to_str().unwrap(), &hal).unwrap();
        let read = landxml::read_landxml_alignment(path.to_str().unwrap()).unwrap();
        assert_eq!(read.elements.len(), 2);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_landxml_profile() {
        let path = std::env::temp_dir().join("profile.xml");
        let valign = VerticalAlignment {
            elements: vec![
                VerticalElement::Grade {
                    start_station: 0.0,
                    end_station: 10.0,
                    start_elev: 0.0,
                    end_elev: 5.0,
                },
                VerticalElement::Parabola {
                    start_station: 10.0,
                    end_station: 20.0,
                    start_elev: 5.0,
                    start_grade: 0.5,
                    end_grade: 0.0,
                },
            ],
        };
        landxml::write_landxml_profile(path.to_str().unwrap(), &valign).unwrap();
        let read = landxml::read_landxml_profile(path.to_str().unwrap()).unwrap();
        assert_eq!(read.elements.len(), 2);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_landxml_cross_sections() {
        let path = std::env::temp_dir().join("cross.xml");
        let secs = vec![
            CrossSection::new(
                0.0,
                vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)],
            ),
            CrossSection::new(
                10.0,
                vec![Point3::new(0.0, 1.0, 0.0), Point3::new(1.0, 1.0, 0.0)],
            ),
        ];
        landxml::write_landxml_cross_sections(path.to_str().unwrap(), &secs).unwrap();
        let read = landxml::read_landxml_cross_sections(path.to_str().unwrap()).unwrap();
        assert_eq!(read.len(), 2);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_landxml_superelevation() {
        let path = std::env::temp_dir().join("sup.xml");
        let table = vec![
            SuperelevationPoint {
                station: 0.0,
                left_slope: 0.02,
                right_slope: -0.02,
            },
            SuperelevationPoint {
                station: 10.0,
                left_slope: 0.03,
                right_slope: -0.03,
            },
        ];
        landxml::write_landxml_superelevation(path.to_str().unwrap(), &table).unwrap();
        let read = landxml::read_landxml_superelevation(path.to_str().unwrap()).unwrap();
        assert_eq!(read.len(), 2);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_points_csv_gnss_creates_file() {
        let path = std::env::temp_dir().join("gnss.csv");
        let pts = vec![Point3::new(1.0, 2.0, 3.0)];
        write_points_csv_gnss(path.to_str().unwrap(), &pts).unwrap();
        let contents = read_to_string(path.to_str().unwrap()).unwrap();
        assert!(contents.starts_with("1.0,2.0,3.0"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_points_raw_creates_file() {
        let path = std::env::temp_dir().join("pts.raw");
        let pts = vec![Point3::new(1.0, 2.0, 3.0)];
        write_points_raw(path.to_str().unwrap(), &pts).unwrap();
        let contents = read_to_string(path.to_str().unwrap()).unwrap();
        assert!(contents.starts_with("1,2.0,1.0,3.0"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_ifc_points_creates_file() {
        let path = std::env::temp_dir().join("pts.ifc");
        let pts = vec![Point3::new(0.0, 1.0, 2.0)];
        ifc::write_ifc_points(path.to_str().unwrap(), &pts, Some(4326)).unwrap();
        let contents = read_to_string(path.to_str().unwrap()).unwrap();
        assert!(contents.contains("IFCCARTESIANPOINT"));
        std::fs::remove_file(path).ok();
    }

    #[cfg(feature = "shapefile")]
    #[test]
    fn shp_point_record_to_feature() {
        use crate::io::shp::{point_record_to_feature, PointRecord};
        use shapefile::dbase::FieldValue;
        let mut attrs = std::collections::BTreeMap::new();
        attrs.insert("NAME".to_string(), FieldValue::Character(Some("A".into())));
        let rec = PointRecord { geom: Point::new(1.0, 2.0), geom_z: None, attrs };
        let feat = point_record_to_feature(rec, Some("test".into()));
        assert_eq!(feat.class.as_deref(), Some("test"));
        assert_eq!(feat.attributes.get("NAME").unwrap(), "A");
    }
}
