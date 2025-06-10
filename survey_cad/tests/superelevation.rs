use survey_cad::superelevation::{slopes_at, SuperelevationPoint, SuperelevationTable};

#[test]
fn empty_table_returns_zero() {
    let table: SuperelevationTable = Vec::new();
    let (left, right) = slopes_at(&table, 10.0);
    assert!((left - 0.0).abs() < 1e-9);
    assert!((right - 0.0).abs() < 1e-9);
}

#[test]
fn before_first_entry_returns_first_slopes() {
    let table = vec![
        SuperelevationPoint { station: 10.0, left_slope: -0.02, right_slope: 0.02 },
        SuperelevationPoint { station: 20.0, left_slope: 0.03, right_slope: -0.03 },
    ];
    let (left, right) = slopes_at(&table, 5.0);
    assert!((left + 0.02).abs() < 1e-9);
    assert!((right - 0.02).abs() < 1e-9);
}

#[test]
fn after_last_entry_returns_last_slopes() {
    let table = vec![
        SuperelevationPoint { station: 0.0, left_slope: -0.01, right_slope: 0.01 },
        SuperelevationPoint { station: 10.0, left_slope: 0.02, right_slope: -0.02 },
    ];
    let (left, right) = slopes_at(&table, 15.0);
    assert!((left - 0.02).abs() < 1e-9);
    assert!((right + 0.02).abs() < 1e-9);
}

#[test]
fn interpolates_between_entries() {
    let table = vec![
        SuperelevationPoint { station: 0.0, left_slope: -0.02, right_slope: 0.02 },
        SuperelevationPoint { station: 100.0, left_slope: 0.04, right_slope: -0.04 },
    ];
    let (left, right) = slopes_at(&table, 50.0);
    assert!((left - 0.01).abs() < 1e-9);
    assert!((right + 0.01).abs() < 1e-9);
}
