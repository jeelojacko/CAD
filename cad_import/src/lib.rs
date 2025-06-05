use std::io;

use survey_cad::{
    geometry::Point,
    io::{read_lines, read_points_csv as sc_read_csv, read_points_geojson as sc_read_geojson},
};

/// Reads a CSV file of `x,y` pairs into [`Point`]s.
pub fn read_points_csv(path: &str) -> io::Result<Vec<Point>> {
    sc_read_csv(path)
}

/// Reads a GeoJSON file of Point features into [`Point`]s.
pub fn read_points_geojson(path: &str) -> io::Result<Vec<Point>> {
    sc_read_geojson(path)
}

/// Reads a very simple ASCII DXF file containing only `POINT` entities and
/// returns their coordinates as [`Point`]s.
pub fn read_points_dxf(path: &str) -> io::Result<Vec<Point>> {
    let lines = read_lines(path)?;
    let mut pts = Vec::new();
    let mut iter = lines.iter();
    while let (Some(code), Some(value)) = (iter.next(), iter.next()) {
        if code.trim() == "0" && value.trim() == "POINT" {
            let mut x = None;
            let mut y = None;
            while let (Some(c), Some(v)) = (iter.next(), iter.next()) {
                match c.trim() {
                    "10" => x = v.trim().parse().ok(),
                    "20" => y = v.trim().parse().ok(),
                    "30" => break,
                    _ => {}
                }
            }
            if let (Some(x), Some(y)) = (x, y) {
                pts.push(Point::new(x, y));
            }
        }
    }
    Ok(pts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use survey_cad::io::write_points_dxf;

    #[test]
    fn read_written_dxf_points() {
        let path = std::env::temp_dir().join("import_pts.dxf");
        let pts = vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)];
        write_points_dxf(path.to_str().unwrap(), &pts).unwrap();
        let read = read_points_dxf(path.to_str().unwrap()).unwrap();
        assert_eq!(read, pts);
        std::fs::remove_file(path).ok();
    }
}
