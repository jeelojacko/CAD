use survey_cad::styles::format_dms;

#[test]
fn dms_basic() {
    assert_eq!(format_dms(123.7516667), "123\u{00B0}45'6\"");
}

#[test]
fn dms_negative() {
    assert_eq!(format_dms(-0.0166667), "-0\u{00B0}1'0\"");
}
