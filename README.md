# Survey CAD

Prototype structure for a surveying-specific CAD application written in Rust.

This repository is organized as a Cargo workspace with a core library and a CLI
binary.

The library exposes basic geometry types (points and lines) along with more
advanced primitives like arcs, polylines and polygonal surfaces. Surveying
utilities cover traverse area calculations as well as vertical angle and
differential leveling helpers.

## Architecture Overview

The workspace contains two crates:

- `survey_cad` &mdash; core library with modules for geometry, surveying, file I/O and simple rendering utilities.
- `survey_cad_cli` &mdash; small command line tool that demonstrates the library capabilities.

Each module in the library is focused on a specific set of tasks and can be used
independently within other Rust projects.

## CLI Tutorial

Build the workspace and view available commands:

```bash
$ cargo run -p survey_cad_cli -- --help
```

Compute the distance between two stations:

```bash
$ cargo run -p survey_cad_cli -- station-distance A 0.0 0.0 B 3.0 4.0
```

Calculate the area of a traverse defined in a CSV file:

```bash
$ cargo run -p survey_cad_cli -- traverse-area points.csv
```

Copy a file using the CLI:

```bash
$ cargo run -p survey_cad_cli -- copy src.txt dest.txt
```

Render a single point (opens a window):

```bash
$ cargo run -p survey_cad_cli -- render-point 1.0 2.0
```

Export survey points to GeoJSON:

```bash
$ cargo run -p survey_cad_cli -- export-geojson points.csv points.geojson
```

View points from a CSV file:

```bash
$ cargo run -p survey_cad_cli -- view-points points.csv
```

Compute the vertical angle between two stations:

```bash
$ cargo run -p survey_cad_cli -- vertical-angle A 0.0 0.0 10.0 B 3.0 4.0 14.0
```

Calculate a new elevation using differential leveling:

```bash
$ cargo run -p survey_cad_cli -- level-elevation 100.0 1.2 0.8
```

## Continuous Integration

GitHub Actions automatically runs `cargo clippy` and `cargo test` for every push
and pull request. The workflow fails if clippy reports warnings or any tests
fail.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
