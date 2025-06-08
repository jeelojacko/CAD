use survey_cad::{
    alignment::{Alignment, HorizontalAlignment, VerticalAlignment},
    corridor::{corridor_cut_fill, corridor_mass_haul},
    dtm::Tin,
    geometry::{Point, Point3},
};

#[test]
fn cut_fill_prism_fill_only() {
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
    let (cut, fill) = corridor_cut_fill(&design, &ground, &align, 1.0, 10.0, 1.0);
    assert!(cut.abs() < 1e-6);
    assert!((fill - 20.0).abs() < 1e-6);

    let haul = corridor_mass_haul(&design, &ground, &align, 1.0, 10.0, 1.0);
    assert_eq!(haul.len(), 2);
    assert!((haul.last().unwrap().1 - 20.0).abs() < 1e-6);
}
