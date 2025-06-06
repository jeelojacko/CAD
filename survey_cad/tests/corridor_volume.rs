use survey_cad::{
    alignment::{Alignment, HorizontalAlignment, VerticalAlignment},
    corridor::corridor_volume,
    dtm::Tin,
    geometry::{Point, Point3},
};

#[test]
fn volume_zero_flat_surfaces() {
    let design = Tin::from_points(vec![
        Point3::new(0.0, -1.0, 0.0),
        Point3::new(0.0, 1.0, 0.0),
        Point3::new(10.0, -1.0, 0.0),
        Point3::new(10.0, 1.0, 0.0),
    ]);
    let ground = design.clone();
    let hal = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    let val = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
    let align = Alignment::new(hal, val);
    let vol = corridor_volume(&design, &ground, &align, 1.0, 10.0, 1.0);
    assert!(vol.abs() < 1e-6);
}

#[test]
fn volume_simple_prism() {
    let design = Tin::from_points(vec![
        Point3::new(0.0, -1.0, 1.0),
        Point3::new(0.0, 1.0, 1.0),
        Point3::new(10.0, -1.0, 1.0),
        Point3::new(10.0, 1.0, 1.0),
    ]);
    let ground = Tin::from_points(vec![
        Point3::new(0.0, -1.0, 0.0),
        Point3::new(0.0, 1.0, 0.0),
        Point3::new(10.0, -1.0, 0.0),
        Point3::new(10.0, 1.0, 0.0),
    ]);
    let hal = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    let val = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
    let align = Alignment::new(hal, val);
    let vol = corridor_volume(&design, &ground, &align, 1.0, 10.0, 1.0);
    assert!((vol - 20.0).abs() < 1e-6);
}
