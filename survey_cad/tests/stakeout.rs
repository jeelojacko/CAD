use survey_cad::alignment::{HorizontalAlignment, HorizontalElement};
use survey_cad::geometry::{Arc, Point};
use survey_cad::surveying::{grid_stakeout_points, optimal_stationing, stakeout_position};

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

#[test]
fn optimal_stationing_alignment() {
    let halign = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    let stas = optimal_stationing(&halign, 3.0);
    assert_eq!(stas, vec![0.0, 3.0, 6.0, 9.0, 10.0]);
}

#[test]
fn grid_points_simple() {
    let pts = grid_stakeout_points(Point::new(0.0, 0.0), Point::new(2.0, 2.0), 1.0);
    assert_eq!(pts.len(), 9);
    assert!(pts.contains(&Point::new(1.0, 1.0)));
}
