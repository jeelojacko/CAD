//! 3D workspace utilities including dynamic UCS.

use crate::geometry::Point3;

/// User Coordinate System represented by origin and orthonormal axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ucs {
    pub origin: Point3,
    pub x_axis: Point3,
    pub y_axis: Point3,
    pub z_axis: Point3,
}

impl Ucs {
    /// Creates a new UCS from orthonormal axes.
    pub fn new(origin: Point3, x_axis: Point3, y_axis: Point3) -> Self {
        let x = normalize(x_axis);
        let y = normalize(y_axis);
        let z = normalize(cross(x, y));
        Self { origin, x_axis: x, y_axis: y, z_axis: z }
    }

    /// Builds a UCS aligned to the plane defined by three points.
    pub fn from_plane(a: Point3, b: Point3, c: Point3) -> Self {
        let x = normalize(subtract(b, a));
        let normal = cross(subtract(b, a), subtract(c, a));
        let z = normalize(normal);
        let y = normalize(cross(z, x));
        Self { origin: a, x_axis: x, y_axis: y, z_axis: z }
    }

    /// Converts a world point to local UCS coordinates.
    pub fn world_to_local(&self, p: Point3) -> Point3 {
        let v = subtract(p, self.origin);
        Point3::new(dot(v, self.x_axis), dot(v, self.y_axis), dot(v, self.z_axis))
    }

    /// Converts a local point to world coordinates.
    pub fn local_to_world(&self, p: Point3) -> Point3 {
        Point3::new(
            self.origin.x + self.x_axis.x * p.x + self.y_axis.x * p.y + self.z_axis.x * p.z,
            self.origin.y + self.x_axis.y * p.x + self.y_axis.y * p.y + self.z_axis.y * p.z,
            self.origin.z + self.x_axis.z * p.x + self.y_axis.z * p.y + self.z_axis.z * p.z,
        )
    }
}

fn subtract(a: Point3, b: Point3) -> Point3 {
    Point3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn cross(a: Point3, b: Point3) -> Point3 {
    Point3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}

fn dot(a: Point3, b: Point3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn normalize(v: Point3) -> Point3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len == 0.0 {
        Point3::new(0.0, 0.0, 0.0)
    } else {
        Point3::new(v.x / len, v.y / len, v.z / len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ucs_world_local_roundtrip() {
        let ucs = Ucs::new(Point3::new(1.0, 2.0, 3.0), Point3::new(1.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0));
        let p = Point3::new(2.0, 3.0, 4.0);
        let local = ucs.world_to_local(p);
        let world = ucs.local_to_world(local);
        assert!((p.x - world.x).abs() < 1e-6);
        assert!((p.y - world.y).abs() < 1e-6);
        assert!((p.z - world.z).abs() < 1e-6);
    }

    #[test]
    fn ucs_from_plane() {
        let a = Point3::new(0.0, 0.0, 0.0);
        let b = Point3::new(1.0, 0.0, 0.0);
        let c = Point3::new(0.0, 1.0, 0.0);
        let ucs = Ucs::from_plane(a, b, c);
        let p = Point3::new(1.0, 1.0, 0.0);
        let local = ucs.world_to_local(p);
        assert!((local.x - 1.0).abs() < 1e-6);
        assert!((local.y - 1.0).abs() < 1e-6);
        assert!(local.z.abs() < 1e-6);
    }
}
