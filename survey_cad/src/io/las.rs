use crate::geometry::Point3;
use las::{point::Format, point::Point as LasPoint, Builder, Reader, Version, Writer};
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

/// Reads a LAS file reporting progress. The callback receives values in the
/// range 0.0..=1.0 and should return `true` to continue or `false` to cancel.
pub fn read_points_las_progress<F>(path: &str, mut progress: F) -> io::Result<Vec<Point3>>
where
    F: FnMut(f32) -> bool,
{
    let mut reader =
        Reader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let total = reader.header().number_of_points();
    let mut pts = Vec::with_capacity(total as usize);
    for (i, wrapped) in reader.points().enumerate() {
        let p: LasPoint = wrapped.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        pts.push(Point3::new(p.x, p.y, p.z));
        if i % 1000 == 0 && !progress(i as f32 / total as f32) {
            break;
        }
    }
    progress(1.0);
    Ok(pts)
}

/// Writes points to a LAS or LAZ file. Compression is inferred from the
/// file extension when the `laz` feature of the `las` crate is enabled.
pub fn write_points_las(path: &str, points: &[Point3]) -> io::Result<()> {
    let mut builder = Builder::default();
    builder.point_format = Format::new(0).unwrap();
    builder.version = Version::new(1, 2);
    let header = builder.into_header().map_err(io::Error::other)?;
    let mut writer = Writer::from_path(path, header).map_err(io::Error::other)?;
    for p in points {
        let lp = LasPoint {
            x: p.x,
            y: p.y,
            z: p.z,
            ..Default::default()
        };
        writer.write_point(lp).map_err(io::Error::other)?;
    }
    writer.close().map_err(io::Error::other)
}
