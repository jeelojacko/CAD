use crate::geometry::Point;
#[cfg(feature = "reporting")]
use genpdf::{elements::Paragraph, Alignment, Document};
#[cfg(feature = "reporting")]
use umya_spreadsheet::{self, writer::xlsx, Spreadsheet};

#[cfg(feature = "reporting")]
fn write_pdf(path: &str, title: &str, rows: &[String]) -> std::io::Result<()> {
    // Load fonts from the crate's `assets` directory. The font files are not
    // stored in the repository; place them in `survey_cad/assets` as needed.
    let font_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    let font_family = genpdf::fonts::from_files(font_dir, "DejaVuSans", None)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut doc = Document::new(font_family);
    doc.set_title(title);
    for r in rows {
        doc.push(Paragraph::new(r).aligned(Alignment::Left));
    }
    doc.render_to_file(path)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(feature = "reporting")]
fn write_excel(path: &str, rows: &[Vec<String>]) -> std::io::Result<()> {
    let mut wb: Spreadsheet = umya_spreadsheet::new_file();
    let ws = wb.get_sheet_mut(&0).unwrap();
    for (r_idx, row) in rows.iter().enumerate() {
        for (c_idx, val) in row.iter().enumerate() {
            ws.get_cell_mut(((c_idx + 1) as u32, (r_idx + 1) as u32))
                .set_value(val);
        }
    }
    xlsx::write(&wb, path).map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(feature = "reporting")]
pub fn points_report_pdf(path: &str, points: &[Point]) -> std::io::Result<()> {
    let rows: Vec<String> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let idx = i + 1;
            format!("{idx}: {x}, {y}", x = p.x, y = p.y)
        })
        .collect();
    write_pdf(path, "Points Report", &rows)
}

#[cfg(feature = "reporting")]
pub fn points_report_excel(path: &str, points: &[Point]) -> std::io::Result<()> {
    let rows: Vec<Vec<String>> = points
        .iter()
        .enumerate()
        .map(|(i, p)| vec![
            (i + 1).to_string(),
            p.x.to_string(),
            p.y.to_string(),
        ])
        .collect();
    write_excel(path, &rows)
}
