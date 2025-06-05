use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn station_distance_command() {
    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args(["station-distance", "A", "0.0", "0.0", "B", "3.0", "4.0"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Distance between A and B is 5.000",
        ));
}

#[test]
fn traverse_area_command() {
    let file = assert_fs::NamedTempFile::new("traverse.csv").unwrap();
    file.write_str("0.0,0.0\n1.0,0.0\n1.0,1.0\n0.0,1.0\n")
        .unwrap();

    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args(["traverse-area", file.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Area: 1.000"));
}

#[test]
fn copy_command() {
    let dir = assert_fs::TempDir::new().unwrap();
    let src = dir.child("src.txt");
    src.write_str("hello world").unwrap();
    let dest = dir.child("dest.txt");

    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args([
            "copy",
            src.path().to_str().unwrap(),
            dest.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Copied"));

    dest.assert("hello world");
    dir.close().unwrap();
}

#[test]
fn export_geojson_command() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("pts.csv");
    input.write_str("1.0,2.0\n3.0,4.0\n").unwrap();
    let output = dir.child("pts.geojson");

    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args([
            "export-geojson",
            input.path().to_str().unwrap(),
            output.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote"));

    output.assert(predicate::path::exists());
    dir.close().unwrap();
}

#[test]
fn import_geojson_command() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("pts.geojson");
    input
        .write_str(
            r#"{ "type": "FeatureCollection", "features": [
            {"type": "Feature", "geometry": {"type": "Point", "coordinates": [1.0,2.0]}},
            {"type": "Feature", "geometry": {"type": "Point", "coordinates": [3.0,4.0]}}
        ] }"#,
        )
        .unwrap();
    let output = dir.child("pts.csv");

    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args([
            "import-geojson",
            input.path().to_str().unwrap(),
            output.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote"));

    output.assert(predicate::path::exists());
    dir.close().unwrap();
}

#[test]
fn export_dxf_command() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("pts.csv");
    input.write_str("0.0,0.0\n1.0,1.0\n").unwrap();
    let output = dir.child("pts.dxf");

    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args([
            "export-dxf",
            input.path().to_str().unwrap(),
            output.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote"));

    output.assert(predicate::path::exists());
    dir.close().unwrap();
}

#[test]
fn import_points_command() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("pts.txt");
    input.write_str("1,100.0,200.0,50.0,TEST\n").unwrap();
    let output = dir.child("out.csv");

    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args([
            "import-points",
            "pnezd",
            input.path().to_str().unwrap(),
            output.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote"));

    output.assert(predicate::path::exists());
    dir.close().unwrap();
}

#[test]
fn view_points_command() {
    let file = assert_fs::NamedTempFile::new("pts.csv").unwrap();
    file.write_str("0.0,0.0\n1.0,1.0\n").unwrap();

    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .env("SURVEY_CAD_TEST", "1")
        .args(["view-points", file.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Rendering 2 points"));
}

#[test]
fn vertical_angle_command() {
    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args([
            "vertical-angle",
            "A",
            "0.0",
            "0.0",
            "10.0",
            "B",
            "3.0",
            "4.0",
            "14.0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Vertical angle between A and B is 0.675 rad",
        ));
}

#[test]
fn level_elevation_command() {
    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args(["level-elevation", "100.0", "1.2", "0.8"])
        .assert()
        .success()
        .stdout(predicate::str::contains("New elevation: 100.400"));
}

#[test]
fn bearing_command() {
    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args(["bearing", "0.0", "0.0", "1.0", "1.0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bearing:"));
}

#[test]
fn forward_command() {
    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args(["forward", "0.0", "0.0", "1.57079632679", "2.0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Point:"));
}

#[test]
fn intersection_command() {
    Command::cargo_bin("survey_cad_cli")
        .unwrap()
        .args([
            "intersection",
            "0.0",
            "0.0",
            "1.0",
            "1.0",
            "0.0",
            "1.0",
            "1.0",
            "0.0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Intersection:"));
}
