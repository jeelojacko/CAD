# Survey CAD

Prototype structure for a surveying-specific CAD application written in Rust.

This repository is organized as a Cargo workspace with a core library and a CLI binary.

## CLI Examples

The `survey_cad_cli` binary provides several small commands demonstrating the
geometry, surveying and I/O utilities.

```bash
$ cargo run -p survey_cad_cli -- --help
```

### Station distance

```bash
$ cargo run -p survey_cad_cli -- station-distance A 0.0 0.0 B 3.0 4.0
```

### Traverse area from CSV

```bash
$ cargo run -p survey_cad_cli -- traverse-area points.csv
```

### Copy a file

```bash
$ cargo run -p survey_cad_cli -- copy src.txt dest.txt
```

### Render a point

```bash
$ cargo run -p survey_cad_cli -- render-point 1.0 2.0
```
