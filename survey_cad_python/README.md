# survey_cad_python

Python bindings for the Survey CAD library built with [PyO3](https://pyo3.rs/).

## Building

Install [maturin](https://github.com/PyO3/maturin) and build the module in-place:

```bash
$ cd survey_cad_python
$ maturin develop
```

This compiles the `survey_cad_python` extension so it can be imported from Python.

## Example

```python
from survey_cad_python import Point, station_distance

a = Point(0.0, 0.0)
b = Point(3.0, 4.0)
print(station_distance(a, b))
```

## Using in GUI Plugins

The `survey_cad_truck_gui` application exposes this module to Python scripts
loaded from the `macros/` folder. Scripts can access geometry through the
`Point` class and call functions like `station_distance` while operating on the
entities and view parameters provided by the GUI.
