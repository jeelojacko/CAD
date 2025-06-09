use crate::geometry::Point3;
use las::{point::Point as LasPoint, Reader, Writer, point::Format, Builder, Version};
use std::io;

/// Reads a LAS file and returns the contained points.
pub fn read_points_las(path: &str) -> io::Result<Vec<Point3>> {
    let mut reader =
        Reader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut pts = Vec::new();
    for wrapped in reader.points() {
        let p: LasPoint = wrapped.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        pts.push(Point3::new(p.x, p.y, p.z));
    }
    Ok(pts)
}

/// Writes points to a LAS or LAZ file. Compression is inferred from the
/// file extension when the `laz` feature of the `las` crate is enabled.
pub fn write_points_las(path: &str, points: &[Point3]) -> io::Result<()> {
    let mut builder = Builder::default();
    builder.point_format = Format::new(0).unwrap();
    builder.version = Version::new(1, 2);
    let header = builder
        .into_header()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let mut writer = Writer::from_path(path, header)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for p in points {
        let lp = LasPoint { x: p.x, y: p.y, z: p.z, ..Default::default() };
        writer
            .write_point(lp)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }
    writer.close().map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
