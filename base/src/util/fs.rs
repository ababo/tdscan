use std::fs::{read, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::defs::{IntoResult, Result};

pub fn open_file<P: AsRef<Path>>(path: P) -> Result<Box<dyn Read>> {
    let path = path.as_ref();
    let reader: Box<dyn Read> = Box::new(File::open(path).res(
        if let Some(filename) = path.to_str() {
            format!("failed to open file '{}'", filename)
        } else {
            format!("failed to open file")
        },
    )?);
    Ok(reader)
}

pub fn create_file<P: AsRef<Path>>(path: P) -> Result<Box<dyn Write>> {
    let path = path.as_ref();
    let writer: Box<dyn Write> = Box::new(File::create(path).res(
        if let Some(filename) = path.to_str() {
            format!("failed to create file '{}'", filename)
        } else {
            format!("failed to create file")
        },
    )?);
    Ok(writer)
}

pub fn read_file<P: AsRef<Path>>(path: P) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let data = read(path).res(if let Some(filename) = path.to_str() {
        format!("failed to read file '{}'", filename)
    } else {
        format!("failed to read file")
    })?;
    Ok(data)
}
