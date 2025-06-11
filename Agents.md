+# Agent Guide for CAD Workspace
+
+This repository contains a Rust workspace with several crates:
+
+- `survey_cad` – core library and the bulk of the code
+- `survey_cad_cli` – command line interface demonstrating library features
+- `survey_cad_gui` – optional GUI
+- `bevy_pmetra`, `cad_import`, `pipe_network` – supporting crates
+- `survey_cad_python` – Python bindings built with `maturin`
+
+## Development workflow
+
+1. Format Rust sources with `cargo fmt`.
+2. Run `cargo clippy --all-targets --all-features -- -D warnings`.
+3. Run `cargo test --all`.
+
+The test suite uses optional features like GDAL. Install dependencies with:
+
+```bash
+sudo apt-get update && sudo apt-get install -y libgdal-dev
+```
+
+Python bindings can be built by running `maturin develop` inside
+`survey_cad_python/`.
+
+## Commit guidelines
+
+- Use short imperative summaries ("Add new traverse tool").
+- Optionally include a longer description after a blank line.
+- Do not commit files inside `target/` or other build artifacts.
+
+## Pull request guidelines
+
+Include a concise summary of your changes and reference any
+relevant issues.
