//! File input and output helpers for project data.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};

use crate::geometry::Point;

/// Reads a file to string.
pub fn read_to_string(path: &str) -> io::Result<String> {
    let mut buffer = String::new();
    File::open(path)?.read_to_string(&mut buffer)?;
    Ok(buffer)
}

/// Writes the provided string slice to the given file path, overwriting any
/// existing contents.
pub fn write_string(path: &str, contents: &str) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents.as_bytes())
}

/// Reads a file and returns a vector of lines as `String`s.
pub fn read_lines(path: &str) -> io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    reader.lines().collect()
}

/// Reads a CSV file containing pairs of `x,y` coordinates into a list of
/// [`Point`]s.
///
/// Each line of the CSV file is expected to contain two floating point numbers
/// separated by a comma.
pub fn read_points_csv(path: &str) -> io::Result<Vec<Point>> {
    let lines = read_lines(path)?;
    lines
        .iter()
        .map(|line| {
            let mut parts = line.split(',');
            let x = parts
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing x"))?
                .trim()
                .parse::<f64>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let y = parts
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing y"))?
                .trim()
                .parse::<f64>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(Point::new(x, y))
        })
        .collect()
}

/// Writes a slice of [`Point`]s to a CSV file with each line in the form
/// `x,y`.
pub fn write_points_csv(path: &str, points: &[Point]) -> io::Result<()> {
    let mut file = File::create(path)?;
    for p in points {
        writeln!(file, "{},{}", p.x, p.y)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_read_string() {
        let path = std::env::temp_dir().join("cad_io_test.txt");
        let path_str = path.to_str().unwrap();
        write_string(path_str, "hello world").unwrap();
        let contents = read_to_string(path_str).unwrap();
        assert_eq!(contents, "hello world");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_read_points() {
        let path = std::env::temp_dir().join("cad_points.csv");
        let path_str = path.to_str().unwrap();
        let pts = vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)];
        write_points_csv(path_str, &pts).unwrap();
        let read_pts = read_points_csv(path_str).unwrap();
        assert_eq!(read_pts, pts);
        std::fs::remove_file(path).ok();
    }
}

