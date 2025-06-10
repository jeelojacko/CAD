/// Simple command line interface demonstrating the survey CAD utilities.
use clap::{Parser, Subcommand};
mod commands;
#[derive(Parser)]
#[command(name = "survey_cad_cli", version)]
struct Cli {
    /// EPSG code for the working coordinate system
    #[arg(long, default_value_t = 4326, global = true)]
    epsg: u32,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compute the distance between two survey stations.
    StationDistance {
        name_a: String,
        x1: f64,
        y1: f64,
        name_b: String,
        x2: f64,
        y2: f64,
    },
    /// Compute the area of a traverse defined in a CSV file of x,y pairs.
    TraverseArea { path: String },
    /// Copy a text file from src to dest.
    Copy { src: String, dest: String },
    /// Render a point (prints to stdout).
    #[cfg(feature = "render")]
    RenderPoint { x: f64, y: f64 },
    /// Export points from a CSV file to GeoJSON.
    ExportGeojson {
        input: String,
        output: String,
        #[arg(long)]
        src_epsg: Option<u32>,
        #[arg(long)]
        dst_epsg: Option<u32>,
    },
    /// Export points from a CSV file to KML or KMZ.
    #[cfg(feature = "kml")]
    ExportKml {
        input: String,
        output: String,
        #[arg(long)]
        src_epsg: Option<u32>,
        #[arg(long)]
        dst_epsg: Option<u32>,
    },
    /// Import points from a GeoJSON file to CSV.
    ImportGeojson {
        input: String,
        output: String,
        #[arg(long)]
        src_epsg: Option<u32>,
        #[arg(long)]
        dst_epsg: Option<u32>,
    },
    /// Import points from a KML or KMZ file to CSV.
    #[cfg(feature = "kml")]
    ImportKml {
        input: String,
        output: String,
        #[arg(long)]
        src_epsg: Option<u32>,
        #[arg(long)]
        dst_epsg: Option<u32>,
    },
    /// Import points from a File Geodatabase layer to CSV.
    #[cfg(feature = "fgdb")]
    ImportFgdb {
        path: String,
        layer: String,
        output: String,
    },
    /// Import survey points from a text file in a given format to CSV.
    ImportPoints {
        format: String,
        input: String,
        output: String,
    },
    /// Export points from a CSV file to a simple DXF.
    ExportDxf {
        input: String,
        output: String,
        #[arg(long)]
        src_epsg: Option<u32>,
        #[arg(long)]
        dst_epsg: Option<u32>,
    },
    /// Export points from a CSV file to a shapefile.
    #[cfg(feature = "shapefile")]
    ExportShp { input: String, output: String },
    /// Import points from a shapefile to CSV.
    #[cfg(feature = "shapefile")]
    ImportShp { input: String, output: String },
    /// Export polylines from a CSV file to a shapefile.
    #[cfg(feature = "shapefile")]
    ExportPolylinesShp { input: String, output: String },
    /// Import polylines from a shapefile to CSV.
    #[cfg(feature = "shapefile")]
    ImportPolylinesShp { input: String, output: String },
    /// Export polygons from a CSV file to a shapefile.
    #[cfg(feature = "shapefile")]
    ExportPolygonsShp { input: String, output: String },
    /// Generate contour polylines from a surface file.
    #[cfg(feature = "shapefile")]
    Contours {
        surface: String,
        output: String,
        #[arg(long)]
        interval: f64,
        #[arg(long, default_value_t = 0)]
        smooth: usize,
    },
    /// Import polygons from a shapefile to CSV.
    #[cfg(feature = "shapefile")]
    ImportPolygonsShp { input: String, output: String },
    /// Import points from a LAS file to CSV (x,y,z).
    #[cfg(feature = "las")]
    ImportLas { input: String, output: String },
    /// Export points from a CSV file to LAS/LAZ.
    #[cfg(feature = "las")]
    ExportLas { input: String, output: String },
    /// Import points from an E57 file to CSV (x,y,z).
    #[cfg(feature = "e57")]
    ImportE57 { input: String, output: String },
    /// Export points from a CSV file to E57.
    #[cfg(feature = "e57")]
    ExportE57 { input: String, output: String },
    /// Filter noise from a CSV point cloud.
    #[cfg(feature = "las")]
    FilterNoise {
        input: String,
        output: String,
        #[arg(long, default_value_t = 1.0)]
        radius: f64,
        #[arg(long, default_value_t = 3)]
        min_neighbors: usize,
    },
    /// Classify a CSV point cloud into ground, vegetation and buildings.
    #[cfg(feature = "las")]
    ClassifyCloud {
        input: String,
        output: String,
        #[arg(long, default_value_t = 1.0)]
        cell_size: f64,
        #[arg(long, default_value_t = 0.3)]
        ground_threshold: f64,
        #[arg(long, default_value_t = 2.0)]
        veg_threshold: f64,
    },
    /// View points from a CSV file.
    #[cfg(feature = "render")]
    ViewPoints { input: String },
    /// Compute the vertical angle between two stations given their elevations.
    VerticalAngle {
        name_a: String,
        x1: f64,
        y1: f64,
        elev_a: f64,
        name_b: String,
        x2: f64,
        y2: f64,
        elev_b: f64,
    },
    /// Compute the bearing between two points.
    Bearing { x1: f64, y1: f64, x2: f64, y2: f64 },
    /// Compute a new point from a start point, bearing and distance.
    Forward {
        x: f64,
        y: f64,
        bearing: f64,
        distance: f64,
    },
    /// Determine the intersection point of two lines.
    Intersection {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x3: f64,
        y3: f64,
        x4: f64,
        y4: f64,
    },
    /// Compute a new elevation using differential leveling.
    LevelElevation {
        start_elev: f64,
        backsight: f64,
        foresight: f64,
    },
    /// Adjust a 2D network from CSV files of points and observations.
    NetworkAdjust {
        points: String,
        observations: String,
    },
    /// Compute cut/fill volume between two surfaces along an alignment.
    CorridorVolume {
        design: String,
        ground: String,
        halign: String,
        valign: String,
        width: f64,
        #[arg(long, default_value_t = 10.0)]
        interval: f64,
        #[arg(long, default_value_t = 1.0)]
        offset_step: f64,
    },
    /// Generate a mass haul diagram between two surfaces along an alignment.
    MassHaul {
        design: String,
        ground: String,
        halign: String,
        valign: String,
        width: f64,
        #[arg(long, default_value_t = 10.0)]
        interval: f64,
        #[arg(long, default_value_t = 1.0)]
        offset_step: f64,
    },
    /// Create a curb return intersection between two alignments defined by CSV files.
    CreateIntersection {
        align_a: String,
        align_b: String,
        radius: f64,
    },
    /// Create a full intersection alignment with vertical design.
    CreateFullIntersection {
        halign_a: String,
        valign_a: String,
        halign_b: String,
        valign_b: String,
        radius: f64,
    },
    /// Create a pipe network LandXML file from CSV inputs.
    CreatePipeNetwork {
        structures: String,
        pipes: String,
        output: String,
    },
    /// Compute head loss for each pipe using Hazen-Williams.
    PipeNetworkGrade {
        structures: String,
        pipes: String,
        flow: f64,
    },
    /// Analyze a pipe network using design flows stored in the pipe CSV.
    PipeNetworkAnalyze {
        structures: String,
        pipes: String,
        out_csv: String,
        out_xml: String,
    },
    /// Apply slope rules to design pipe inverts.
    PipeNetworkDesign {
        structures: String,
        pipes: String,
        rules: String,
        out_structs: String,
        out_pipes: String,
    },
    /// Detailed analysis including velocity.
    PipeNetworkAnalyzeDetailed {
        structures: String,
        pipes: String,
        out_csv: String,
        out_xml: String,
    },
    /// Compute optimal station points along an alignment and export to a file.
    Stakeout {
        halign: String,
        output: String,
        format: String,
        #[arg(long, default_value_t = 10.0)]
        interval: f64,
        #[arg(long, default_value_t = 0.0)]
        offset: f64,
    },
    /// Record or play a command macro.
    Macro {
        #[command(subcommand)]
        action: MacroAction,
    },
}

#[derive(Subcommand)]
enum MacroAction {
    /// Record commands to a file. Enter commands line by line, blank line to finish.
    Record { file: String },
    /// Play commands from a file.
    Play { file: String },
}

fn main() {
    let cli = Cli::parse();
    commands::run(cli.command, cli.epsg);
}
