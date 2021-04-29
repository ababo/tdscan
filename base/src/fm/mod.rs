mod reader;
mod writer;

pub use reader::*;
pub use writer::*;

use std::str::FromStr;

use structopt::StructOpt;

pub const MAGIC: u32 = 0xD0932177;
pub const VERSION: u32 = 1;

#[derive(Clone, Copy)]
pub enum Compression {
    None = 0,
    Gzip = 1,
}

impl FromStr for Compression {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "none" => Ok(Compression::None),
            "gzip" => Ok(Compression::Gzip),
            _ => Err("can be 'none' or 'gzip'"),
        }
    }
}

pub const DEFAULT_COMPRESSION: &'static str = "gzip";
pub const DEFAULT_GZIP_LEVEL: &'static str = "6";

fn validate_gzip_level(value: String) -> std::result::Result<(), String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| "must be a positive integer".to_string())?;
    if parsed > 9 {
        return Err("can be from 0 to 9".to_string());
    }
    Ok(())
}

#[derive(StructOpt)]
pub struct WriteParams {
    #[structopt(
        name = "fm-compression",
        default_value = DEFAULT_COMPRESSION,
        long
    )]
    pub compression: Compression,
    #[structopt(
        name = "fm-gzip-level",
        default_value = DEFAULT_GZIP_LEVEL,
        long,
        validator = validate_gzip_level
    )]
    pub gzip_level: u32,
}

impl Default for WriteParams {
    fn default() -> Self {
        Self {
            compression: Compression::from_str(DEFAULT_COMPRESSION).unwrap(),
            gzip_level: DEFAULT_GZIP_LEVEL.parse::<u32>().unwrap(),
        }
    }
}
