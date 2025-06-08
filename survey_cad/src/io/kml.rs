use crate::geometry::Point;
use std::io;

use kml::types::{Geometry as KmlGeometry, Placemark, Point as KmlPoint};
use kml::{Kml, KmlReader, KmlWriter};

/// Reads Point geometries from a KML or KMZ file.
pub fn read_points_kml(path: &str) -> io::Result<Vec<Point>> {
    let mut reader = if path.to_ascii_lowercase().ends_with(".kmz") {
        KmlReader::<_, f64>::from_kmz_path(path)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    } else {
        KmlReader::<_, f64>::from_path(path)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    };
    let kml = reader
        .read()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let collection = geo_types::GeometryCollection::<f64>::try_from(kml)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut pts = Vec::new();
    for geom in collection {
        if let geo_types::Geometry::Point(p) = geom {
            pts.push(Point::new(p.x(), p.y()));
        }
    }
    Ok(pts)
}

/// Writes Point geometries to a KML file.
pub fn write_points_kml(path: &str, points: &[Point]) -> io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut writer = KmlWriter::<_, f64>::from_writer(file);
    let placemarks: Vec<Kml> = points
        .iter()
        .map(|p| {
            Kml::Placemark(Placemark {
                geometry: Some(KmlGeometry::Point(KmlPoint::new(p.x, p.y, None))),
                ..Default::default()
            })
        })
        .collect();
    let doc = Kml::Document {
        attrs: Default::default(),
        elements: placemarks,
    };
    writer
        .write(&doc)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
