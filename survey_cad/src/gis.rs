use std::collections::BTreeMap;

/// Wrapper linking CAD geometry with optional feature class and GIS attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct Feature<T> {
    /// Optional feature class name, e.g. layer or category.
    pub class: Option<String>,
    /// Arbitrary attribute key/value pairs.
    pub attributes: BTreeMap<String, String>,
    /// Underlying CAD geometry.
    pub geometry: T,
}

impl<T> Feature<T> {
    /// Creates a new feature with empty attributes.
    pub fn new(geometry: T) -> Self {
        Self {
            class: None,
            attributes: BTreeMap::new(),
            geometry,
        }
    }
}
