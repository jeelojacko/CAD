use crate::geometry::{Point, Point3};
use shapefile::{
    Point as ShpPoint, PointZ as ShpPointZ, Polygon as ShpPolygon, PolygonRing, PolygonZ,
    Polyline as ShpPolyline, PolylineZ, Shape, ShapeReader, ShapeWriter, NO_DATA,
};
use std::io;

/// Reads a shapefile containing Point geometries and returns them as [`Point`] values.
pub fn read_points_shp(path: &str) -> io::Result<(Vec<Point>, Option<Vec<Point3>>)> {
    let mut reader =
        ShapeReader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut pts = Vec::new();
    let mut pts3: Option<Vec<Point3>> = None;
    for record in reader.iter_shapes() {
        match record.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))? {
            Shape::Point(p) => {
                pts.push(Point::new(p.x, p.y));
                if let Some(ref mut list) = pts3 {
                    list.push(Point3::new(p.x, p.y, 0.0));
                }
            }
            Shape::PointZ(p) => {
                pts.push(Point::new(p.x, p.y));
                match pts3 {
                    Some(ref mut list) => list.push(Point3::new(p.x, p.y, p.z)),
                    None => {
                        pts3 = Some(vec![Point3::new(p.x, p.y, p.z)]);
                    }
                }
            }
            _ => {}
        }
    }
    Ok((pts, pts3))
}

/// Writes a list of [`Point`]s to a shapefile.
pub fn write_points_shp(path: &str, points: &[Point], points_z: Option<&[Point3]>) -> io::Result<()> {
    let mut writer =
        ShapeWriter::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    if let Some(zpts) = points_z {
        for p in zpts {
            let shp = ShpPointZ::new(p.x, p.y, p.z, NO_DATA);
            writer
                .write_shape(&shp)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    } else {
        for p in points {
            let shp = ShpPoint { x: p.x, y: p.y };
            writer
                .write_shape(&shp)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    }
    writer.finalize().map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

/// Reads a shapefile containing PolyLine geometries and returns them as [`Polyline`] values.
pub fn read_polylines_shp(
    path: &str,
) -> io::Result<(Vec<crate::geometry::Polyline>, Option<Vec<Vec<Point3>>>)> {
    let mut reader =
        ShapeReader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut lines = Vec::new();
    let mut lines3: Option<Vec<Vec<Point3>>> = None;
    for record in reader.iter_shapes() {
        match record.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))? {
            Shape::Polyline(pl) => {
                for part in pl.parts() {
                    let verts: Vec<Point> = part.iter().map(|p| Point::new(p.x, p.y)).collect();
                    if let Some(ref mut v3) = lines3 {
                        v3.push(part.iter().map(|p| Point3::new(p.x, p.y, 0.0)).collect());
                    }
                    lines.push(crate::geometry::Polyline::new(verts));
                }
            }
            Shape::PolylineZ(pl) => {
                for part in pl.parts() {
                    let verts: Vec<Point> = part.iter().map(|p| Point::new(p.x, p.y)).collect();
                    match lines3 {
                        Some(ref mut v3) => v3.push(part.iter().map(|p| Point3::new(p.x, p.y, p.z)).collect()),
                        None => {
                            lines3 = Some(vec![part.iter().map(|p| Point3::new(p.x, p.y, p.z)).collect()]);
                        }
                    }
                    lines.push(crate::geometry::Polyline::new(verts));
                }
            }
            _ => {}
        }
    }
    Ok((lines, lines3))
}

/// Writes a list of [`Polyline`]s to a shapefile.
pub fn write_polylines_shp(
    path: &str,
    polylines: &[crate::geometry::Polyline],
    polylines_z: Option<&[Vec<Point3>]>,
) -> io::Result<()> {
    let mut writer =
        ShapeWriter::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    if let Some(pl3s) = polylines_z {
        for pl in pl3s {
            if pl.len() < 2 {
                continue;
            }
            let shp_pts: Vec<ShpPointZ> = pl
                .iter()
                .map(|p| ShpPointZ::new(p.x, p.y, p.z, NO_DATA))
                .collect();
            let shp_pl = PolylineZ::new(shp_pts);
            writer
                .write_shape(&shp_pl)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    } else {
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
    }
    writer.finalize().map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

/// Reads a shapefile containing Polygon geometries and returns them as vectors of [`Point`].
pub fn read_polygons_shp(path: &str) -> io::Result<(Vec<Vec<Point>>, Option<Vec<Vec<Point3>>>)> {
    let mut reader =
        ShapeReader::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut polys = Vec::new();
    let mut polys3: Option<Vec<Vec<Point3>>> = None;
    for record in reader.iter_shapes() {
        match record.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))? {
            Shape::Polygon(pg) => {
                for ring in pg.rings() {
                    let verts: Vec<Point> = ring
                        .points()
                        .iter()
                        .map(|p| Point::new(p.x, p.y))
                        .collect();
                    if let Some(ref mut v3) = polys3 {
                        v3.push(
                            ring.points()
                                .iter()
                                .map(|p| Point3::new(p.x, p.y, 0.0))
                                .collect(),
                        );
                    }
                    polys.push(verts);
                }
            }
            Shape::PolygonZ(pg) => {
                for ring in pg.rings() {
                    let verts: Vec<Point> = ring
                        .points()
                        .iter()
                        .map(|p| Point::new(p.x, p.y))
                        .collect();
                    match polys3 {
                        Some(ref mut v3) => v3.push(
                            ring.points()
                                .iter()
                                .map(|p| Point3::new(p.x, p.y, p.z))
                                .collect(),
                        ),
                        None => {
                            polys3 = Some(vec![
                                ring.points()
                                    .iter()
                                    .map(|p| Point3::new(p.x, p.y, p.z))
                                    .collect(),
                            ]);
                        }
                    }
                    polys.push(verts);
                }
            }
            _ => {}
        }
    }
    Ok((polys, polys3))
}

/// Writes a list of polygons to a shapefile.
pub fn write_polygons_shp(
    path: &str,
    polygons: &[Vec<Point>],
    polygons_z: Option<&[Vec<Point3>]>,
) -> io::Result<()> {
    let mut writer =
        ShapeWriter::from_path(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    if let Some(polys3) = polygons_z {
        for poly in polys3 {
            if poly.len() < 3 {
                continue;
            }
            let shp_pts: Vec<ShpPointZ> =
                poly.iter().map(|p| ShpPointZ::new(p.x, p.y, p.z, NO_DATA)).collect();
            let ring = PolygonRing::Outer(shp_pts);
            let shp_poly = PolygonZ::new(ring);
            writer
                .write_shape(&shp_poly)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    } else {
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
    }
    writer.finalize().map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
