use crate::geometry::{Point, Point3};
use std::collections::BTreeMap;
use shapefile::{
    Point as ShpPoint, PointZ as ShpPointZ, Polygon as ShpPolygon, PolygonRing, PolygonZ,
    Polyline as ShpPolyline, PolylineZ, Shape, ShapeReader, ShapeWriter, Reader, Writer, NO_DATA,
};
use shapefile::dbase::{FieldName, FieldValue, Record};
use shapefile::dbase::TableWriterBuilder;
use std::io;
use crate::gis::Feature;

/// Record type for a point geometry and its attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct PointRecord {
    pub geom: Point,
    pub geom_z: Option<Point3>,
    pub attrs: BTreeMap<String, FieldValue>,
}

/// Record type for a polyline geometry and its attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct PolylineRecord {
    pub geom: crate::geometry::Polyline,
    pub geom_z: Option<Vec<Point3>>, // vertices with Z if present
    pub attrs: BTreeMap<String, FieldValue>,
}

/// Record type for a polygon geometry and its attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonRecord {
    pub geom: Vec<Point>,
    pub geom_z: Option<Vec<Point3>>, // vertices with Z if present
    pub attrs: BTreeMap<String, FieldValue>,
}

/// Output type for polylines with optional Z coordinates.
pub type PolylineOutput = (Vec<crate::geometry::Polyline>, Option<Vec<Vec<Point3>>>);

/// Output type for polygons with optional Z coordinates.
pub type PolygonOutput = (Vec<Vec<Point>>, Option<Vec<Vec<Point3>>>);

fn build_table_builder(attrs: &BTreeMap<String, FieldValue>) -> TableWriterBuilder {
    use std::convert::TryFrom;
    let mut builder = TableWriterBuilder::new();
    for (name, value) in attrs {
        let field_name = FieldName::try_from(name.as_str()).unwrap_or_else(|_| FieldName::try_from("FIELD").unwrap());
        builder = match value {
            FieldValue::Character(_) | FieldValue::Memo(_) =>
                builder.add_character_field(field_name, 64),
            FieldValue::Numeric(_) => builder.add_numeric_field(field_name, 18, 5),
            FieldValue::Logical(_) => builder.add_logical_field(field_name),
            FieldValue::Integer(_) => builder.add_integer_field(field_name),
            FieldValue::Float(_) => builder.add_float_field(field_name, 18, 5),
            FieldValue::Double(_) => builder.add_double_field(field_name),
            FieldValue::Date(_) => builder.add_date_field(field_name),
            FieldValue::Currency(_) => builder.add_currency_field(field_name),
            FieldValue::DateTime(_) => builder.add_datetime_field(field_name),
        };
    }
    builder
}

fn field_value_to_string(v: &FieldValue) -> String {
    match v {
        FieldValue::Character(Some(s)) => s.clone(),
        FieldValue::Character(None) => String::new(),
        FieldValue::Numeric(Some(n)) => n.to_string(),
        FieldValue::Numeric(None) => String::new(),
        FieldValue::Logical(Some(b)) => b.to_string(),
        FieldValue::Logical(None) => String::new(),
        FieldValue::Date(Some(d)) => format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()),
        FieldValue::Date(None) => String::new(),
        FieldValue::Float(Some(f)) => f.to_string(),
        FieldValue::Float(None) => String::new(),
        FieldValue::Integer(i) => i.to_string(),
        FieldValue::Currency(c) => c.to_string(),
        FieldValue::DateTime(dt) => format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            dt.date().year(),
            dt.date().month(),
            dt.date().day(),
            dt.time().hours(),
            dt.time().minutes(),
            dt.time().seconds()
        ),
        FieldValue::Double(d) => d.to_string(),
        FieldValue::Memo(s) => s.clone(),
    }
}

pub fn point_record_to_feature(rec: PointRecord, class: Option<String>) -> Feature<Point> {
    let attrs = rec
        .attrs
        .iter()
        .map(|(k, v)| (k.clone(), field_value_to_string(v)))
        .collect();
    Feature { class, attributes: attrs, geometry: rec.geom }
}

pub fn polyline_record_to_feature(rec: PolylineRecord, class: Option<String>) -> Feature<crate::geometry::Polyline> {
    let attrs = rec
        .attrs
        .iter()
        .map(|(k, v)| (k.clone(), field_value_to_string(v)))
        .collect();
    Feature { class, attributes: attrs, geometry: rec.geom }
}

