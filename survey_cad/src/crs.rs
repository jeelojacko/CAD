//! Coordinate reference system utilities built on top of the `proj` crate.

use proj::Proj;

/// Simple wrapper representing a coordinate reference system identified by an EPSG code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crs {
    code: u32,
}

impl Crs {
    /// Creates a new CRS from the given EPSG code.
    pub fn from_epsg(code: u32) -> Self {
        Self { code }
    }

    /// Returns the EPSG code for this CRS.
    pub fn epsg(&self) -> u32 {
        self.code
    }

    /// Transforms an `(x, y)` coordinate from this CRS to the target CRS.
    pub fn transform_point(&self, target: &Crs, x: f64, y: f64) -> Option<(f64, f64)> {
        let from_def = format!("EPSG:{}", self.code);
        let to_def = format!("EPSG:{}", target.code);
        let proj = Proj::new_known_crs(&from_def, &to_def, None).ok()?;
        proj.convert((x, y)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgs84_to_web_mercator() {
        let wgs84 = Crs::from_epsg(4326);
        let webm = Crs::from_epsg(3857);
        let (x, y) = wgs84.transform_point(&webm, 0.0, 0.0).unwrap();
        assert!(x.abs() < 1e-6 && y.abs() < 1e-6);
    }
}
