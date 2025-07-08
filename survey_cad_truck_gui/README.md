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

## Command Line

The main window now contains a simple command line interface below the
workspace. Type commands such as `point`, `line`, `undo` or `redo` and press
Enter to execute them. Entered commands are kept in a history list.

## Scripts and Plugins

Python scripts located in the `macros/` directory can be executed from the
**Plugins Panel** available through the `Macro` menu. When a script runs the
following variables are provided:

- `survey_cad_python` &ndash; bindings to the core library.
- `points`, `lines`, `surfaces` &ndash; all entities in the current project.
- `selected_points`, `selected_lines` &ndash; the current selection.
- `view` &ndash; a dictionary with `offset` and `zoom` describing the active view.

Scripts can use these values to query or modify the project. An example script:

```python
from survey_cad_python import Point

for idx in selected_points:
    p = points[idx]
    print("Selected:", p.x, p.y)

print("Zoom:", view["zoom"])
```

## Fonts

This application bundles the `DejaVuSans.ttf` font located in the `assets/`
directory. The build script checks for this file and aborts if it is missing.
Replace it with a different font by copying the `.ttf` file into `assets/` before
running `cargo build`.
