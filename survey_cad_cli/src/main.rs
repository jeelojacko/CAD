use clap::{Parser, Subcommand};
use survey_cad::{
    geometry::Point,
    io::{
        read_points_csv, read_points_geojson, read_to_string, write_points_csv, write_points_dxf,
        write_points_geojson, write_string,
    },
    render::{render_point, render_points},
    surveying::{station_distance, Station, Traverse},
};

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
    /// Export points from a CSV file to a simple DXF.
    ExportDxf { input: String, output: String },
    /// View points from a CSV file.
    ViewPoints { input: String },
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
            render_point(p);
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
        Commands::ExportDxf { input, output } => match read_points_csv(&input) {
            Ok(pts) => match write_points_dxf(&output, &pts) {
                Ok(()) => println!("Wrote {}", output),
                Err(e) => eprintln!("Error writing {}: {}", output, e),
            },
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
        Commands::ViewPoints { input } => match read_points_csv(&input) {
            Ok(pts) => {
                render_points(&pts);
            }
            Err(e) => eprintln!("Error reading {}: {}", input, e),
        },
    }
}
