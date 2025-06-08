use pyo3::prelude::*;
use survey_cad::geometry::Point as CadPoint;
use survey_cad::surveying::{station_distance as cad_station_distance, Station};

#[pyclass]
#[derive(Clone)]
struct Point {
    inner: CadPoint,
}

#[pymethods]
impl Point {
    #[new]
    fn new(x: f64, y: f64) -> Self {
        Self { inner: CadPoint::new(x, y) }
    }

    #[getter]
    fn x(&self) -> f64 {
        self.inner.x
    }

    #[setter]
    fn set_x(&mut self, value: f64) {
        self.inner.x = value;
    }

    #[getter]
    fn y(&self) -> f64 {
        self.inner.y
    }

    #[setter]
    fn set_y(&mut self, value: f64) {
        self.inner.y = value;
    }
}

#[pyfunction]
fn station_distance(a: &Point, b: &Point) -> f64 {
    let sa = Station::new("a", a.inner);
    let sb = Station::new("b", b.inner);
    cad_station_distance(&sa, &sb)
}

#[pymodule]
fn survey_cad_python(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<Point>()?;
    m.add_function(wrap_pyfunction!(station_distance, m)?)?;
    Ok(())
}
