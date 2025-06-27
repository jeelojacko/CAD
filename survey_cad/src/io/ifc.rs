use std::fs::File;
use std::io::{self, Write};

use crate::geometry::Point3;

/// Writes a very small IFC file containing `IfcCartesianPoint` entities.
/// Coordinates are written in the provided EPSG reference if given.
pub fn write_ifc_points(path: &str, points: &[Point3], epsg: Option<u32>) -> io::Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "ISO-10303-21;")?;
    writeln!(file, "HEADER;")?;
    writeln!(file, "FILE_DESCRIPTION(('Survey CAD IFC export'),'2;1');")?;
    if let Some(code) = epsg {
        writeln!(
            file,
            "FILE_NAME('', '', (), (), 'EPSG:{code}', 'SurveyCAD', '');"
        )?;
    } else {
        writeln!(file, "FILE_NAME('', '', (), (), '', 'SurveyCAD', '');")?;
    }
    writeln!(file, "FILE_SCHEMA(('IFC4'));")?;
    writeln!(file, "ENDSEC;")?;
    writeln!(file, "DATA;")?;
    for (idx, p) in points.iter().enumerate() {
        writeln!(
            file,
            "#{}=IFCCARTESIANPOINT(({},{},{}));",
            idx + 1,
            p.x,
            p.y,
            p.z
        )?;
    }
    writeln!(file, "ENDSEC;")?;
    writeln!(file, "END-ISO-10303-21;")?;
    Ok(())
}
