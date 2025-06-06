//! Core library for the Survey CAD application.

pub mod dtm;
pub mod geometry;
pub mod alignment;
pub mod corridor;
pub mod io;
#[cfg(feature = "pmetra")]
pub mod pmetra;
pub mod render;
pub mod surveying;
pub mod truck_integration;

/// Adds two numbers together. Example function.
#[allow(dead_code)]
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
