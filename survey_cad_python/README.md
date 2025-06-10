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
