use std::fmt::Write as _;
use std::io;

use roxmltree::Document;

use crate::alignment::HorizontalAlignment;
use crate::dtm::Tin;
use crate::geometry::{Point, Point3};

use super::{read_to_string, write_string};

/// Reads a LandXML file containing a surface and returns it as a [`Tin`].
pub fn read_landxml_surface(path: &str) -> io::Result<Tin> {
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
    Ok(Tin {
        vertices,
        triangles,
    })
}

/// Writes a [`Tin`] to a LandXML surface file.
pub fn write_landxml_surface(path: &str, tin: &Tin) -> io::Result<()> {
    let mut xml = String::new();
    writeln!(&mut xml, "<?xml version=\"1.0\"?>")?;
    writeln!(&mut xml, "<LandXML>")?;
    writeln!(&mut xml, "  <Surfaces>")?;
    writeln!(&mut xml, "    <Surface name=\"TIN\">")?;
    writeln!(&mut xml, "      <Definition surfType=\"TIN\">")?;
    writeln!(&mut xml, "        <Pnts>")?;
    for (i, v) in tin.vertices.iter().enumerate() {
        writeln!(
            &mut xml,
            "          <P id=\"{}\">{} {} {}</P>",
            i + 1,
            v.x,
            v.y,
            v.z
        )?;
    }
    writeln!(&mut xml, "        </Pnts>")?;
    writeln!(&mut xml, "        <Faces>")?;
    for t in &tin.triangles {
        writeln!(
            &mut xml,
            "          <F>{} {} {}</F>",
            t[0] + 1,
            t[1] + 1,
            t[2] + 1
        )?;
    }
    writeln!(&mut xml, "        </Faces>")?;
    writeln!(&mut xml, "      </Definition>")?;
    writeln!(&mut xml, "    </Surface>")?;
    writeln!(&mut xml, "  </Surfaces>")?;
    writeln!(&mut xml, "</LandXML>")?;
    write_string(path, &xml)
}

/// Reads a LandXML file containing an alignment represented by `<PntList2D>`.
pub fn read_landxml_alignment(path: &str) -> io::Result<HorizontalAlignment> {
    let xml = read_to_string(path)?;
    let doc = Document::parse(&xml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut vertices = Vec::new();
    if let Some(list) = doc.descendants().find(|n| n.has_tag_name("PntList2D")) {
        if let Some(text) = list.text() {
            let nums: Vec<f64> = text
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            for chunk in nums.chunks(2) {
                if chunk.len() == 2 {
                    vertices.push(Point::new(chunk[0], chunk[1]));
                }
            }
        }
    }
    Ok(HorizontalAlignment::new(vertices))
}

/// Writes a [`HorizontalAlignment`] to a simple LandXML file using `<PntList2D>`.
pub fn write_landxml_alignment(path: &str, alignment: &HorizontalAlignment) -> io::Result<()> {
    let mut xml = String::new();
    writeln!(&mut xml, "<?xml version=\"1.0\"?>")?;
    writeln!(&mut xml, "<LandXML>")?;
    writeln!(&mut xml, "  <Alignments>")?;
    writeln!(&mut xml, "    <Alignment name=\"HAL\">")?;
    writeln!(&mut xml, "      <CoordGeom>")?;
    write!(&mut xml, "        <PntList2D>")?;
    for (i, p) in alignment.centerline.vertices.iter().enumerate() {
        if i > 0 {
            write!(&mut xml, " ")?;
        }
        write!(&mut xml, "{} {}", p.x, p.y)?;
    }
    writeln!(&mut xml, "</PntList2D>")?;
    writeln!(&mut xml, "      </CoordGeom>")?;
    writeln!(&mut xml, "    </Alignment>")?;
    writeln!(&mut xml, "  </Alignments>")?;
    writeln!(&mut xml, "</LandXML>")?;
    write_string(path, &xml)
}
