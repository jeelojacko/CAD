use survey_cad::{
    alignment::{Alignment, HorizontalAlignment, VerticalAlignment},
    geometry::Point,
    intersection::{
        crest_curve_between_alignments, curb_return_between_alignments, intersection_alignment,
        sag_curve_between_alignments,
    },
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

#[test]
fn crest_curve_geometry() {
    let a = VerticalAlignment::new(vec![(0.0, 0.0), (50.0, 1.0)]);
    let b = VerticalAlignment::new(vec![(50.0, 1.0), (100.0, 0.0)]);
    let res = crest_curve_between_alignments(&a, &b, 50.0, 0.02, -0.02).unwrap();
    assert!((res.length - 100.0).abs() < 1e-6);
    assert!((res.high_low_station - 50.0).abs() < 1e-6);
    assert!((res.high_low_elev - 1.0).abs() < 1e-6);
    assert!(res.grade_adjustment.abs() < 1e-6);
}

#[test]
fn sag_curve_geometry() {
    let a = VerticalAlignment::new(vec![(0.0, 0.0), (50.0, -1.0)]);
    let b = VerticalAlignment::new(vec![(50.0, -1.0), (100.0, 0.0)]);
    let res = sag_curve_between_alignments(&a, &b, 50.0, -0.02, 0.02).unwrap();
    assert!((res.length - 100.0).abs() < 1e-6);
    assert!((res.high_low_station - 50.0).abs() < 1e-6);
    assert!((res.high_low_elev + 1.0).abs() < 1e-6);
    assert!(res.grade_adjustment.abs() < 1e-6);
}

#[test]
fn intersection_alignment_basic() {
    let ha = HorizontalAlignment::new(vec![Point::new(-10.0, 0.0), Point::new(0.0, 0.0)]);
    let hb = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(0.0, 10.0)]);
    let va = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
    let vb = VerticalAlignment::new(vec![(10.0, 0.0), (20.0, 0.0)]);
    let a = Alignment::new(ha, va);
    let b = Alignment::new(hb, vb);
    let res = intersection_alignment(&a, &b, 5.0).unwrap();
    assert_eq!(res.horizontal.elements.len(), 3);
    assert_eq!(res.vertical.elements.len(), 1);
}
