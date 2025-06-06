use survey_cad::{
    alignment::HorizontalAlignment, geometry::Point, intersection::curb_return_between_alignments,
};

#[test]
fn curb_return_t_intersection() {
    let a = HorizontalAlignment::new(vec![Point::new(-10.0, 0.0), Point::new(0.0, 0.0)]);
    let b = HorizontalAlignment::new(vec![Point::new(0.0, -10.0), Point::new(0.0, 0.0)]);
    let res = curb_return_between_alignments(&a, &b, 5.0).unwrap();
    assert!((res.start.x + 5.0).abs() < 1e-6);
    assert!((res.end.y - 5.0).abs() < 1e-6);
    assert!((res.arc.center.x + 5.0).abs() < 1e-6);
    assert!((res.arc.center.y - 5.0).abs() < 1e-6);
}

#[test]
fn curb_return_cross_intersection() {
    let a = HorizontalAlignment::new(vec![Point::new(-10.0, 0.0), Point::new(0.0, 0.0)]);
    let b = HorizontalAlignment::new(vec![Point::new(0.0, 10.0), Point::new(0.0, 0.0)]);
    let res = curb_return_between_alignments(&a, &b, 3.0).unwrap();
    assert!((res.start.x + 3.0).abs() < 1e-6);
    assert!((res.end.y + 3.0).abs() < 1e-6);
    assert!((res.arc.center.x + 3.0).abs() < 1e-6);
    assert!((res.arc.center.y + 3.0).abs() < 1e-6);
}
