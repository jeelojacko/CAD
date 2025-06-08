use cad_import::{read_point_file, PointFileFormat};
use clap::{Parser, Subcommand};
use pipe_network;
use std::str::FromStr;
#[cfg(feature = "fgdb")]
use survey_cad::io::fgdb::read_points_fgdb;
#[cfg(feature = "kml")]
use survey_cad::io::kml::{read_points_kml, write_points_kml};
#[cfg(feature = "render")]
use survey_cad::render::{render_point, render_points};
use survey_cad::{
    alignment::{
        Alignment, HorizontalAlignment, HorizontalElement, VerticalAlignment, VerticalElement,
    },
    corridor::{corridor_mass_haul, corridor_volume},
    crs::Crs,
    dtm::Tin,
    geometry::{Point, Point3},
    io::{
        landxml::read_landxml_surface, read_lines, read_points_csv, read_points_geojson,
        read_to_string, write_points_csv, write_points_csv_gnss, write_points_dxf,
        write_points_geojson, write_points_raw, write_string,
    },
    surveying::{
        bearing, forward, level_elevation, line_intersection, optimal_stationing,
        stakeout_position, station_distance, vertical_angle, Station, Traverse,
    },
};

#[cfg(feature = "las")]
use survey_cad::io::las::read_points_las;
#[cfg(feature = "shapefile")]
use survey_cad::io::shp::{
    read_points_shp, read_polygons_shp, read_polylines_shp, write_points_shp, write_polygons_shp,
    write_polylines_shp,
};

fn no_render() -> bool {
    std::env::var("SURVEY_CAD_TEST").is_ok()
}

#[cfg(feature = "las")]
fn write_points_csv_3d(path: &str, points: &[Point3]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    for p in points {
        writeln!(file, "{},{},{}", p.x, p.y, p.z)?;
    }
    Ok(())
}

#[cfg(feature = "las")]
fn read_points_csv_3d(path: &str) -> std::io::Result<Vec<Point3>> {
    use std::io::{self};
    let lines = read_lines(path)?;
    let mut pts = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: expected three comma-separated values", idx + 1),
            ));
        }
        let x = parts[0].trim().parse::<f64>().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: {}", idx + 1, e),
            )
        })?;
        let y = parts[1].trim().parse::<f64>().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: {}", idx + 1, e),
            )
        })?;
        let z = parts[2].trim().parse::<f64>().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: {}", idx + 1, e),
            )
        })?;
        pts.push(Point3::new(x, y, z));
    }
    Ok(pts)
}

fn write_points_classified(
    path: &str,
    points: &[Point3],
    classes: &[survey_cad::Classification],
) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    for (p, c) in points.iter().zip(classes.iter()) {
        writeln!(file, "{},{},{},{:?}", p.x, p.y, p.z, c)?;
    }
    Ok(())
}

fn print_point(p: Point) {
    println!("{:.3},{:.3}", p.x, p.y);
}

fn print_station(sta: f64, elev: f64) {
    println!("{:.3},{:.3}", sta, elev);
}

#[cfg(feature = "shapefile")]
fn read_polylines_csv(path: &str) -> std::io::Result<Vec<survey_cad::geometry::Polyline>> {
    let lines = read_lines(path)?;
    let mut polylines = Vec::new();
    let mut current = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            if current.len() >= 2 {
                polylines.push(survey_cad::geometry::Polyline::new(current.clone()));
            } else if !current.is_empty() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("line {}: polyline has less than 2 points", idx + 1),
                ));
            }
            current.clear();
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("line {}: expected x,y", idx + 1),
            ));
        }
        let x: f64 = parts[0].trim().parse().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("line {}: {}", idx + 1, e),
            )
        })?;
        let y: f64 = parts[1].trim().parse().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("line {}: {}", idx + 1, e),
            )
        })?;
        current.push(Point::new(x, y));
    }
    if !current.is_empty() {
        if current.len() >= 2 {
            polylines.push(survey_cad::geometry::Polyline::new(current));
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "final polyline has less than 2 points",
            ));
        }
    }
    Ok(polylines)
}

