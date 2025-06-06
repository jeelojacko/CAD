use survey_cad::geometry::Point;
use survey_cad::surveying::{adjust_network, Observation};

#[test]
fn adjust_simple_traverse() {
    let pts = vec![
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(45.0, 25.0),
    ];
    let dist = 53.85164807134504f64;
    let angle = 2.3805798993650633f64;
    let obs = vec![
        Observation::Distance { from: 0, to: 2, value: dist, weight: 1.0 },
        Observation::Distance { from: 1, to: 2, value: dist, weight: 1.0 },
        Observation::Angle { at: 2, from: 0, to: 1, value: angle, weight: 1.0 },
    ];
    let res = adjust_network(&pts, &[0, 1], &obs);
    let c = res.points[2];
    assert!((c.x - 50.0).abs() < 1e-2);
    assert!((c.y - 20.0).abs() < 1e-2);
    assert!(res.residuals.iter().all(|v| v.abs() < 1e-6));
}
