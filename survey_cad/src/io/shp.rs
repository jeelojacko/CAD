use crate::geometry::Point;
use shapefile::{
    Point as ShpPoint, Polygon as ShpPolygon, PolygonRing, Polyline as ShpPolyline, Shape,
    ShapeReader, ShapeWriter,
};
use std::io;

/// Reads a shapefile containing Point geometries and returns them as [`Point`] values.
pub fn read_points_shp(path: &str) -> io::Result<Vec<Point>> {
    let mut reader =
        ShapeReader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut pts = Vec::new();
    for record in reader.iter_shapes() {
        match record.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))? {
            Shape::Point(p) => pts.push(Point::new(p.x, p.y)),
            _ => {}
        }
    }
    Ok(pts)
}

/// Writes a list of [`Point`]s to a shapefile.
pub fn write_points_shp(path: &str, points: &[Point]) -> io::Result<()> {
    let mut writer =
        ShapeWriter::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for p in points {
        let shp = ShpPoint { x: p.x, y: p.y };
        writer
            .write_shape(&shp)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }
    writer.finalize().map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

/// Reads a shapefile containing PolyLine geometries and returns them as [`Polyline`] values.
pub fn read_polylines_shp(path: &str) -> io::Result<Vec<crate::geometry::Polyline>> {
    let mut reader =
        ShapeReader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut lines = Vec::new();
    for record in reader.iter_shapes() {
        match record.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))? {
            Shape::Polyline(pl) => {
                for part in pl.parts() {
                    let verts = part
                        .iter()
                        .map(|p| Point::new(p.x, p.y))
                        .collect();
                    lines.push(crate::geometry::Polyline::new(verts));
                }
            }
            _ => {}
        }
    }
    Ok(lines)
}

/// Writes a list of [`Polyline`]s to a shapefile.
pub fn write_polylines_shp(path: &str, polylines: &[crate::geometry::Polyline]) -> io::Result<()> {
    let mut writer =
        ShapeWriter::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for pl in polylines {
        if pl.vertices.len() < 2 {
            continue;
        }
        let shp_pts: Vec<ShpPoint> = pl
            .vertices
            .iter()
            .map(|p| ShpPoint { x: p.x, y: p.y })
            .collect();
        let shp_pl = ShpPolyline::new(shp_pts);
        writer
            .write_shape(&shp_pl)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }
    writer.finalize().map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

/// Reads a shapefile containing Polygon geometries and returns them as vectors of [`Point`].
pub fn read_polygons_shp(path: &str) -> io::Result<Vec<Vec<Point>>> {
    let mut reader =
        ShapeReader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut polys = Vec::new();
    for record in reader.iter_shapes() {
        match record.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))? {
            Shape::Polygon(pg) => {
                for ring in pg.rings() {
                    let verts = ring
                        .points()
                        .iter()
                        .map(|p| Point::new(p.x, p.y))
                        .collect();
                    polys.push(verts);
                }
            }
            _ => {}
        }
    }
    Ok(polys)
}

/// Writes a list of polygons to a shapefile.
pub fn write_polygons_shp(path: &str, polygons: &[Vec<Point>]) -> io::Result<()> {
    let mut writer =
        ShapeWriter::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for poly in polygons {
        if poly.len() < 3 {
            continue;
        }
        let shp_pts: Vec<ShpPoint> = poly.iter().map(|p| ShpPoint { x: p.x, y: p.y }).collect();
        let ring = PolygonRing::Outer(shp_pts);
        let shp_poly = ShpPolygon::new(ring);
        writer
            .write_shape(&shp_poly)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }
    writer.finalize().map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
