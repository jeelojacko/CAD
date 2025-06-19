# survey_cad_truck_gui

This crate contains an alternative Slint based GUI that uses the Truck CAD
engine for rendering.

## Editing the UI

The user interface is defined in the files inside [`ui/`](ui/). The main
entry point is `main.slint` which imports additional modules such as
`workspace.slint` and `dialogs.slint`.

To modify the UI simply edit these `.slint` files. The Rust bindings generated
from them are rebuilt automatically when running `cargo build`:

```bash
# from the workspace root
cargo build -p survey_cad_truck_gui
```

Rebuilding ensures that any changes in the `.slint` files are reflected in the
Rust code via the generated bindings.

## Fonts

This application bundles the `DejaVuSans.ttf` font located in the `assets/`
directory. The build script checks for this file and aborts if it is missing.
Replace it with a different font by copying the `.ttf` file into `assets/` before
running `cargo build`.
