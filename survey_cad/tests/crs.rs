use survey_cad::crs::{Crs, CrsTransformer};

#[test]
fn transform_point3d_identity() {
    let crs = Crs::from_epsg(4979);
    let (x, y, z) = crs.transform_point3d(&crs, 1.0, 2.0, 3.0).unwrap();
    assert!((x - 1.0).abs() < 1e-6);
    assert!((y - 2.0).abs() < 1e-6);
    assert!((z - 3.0).abs() < 1e-6);
}

#[test]
fn transformer_reuse() {
    let crs = Crs::from_epsg(4979);
    let t = CrsTransformer::new(&crs, &crs).unwrap();
    for _ in 0..3 {
        let (x, y, z) = t.transform(1.0, 2.0, 3.0).unwrap();
        assert!((x - 1.0).abs() < 1e-6);
        assert!((y - 2.0).abs() < 1e-6);
        assert!((z - 3.0).abs() < 1e-6);
    }
}
