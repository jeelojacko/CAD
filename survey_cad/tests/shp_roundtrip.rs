#[cfg(feature = "shapefile")]
use survey_cad::io::shp::{
    read_points_shp, write_points_shp,
    read_polylines_shp, write_polylines_shp,
    read_polygons_shp, write_polygons_shp,
};
#[cfg(feature = "shapefile")]
use survey_cad::geometry::{Point, Polyline};

#[cfg(feature = "shapefile")]
#[test]
fn points_roundtrip() {
    use tempfile::NamedTempFile;
    let pts = vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)];
    let file = NamedTempFile::new().unwrap();
    write_points_shp(file.path().to_str().unwrap(), &pts, None).unwrap();
    let (pts_read, z) = read_points_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(pts_read, pts);
    assert!(z.is_none());
}

#[cfg(feature = "shapefile")]
#[test]
fn polylines_roundtrip() {
    use tempfile::NamedTempFile;
    let pl = Polyline::new(vec![Point::new(0.0,0.0), Point::new(1.0,0.0), Point::new(2.0,1.0)]);
    let file = NamedTempFile::new().unwrap();
    write_polylines_shp(file.path().to_str().unwrap(), &[pl.clone()], None).unwrap();
    let (lines, z) = read_polylines_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(lines[0], pl);
    assert!(z.is_none());
}

#[cfg(feature = "shapefile")]
#[test]
fn polygons_roundtrip() {
    use tempfile::NamedTempFile;
    let poly = vec![
        Point::new(0.0,0.0),
        Point::new(1.0,1.0),
        Point::new(1.0,0.0),
        Point::new(0.0,0.0),
    ];
    let file = NamedTempFile::new().unwrap();
    write_polygons_shp(file.path().to_str().unwrap(), &[poly.clone()], None).unwrap();
    let (polys, z) = read_polygons_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(polys[0], poly);
    assert!(z.is_none());
}

