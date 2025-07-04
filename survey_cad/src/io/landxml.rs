use std::fmt::Write as _;
use std::io;

use roxmltree::Document;

use crate::alignment::{HorizontalAlignment, HorizontalElement};
use crate::corridor::CrossSection;
use crate::dtm::Tin;
use crate::geometry::{Arc, Point, Point3};
use crate::superelevation::SuperelevationPoint;

use super::{read_to_string, write_string};

/// Additional attributes found in LandXML files.
#[derive(Debug, Default, Clone)]
pub struct LandxmlExtras {
    pub units: Option<String>,
    pub style: Option<String>,
    pub description: Option<String>,
}

/// Reads a LandXML file containing a surface and returns it and any extra metadata.
pub fn read_landxml_surface(path: &str) -> io::Result<(Tin, LandxmlExtras)> {
    let xml = read_to_string(path)?;
    let doc = Document::parse(&xml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut vertices = Vec::new();
    if let Some(pnts) = doc.descendants().find(|n| n.has_tag_name("Pnts")) {
        for p in pnts.children().filter(|c| c.has_tag_name("P")) {
            if let Some(text) = p.text() {
                let nums: Vec<f64> = text
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if nums.len() >= 3 {
                    vertices.push(Point3::new(nums[0], nums[1], nums[2]));
                }
            }
        }
    }
    let mut triangles = Vec::new();
    if let Some(faces) = doc.descendants().find(|n| n.has_tag_name("Faces")) {
        for f in faces.children().filter(|c| c.has_tag_name("F")) {
            if let Some(text) = f.text() {
                let nums: Vec<usize> = text
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if nums.len() >= 3 {
                    triangles.push([nums[0] - 1, nums[1] - 1, nums[2] - 1]);
                }
            }
        }
    }
    let extras = LandxmlExtras {
        units: doc
            .descendants()
            .find(|n| n.has_tag_name("Units"))
            .and_then(|n| n.attribute("linearUnit"))
            .map(|s| s.to_string()),
        style: doc
            .descendants()
            .find(|n| n.has_tag_name("Surface"))
            .and_then(|n| n.attribute("style"))
            .map(|s| s.to_string()),
        description: doc
            .descendants()
            .find(|n| n.has_tag_name("Surface"))
            .and_then(|n| n.attribute("desc"))
            .map(|s| s.to_string()),
    };
    Ok((
        Tin {
            vertices,
            triangles,
        },
        extras,
    ))
}

/// Writes a [`Tin`] to a LandXML surface file.
pub fn write_landxml_surface(path: &str, tin: &Tin, extras: Option<&LandxmlExtras>) -> io::Result<()> {
    let mut xml = String::new();
    writeln!(&mut xml, "<?xml version=\"1.0\"?>").unwrap();
    writeln!(&mut xml, "<LandXML>").unwrap();
    if let Some(ex) = extras {
        if let Some(u) = &ex.units {
            writeln!(&mut xml, "  <Units linearUnit=\"{}\"/>", u).unwrap();
        }
    }
    writeln!(&mut xml, "  <Surfaces>").unwrap();
    let style_attr = extras
        .and_then(|e| e.style.as_deref())
        .map(|s| format!(" style=\"{}\"", s))
        .unwrap_or_default();
    let desc_attr = extras
        .and_then(|e| e.description.as_deref())
        .map(|s| format!(" desc=\"{}\"", s))
        .unwrap_or_default();
    writeln!(&mut xml, "    <Surface name=\"TIN\"{}{}>", style_attr, desc_attr).unwrap();
    writeln!(&mut xml, "      <Definition surfType=\"TIN\">").unwrap();
    writeln!(&mut xml, "        <Pnts>").unwrap();
    for (i, v) in tin.vertices.iter().enumerate() {
        writeln!(
            &mut xml,
            "          <P id=\"{}\">{} {} {}</P>",
            i + 1,
            v.x,
            v.y,
            v.z
        )
        .unwrap();
    }
    writeln!(&mut xml, "        </Pnts>").unwrap();
    writeln!(&mut xml, "        <Faces>").unwrap();
    for t in &tin.triangles {
        writeln!(
            &mut xml,
            "          <F>{} {} {}</F>",
            t[0] + 1,
            t[1] + 1,
            t[2] + 1
        )
        .unwrap();
    }
    writeln!(&mut xml, "        </Faces>").unwrap();
    writeln!(&mut xml, "      </Definition>").unwrap();
    writeln!(&mut xml, "    </Surface>").unwrap();
    writeln!(&mut xml, "  </Surfaces>").unwrap();
    writeln!(&mut xml, "</LandXML>").unwrap();
    write_string(path, &xml)
}

/// Reads a LandXML file containing an alignment represented by `<PntList2D>`.
pub fn read_landxml_alignment(path: &str) -> io::Result<(HorizontalAlignment, LandxmlExtras)> {
    let xml = read_to_string(path)?;
    let doc = Document::parse(&xml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut elements = Vec::new();
    if let Some(coord) = doc.descendants().find(|n| n.has_tag_name("CoordGeom")) {
        for child in coord.children().filter(|c| c.is_element()) {
            match child.tag_name().name() {
                "PntList2D" => {
                    if let Some(text) = child.text() {
                        let nums: Vec<f64> = text
                            .split_whitespace()
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        for pair in nums.chunks(2).collect::<Vec<_>>().windows(2) {
                            if let ([a, b], [c, d]) = (pair[0], pair[1]) {
                                elements.push(HorizontalElement::Tangent {
                                    start: Point::new(*a, *b),
                                    end: Point::new(*c, *d),
                                });
                            }
                        }
                    }
                }
                "Line" => {
                    let mut start = None;
                    let mut end = None;
                    for n in child.children().filter(|c| c.is_element()) {
                        match n.tag_name().name() {
                            "Start" => {
                                if let Some(t) = n.text() {
                                    let vals: Vec<f64> = t
                                        .split_whitespace()
                                        .filter_map(|s| s.parse().ok())
                                        .collect();
                                    if vals.len() >= 2 {
                                        start = Some(Point::new(vals[0], vals[1]));
                                    }
                                }
                            }
                            "End" => {
                                if let Some(t) = n.text() {
                                    let vals: Vec<f64> = t
                                        .split_whitespace()
                                        .filter_map(|s| s.parse().ok())
                                        .collect();
                                    if vals.len() >= 2 {
                                        end = Some(Point::new(vals[0], vals[1]));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if let (Some(s), Some(e)) = (start, end) {
                        elements.push(HorizontalElement::Tangent { start: s, end: e });
                    }
                }
                "Curve" => {
                    let mut start = None;
                    let mut end = None;
                    let mut center = None;
                    let mut radius = None;
                    for attr in child.attributes() {
                        if attr.name() == "radius" {
                            radius = attr.value().parse().ok();
                        }
                    }
                    for n in child.children().filter(|c| c.is_element()) {
                        match n.tag_name().name() {
                            "Start" => {
                                if let Some(t) = n.text() {
                                    let vals: Vec<f64> = t
                                        .split_whitespace()
                                        .filter_map(|s| s.parse().ok())
                                        .collect();
                                    if vals.len() >= 2 {
                                        start = Some(Point::new(vals[0], vals[1]));
                                    }
                                }
                            }
                            "End" => {
                                if let Some(t) = n.text() {
                                    let vals: Vec<f64> = t
                                        .split_whitespace()
                                        .filter_map(|s| s.parse().ok())
                                        .collect();
                                    if vals.len() >= 2 {
                                        end = Some(Point::new(vals[0], vals[1]));
                                    }
                                }
                            }
                            "Center" => {
                                if let Some(t) = n.text() {
                                    let vals: Vec<f64> = t
                                        .split_whitespace()
                                        .filter_map(|s| s.parse().ok())
                                        .collect();
                                    if vals.len() >= 2 {
                                        center = Some(Point::new(vals[0], vals[1]));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if let (Some(c), Some(s), Some(e), Some(r)) = (center, start, end, radius) {
                        let sa = (s.y - c.y).atan2(s.x - c.x);
                        let ea = (e.y - c.y).atan2(e.x - c.x);
                        let arc = Arc::new(c, r, sa, ea);
                        elements.push(HorizontalElement::Curve { arc });
                    }
                }
                "Spiral" => {
                    let mut start = None;
                    let mut end = None;
                    for n in child.children().filter(|c| c.is_element()) {
                        match n.tag_name().name() {
                            "Start" => {
                                if let Some(t) = n.text() {
                                    let vals: Vec<f64> = t
                                        .split_whitespace()
                                        .filter_map(|s| s.parse().ok())
                                        .collect();
                                    if vals.len() >= 2 {
                                        start = Some(Point::new(vals[0], vals[1]));
                                    }
                                }
                            }
                            "End" => {
                                if let Some(t) = n.text() {
                                    let vals: Vec<f64> = t
                                        .split_whitespace()
                                        .filter_map(|s| s.parse().ok())
                                        .collect();
                                    if vals.len() >= 2 {
                                        end = Some(Point::new(vals[0], vals[1]));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if let (Some(s), Some(e)) = (start, end) {
                        let len = crate::geometry::distance(s, e);
                        let ori = (e.y - s.y).atan2(e.x - s.x);
                        let spiral = crate::alignment::Spiral {
                            start: s,
                            orientation: ori,
                            length: len,
                            start_radius: f64::INFINITY,
                            end_radius: f64::INFINITY,
                        };
                        elements.push(HorizontalElement::Spiral { spiral });
                    }
                }
                _ => {}
            }
        }
    }
    if elements.is_empty() {
        // fallback to legacy <PntList2D> only structure
        if let Some(list) = doc.descendants().find(|n| n.has_tag_name("PntList2D")) {
            if let Some(text) = list.text() {
                let nums: Vec<f64> = text
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                for pair in nums.chunks(2).collect::<Vec<_>>().windows(2) {
                    if let ([a, b], [c, d]) = (pair[0], pair[1]) {
                        elements.push(HorizontalElement::Tangent {
                            start: Point::new(*a, *b),
                            end: Point::new(*c, *d),
                        });
                    }
                }
            }
        }
    }
    let extras = LandxmlExtras {
        units: doc
            .descendants()
            .find(|n| n.has_tag_name("Units"))
            .and_then(|n| n.attribute("linearUnit"))
            .map(|s| s.to_string()),
        style: doc
            .descendants()
            .find(|n| n.has_tag_name("Alignment"))
            .and_then(|n| n.attribute("style"))
            .map(|s| s.to_string()),
        description: doc
            .descendants()
            .find(|n| n.has_tag_name("Alignment"))
            .and_then(|n| n.attribute("desc"))
            .map(|s| s.to_string()),
    };
    Ok((HorizontalAlignment { elements }, extras))
}

/// Writes a [`HorizontalAlignment`] to a simple LandXML file using `<PntList2D>`.
pub fn write_landxml_alignment(
    path: &str,
    alignment: &HorizontalAlignment,
    extras: Option<&LandxmlExtras>,
) -> io::Result<()> {
    let mut xml = String::new();
    writeln!(&mut xml, "<?xml version=\"1.0\"?>").unwrap();
    writeln!(&mut xml, "<LandXML>").unwrap();
    if let Some(ex) = extras {
        if let Some(u) = &ex.units {
            writeln!(&mut xml, "  <Units linearUnit=\"{}\"/>", u).unwrap();
        }
    }
    writeln!(&mut xml, "  <Alignments>").unwrap();
    let style_attr = extras
        .and_then(|e| e.style.as_deref())
        .map(|s| format!(" style=\"{}\"", s))
        .unwrap_or_default();
    let desc_attr = extras
        .and_then(|e| e.description.as_deref())
        .map(|s| format!(" desc=\"{}\"", s))
        .unwrap_or_default();
    writeln!(&mut xml, "    <Alignment name=\"HAL\"{}{}>", style_attr, desc_attr).unwrap();
    writeln!(&mut xml, "      <CoordGeom>").unwrap();
    for elem in &alignment.elements {
        match elem {
            HorizontalElement::Tangent { start, end } => {
                writeln!(&mut xml, "        <Line>").unwrap();
                writeln!(&mut xml, "          <Start>{} {}</Start>", start.x, start.y).unwrap();
                writeln!(&mut xml, "          <End>{} {}</End>", end.x, end.y).unwrap();
                writeln!(&mut xml, "        </Line>").unwrap();
            }
            HorizontalElement::Curve { arc } => {
                writeln!(&mut xml, "        <Curve radius=\"{}\">", arc.radius).unwrap();
                let sp = Point::new(
                    arc.center.x + arc.radius * arc.start_angle.cos(),
                    arc.center.y + arc.radius * arc.start_angle.sin(),
                );
                let ep = Point::new(
                    arc.center.x + arc.radius * arc.end_angle.cos(),
                    arc.center.y + arc.radius * arc.end_angle.sin(),
                );
                writeln!(&mut xml, "          <Start>{} {}</Start>", sp.x, sp.y).unwrap();
                writeln!(&mut xml, "          <End>{} {}</End>", ep.x, ep.y).unwrap();
                writeln!(
                    &mut xml,
                    "          <Center>{} {}</Center>",
                    arc.center.x, arc.center.y
                )
                .unwrap();
                writeln!(&mut xml, "        </Curve>").unwrap();
            }
            HorizontalElement::Spiral { spiral } => {
                let s = spiral.start_point();
                let e = spiral.end_point();
                writeln!(&mut xml, "        <Spiral>").unwrap();
                writeln!(&mut xml, "          <Start>{} {}</Start>", s.x, s.y).unwrap();
                writeln!(&mut xml, "          <End>{} {}</End>", e.x, e.y).unwrap();
                writeln!(&mut xml, "        </Spiral>").unwrap();
            }
        }
    }
    writeln!(&mut xml, "      </CoordGeom>").unwrap();
    writeln!(&mut xml, "    </Alignment>").unwrap();
    writeln!(&mut xml, "  </Alignments>").unwrap();
    writeln!(&mut xml, "</LandXML>").unwrap();
    write_string(path, &xml)
}

/// Reads a LandXML file containing a vertical profile.
pub fn read_landxml_profile(path: &str) -> io::Result<crate::alignment::VerticalAlignment> {
    let xml = read_to_string(path)?;
    let doc = Document::parse(&xml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut elements = Vec::new();
    if let Some(profile) = doc.descendants().find(|n| n.has_tag_name("Profile")) {
        for child in profile.children().filter(|c| c.is_element()) {
            match child.tag_name().name() {
                "Grade" => {
                    let mut ss = None;
                    let mut es = None;
                    let mut se = None;
                    let mut ee = None;
                    for a in child.attributes() {
                        match a.name() {
                            "startSta" | "startStation" => ss = a.value().parse().ok(),
                            "endSta" | "endStation" => es = a.value().parse().ok(),
                            "startElev" => se = a.value().parse().ok(),
                            "endElev" => ee = a.value().parse().ok(),
                            _ => {}
                        }
                    }
                    if let (Some(ss), Some(es), Some(se), Some(ee)) = (ss, es, se, ee) {
                        elements.push(crate::alignment::VerticalElement::Grade {
                            start_station: ss,
                            end_station: es,
                            start_elev: se,
                            end_elev: ee,
                        });
                    }
                }
                "Parabola" | "Curve" => {
                    let mut ss = None;
                    let mut es = None;
                    let mut se = None;
                    let mut sg = None;
                    let mut eg = None;
                    for a in child.attributes() {
                        match a.name() {
                            "startSta" | "startStation" => ss = a.value().parse().ok(),
                            "endSta" | "endStation" => es = a.value().parse().ok(),
                            "startElev" => se = a.value().parse().ok(),
                            "startGrade" => sg = a.value().parse().ok(),
                            "endGrade" => eg = a.value().parse().ok(),
                            _ => {}
                        }
                    }
                    if let (Some(ss), Some(es), Some(se), Some(sg), Some(eg)) = (ss, es, se, sg, eg)
                    {
                        elements.push(crate::alignment::VerticalElement::Parabola {
                            start_station: ss,
                            end_station: es,
                            start_elev: se,
                            start_grade: sg,
                            end_grade: eg,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    Ok(crate::alignment::VerticalAlignment { elements })
}

/// Writes a [`VerticalAlignment`] to a LandXML file.
pub fn write_landxml_profile(
    path: &str,
    profile: &crate::alignment::VerticalAlignment,
) -> io::Result<()> {
    let mut xml = String::new();
    writeln!(&mut xml, "<?xml version=\"1.0\"?>").unwrap();
    writeln!(&mut xml, "<LandXML>").unwrap();
    writeln!(&mut xml, "  <Alignments>").unwrap();
    writeln!(&mut xml, "    <Alignment name=\"VAL\">").unwrap();
    writeln!(&mut xml, "      <Profile>").unwrap();
    for elem in &profile.elements {
        match elem {
            crate::alignment::VerticalElement::Grade {
                start_station,
                end_station,
                start_elev,
                end_elev,
            } => {
                writeln!(
                    &mut xml,
                    "        <Grade startSta=\"{start_station}\" endSta=\"{end_station}\" startElev=\"{start_elev}\" endElev=\"{end_elev}\"/>"
                )
                .unwrap();
            }
            crate::alignment::VerticalElement::Parabola {
                start_station,
                end_station,
                start_elev,
                start_grade,
                end_grade,
            } => {
                writeln!(
                    &mut xml,
                    "        <Parabola startSta=\"{start_station}\" endSta=\"{end_station}\" startElev=\"{start_elev}\" startGrade=\"{start_grade}\" endGrade=\"{end_grade}\"/>"
                )
                .unwrap();
            }
        }
    }
    writeln!(&mut xml, "      </Profile>").unwrap();
    writeln!(&mut xml, "    </Alignment>").unwrap();
    writeln!(&mut xml, "  </Alignments>").unwrap();
    writeln!(&mut xml, "</LandXML>").unwrap();
    write_string(path, &xml)
}

/// Reads a LandXML file containing corridor cross sections.
pub fn read_landxml_cross_sections(path: &str) -> io::Result<(Vec<CrossSection>, LandxmlExtras)> {
    let xml = read_to_string(path)?;
    let doc = Document::parse(&xml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut sections = Vec::new();
    for cs in doc.descendants().filter(|n| n.has_tag_name("CrossSection")) {
        let station = cs
            .attribute("sta")
            .or_else(|| cs.attribute("station"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        if let Some(list) = cs.children().find(|n| n.has_tag_name("PntList3D")) {
            if let Some(text) = list.text() {
                let nums: Vec<f64> = text
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                let mut pts = Vec::new();
                for chunk in nums.chunks(3) {
                    if chunk.len() == 3 {
                        pts.push(Point3::new(chunk[0], chunk[1], chunk[2]));
                    }
                }
                sections.push(CrossSection::new(station, pts));
            }
        }
    }
    let extras = LandxmlExtras {
        units: doc
            .descendants()
            .find(|n| n.has_tag_name("Units"))
            .and_then(|n| n.attribute("linearUnit"))
            .map(|s| s.to_string()),
        style: None,
        description: None,
    };
    Ok((sections, extras))
}

/// Writes corridor cross sections to a LandXML file.
pub fn write_landxml_cross_sections(
    path: &str,
    sections: &[CrossSection],
    extras: Option<&LandxmlExtras>,
) -> io::Result<()> {
    let mut xml = String::new();
    writeln!(&mut xml, "<?xml version=\"1.0\"?>").unwrap();
    writeln!(&mut xml, "<LandXML>").unwrap();
    if let Some(ex) = extras {
        if let Some(u) = &ex.units {
            writeln!(&mut xml, "  <Units linearUnit=\"{}\"/>", u).unwrap();
        }
    }
    writeln!(&mut xml, "  <CrossSections>").unwrap();
    for sec in sections {
        writeln!(&mut xml, "    <CrossSection sta=\"{}\">", sec.station).unwrap();
        let coords: Vec<String> = sec
            .points
            .iter()
            .map(|p| format!("{} {} {}", p.x, p.y, p.z))
            .collect();
        writeln!(
            &mut xml,
            "      <PntList3D>{}</PntList3D>",
            coords.join(" ")
        )
        .unwrap();
        writeln!(&mut xml, "    </CrossSection>").unwrap();
    }
    writeln!(&mut xml, "  </CrossSections>").unwrap();
    writeln!(&mut xml, "</LandXML>").unwrap();
    write_string(path, &xml)
}

/// Reads a LandXML superelevation table.
pub fn read_landxml_superelevation(path: &str) -> io::Result<Vec<SuperelevationPoint>> {
    let xml = read_to_string(path)?;
    let doc = Document::parse(&xml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut table = Vec::new();
    for sp in doc
        .descendants()
        .filter(|n| n.has_tag_name("SuperelevationPoint"))
    {
        let station = sp
            .attribute("sta")
            .or_else(|| sp.attribute("station"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let left = sp
            .attribute("left")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let right = sp
            .attribute("right")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        table.push(SuperelevationPoint {
            station,
            left_slope: left,
            right_slope: right,
        });
    }
    Ok(table)
}

/// Writes a superelevation table to a LandXML file.
pub fn write_landxml_superelevation(path: &str, table: &[SuperelevationPoint]) -> io::Result<()> {
    let mut xml = String::new();
    writeln!(&mut xml, "<?xml version=\"1.0\"?>").unwrap();
    writeln!(&mut xml, "<LandXML>").unwrap();
    writeln!(&mut xml, "  <Superelevation>").unwrap();
    for pt in table {
        writeln!(
            &mut xml,
            "    <SuperelevationPoint sta=\"{}\" left=\"{}\" right=\"{}\"/>",
            pt.station, pt.left_slope, pt.right_slope
        )
        .unwrap();
    }
    writeln!(&mut xml, "  </Superelevation>").unwrap();
    writeln!(&mut xml, "</LandXML>").unwrap();
    write_string(path, &xml)
}
