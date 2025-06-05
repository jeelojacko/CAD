use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn station_distance_command() {
    Command::cargo_bin("survey_cad_cli").unwrap()
        .args(["station-distance", "A", "0.0", "0.0", "B", "3.0", "4.0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Distance between A and B is 5.000"));
}

#[test]
fn traverse_area_command() {
    let file = assert_fs::NamedTempFile::new("traverse.csv").unwrap();
    file.write_str("0.0,0.0\n1.0,0.0\n1.0,1.0\n0.0,1.0\n").unwrap();

    Command::cargo_bin("survey_cad_cli").unwrap()
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

    Command::cargo_bin("survey_cad_cli").unwrap()
        .args(["copy", src.path().to_str().unwrap(), dest.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Copied"));

    dest.assert("hello world");
    dir.close().unwrap();
}
