use crate::geometry::Point;
use std::io;

use gdal::vector::LayerAccess;
use gdal::vector::OGRwkbGeometryType;
use gdal::Dataset;

/// Reads Point features from an ESRI File Geodatabase layer.
pub fn read_points_fgdb(path: &str, layer_name: &str) -> io::Result<Vec<Point>> {
    let ds = Dataset::open(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let mut layer = ds
        .layer_by_name(layer_name)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let mut pts = Vec::new();
    for feature in layer.features() {
        if let Some(geom) = feature.geometry() {
            match geom.geometry_type() {
                OGRwkbGeometryType::wkbPoint | OGRwkbGeometryType::wkbPoint25D => {
                    let (x, y, _) = geom.get_point(0);
                    pts.push(Point::new(x, y));
                }
                _ => {}
            }
        }
    }
    Ok(pts)
}