#[cfg(feature = "shapefile")]
fn write_polylines_csv(
    path: &str,
    polylines: &[survey_cad::geometry::Polyline],
) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    for (i, pl) in polylines.iter().enumerate() {
        for v in &pl.vertices {
            writeln!(file, "{},{}", v.x, v.y)?;
        }
        if i + 1 < polylines.len() {
            writeln!(file)?;
        }
    }
    Ok(())
}

#[cfg(feature = "shapefile")]
fn read_polygons_csv(path: &str) -> std::io::Result<Vec<Vec<Point>>> {
    let lines = read_lines(path)?;
    let mut polygons = Vec::new();
    let mut current = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                polygons.push(current.clone());
                current.clear();
            }
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("line {}: expected x,y", idx + 1),
            ));
        }
        let x: f64 = parts[0].trim().parse().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("line {}: {}", idx + 1, e),
            )
        })?;
        let y: f64 = parts[1].trim().parse().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("line {}: {}", idx + 1, e),
            )
        })?;
        current.push(Point::new(x, y));
    }
    if !current.is_empty() {
        polygons.push(current);
    }
    Ok(polygons)
}

#[cfg(feature = "shapefile")]
fn write_polygons_csv(path: &str, polygons: &[Vec<Point>]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    for (i, poly) in polygons.iter().enumerate() {
        for p in poly {
            writeln!(file, "{},{}", p.x, p.y)?;
        }
        if i + 1 < polygons.len() {
            writeln!(file)?;
        }
    }
    Ok(())
}

fn read_surface(path: &str) -> std::io::Result<Tin> {
    if path.to_ascii_lowercase().ends_with(".xml") {
        read_landxml_surface(path)
    } else {
        let lines = read_lines(path)?;
        let mut pts = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 3 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("line {}: expected x,y,z", idx + 1),
                ));
            }
            let x: f64 = parts[0].trim().parse().map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("line {}: {}", idx + 1, e),
                )
            })?;
            let y: f64 = parts[1].trim().parse().map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("line {}: {}", idx + 1, e),
                )
            })?;
            let z: f64 = parts[2].trim().parse().map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("line {}: {}", idx + 1, e),
                )
            })?;
            pts.push(Point3::new(x, y, z));
        }
        Ok(Tin::from_points(pts))
    }
}

/// Simple command line interface demonstrating the survey CAD utilities.
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
    FilterNoise {
        input: String,
        output: String,
        #[arg(long, default_value_t = 1.0)]
        radius: f64,
        #[arg(long, default_value_t = 3)]
        min_neighbors: usize,
    },
    /// Classify a CSV point cloud into ground, vegetation and buildings.
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
}

