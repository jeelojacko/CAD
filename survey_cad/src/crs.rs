//! Coordinate reference system utilities built on top of the `proj` crate.

use proj::Proj;
use rusqlite::Connection;

/// Reusable transformation object between two coordinate reference systems.
///
/// The underlying PROJ context is not thread safe, therefore
/// `CrsTransformer` should not be shared between threads. Construct a new
/// transformer per thread if needed.
#[derive(Debug)]
pub struct CrsTransformer {
    ctx: *mut proj_sys::PJ_CONTEXT,
    pj: *mut proj_sys::PJ,
    // raw pointers are Send + Sync by default, but the PROJ context isn't
    // thread-safe. Use `Rc` to opt-out of automatic Send/Sync impls.
    _nosend: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Drop for CrsTransformer {
    fn drop(&mut self) {
        unsafe {
            proj_sys::proj_destroy(self.pj);
            proj_sys::proj_context_destroy(self.ctx);
        }
    }
}

impl CrsTransformer {
    /// Builds a new transformation between `source` and `target` CRS.
    pub fn new(source: &Crs, target: &Crs) -> Option<Self> {
        use proj_sys::*;
        use std::ffi::CString;

        unsafe {
            let ctx = proj_context_create();
            if ctx.is_null() {
                return None;
            }
            // allow grid downloads for datum shifts and geoid transformations
            proj_context_set_enable_network(ctx, 1);

            let from_c = CString::new(source.definition.as_str()).ok()?;
            let to_c = CString::new(target.definition.as_str()).ok()?;
            let area = proj_area_create();
            let mut pj = proj_create_crs_to_crs(ctx, from_c.as_ptr(), to_c.as_ptr(), area);
            proj_area_destroy(area);
            if pj.is_null() {
                proj_context_destroy(ctx);
                return None;
            }
            pj = proj_normalize_for_visualization(ctx, pj);
            if pj.is_null() {
                proj_context_destroy(ctx);
                return None;
            }
            Some(Self {
                ctx,
                pj,
                _nosend: std::marker::PhantomData,
            })
        }
    }

    /// Transforms a 3D point using the prepared transformation.
    pub fn transform(&self, x: f64, y: f64, z: f64) -> Option<(f64, f64, f64)> {
        use proj_sys::*;
        unsafe {
            let coord = PJ_COORD {
                xyzt: PJ_XYZT { x, y, z, t: 0.0 },
            };
            let res = proj_trans(self.pj, PJ_DIRECTION_PJ_FWD, coord);
            if proj_errno(self.pj) != 0 {
                return None;
            }
            Some((res.xyzt.x, res.xyzt.y, res.xyzt.z))
        }
    }
}

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
            definition: format!("EPSG:{code}"),
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

    /// Transforms an `(x, y, z)` coordinate from this CRS to the target CRS.
    /// Network access is enabled to allow on-the-fly grid/geoid downloads.
    pub fn transform_point3d(
        &self,
        target: &Crs,
        x: f64,
        y: f64,
        z: f64,
    ) -> Option<(f64, f64, f64)> {
        CrsTransformer::new(self, target)?.transform(x, y, z)
    }
}

/// Simple CRS information record loaded from the PROJ database.
#[derive(Debug, Clone)]
pub struct CrsEntry {
    /// Combined authority and code string (e.g. "EPSG:4326").
    pub code: String,
    /// Human readable name of the coordinate system.
    pub name: String,
}

/// Loads available coordinate reference systems from the system PROJ database.
pub fn list_known_crs() -> Vec<CrsEntry> {
    let path = "/usr/share/proj/proj.db";
    let conn = match Connection::open(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut stmt = match conn
        .prepare("SELECT auth_name || ':' || code, name FROM crs_view WHERE deprecated = 0")
    {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        Ok(CrsEntry {
            code: row.get(0)?,
            name: row.get(1)?,
        })
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for r in rows.flatten() {
        out.push(r);
    }
    out
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
