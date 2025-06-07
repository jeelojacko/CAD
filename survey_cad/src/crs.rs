//! Coordinate reference system utilities built on top of the `proj` crate.

use proj::Proj;

/// Representation of a coordinate reference system.
///
/// A CRS is stored internally as a definition string which can be an EPSG
/// identifier (`"EPSG:4326"`), a Proj4 definition or a WKT definition.  When
/// created from an EPSG code the numeric value is retained so that callers can
/// inspect it if necessary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crs {
    definition: String,
    epsg: Option<u32>,
}

impl Crs {
    /// Creates a new CRS from the given EPSG code.
    pub fn from_epsg(code: u32) -> Self {
        Self {
            definition: format!("EPSG:{}", code),
            epsg: Some(code),
        }
    }

    /// Creates a CRS from a Proj4 definition string.
    pub fn from_proj4(definition: &str) -> Self {
        Self {
            definition: definition.to_string(),
            epsg: None,
        }
    }

    /// Creates a CRS from a WKT definition string.
    pub fn from_wkt(definition: &str) -> Self {
        Self {
            definition: definition.to_string(),
            epsg: None,
        }
    }

    /// Returns the EPSG code for this CRS, if available.
    pub fn epsg(&self) -> Option<u32> {
        self.epsg
    }

    /// Returns the underlying definition string.
    pub fn definition(&self) -> &str {
        &self.definition
    }

    /// Common global CRS definition: WGS84 (EPSG:4326).
    pub fn wgs84() -> Self {
        Self::from_epsg(4326)
    }

    /// Common global CRS definition: Web Mercator (EPSG:3857).
    pub fn web_mercator() -> Self {
        Self::from_epsg(3857)
    }

    /// Example national CRS definition (NAD83 / Canada CSRS).
    pub fn nad83_csrs() -> Self {
        Self::from_epsg(4617)
    }

    /// Example provincial CRS definition (NAD83 / Alberta 10TM). This is just a
    /// representative sample of a provincial CRS.
    pub fn alberta_10tm() -> Self {
        Self::from_epsg(3400)
    }

    /// Transforms an `(x, y)` coordinate from this CRS to the target CRS.
    pub fn transform_point(&self, target: &Crs, x: f64, y: f64) -> Option<(f64, f64)> {
        let proj = Proj::new_known_crs(&self.definition, &target.definition, None).ok()?;
        proj.convert((x, y)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgs84_to_web_mercator() {
        let wgs84 = Crs::wgs84();
        let webm = Crs::web_mercator();
        let (x, y) = wgs84.transform_point(&webm, 0.0, 0.0).unwrap();
        assert!(x.abs() < 1e-6 && y.abs() < 1e-6);
    }
}