fn main() {
    let cli = Cli::parse();
    let _working_crs = Crs::from_epsg(cli.epsg);
    println!("Using CRS: {}", _working_crs.definition());
    match cli.command {
        Commands::StationDistance {
            name_a,
            x1,
            y1,
            name_b,
            x2,
            y2,
        } => {
            let a = Station::new(name_a, Point::new(x1, y1));
            let b = Station::new(name_b, Point::new(x2, y2));
            let dist = station_distance(&a, &b);
            println!("Distance between {} and {} is {:.3}", a.name, b.name, dist);
        }
        Commands::TraverseArea { path } => match read_points_csv(&path, None, None) {
            Ok(pts) => {
                let traverse = Traverse::new(pts);
                println!("Area: {:.3}", traverse.area());
            }
            Err(e) => eprintln!("Error reading {}: {}", path, e),
        },
        Commands::Copy { src, dest } => match read_to_string(&src) {
            Ok(contents) => match write_string(&dest, &contents) {
                Ok(()) => println!("Copied {} to {}", src, dest),
                Err(e) => eprintln!("Error writing {}: {}", dest, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", src, e),
        },
        #[cfg(feature = "render")]
        Commands::RenderPoint { x, y } => {
            let p = Point::new(x, y);
            if no_render() {
                println!("Rendering point ({}, {})", p.x, p.y);
            } else {
                render_point(p);
            }
        }
        Commands::ExportGeojson {
            input,
            output,
            src_epsg,
            dst_epsg,
        } => match read_points_csv(&input, src_epsg, dst_epsg) {
            Ok(pts) => match write_points_geojson(&output, &pts, None, None) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "kml")]
        Commands::ExportKml {
            input,
            output,
            src_epsg,
            dst_epsg,
        } => match read_points_csv(&input, src_epsg, dst_epsg) {
            Ok(pts) => match write_points_kml(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        Commands::ImportGeojson {
            input,
            output,
            src_epsg,
            dst_epsg,
        } => match read_points_geojson(&input, src_epsg, dst_epsg) {
            Ok(pts) => match write_points_csv(&output, &pts, None, None) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "kml")]
        Commands::ImportKml {
            input,
            output,
            src_epsg,
            dst_epsg,
        } => match read_points_kml(&input) {
            Ok(pts) => match write_points_csv(&output, &pts, src_epsg, dst_epsg) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "fgdb")]
        Commands::ImportFgdb {
            path,
            layer,
            output,
        } => match read_points_fgdb(&path, &layer) {
            Ok(pts) => match write_points_csv(&output, &pts, None, None) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", path, e),
        },
        Commands::ImportPoints {
            format,
            input,
            output,
        } => match PointFileFormat::from_str(&format) {
            Ok(fmt) => match read_point_file(&input, fmt) {
                Ok(pts) => {
                    use std::io::Write;
                    match std::fs::File::create(&output) {
                        Ok(mut file) => {
                            for p in pts {
                                if let Some(n) = p.number {
                                    if write!(file, "{}", n).is_err() {
                                        continue;
                                    }
                                }
                                if write!(file, ",{},{},{},", p.point.x, p.point.y, p.point.z)
                                    .is_err()
                                {
                                    continue;
                                }
                                if let Some(desc) = p.description {
                                    let _ = writeln!(file, "{}", desc);
                                } else {
                                    let _ = writeln!(file);
                                }
                            }
                            println!("Wrote {}", output);
                        }
                        Err(e) => eprintln!("Error writing {}: {}", output, e),
                    }
                }
                Err(e) => eprintln!("Error reading {}: {}", input, e),
            },
            Err(_) => eprintln!("Unknown format {}", format),
        },
        Commands::ExportDxf {
            input,
            output,
            src_epsg,
            dst_epsg,
        } => match read_points_csv(&input, src_epsg, dst_epsg) {
            Ok(pts) => match write_points_dxf(&output, &pts, None, None) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "shapefile")]
        Commands::ExportShp { input, output } => match read_points_csv(&input, None, None) {
            Ok(pts) => match write_points_shp(&output, &pts, None) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "shapefile")]
        Commands::ImportShp { input, output } => match read_points_shp(&input) {
            Ok((pts, pts3)) => {
                if let Some(z) = pts3 {
                    match write_points_csv_3d(&output, &z) {
                        Ok(()) => println!("Wrote {}", output),
                        Err(e) => eprintln!("Error writing {}: {}", output, e),
                    }
                } else {
                    match write_points_csv(&output, &pts, None, None) {
                        Ok(()) => println!("Wrote {}", output),
                        Err(e) => eprintln!("Error writing {}: {}", output, e),
                    }
                }
            }
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "shapefile")]
        Commands::ExportPolylinesShp { input, output } => match read_polylines_csv(&input) {
            Ok(lines) => match write_polylines_shp(&output, &lines, None) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "shapefile")]
        Commands::ImportPolylinesShp { input, output } => match read_polylines_shp(&input) {
            Ok((lines, _)) => match write_polylines_csv(&output, &lines) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "shapefile")]
        Commands::ExportPolygonsShp { input, output } => match read_polygons_csv(&input) {
            Ok(polys) => match write_polygons_shp(&output, &polys, None) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "shapefile")]
        Commands::Contours {
            surface,
            output,
            interval,
            smooth,
        } => match read_surface(&surface) {
            Ok(tin) => {
                let (lines, lines_z) = tin.contour_polylines(interval, smooth);
                if output.to_ascii_lowercase().ends_with(".shp") {
                    match survey_cad::io::shp::write_polylines_shp(&output, &lines, Some(&lines_z))
                    {
                        Ok(()) => println!("Wrote {}", output),
                        Err(e) => eprintln!("Error writing {}: {}", output, e),
                    }
                } else {
                    match write_polylines_csv(&output, &lines) {
                        Ok(()) => println!("Wrote {}", output),
                        Err(e) => eprintln!("Error writing {}: {}", output, e),
                    }
                }
            }
            Err(e) => eprintln!("Error reading {}: {}", surface, e),
        },
        #[cfg(feature = "shapefile")]
        Commands::ImportPolygonsShp { input, output } => match read_polygons_shp(&input) {
            Ok((polys, _)) => match write_polygons_csv(&output, &polys) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "las")]
        Commands::ImportLas { input, output } => match read_points_las(&input) {
            Ok(pts) => match write_points_csv_3d(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "las")]
        Commands::ExportLas { input, output } => match read_points_csv_3d(&input) {
            Ok(pts) => match write_points_las(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "e57")]
        Commands::ImportE57 { input, output } => match read_points_e57(&input) {
            Ok(pts) => match write_points_csv_3d(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "e57")]
        Commands::ExportE57 { input, output } => match read_points_csv_3d(&input) {
            Ok(pts) => match write_points_e57(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        Commands::FilterNoise {
            input,
            output,
            radius,
            min_neighbors,
        } => match read_points_csv_3d(&input) {
            Ok(pts) => {
                let filtered = survey_cad::filter_noise(&pts, radius, min_neighbors);
                if let Err(e) = write_points_csv_3d(&output, &filtered) {
                    eprintln!("Error writing {}: {}", output, e);
                } else {
                    println!("Wrote {}", output);
                }
            }
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        Commands::ClassifyCloud {
            input,
            output,
            cell_size,
            ground_threshold,
            veg_threshold,
        } => match read_points_csv_3d(&input) {
            Ok(pts) => {
                let classes =
                    survey_cad::classify_points(&pts, cell_size, ground_threshold, veg_threshold);
                if let Err(e) = write_points_classified(&output, &pts, &classes) {
                    eprintln!("Error writing {}: {}", output, e);
                } else {
                    println!("Wrote {}", output);
                }
            }
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        #[cfg(feature = "render")]
        Commands::ViewPoints { input } => match read_points_csv(&input, None, None) {
            Ok(pts) => {
                if no_render() {
                    println!("Rendering {} points", pts.len());
                } else {
                    render_points(&pts);
                }
            }
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        Commands::VerticalAngle {
            name_a,
            x1,
            y1,
            elev_a,
            name_b,
            x2,
            y2,
            elev_b,
        } => {
            let a = Station::new(name_a, Point::new(x1, y1));
            let b = Station::new(name_b, Point::new(x2, y2));
            let ang = vertical_angle(&a, elev_a, &b, elev_b);
            println!(
                "Vertical angle between {} and {} is {:.3} rad",
                a.name, b.name, ang
            );
        }
        Commands::Bearing { x1, y1, x2, y2 } => {
            let bng = bearing(Point::new(x1, y1), Point::new(x2, y2));
            println!("Bearing: {:.3} rad", bng);
        }
        Commands::Forward {
            x,
            y,
            bearing: bng,
            distance,
        } => {
            let p = forward(Point::new(x, y), bng, distance);
            println!("Point: {:.3},{:.3}", p.x, p.y);
        }
        Commands::Intersection {
            x1,
            y1,
            x2,
            y2,
            x3,
            y3,
            x4,
            y4,
        } => match line_intersection(
            Point::new(x1, y1),
            Point::new(x2, y2),
            Point::new(x3, y3),
            Point::new(x4, y4),
        ) {
            Some(pt) => println!("Intersection: {:.3},{:.3}", pt.x, pt.y),
            None => println!("Lines are parallel"),
        },
        Commands::LevelElevation {
            start_elev,
            backsight,
            foresight,
        } => {
            let elev = level_elevation(start_elev, backsight, foresight);
            println!("New elevation: {:.3}", elev);
        }
        Commands::NetworkAdjust {
            points,
            observations,
        } => {
            use std::collections::HashMap;
            use survey_cad::surveying::{adjust_network_report, Observation};
            match (read_lines(&points), read_lines(&observations)) {
                (Ok(p_lines), Ok(o_lines)) => {
                    let mut names = Vec::new();
                    let mut pts = Vec::new();
                    let mut fixed = Vec::new();
                    for (idx, line) in p_lines.iter().enumerate() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let parts: Vec<&str> = line.split(',').collect();
                        if parts.len() < 3 {
                            eprintln!("{} line {} invalid", points, idx + 1);
                            return;
                        }
                        names.push(parts[0].trim().to_string());
                        let x: f64 = parts[1].trim().parse().unwrap_or(0.0);
                        let y: f64 = parts[2].trim().parse().unwrap_or(0.0);
                        if parts.get(3).map_or(false, |v| v.trim() == "1") {
                            fixed.push(idx);
                        }
                        pts.push(Point::new(x, y));
                    }
                    let mut index: HashMap<String, usize> = HashMap::new();
                    for (i, n) in names.iter().enumerate() {
                        index.insert(n.clone(), i);
                    }
                    let mut obs = Vec::new();
                    for (idx, line) in o_lines.iter().enumerate() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let parts: Vec<&str> = line.split(',').collect();
                        if parts.is_empty() {
                            continue;
                        }
                        match parts[0].trim().to_ascii_lowercase().as_str() {
                            "dist" | "distance" => {
                                if parts.len() < 4 {
                                    eprintln!("{} line {} invalid", observations, idx + 1);
                                    return;
                                }
                                let from = index[parts[1].trim()];
                                let to = index[parts[2].trim()];
                                let val: f64 = parts[3].trim().parse().unwrap_or(0.0);
                                obs.push(Observation::Distance {
                                    from,
                                    to,
                                    value: val,
                                    weight: 1.0,
                                });
                            }
                            "angle" => {
                                if parts.len() < 5 {
                                    eprintln!("{} line {} invalid", observations, idx + 1);
                                    return;
                                }
                                let at = index[parts[1].trim()];
                                let from = index[parts[2].trim()];
                                let to = index[parts[3].trim()];
                                let val: f64 = parts[4].trim().parse().unwrap_or(0.0);
                                obs.push(Observation::Angle {
                                    at,
                                    from,
                                    to,
                                    value: val,
                                    weight: 1.0,
                                });
                            }
                            _ => {
                                eprintln!("{} line {} unknown obs", observations, idx + 1);
                                return;
                            }
                        }
                    }
                    let (result, report) = adjust_network_report(&pts, &fixed, &obs, 1e-6, 10);
                    println!("iterations: {}", report.iterations.len());
                    for (name, p) in names.iter().zip(result.points.iter()) {
                        println!("{}, {:.3}, {:.3}", name, p.x, p.y);
                    }
                    for (o, v) in obs.iter().zip(result.residuals.iter()) {
                        match o {
                            Observation::Distance { .. } => println!("distance residual {:.4}", v),
                            Observation::Angle { .. } => println!("angle residual {:.6}", v),
                        }
                    }
                }
                (Err(e), _) => eprintln!("Error reading {}: {}", points, e),
                (_, Err(e)) => eprintln!("Error reading {}: {}", observations, e),
            }
        }
        Commands::CorridorVolume {
            design,
            ground,
            halign,
            valign,
            width,
            interval,
            offset_step,
        } => {
            match (
                read_surface(&design),
                read_surface(&ground),
                read_points_csv(&halign, None, None),
                read_points_csv(&valign, None, None),
            ) {
                (Ok(des), Ok(grd), Ok(h_pts), Ok(v_pts)) => {
                    let hal = HorizontalAlignment::new(h_pts);
                    let v_pairs: Vec<(f64, f64)> = v_pts.iter().map(|p| (p.x, p.y)).collect();
                    let val = VerticalAlignment::new(v_pairs);
                    let align = Alignment::new(hal, val);
                    let vol = corridor_volume(&des, &grd, &align, width, interval, offset_step);
                    println!("Volume: {:.3}", vol);
                }
                (Err(e), _, _, _) => eprintln!("Error reading {}: {}", design, e),
                (_, Err(e), _, _) => eprintln!("Error reading {}: {}", ground, e),
                (_, _, Err(e), _) => eprintln!("Error reading {}: {}", halign, e),
                (_, _, _, Err(e)) => eprintln!("Error reading {}: {}", valign, e),
            }
        }
        Commands::MassHaul {
            design,
            ground,
            halign,
            valign,
            width,
            interval,
            offset_step,
        } => {
            match (
                read_surface(&design),
                read_surface(&ground),
                read_points_csv(&halign, None, None),
                read_points_csv(&valign, None, None),
            ) {
                (Ok(des), Ok(grd), Ok(h_pts), Ok(v_pts)) => {
                    let hal = HorizontalAlignment::new(h_pts);
                    let v_pairs: Vec<(f64, f64)> = v_pts.iter().map(|p| (p.x, p.y)).collect();
                    let val = VerticalAlignment::new(v_pairs);
                    let align = Alignment::new(hal, val);
                    let haul = corridor_mass_haul(&des, &grd, &align, width, interval, offset_step);
                    for (sta, vol) in haul {
                        println!("{:.3},{:.3}", sta, vol);
                    }
                }
                (Err(e), _, _, _) => eprintln!("Error reading {}: {}", design, e),
                (_, Err(e), _, _) => eprintln!("Error reading {}: {}", ground, e),
                (_, _, Err(e), _) => eprintln!("Error reading {}: {}", halign, e),
                (_, _, _, Err(e)) => eprintln!("Error reading {}: {}", valign, e),
            }
        }
        Commands::CreateIntersection {
            align_a,
            align_b,
            radius,
        } => {
            match (
                read_points_csv(&align_a, None, None),
                read_points_csv(&align_b, None, None),
            ) {
                (Ok(a_pts), Ok(b_pts)) => {
                    let a = HorizontalAlignment::new(a_pts);
                    let b = HorizontalAlignment::new(b_pts);
                    if let Some(res) =
                        survey_cad::intersection::curb_return_between_alignments(&a, &b, radius)
                    {
                        println!("tangent_a: {:.3},{:.3}", res.start.x, res.start.y);
                        println!("tangent_b: {:.3},{:.3}", res.end.x, res.end.y);
                        println!(
                            "center: {:.3},{:.3} radius: {:.3}",
                            res.arc.center.x, res.arc.center.y, res.arc.radius
                        );
                    } else {
                        println!("No intersection");
                    }
                }
                (Err(e), _) => eprintln!("Error reading {}: {}", align_a, e),
                (_, Err(e)) => eprintln!("Error reading {}: {}", align_b, e),
            }
        }
        Commands::CreateFullIntersection {
            halign_a,
            valign_a,
            halign_b,
            valign_b,
            radius,
        } => {
            match (
                read_points_csv(&halign_a, None, None),
                read_points_csv(&valign_a, None, None),
                read_points_csv(&halign_b, None, None),
                read_points_csv(&valign_b, None, None),
            ) {
                (Ok(ha_pts), Ok(va_pts), Ok(hb_pts), Ok(vb_pts)) => {
                    let ha = HorizontalAlignment::new(ha_pts);
                    let va_pairs: Vec<(f64, f64)> = va_pts.iter().map(|p| (p.x, p.y)).collect();
                    let va = VerticalAlignment::new(va_pairs);
                    let hb = HorizontalAlignment::new(hb_pts);
                    let vb_pairs: Vec<(f64, f64)> = vb_pts.iter().map(|p| (p.x, p.y)).collect();
                    let vb = VerticalAlignment::new(vb_pairs);
                    let a = Alignment::new(ha, va);
                    let b = Alignment::new(hb, vb);
                    if let Some(res) =
                        survey_cad::intersection::intersection_alignment(&a, &b, radius)
                    {
                        println!("Horizontal alignment:");
                        if let Some(first) = res.horizontal.elements.first() {
                            match first {
                                HorizontalElement::Tangent { start, .. } => print_point(*start),
                                HorizontalElement::Curve { arc } => {
                                    let p = Point::new(
                                        arc.center.x + arc.radius * arc.start_angle.cos(),
                                        arc.center.y + arc.radius * arc.start_angle.sin(),
                                    );
                                    print_point(p);
                                }
                                HorizontalElement::Spiral { spiral } => {
                                    print_point(spiral.start_point())
                                }
                            }
                        }
                        for elem in &res.horizontal.elements {
                            match elem {
                                HorizontalElement::Tangent { end, .. } => print_point(*end),
                                HorizontalElement::Curve { arc } => {
                                    let p = Point::new(
                                        arc.center.x + arc.radius * arc.end_angle.cos(),
                                        arc.center.y + arc.radius * arc.end_angle.sin(),
                                    );
                                    print_point(p);
                                }
                                HorizontalElement::Spiral { spiral } => {
                                    print_point(spiral.end_point())
                                }
                            }
                        }
                        println!("Vertical alignment:");
                        if let Some(first) = res.vertical.elements.first() {
                            match first {
                                VerticalElement::Grade {
                                    start_station,
                                    start_elev,
                                    ..
                                } => {
                                    print_station(*start_station, *start_elev);
                                }
                                VerticalElement::Parabola {
                                    start_station,
                                    start_elev,
                                    ..
                                } => {
                                    print_station(*start_station, *start_elev);
                                }
                            }
                        }
                        for elem in &res.vertical.elements {
                            match elem {
                                VerticalElement::Grade {
                                    end_station,
                                    end_elev,
                                    ..
                                } => {
                                    print_station(*end_station, *end_elev);
                                }
                                VerticalElement::Parabola {
                                    start_station,
                                    end_station,
                                    start_elev,
                                    start_grade,
                                    end_grade,
                                } => {
                                    let l = end_station - start_station;
                                    let elev = *start_elev
                                        + start_grade * l
                                        + 0.5 * (end_grade - start_grade) * l;
                                    print_station(*end_station, elev);
                                }
                            }
                        }
                    } else {
                        println!("No intersection");
                    }
                }
                (Err(e), _, _, _) => eprintln!("Error reading {}: {}", halign_a, e),
                (_, Err(e), _, _) => eprintln!("Error reading {}: {}", valign_a, e),
                (_, _, Err(e), _) => eprintln!("Error reading {}: {}", halign_b, e),
                (_, _, _, Err(e)) => eprintln!("Error reading {}: {}", valign_b, e),
            }
        }
        Commands::CreatePipeNetwork {
            structures,
            pipes,
            output,
        } => match pipe_network::read_network_csv(&structures, &pipes) {
            Ok(net) => {
                if let Err(e) = pipe_network::write_network_landxml(&output, &net) {
                    eprintln!("Error writing {}: {}", output, e);
                } else {
                    println!("Wrote {}", output);
                }
            }
            Err(e) => eprintln!("Error reading network: {}", e),
        },
        Commands::PipeNetworkGrade {
            structures,
            pipes,
            flow,
        } => match pipe_network::read_network_csv(&structures, &pipes) {
            Ok(net) => {
                let idx = net.structure_index();
                for pipe in &net.pipes {
                    if let (Some(&from_idx), Some(&to_idx)) =
                        (idx.get(pipe.from.as_str()), idx.get(pipe.to.as_str()))
                    {
                        let a = &net.structures[from_idx];
                        let b = &net.structures[to_idx];
                        let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                        let hl =
                            pipe_network::hazen_williams_headloss(flow, len, pipe.diameter, pipe.c);
                        let end = pipe_network::hydraulic_grade(a.z, hl);
                        println!("{}: headloss {:.3}, end grade {:.3}", pipe.id, hl, end);
                    }
                }
            }
            Err(e) => eprintln!("Error reading network: {}", e),
        },
        Commands::PipeNetworkAnalyze {
            structures,
            pipes,
            out_csv,
            out_xml,
        } => match pipe_network::read_network_csv(&structures, &pipes) {
            Ok(net) => {
                let res = pipe_network::analyze_network(&net);
                if let Err(e) = pipe_network::write_analysis_csv(&out_csv, &res) {
                    eprintln!("Error writing {}: {}", out_csv, e);
                }
                if let Err(e) = pipe_network::write_analysis_landxml(&out_xml, &res) {
                    eprintln!("Error writing {}: {}", out_xml, e);
                }
            }
            Err(e) => eprintln!("Error reading network: {}", e),
        },
        Commands::PipeNetworkDesign {
            structures,
            pipes,
            rules,
            out_structs,
            out_pipes,
        } => match (
            pipe_network::read_network_csv(&structures, &pipes),
            pipe_network::read_slope_rules_csv(&rules),
        ) {
            (Ok(mut net), Ok(rules)) => {
                pipe_network::apply_slope_rules(&mut net, &rules);
                if let Err(e) = pipe_network::write_network_csv(&net, &out_structs, &out_pipes) {
                    eprintln!("Error writing network: {}", e);
                }
            }
            (Err(e), _) | (_, Err(e)) => eprintln!("Error: {}", e),
        },
        Commands::PipeNetworkAnalyzeDetailed {
            structures,
            pipes,
            out_csv,
            out_xml,
        } => match pipe_network::read_network_csv(&structures, &pipes) {
            Ok(net) => {
                let res = pipe_network::analyze_network_detailed(&net);
                if let Err(e) = pipe_network::write_detailed_analysis_csv(&out_csv, &res) {
                    eprintln!("Error writing {}: {}", out_csv, e);
                }
                if let Err(e) = pipe_network::write_detailed_analysis_landxml(&out_xml, &res) {
                    eprintln!("Error writing {}: {}", out_xml, e);
                }
            }
            Err(e) => eprintln!("Error reading network: {}", e),
        },
        Commands::Stakeout {
            halign,
            output,
            format,
            interval,
            offset,
        } => match read_points_csv(&halign, None, None) {
            Ok(pts) => {
                let hal = HorizontalAlignment::new(pts);
                let stas = optimal_stationing(&hal, interval);
                let mut out_pts = Vec::new();
                for s in stas {
                    if let Some(p) = stakeout_position(&hal, s, offset) {
                        out_pts.push(p);
                    }
                }
                match format.as_str() {
                    "csv" => {
                        if let Err(e) = write_points_csv(&output, &out_pts, None, None) {
                            eprintln!("Error writing {}: {}", output, e);
                        }
                    }
                    "csv-gnss" => {
                        let pts3: Vec<Point3> =
                            out_pts.iter().map(|p| Point3::new(p.x, p.y, 0.0)).collect();
                        if let Err(e) = write_points_csv_gnss(&output, &pts3) {
                            eprintln!("Error writing {}: {}", output, e);
                        }
                    }
                    "raw" => {
                        let pts3: Vec<Point3> =
                            out_pts.iter().map(|p| Point3::new(p.x, p.y, 0.0)).collect();
                        if let Err(e) = write_points_raw(&output, &pts3) {
                            eprintln!("Error writing {}: {}", output, e);
                        }
                    }
                    _ => eprintln!("Unknown format {}", format),
                }
            }
            Err(e) => eprintln!("Error reading {}: {}", halign, e),
        },
    }
}
