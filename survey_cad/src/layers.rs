use std::collections::HashMap;

use crate::geometry::LineType;
use crate::styles::{LineWeight, TextStyle};

/// Representation of a drawing layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Layer {
    pub name: String,
    pub is_on: bool,
    pub is_locked: bool,
    pub dependencies: Vec<String>,
    pub line_type: Option<LineType>,
    pub line_weight: Option<LineWeight>,
    pub text_style: Option<TextStyle>,
}

impl Layer {
    /// Creates a new layer with default state.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            is_on: true,
            is_locked: false,
            dependencies: Vec::new(),
            line_type: None,
            line_weight: None,
            text_style: None,
        }
    }
}

/// Manager for an arbitrary number of layers.
#[derive(Debug, Default)]
pub struct LayerManager {
    layers: HashMap<String, Layer>,
}

impl LayerManager {
    /// Creates an empty layer manager.
    pub fn new() -> Self {
        Self {
            layers: HashMap::new(),
        }
    }

    /// Adds or replaces a layer by name.
    pub fn add_layer(&mut self, layer: Layer) {
        self.layers.insert(layer.name.clone(), layer);
    }

    /// Retrieves a layer by name.
    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.get(name)
    }

    /// Retrieves a mutable reference to a layer by name.
    pub fn layer_mut(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.get_mut(name)
    }

    /// Sets the on/off state for the named layer.
    pub fn set_layer_state(&mut self, name: &str, on: bool) {
        if let Some(layer) = self.layers.get_mut(name) {
            layer.is_on = on;
        }
    }

    /// Returns all layers matching `predicate`.
    pub fn filter<F>(&self, predicate: F) -> Vec<&Layer>
    where
        F: Fn(&Layer) -> bool,
    {
        self.layers.values().filter(|l| predicate(l)).collect()
    }

    /// Iterator over all layer names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.layers.keys().map(|k| k.as_str())
    }

    /// Iterator over all layers.
    pub fn iter(&self) -> impl Iterator<Item = &Layer> {
        self.layers.values()
    }
}
