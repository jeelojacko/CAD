use crate::geometry::Point3;
use las::{point::Point as LasPoint, Reader, Write as _};
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
