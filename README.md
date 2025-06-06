# Survey CAD

Prototype structure for a surveying-specific CAD application written in Rust.

This repository is organized as a Cargo workspace with a core library and a CLI
binary.

The library exposes basic geometry types (points and lines) along with more
advanced primitives like arcs, polylines and polygonal surfaces. Surveying
utilities cover traverse area calculations as well as vertical angle and
differential leveling helpers.

Supported file formats include CSV, GeoJSON, simple DXF and LandXML. Optional
features provide shapefile and LAS point cloud readers to ease interoperability
with other CAD and GIS tools. Basic DWG interoperability is available through
the `dwg2dxf` and `dxf2dwg` command line tools from the LibreDWG project. The
library converts DWG files to DXF and back using these utilities when present,
returning an error if they are missing.

## Architecture Overview

The workspace contains two crates:

- `survey_cad` &mdash; core library with modules for geometry, surveying, file I/O and simple rendering utilities.
- `survey_cad_cli` &mdash; small command line tool that demonstrates the library capabilities.

Each module in the library is focused on a specific set of tasks and can be used
independently within other Rust projects. Heavy rendering dependencies are
optional and enabled with the `render` feature.

The command line tool depends on these rendering crates by default. To build a
lightweight binary without them, disable default features:

```bash
$ cargo run -p survey_cad_cli --no-default-features -- <command>
```
Enable rendering explicitly with `--features render` when needed.

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

Render a single point using Bevy (opens a window):

```bash
$ cargo run -p survey_cad_cli -- render-point 1.0 2.0
```

Run the parametric box example (requires the `pmetra` feature):

```bash
$ cargo run -p survey_cad --example parametric_box --features pmetra
```

Export survey points to GeoJSON:

```bash
$ cargo run -p survey_cad_cli -- export-geojson points.csv points.geojson
```

View points from a CSV file using Bevy:

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

Compute cut/fill volume along an alignment:

```bash
$ cargo run -p survey_cad_cli -- corridor-volume design.csv ground.csv halign.csv valign.csv 10.0 --interval 10.0 --offset-step 1.0
```

## Continuous Integration

GitHub Actions automatically runs `cargo clippy` and `cargo test` for every push
and pull request. The workflow fails if clippy reports warnings or any tests
fail.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
