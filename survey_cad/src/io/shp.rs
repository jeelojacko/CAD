use super::read_points_csv;
use crate::geometry::Point;
use shapefile::{Point as ShpPoint, Shape, ShapeReader, ShapeWriter};
use std::io::{self, Write};

/// Reads a shapefile containing Point geometries and returns them as [`Point`] values.
pub fn read_points_shp(path: &str) -> io::Result<Vec<Point>> {
    let mut reader =
        ShapeReader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut pts = Vec::new();
    for record in reader.iter_shapes() {
        match record.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))? {
            Shape::Point(p) => pts.push(Point::new(p.x, p.y)),
            _ => {}
        }
    }
    Ok(pts)
}

/// Writes a list of [`Point`]s to a shapefile.
pub fn write_points_shp(path: &str, points: &[Point]) -> io::Result<()> {
    let mut writer =
        ShapeWriter::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for p in points {
        writer
            .write_shape(Shape::Point(ShpPoint { x: p.x, y: p.y }))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }
    writer
        .close()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
