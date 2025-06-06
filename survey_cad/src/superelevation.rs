#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuperelevationPoint {
    pub station: f64,
    pub left_slope: f64,
    pub right_slope: f64,
}

pub type SuperelevationTable = Vec<SuperelevationPoint>;

/// Linearly interpolate left and right cross slopes from a table.
pub fn slopes_at(table: &SuperelevationTable, station: f64) -> (f64, f64) {
    if table.is_empty() {
        return (0.0, 0.0);
    }

    if station <= table[0].station {
        return (table[0].left_slope, table[0].right_slope);
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
            let left = a.left_slope + t * (b.left_slope - a.left_slope);
            let right = a.right_slope + t * (b.right_slope - a.right_slope);
            return (left, right);
        }
    }

    let last = table.last().unwrap();
    (last.left_slope, last.right_slope)
}
