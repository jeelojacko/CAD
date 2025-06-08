use survey_cad::{
    alignment::{Alignment, HorizontalAlignment, VerticalAlignment},
    corridor::{Subassembly, extract_design_cross_sections},
    sheet::write_cross_section_sheet_svg,
    geometry::Point,
};
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn write_sheet_svg() {
    let hal = HorizontalAlignment::new(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    let val = VerticalAlignment::new(vec![(0.0, 0.0), (10.0, 0.0)]);
    let align = Alignment::new(hal, val);
    let subs = vec![Subassembly::new(vec![(-1.0, 0.0), (1.0, 0.0)])];
    let sections = extract_design_cross_sections(&align, &subs, None, 10.0);
    let dir = assert_fs::TempDir::new().unwrap();
    let file = dir.child("sheet.svg");
    write_cross_section_sheet_svg(
        file.path().to_str().unwrap(),
        &align,
        &sections,
        40.0,
        10.0,
        10.0,
        5.0,
    )
    .unwrap();
    file.assert(predicate::path::exists());
    dir.close().unwrap();
}
