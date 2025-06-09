//! Local grid definition with origin, rotation and scale.

use crate::geometry::Point;

/// Simple local grid definition.
///
/// A local grid transforms coordinates from a global CRS into
/// project-specific values using a translation, rotation and scale.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct LocalGrid {
    /// Global coordinates of the local origin.
    pub origin: Point,
    /// Counter-clockwise rotation from global X axis to local East in radians.
    pub rotation: f64,
    /// Scale factor applied after rotation when converting to local coordinates.
    pub scale: f64,
}

impl LocalGrid {
    /// Creates a new local grid definition.
    pub fn new(origin: Point, rotation: f64, scale: f64) -> Self {
        Self {
            origin,
            rotation,
            scale,
        }
    }

    /// Converts a global coordinate to this local grid.
    pub fn to_local(&self, p: Point) -> Point {
        let dx = p.x - self.origin.x;
        let dy = p.y - self.origin.y;
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        let x = (dx * cos + dy * sin) * self.scale;
        let y = (-dx * sin + dy * cos) * self.scale;
        Point::new(x, y)
    }

    /// Converts a local coordinate back to global coordinates.
    pub fn from_local(&self, p: Point) -> Point {
        let inv_scale = 1.0 / self.scale;
        let x = p.x * inv_scale;
        let y = p.y * inv_scale;
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        let gx = x * cos - y * sin + self.origin.x;
        let gy = x * sin + y * cos + self.origin.y;
        Point::new(gx, gy)
    }

    /// Saves this grid definition to a JSON file.
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).unwrap();
        std::fs::write(path, json)
    }

    /// Loads a grid definition from a JSON file.
    pub fn load(path: &str) -> std::io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let grid: LocalGrid = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(grid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let grid = LocalGrid::new(Point::new(100.0, 200.0), std::f64::consts::FRAC_PI_4, 2.0);
        let global = Point::new(110.0, 210.0);
        let local = grid.to_local(global);
        let back = grid.from_local(local);
        assert!((back.x - global.x).abs() < 1e-6);
        assert!((back.y - global.y).abs() < 1e-6);
    }
}
