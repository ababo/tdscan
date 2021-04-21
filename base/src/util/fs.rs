use std::fs::File;
use std::io::{stdin, stdout, Read, Write};

use crate::defs::{IntoResult, Result};

pub fn open_file_or_stdin(filename: &Option<&str>) -> Result<Box<dyn Read>> {
    let reader: Box<dyn Read> = match filename {
        Some(ref path) => Box::new(
            File::open(path).res(format!("failed to open file '{}'", path))?,
        ),
        None => Box::new(stdin()),
    };
    Ok(reader)
}

pub fn open_file_or_stdout(filename: &Option<&str>) -> Result<Box<dyn Write>> {
    let writer: Box<dyn Write> = match filename {
        Some(ref path) => Box::new(
            File::create(path)
                .res(format!("failed to create file '{}'", path))?,
        ),
        None => Box::new(stdout()),
    };
    Ok(writer)
}
