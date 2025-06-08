use survey_cad::alignment::{HorizontalAlignment, HorizontalElement};
use survey_cad::geometry::{Arc, Point};
use survey_cad::surveying::stakeout_position;

#[test]
fn tangent_offset() {
    let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    let p = stakeout_position(&halign, 5.0, 1.0).unwrap();
    assert!((p.x - 5.0).abs() < 1e-6);
    assert!((p.y - 1.0).abs() < 1e-6);
}

#[test]
fn curve_radial() {
    let arc = Arc::new(Point::new(0.0, 0.0), 5.0, 0.0, std::f64::consts::FRAC_PI_2);
    let halign = HorizontalAlignment { elements: vec![HorizontalElement::Curve { arc }] };
    let len = arc.length();
    let p = stakeout_position(&halign, len / 2.0, 1.0).unwrap();
    let ang = std::f64::consts::FRAC_PI_4; // half sweep
    assert!((p.x - 6.0 * ang.cos()).abs() < 1e-6);
    assert!((p.y - 6.0 * ang.sin()).abs() < 1e-6);
}
