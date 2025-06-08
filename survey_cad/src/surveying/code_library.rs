use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;

/// Mapping of a field code to a block, attributes and linework semantics.
#[derive(Debug, Clone, Deserialize)]
pub struct CodeEntry {
    /// Optional block name to insert at a point with this code.
    #[serde(default)]
    pub block: Option<String>,
    /// Attributes associated with the block insertion.
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
    /// If true, points with this code should be connected into linework.
    #[serde(default)]
    pub linework: bool,
}

/// Collection of code mappings loaded from an external JSON file.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CodeLibrary {
    #[serde(default)]
    pub codes: BTreeMap<String, CodeEntry>,
}

impl CodeLibrary {
    /// Loads a code library from a JSON file.
    pub fn from_json(path: &str) -> std::io::Result<Self> {
        let data = fs::read_to_string(path)?;
        let lib: Self = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(lib)
    }

    /// Retrieves the entry for a given code if present.
    pub fn get(&self, code: &str) -> Option<&CodeEntry> {
        self.codes.get(code)
    }
}

/// Representation of a block insertion generated from a survey point.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockRef {
    pub location: crate::geometry::Point3,
    pub name: String,
    pub attributes: BTreeMap<String, String>,
}
