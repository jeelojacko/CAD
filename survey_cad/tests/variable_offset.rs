use survey_cad::variable_offset::{OffsetPoint, OffsetTable, offset_at};

#[test]
fn empty_table_returns_zero() {
    let table: OffsetTable = Vec::new();
    let value = offset_at(&table, 10.0);
    assert!((value - 0.0).abs() < 1e-9);
}

#[test]
fn before_first_entry_returns_first_offset() {
    let table = vec![
        OffsetPoint { station: 10.0, offset: 5.0 },
        OffsetPoint { station: 20.0, offset: 7.0 },
    ];
    let value = offset_at(&table, 5.0);
    assert!((value - 5.0).abs() < 1e-9);
}

#[test]
fn interpolates_between_entries() {
    let table = vec![
        OffsetPoint { station: 0.0, offset: 0.0 },
        OffsetPoint { station: 10.0, offset: 10.0 },
    ];
    let value = offset_at(&table, 5.0);
    assert!((value - 5.0).abs() < 1e-9);
}

#[test]
fn after_last_entry_returns_last_offset() {
    let table = vec![
        OffsetPoint { station: 0.0, offset: 1.0 },
        OffsetPoint { station: 10.0, offset: 3.0 },
    ];
    let value = offset_at(&table, 20.0);
    assert!((value - 3.0).abs() < 1e-9);
}
