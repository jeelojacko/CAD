use thiserror::Error;
#[cfg(feature = "python")]
use pyo3::PyErr;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("UI error: {0}")]
    Slint(#[from] slint::PlatformError),
    #[cfg(feature = "python")]
    #[error("Python error: {0}")]
    Python(#[from] PyErr),
    #[error("{0}")]
    Msg(String),
}

impl From<String> for GuiError {
    fn from(msg: String) -> Self {
        GuiError::Msg(msg)
    }
}

impl From<&str> for GuiError {
    fn from(msg: &str) -> Self {
        GuiError::Msg(msg.to_string())
    }
}
