pub type Time = i64; // Monotonic time with nanosecond precision.

mod data {
    include!(concat!(env!("OUT_DIR"), "/base.fm.data.rs"));
}

mod reader;
mod writer;

use std::result::Result as StdResult;
use std::str::FromStr;

use structopt::StructOpt;

use crate::defs::{Error, ErrorKind::*, Result};
pub use data::*;
pub use reader::*;
pub use writer::*;

pub const MAGIC: u32 = 0xD0932177;
pub const VERSION: u32 = 1;

#[derive(Clone, Copy)]
pub enum Compression {
    None = 0,
    Gzip = 1,
}

impl FromStr for Compression {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "none" => Ok(Compression::None),
            "gzip" => Ok(Compression::Gzip),
            _ => Err(Error::new(
                MalformedData,
                format!("unknown .fm compression (can be 'none' or 'gzip')"),
            )),
        }
    }
}

pub const DEFAULT_COMPRESSION: &'static str = "gzip";
pub const DEFAULT_GZIP_LEVEL: &'static str = "6";

fn validate_gzip_level(value: String) -> StdResult<(), String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| "must be a positive integer".to_string())?;
    if parsed > 9 {
        return Err(format!("unsupported gzip level (can be from 0 to 9"));
    }
    Ok(())
}

#[derive(StructOpt)]
pub struct WriterParams {
    #[structopt(
        name = "fm-compression",
        help = "Type of compression for output .fm file",
        default_value = DEFAULT_COMPRESSION,
        long
    )]
    pub compression: Compression,
    #[structopt(
        name = "fm-gzip-level",
        help = "Level of gzip-compression for output .fm file",
        default_value = DEFAULT_GZIP_LEVEL,
        long,
        validator = validate_gzip_level
    )]
    pub gzip_level: u32,
}

impl Default for WriterParams {
    fn default() -> Self {
        Self {
            compression: Compression::from_str(DEFAULT_COMPRESSION).unwrap(),
            gzip_level: DEFAULT_GZIP_LEVEL.parse::<u32>().unwrap(),
        }
    }
}
