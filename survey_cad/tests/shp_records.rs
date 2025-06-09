#[cfg(feature = "shapefile")]
use shapefile::dbase::FieldValue;
#[cfg(feature = "shapefile")]
use survey_cad::geometry::{Point, Point3, Polyline};
#[cfg(feature = "shapefile")]
use survey_cad::io::shp::{
    read_point_records_shp, read_polygon_records_shp, read_polyline_records_shp,
    write_point_records_shp, write_polygon_records_shp, write_polyline_records_shp, PointRecord,
    PolygonRecord, PolylineRecord,
};

#[cfg(feature = "shapefile")]
#[test]
fn point_record_roundtrip() {
    use std::collections::BTreeMap;
    use tempfile::NamedTempFile;
    let mut attrs = BTreeMap::new();
    attrs.insert(
        "Name".to_string(),
        FieldValue::Character(Some("Pt".to_string())),
    );
    let rec = PointRecord {
        geom: Point::new(1.0, 2.0),
        geom_z: None,
        attrs,
    };
    let file = NamedTempFile::new().unwrap();
    write_point_records_shp(file.path().to_str().unwrap(), &[rec.clone()]).unwrap();
    let records = read_point_records_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(records[0].geom, rec.geom);
    assert_eq!(records[0].attrs, rec.attrs);
}

#[cfg(feature = "shapefile")]
#[test]
fn polyline_record_roundtrip() {
    use std::collections::BTreeMap;
    use tempfile::NamedTempFile;
    let mut attrs = BTreeMap::new();
    attrs.insert("ID".to_string(), FieldValue::Numeric(Some(1.0)));
    let pl = Polyline::new(vec![Point::new(0.0, 0.0), Point::new(1.0, 0.0)]);
    let rec = PolylineRecord {
        geom: pl.clone(),
        geom_z: None,
        attrs,
    };
    let file = NamedTempFile::new().unwrap();
    write_polyline_records_shp(file.path().to_str().unwrap(), &[rec.clone()]).unwrap();
    let records = read_polyline_records_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(records[0].geom, rec.geom);
    assert_eq!(records[0].attrs, rec.attrs);
}

#[cfg(feature = "shapefile")]
#[test]
fn polygon_record_roundtrip() {
    use std::collections::BTreeMap;
    use tempfile::NamedTempFile;
    let mut attrs = BTreeMap::new();
    attrs.insert("Val".to_string(), FieldValue::Integer(5));
    let poly = vec![
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 0.0),
    ];
    let rec = PolygonRecord {
        geom: poly.clone(),
        geom_z: None,
        attrs,
    };
    let file = NamedTempFile::new().unwrap();
    write_polygon_records_shp(file.path().to_str().unwrap(), &[rec.clone()]).unwrap();
    let records = read_polygon_records_shp(file.path().to_str().unwrap()).unwrap();
    assert_eq!(records[0].geom.len(), rec.geom.len());
    assert_eq!(records[0].attrs, rec.attrs);
}
