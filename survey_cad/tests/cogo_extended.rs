use survey_cad::geometry::Point;
use survey_cad::surveying::{
    bearing_bearing_intersection, bearing_distance_intersection, deflection_angle, point_offset,
};

#[test]
fn bearing_distance_simple() {
    let start = Point::new(0.0, 0.0);
    let center = Point::new(1.0, 1.0);
    let res = bearing_distance_intersection(start, 0.0, center, 1.0).unwrap();
    assert_eq!(res.len(), 1);
    let p = res[0];
    assert!((p.x - 1.0).abs() < 1e-6);
    assert!(p.y.abs() < 1e-6);
}

#[test]
fn deflection_angle_right() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(1.0, 0.0);
    let c = Point::new(1.0, 1.0);
    let ang = deflection_angle(a, b, c).abs();
    assert!((ang - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
}

#[test]
fn point_offset_basic() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(10.0, 0.0);
    let p = point_offset(a, b, 5.0, 2.0);
    assert!((p.x - 5.0).abs() < 1e-6);
    assert!((p.y - 2.0).abs() < 1e-6);
}

#[test]
fn bearing_bearing_basic() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(1.0, 1.0);
    let pt = bearing_bearing_intersection(a, 0.0, b, -std::f64::consts::FRAC_PI_2).unwrap();
    assert!((pt.x - 1.0).abs() < 1e-6);
    assert!(pt.y.abs() < 1e-6);
}
