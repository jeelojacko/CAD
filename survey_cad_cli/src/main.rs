use clap::{Parser, Subcommand};
use survey_cad::{
    geometry::Point,
    io::{
        read_points_csv, read_points_geojson, read_to_string, write_points_csv, write_points_dxf,
        write_points_geojson, write_string,
    },
    render::{render_point, render_points},
    surveying::{
        bearing, forward, level_elevation, line_intersection, station_distance, vertical_angle,
        Station, Traverse,
    },
};
use cad_import::{read_point_file, PointFileFormat};

fn no_render() -> bool {
    std::env::var("SURVEY_CAD_TEST").is_ok()
}

/// Simple command line interface demonstrating the survey CAD utilities.
#[derive(Parser)]
#[command(name = "survey_cad_cli", version)]
struct Cli {
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
    RenderPoint { x: f64, y: f64 },
    /// Export points from a CSV file to GeoJSON.
    ExportGeojson { input: String, output: String },
    /// Import points from a GeoJSON file to CSV.
    ImportGeojson { input: String, output: String },
    /// Import survey points from a text file in a given format to CSV.
    ImportPoints {
        format: String,
        input: String,
        output: String,
    },
    /// Export points from a CSV file to a simple DXF.
    ExportDxf { input: String, output: String },
    /// View points from a CSV file.
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
}

fn main() {
    let cli = Cli::parse();
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
        Commands::TraverseArea { path } => match read_points_csv(&path) {
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
        Commands::RenderPoint { x, y } => {
            let p = Point::new(x, y);
            if no_render() {
                println!("Rendering point ({}, {})", p.x, p.y);
            } else {
                render_point(p);
            }
        }
        Commands::ExportGeojson { input, output } => match read_points_csv(&input) {
            Ok(pts) => match write_points_geojson(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        Commands::ImportGeojson { input, output } => match read_points_geojson(&input) {
            Ok(pts) => match write_points_csv(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        Commands::ImportPoints { format, input, output } => {
            match PointFileFormat::from_str(&format) {
                Some(fmt) => match read_point_file(&input, fmt) {
                    Ok(pts) => {
                        use std::io::Write;
                        match std::fs::File::create(&output) {
                            Ok(mut file) => {
                                for p in pts {
                                    if let Some(n) = p.number {
                                        if write!(file, "{}", n).is_err() { continue; }
                                    }
                                    if write!(file, ",{},{},{},", p.point.x, p.point.y, p.point.z).is_err() { continue; }
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
                None => eprintln!("Unknown format {}", format),
            }
        }
        Commands::ExportDxf { input, output } => match read_points_csv(&input) {
            Ok(pts) => match write_points_dxf(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        Commands::ViewPoints { input } => match read_points_csv(&input) {
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
    }
}
