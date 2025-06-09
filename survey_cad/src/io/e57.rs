use crate::geometry::Point3;
use e57::{E57Reader, E57Writer, Record, RecordValue};
use uuid::Uuid;
use std::io;

/// Reads an E57 file and returns all point coordinates found in the file.
pub fn read_points_e57(path: &str) -> io::Result<Vec<Point3>> {
    let mut reader = E57Reader::from_file(path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
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

/// Writes a list of 3D points to an E57 file.
pub fn write_points_e57(path: &str, points: &[Point3]) -> io::Result<()> {
    let guid = Uuid::new_v4().to_string();
    let mut writer = E57Writer::from_file(path, &guid)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let prototype = vec![
        Record::CARTESIAN_X_F64,
        Record::CARTESIAN_Y_F64,
        Record::CARTESIAN_Z_F64,
    ];
    let mut pc_writer = writer
        .add_pointcloud(&guid, prototype)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for p in points {
        let values = vec![
            RecordValue::Double(p.x),
            RecordValue::Double(p.y),
            RecordValue::Double(p.z),
        ];
        pc_writer.add_point(values)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }
    pc_writer.finalize()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    writer.finalize()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
