//! Simple 3D grip handle for direct manipulation of entities.

use crate::geometry::Point3;

/// Representation of a grip point that can be dragged in 3D space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grip3d {
    pub position: Point3,
}

impl Grip3d {
    /// Creates a new grip at the given position.
    pub fn new(position: Point3) -> Self {
        Self { position }
    }

    /// Applies a translation to the grip, returning the updated position.
    pub fn translate(&mut self, delta: Point3) -> Point3 {
        self.position.x += delta.x;
        self.position.y += delta.y;
        self.position.z += delta.z;
        self.position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grip_translate() {
        let mut g = Grip3d::new(Point3::new(1.0, 2.0, 3.0));
        let p = g.translate(Point3::new(0.5, -1.0, 2.0));
        assert_eq!(p, Point3::new(1.5, 1.0, 5.0));
    }
}
