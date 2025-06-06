use truck_modeling::{self as truck, builder};
use truck_modeling::base::{Point2 as TPoint2, Point3 as TPoint3, Vector3};
use truck_geometry::specifieds::Line as TLine;

use crate::geometry::{Line, Point};

/// Convert our 2D [`Point`] to Truck [`TPoint2`].
pub fn point_to_truck(p: Point) -> TPoint2 {
    TPoint2::new(p.x, p.y)
}

/// Convert Truck [`TPoint2`] to our [`Point`].
pub fn point_from_truck(p: TPoint2) -> Point {
    Point::new(p.x, p.y)
}

/// Convert our [`Line`] to Truck [`TLine`].
pub fn line_to_truck(line: Line) -> TLine<TPoint2> {
    TLine(point_to_truck(line.start), point_to_truck(line.end))
}

/// Convert Truck [`TLine`] to our [`Line`].
pub fn line_from_truck(tline: TLine<TPoint2>) -> Line {
    Line::new(point_from_truck(tline.0), point_from_truck(tline.1))
}

/// Creates a unit cube using Truck builder utilities.
pub fn unit_cube() -> truck::topology::Solid {
    let v = builder::vertex(TPoint3::new(-0.5, -0.5, -0.5));
    let e = builder::tsweep(&v, Vector3::unit_x());
    let f = builder::tsweep(&e, Vector3::unit_y());
    builder::tsweep(&f, Vector3::unit_z())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{write_points_csv, write_points_geojson};
    use std::fs;

    #[test]
    fn export_cube_vertices() {
        let cube = unit_cube();
        let boundary = &cube.boundaries()[0];
        let mut points = Vec::new();
        for face in boundary.iter() {
            for v in face.boundaries()[0].vertex_iter() {
                let pt = v.point();
                points.push(point_from_truck(TPoint2::new(pt.x, pt.y)));
            }
        }
        let tmp_csv = std::env::temp_dir().join("cube_pts.csv");
        write_points_csv(tmp_csv.to_str().unwrap(), &points, None, None).unwrap();
        fs::remove_file(tmp_csv).ok();

        let tmp_json = std::env::temp_dir().join("cube_pts.geojson");
        write_points_geojson(tmp_json.to_str().unwrap(), &points, None, None).unwrap();
        fs::remove_file(tmp_json).ok();
    }
}
