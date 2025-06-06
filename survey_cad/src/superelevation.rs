#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuperelevationPoint {
    pub station: f64,
    pub left_slope: f64,
    pub right_slope: f64,
}

pub type SuperelevationTable = Vec<SuperelevationPoint>;
