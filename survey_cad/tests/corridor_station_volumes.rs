use survey_cad::{
    alignment::{Alignment, HorizontalAlignment, VerticalAlignment},
    corridor::{corridor_station_volumes},
    dtm::Tin,
    geometry::{Point, Point3},
};

#[test]
fn station_volumes_prism() {
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
    let vols = corridor_station_volumes(&design, &ground, &align, 1.0, 10.0, 1.0);
    assert_eq!(vols.len(), 2);
    let last = vols.last().unwrap();
    assert!((last.cumulative - 20.0).abs() < 1e-6);
    assert!(last.haul > 0.0);
}
