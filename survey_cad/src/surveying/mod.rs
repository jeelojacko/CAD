//! Surveying specific utilities.

use crate::geometry::Point;

/// Representation of a simple survey station.
#[derive(Debug)]
pub struct Station {
    pub name: String,
    pub position: Point,
}

impl Station {
    pub fn new(name: impl Into<String>, position: Point) -> Self {
        Self {
            name: name.into(),
            position,
        }
    }
}
