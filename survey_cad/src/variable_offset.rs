#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OffsetPoint {
    pub station: f64,
    pub offset: f64,
}

pub type OffsetTable = Vec<OffsetPoint>;

/// Linear interpolation of an offset table.
pub fn offset_at(table: &OffsetTable, station: f64) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    if station <= table[0].station {
        return table[0].offset;
    }
    for pair in table.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        if station >= a.station && station <= b.station {
            let t = if (b.station - a.station).abs() < f64::EPSILON {
                0.0
            } else {
                (station - a.station) / (b.station - a.station)
            };
            return a.offset + t * (b.offset - a.offset);
        }
    }
    table
        .last()
        .expect("offset_at called on empty table")
        .offset
}