pub fn polygon_record_to_feature(rec: PolygonRecord, class: Option<String>) -> Feature<Vec<Point>> {
    let attrs = rec
        .attrs
        .iter()
        .map(|(k, v)| (k.clone(), field_value_to_string(v)))
        .collect();
    Feature { class, attributes: attrs, geometry: rec.geom }
}

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
) -> io::Result<PolylineOutput> {
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
pub fn read_polygons_shp(path: &str) -> io::Result<PolygonOutput> {
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

/// Reads Point records with attributes from a shapefile.
pub fn read_point_records_shp(path: &str) -> io::Result<Vec<PointRecord>> {
    let mut reader = Reader::from_path(path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut out = Vec::new();
    for res in reader.iter_shapes_and_records() {
        let (shape, record) = res.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let attrs: BTreeMap<_, _> = record.into_iter().collect();
        match shape {
            Shape::Point(p) => out.push(PointRecord { geom: Point::new(p.x, p.y), geom_z: None, attrs }),
            Shape::PointZ(p) => out.push(PointRecord { geom: Point::new(p.x, p.y), geom_z: Some(Point3::new(p.x, p.y, p.z)), attrs }),
            _ => {}
        }
    }
    Ok(out)
}

/// Writes Point records with attributes to a shapefile.
pub fn write_point_records_shp(path: &str, records: &[PointRecord]) -> io::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let builder = build_table_builder(&records[0].attrs);
    let mut writer = Writer::from_path(path, builder)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for rec in records {
        let mut r = Record::default();
        for (k, v) in &rec.attrs {
            r.insert(k.clone(), v.clone());
        }
        if let Some(z) = &rec.geom_z {
            let shp = ShpPointZ::new(z.x, z.y, z.z, NO_DATA);
            writer.write_shape_and_record(&shp, &r).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        } else {
            let shp = ShpPoint { x: rec.geom.x, y: rec.geom.y };
            writer.write_shape_and_record(&shp, &r).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    }
    Ok(())
}

/// Reads Polyline records with attributes from a shapefile.
pub fn read_polyline_records_shp(path: &str) -> io::Result<Vec<PolylineRecord>> {
    let mut reader = Reader::from_path(path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut out = Vec::new();
    for res in reader.iter_shapes_and_records() {
        let (shape, record) = res.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let attrs: BTreeMap<_, _> = record.into_iter().collect();
        match shape {
            Shape::Polyline(pl) => {
                for part in pl.parts() {
                    let verts: Vec<Point> = part.iter().map(|p| Point::new(p.x, p.y)).collect();
                    out.push(PolylineRecord { geom: crate::geometry::Polyline::new(verts), geom_z: None, attrs: attrs.clone() });
                }
            }
            Shape::PolylineZ(pl) => {
                for part in pl.parts() {
                    let verts2: Vec<Point> = part.iter().map(|p| Point::new(p.x, p.y)).collect();
                    let verts3: Vec<Point3> = part.iter().map(|p| Point3::new(p.x, p.y, p.z)).collect();
                    out.push(PolylineRecord { geom: crate::geometry::Polyline::new(verts2), geom_z: Some(verts3), attrs: attrs.clone() });
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Writes Polyline records with attributes to a shapefile.
pub fn write_polyline_records_shp(path: &str, records: &[PolylineRecord]) -> io::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let builder = build_table_builder(&records[0].attrs);
    let mut writer = Writer::from_path(path, builder)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for rec in records {
        let mut r = Record::default();
        for (k, v) in &rec.attrs {
            r.insert(k.clone(), v.clone());
        }
        if let Some(ref zs) = rec.geom_z {
            if zs.len() < 2 { continue; }
            let shp_pts: Vec<ShpPointZ> = zs.iter().map(|p| ShpPointZ::new(p.x, p.y, p.z, NO_DATA)).collect();
            let shp_pl = PolylineZ::new(shp_pts);
            writer.write_shape_and_record(&shp_pl, &r).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        } else {
            if rec.geom.vertices.len() < 2 { continue; }
            let shp_pts: Vec<ShpPoint> = rec.geom.vertices.iter().map(|p| ShpPoint { x: p.x, y: p.y }).collect();
            let shp_pl = ShpPolyline::new(shp_pts);
            writer.write_shape_and_record(&shp_pl, &r).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    }
    Ok(())
}

/// Reads Polygon records with attributes from a shapefile.
pub fn read_polygon_records_shp(path: &str) -> io::Result<Vec<PolygonRecord>> {
    let mut reader = Reader::from_path(path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut out = Vec::new();
    for res in reader.iter_shapes_and_records() {
        let (shape, record) = res.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let attrs: BTreeMap<_, _> = record.into_iter().collect();
        match shape {
            Shape::Polygon(pg) => {
                for ring in pg.rings() {
                    let verts: Vec<Point> = ring.points().iter().map(|p| Point::new(p.x, p.y)).collect();
                    out.push(PolygonRecord { geom: verts, geom_z: None, attrs: attrs.clone() });
                }
            }
            Shape::PolygonZ(pg) => {
                for ring in pg.rings() {
                    let verts2: Vec<Point> = ring.points().iter().map(|p| Point::new(p.x, p.y)).collect();
                    let verts3: Vec<Point3> = ring.points().iter().map(|p| Point3::new(p.x, p.y, p.z)).collect();
                    out.push(PolygonRecord { geom: verts2, geom_z: Some(verts3), attrs: attrs.clone() });
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Writes Polygon records with attributes to a shapefile.
pub fn write_polygon_records_shp(path: &str, records: &[PolygonRecord]) -> io::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let builder = build_table_builder(&records[0].attrs);
    let mut writer = Writer::from_path(path, builder)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for rec in records {
        let mut r = Record::default();
        for (k, v) in &rec.attrs {
            r.insert(k.clone(), v.clone());
        }
        if let Some(ref zs) = rec.geom_z {
            if zs.len() < 3 { continue; }
            let shp_pts: Vec<ShpPointZ> = zs.iter().map(|p| ShpPointZ::new(p.x, p.y, p.z, NO_DATA)).collect();
            let ring = PolygonRing::Outer(shp_pts);
            let shp_poly = PolygonZ::new(ring);
            writer.write_shape_and_record(&shp_poly, &r).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        } else {
            if rec.geom.len() < 3 { continue; }
            let shp_pts: Vec<ShpPoint> = rec.geom.iter().map(|p| ShpPoint { x: p.x, y: p.y }).collect();
            let ring = PolygonRing::Outer(shp_pts);
            let shp_poly = ShpPolygon::new(ring);
            writer.write_shape_and_record(&shp_poly, &r).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
    }
    Ok(())
}
