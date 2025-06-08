//! Core library for the Survey CAD application.

pub mod alignment;
pub mod corridor;
pub mod crs;
pub mod dtm;
pub mod geometry;
pub mod grip;
pub mod intersection;
pub mod io;
pub mod layers;
pub mod lidar;
pub mod local_grid;
pub mod parcel;
#[cfg(feature = "pmetra")]
pub mod pmetra;
#[cfg(feature = "render")]
pub mod render;
pub mod sheet;
pub mod snap;
pub mod styles;
pub mod subassembly;
pub mod superelevation;
pub mod surveying;
pub mod truck_integration;
pub mod variable_offset;
pub mod workspace;

pub use lidar::{classify_points, filter_noise, Classification};
pub use local_grid::LocalGrid;
