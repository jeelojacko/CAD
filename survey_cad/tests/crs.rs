use survey_cad::crs::Crs;

#[test]
fn transform_point3d_identity() {
    let crs = Crs::from_epsg(4979);
    let (x, y, z) = crs.transform_point3d(&crs, 1.0, 2.0, 3.0).unwrap();
    assert!((x - 1.0).abs() < 1e-6);
    assert!((y - 2.0).abs() < 1e-6);
    assert!((z - 3.0).abs() < 1e-6);
}
