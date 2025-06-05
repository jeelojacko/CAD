use clap::{Parser, Subcommand};
use survey_cad::{
    geometry::Point,
    io::{read_points_csv, read_to_string, write_string},
    render::render_point,
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
    }
}
