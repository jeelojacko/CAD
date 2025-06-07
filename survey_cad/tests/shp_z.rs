#[cfg(feature = "shapefile")]
use survey_cad::io::shp::{read_points_shp, write_points_shp, read_polylines_shp, write_polylines_shp, read_polygons_shp, write_polygons_shp};
#[cfg(feature = "shapefile")]
use survey_cad::geometry::{Point, Point3, Polyline};

#[cfg(feature = "shapefile")]
#[test]
fn points_z_roundtrip() {
    use tempfile::NamedTempFile;
    let pts3 = vec![Point3::new(1.0,2.0,3.0), Point3::new(4.0,5.0,6.0)];
    let pts2: Vec<Point> = pts3.iter().map(|p| Point::new(p.x,p.y)).collect();
    let file = NamedTempFile::new().unwrap();
    write_points_shp(file.path().to_str().unwrap(), &pts2, Some(&pts3)).unwrap();
    let (_pts, pts_read) = read_points_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(pts_read.unwrap(), pts3);
}

#[cfg(feature = "shapefile")]
#[test]
fn polylines_z_roundtrip() {
    use tempfile::NamedTempFile;
    let line3 = vec![Point3::new(0.0,0.0,0.0), Point3::new(1.0,0.0,1.0)];
    let line2: Vec<Point> = line3.iter().map(|p| Point::new(p.x,p.y)).collect();
    let file = NamedTempFile::new().unwrap();
    write_polylines_shp(file.path().to_str().unwrap(), &[Polyline::new(line2.clone())], Some(&[line3.clone()])).unwrap();
    let (_lines, lines3) = read_polylines_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(lines3.unwrap()[0], line3);
}

#[cfg(feature = "shapefile")]
#[test]
fn polygons_z_roundtrip() {
    use tempfile::NamedTempFile;
    let poly3 = vec![
        Point3::new(0.0,0.0,0.0),
        Point3::new(1.0,0.0,0.0),
        Point3::new(1.0,1.0,0.0),
        Point3::new(0.0,0.0,0.0),
    ];
    let poly2: Vec<Point> = poly3.iter().map(|p| Point::new(p.x,p.y)).collect();
    let file = NamedTempFile::new().unwrap();
    write_polygons_shp(file.path().to_str().unwrap(), &[poly2.clone()], Some(&[poly3.clone()])).unwrap();
    let (_polys, polys3) = read_polygons_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(polys3.unwrap()[0], poly3);
}
