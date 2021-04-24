use std::io::{copy, Read, Write};
use std::str::FromStr;
use structopt::StructOpt;

use flate2;
use prost::Message;

use crate::defs::{Error, ErrorKind::*, IntoResult, Result};
use crate::model::Model;

const MAGIC: u32 = 0x01C3ADF8;
const VERSION: u32 = 1;

const DEFAULT_COMPRESSION: &'static str = "gzip";
const DEFAULT_GZIP_LEVEL: &'static str = "6";

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

fn validate_gzip_level(value: String) -> std::result::Result<(), String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| "failed to parse GZIP compression level".to_string())?;
    if parsed > 9 {
        return Err("can be from 0 to 9".to_string());
    }
    Ok(())
}

#[derive(StructOpt)]
pub struct Params {
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

impl Default for Params {
    fn default() -> Self {
        Self {
            compression: Compression::from_str(DEFAULT_COMPRESSION).unwrap(),
            gzip_level: DEFAULT_GZIP_LEVEL.parse::<u32>().unwrap(),
        }
    }
}

pub fn encode<W: Write>(
    model: &Model,
    params: &Params,
    mut writer: W,
) -> Result<()> {
    writer
        .write(&MAGIC.to_le_bytes())
        .res("failed to write .fm magic".to_string())?;
    writer
        .write(&VERSION.to_le_bytes())
        .res("failed to write .fm version".to_string())?;
    writer
        .write(&(params.compression as i32).to_le_bytes())
        .res("failed to write .fm compression".to_string())?;

    let mut enc_writer: Box<dyn Write>;

    match params.compression {
        Compression::None => {
            enc_writer = Box::new(writer);
        }
        Compression::Gzip => {
            let compression = flate2::Compression::new(params.gzip_level);
            let encoder = flate2::write::GzEncoder::new(writer, compression);
            enc_writer = Box::new(encoder);
        }
    }

    let mut buf = Vec::with_capacity(model.encoded_len());
    model.encode(&mut buf).unwrap();

    copy(&mut buf.as_slice(), &mut enc_writer)
        .res("failed to write .fm model".to_string())?;

    Ok(())
}

pub fn decode<R: Read>(mut reader: R) -> Result<Model> {
    let mut buf = [0; 4];

    reader
        .read(&mut buf)
        .res("failed to read .fm magic".to_string())?;
    let val = u32::from_le_bytes(buf);
    if val != MAGIC {
        return Err(Error::new(
            MalformedData,
            format!("bad .fm magic '{:#X}'", val),
        ));
    }

    reader
        .read(&mut buf)
        .res("failed to read .fm version".to_string())?;
    let val = u32::from_le_bytes(buf);
    if val != VERSION {
        return Err(Error::new(
            FeatureNotSupported,
            format!("unsupported .fm version '{}'", val),
        ));
    }

    reader
        .read(&mut buf)
        .res("failed to read .fm compression".to_string())?;
    let val = i32::from_le_bytes(buf);

    const COMPRESSION_NONE: i32 = Compression::None as i32;
    const COMPRESSION_GZIP: i32 = Compression::Gzip as i32;

    let mut dec_reader: Box<dyn Read>;

    match val {
        COMPRESSION_NONE => {
            dec_reader = Box::new(reader);
        }
        COMPRESSION_GZIP => {
            dec_reader = Box::new(flate2::read::GzDecoder::new(reader));
        }
        _ => {
            return Err(Error::new(
                MalformedData,
                format!("unknown compression '{}'", val),
            ));
        }
    }

    let mut buf = vec![];
    copy(&mut dec_reader, &mut buf)
        .res("failed to read .fm model".to_string())?;

    let model = Model::decode(buf.as_slice())
        .res("failed to decode .fm model".to_string())?;

    Ok(model)
}
