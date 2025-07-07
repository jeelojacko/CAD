use crate::geometry::Point3;
use e57::{E57Reader, E57Writer, Record, RecordValue};
use std::io;
use uuid::Uuid;

/// Reads an E57 file and returns all point coordinates found in the file.
pub fn read_points_e57(path: &str) -> io::Result<Vec<Point3>> {
    let mut reader =
        E57Reader::from_file(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut pts = Vec::new();
    for pc in reader.pointclouds() {
        let mut iter = reader
            .pointcloud_simple(&pc)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        for p in &mut iter {
            let p = p.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if let e57::CartesianCoordinate::Valid { x, y, z } = p.cartesian {
                pts.push(Point3::new(x, y, z));
            }
        }
    }
    Ok(pts)
}

/// Reads an E57 file while reporting progress. The callback receives a fraction
/// between 0.0 and 1.0 and should return `true` to continue or `false` to cancel.
pub fn read_points_e57_progress<F>(path: &str, mut progress: F) -> io::Result<Vec<Point3>>
where
    F: FnMut(f32) -> bool,
{
    let mut reader =
        E57Reader::from_file(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let pcs = reader.pointclouds();
    let total: u64 = pcs.iter().map(|pc| pc.records).sum();
    let mut pts = Vec::with_capacity(total as usize);
    let mut read = 0u64;
    for pc in pcs {
        let mut iter = reader
            .pointcloud_simple(&pc)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        for p in &mut iter {
            let p = p.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if let e57::CartesianCoordinate::Valid { x, y, z } = p.cartesian {
                pts.push(Point3::new(x, y, z));
            }
            read += 1;
            if read % 1000 == 0 && !progress(read as f32 / total as f32) {
                return Ok(pts);
            }
        }
    }
    progress(1.0);
    Ok(pts)
}

/// Writes a list of 3D points to an E57 file.
pub fn write_points_e57(path: &str, points: &[Point3]) -> io::Result<()> {
    let guid = Uuid::new_v4().to_string();
    let mut writer = E57Writer::from_file(path, &guid).map_err(io::Error::other)?;
    let prototype = vec![
        Record::CARTESIAN_X_F64,
        Record::CARTESIAN_Y_F64,
        Record::CARTESIAN_Z_F64,
    ];
    let mut pc_writer = writer
        .add_pointcloud(&guid, prototype)
        .map_err(io::Error::other)?;
    for p in points {
        let values = vec![
            RecordValue::Double(p.x),
            RecordValue::Double(p.y),
            RecordValue::Double(p.z),
        ];
        pc_writer.add_point(values).map_err(io::Error::other)?;
    }
    pc_writer.finalize().map_err(io::Error::other)?;
    writer.finalize().map_err(io::Error::other)
}
