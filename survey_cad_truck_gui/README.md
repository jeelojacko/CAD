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
**Plugins Panel** available through the `Macro` menu. See the next section for
the variables that are provided to a script and a small example.

### Available Variables

Python files must be placed inside the `macros/` directory located at the
workspace root. When executed from **Macro → Plugins Panel** each script
receives the following globals:

- `survey_cad_python` – bindings for geometry helpers.
- `points` – list of all points as `survey_cad_python.Point` objects.
- `lines` – tuples representing each line segment.
- `surfaces` – dictionaries with `vertices` and `triangles` for every surface.
- `selected_points` – indices of the currently selected points.
- `selected_lines` – selected line segments.
- `view` – dictionary with the current `offset` and `zoom`.

Errors raised by the script are shown in the application's status bar. A
successful run reports *"Python script finished"* in the same location.

#### Example – adding a point

Create `macros/add_point.py` with the following contents:

```python
from survey_cad_python import Point

points.append(Point(100.0, 50.0))
print("Total points:", len(points))
```

Running this file from the Plugins Panel appends a new point to the project and
prints the updated count in the console.

## Fonts

This application bundles the `DejaVuSans.ttf` font located in the `assets/`
directory. The font can be replaced in three different ways:

1. **Command Line** – pass `--font-path /path/to/font.ttf` when launching the
   application to use a specific font. This also updates the saved
   configuration.
2. **Configuration** – `config.json` now contains a `font_path` entry that can
   be edited manually.
3. **User Interface** – the *Workspace Settings* dialog includes a drop down
   listing available `.ttf` files from the `assets/` directory.

The build script accepts the `SURVEY_CAD_FONT` environment variable to embed a
different default font at compile time. Without it the bundled `DejaVuSans.ttf`
is used.
