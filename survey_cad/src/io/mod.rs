//! File input and output helpers for project data.

use std::fs::File;
use std::io::{self, Read};

/// Reads a file to string.
pub fn read_to_string(path: &str) -> io::Result<String> {
    let mut buffer = String::new();
    File::open(path)?.read_to_string(&mut buffer)?;
    Ok(buffer)
}
