use serde::{Deserialize, Serialize};

use crate::dtm::Tin;
use crate::geometry::{Arc, Line, Point, Polyline};
use crate::layers::Layer;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GridSettings {
    pub spacing: f32,
    pub color: [u8; 3],
    pub visible: bool,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            spacing: 50.0,
            color: [60, 60, 60],
            visible: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub points: Vec<Point>,
    pub lines: Vec<Line>,
    pub polygons: Vec<Vec<Point>>,
    pub polylines: Vec<Polyline>,
    pub arcs: Vec<Arc>,
    #[serde(default)]
    pub dimensions: Vec<crate::geometry::LinearDimension>,
    #[serde(default)]
    pub alignments: Vec<crate::alignment::Alignment>,
    pub surfaces: Vec<Tin>,
    #[serde(default)]
    pub surface_units: Vec<String>,
    #[serde(default)]
    pub surface_styles: Vec<String>,
    #[serde(default)]
    pub surface_descriptions: Vec<String>,
    pub layers: Vec<Layer>,
    pub point_style_indices: Vec<usize>,
    pub line_style_indices: Vec<usize>,
    #[serde(default)]
    pub polygon_style_indices: Vec<usize>,
    #[serde(default)]
    pub grid: GridSettings,
    #[serde(default)]
    pub crs_epsg: u32,
    #[serde(default)]
    pub point_label_font: String,
    #[serde(default)]
    pub point_label_offset: [f32; 2],
}

impl Project {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            lines: Vec::new(),
            polygons: Vec::new(),
            polylines: Vec::new(),
            arcs: Vec::new(),
            dimensions: Vec::new(),
            alignments: Vec::new(),
            surfaces: Vec::new(),
            surface_units: Vec::new(),
            surface_styles: Vec::new(),
            surface_descriptions: Vec::new(),
            layers: Vec::new(),
            point_style_indices: Vec::new(),
            line_style_indices: Vec::new(),
            polygon_style_indices: Vec::new(),
            grid: GridSettings::default(),
            crs_epsg: 4326,
            point_label_font: "DejaVuSans".to_string(),
            point_label_offset: [5.0, 5.0],
        }
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read_project_json(path: &str) -> std::io::Result<Project> {
    let contents = crate::io::read_to_string(path)?;
    let proj: Project = serde_json::from_str(&contents)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(proj)
}

pub fn write_project_json(path: &str, project: &Project) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(project)
        .map_err(std::io::Error::other)?;
    crate::io::write_string(path, &json)
}
