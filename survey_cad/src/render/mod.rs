//! Rendering utilities. Placeholder for drawing CAD entities.

use crate::geometry::Point;

/// Simple rendering of a point. In real application this would draw to screen.
#[allow(unused)]
pub fn render_point(p: Point) {
    println!("Rendering point at ({}, {})", p.x, p.y);
}
