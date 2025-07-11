use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("UI error: {0}")]
    Slint(#[from] slint::PlatformError),
    #[error("Python error: {0}")]
    Python(#[from] pyo3::PyErr),
    #[error("{0}")]
    Msg(String),
}
