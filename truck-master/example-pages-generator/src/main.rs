#![cfg_attr(not(debug_assertions), deny(warnings))]
#![deny(clippy::all, rust_2018_idioms)]
#![warn(
    //missing_docs,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command};
use std::thread::JoinHandle;

const EXAMPLES: &[&str] = &[
    "wgsl-sandbox",
    "bsp-animation",
    "collision-sphere",
    "material-samples",
    "rotate-objects",
    "simple-obj-viewer",
    "simple-shape-viewer",
    "textured-cube",
];

fn out_threads(child: &mut Child) -> (JoinHandle<()>, JoinHandle<()>) {
    let stdout = BufReader::new(child.stdout.take().expect("no stdout"));
    let stderr = BufReader::new(child.stderr.take().expect("no stderr"));
    (
        std::thread::spawn(move || {
            stdout
                .lines()
                .map_while(|line| line.ok())
                .for_each(|line| println!("{line}"))
        }),
        std::thread::spawn(move || {
            stderr
                .lines()
                .map_while(|line| line.ok())
                .for_each(|line| println!("{line}"))
        }),
    )
}

fn main() {
    let mut child = Command::new("cargo")
        .args([
            "build",
            "--target",
            "wasm32-unknown-unknown",
            "--examples",
            "--release",
            "-p",
            "truck-platform",
            "-p",
            "truck-rendimpl",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("{}", e));
    let _threads = out_threads(&mut child);
    assert!(child.wait().unwrap_or_else(|e| panic!("{}", e)).success());
    let mut sum = String::new();
    for dir in EXAMPLES {
        let output_dir = format!("dist/{dir}");
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|e| panic!("{}", e));
        let mut child = Command::new("wasm-bindgen")
            .args([
                "--target",
                "web",
                "--out-dir",
                &output_dir,
                &format!("target/wasm32-unknown-unknown/release/examples/{dir}.wasm",),
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("{}", e));
        let _threads = out_threads(&mut child);
        assert!(child.wait().unwrap_or_else(|e| panic!("{}", e)).success());
        std::fs::write(
            format!("{output_dir}/index.html"),
            include_str!("example-index.html").replace("{example}", dir),
        )
        .unwrap_or_else(|e| panic!("{}", e));
        std::fmt::Write::write_fmt(
            &mut sum,
            format_args!("<li><a href=\"{dir}/index.html\">{dir}</a></li>"),
        )
        .unwrap_or_else(|e| panic!("{}", e));
    }
    std::fs::write(
        "dist/index.html",
        include_str!("index.html").replace("<!-- index -->", &sum),
    )
    .unwrap_or_else(|e| panic!("{}", e));
}
