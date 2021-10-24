use std::fs::{read, read_to_string, File};
use std::path::Path;

use crate::defs::{IntoResult, Result};

pub fn open_file<P: AsRef<Path>>(path: P) -> Result<File> {
    let path = path.as_ref();
    File::open(path).into_result(|| {
        if let Some(path) = path.to_str() {
            format!("failed to open file '{}'", path)
        } else {
            "failed to open file".to_string()
        }
    })
}

pub fn create_file<P: AsRef<Path>>(path: P) -> Result<File> {
    let path = path.as_ref();
    File::create(path).into_result(|| {
        if let Some(path) = path.to_str() {
            format!("failed to create file '{}'", path)
        } else {
            "failed to create file".to_string()
        }
    })
}

pub fn read_file<P: AsRef<Path>>(path: P) -> Result<Vec<u8>> {
    let path = path.as_ref();
    read(path).into_result(|| {
        if let Some(path) = path.to_str() {
            format!("failed to read file '{}'", path)
        } else {
            "failed to read file".to_string()
        }
    })
}

pub fn read_file_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    read_to_string(path).into_result(|| {
        if let Some(path) = path.to_str() {
            format!("failed to read file '{}' into string", path)
        } else {
            "failed to read file into string".to_string()
        }
    })
}
