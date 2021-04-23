use std::io::{Read, Write};
use std::str::FromStr;
use structopt::StructOpt;

use brotli;
use prost::Message;

use crate::defs::{Error, ErrorKind::*, IntoResult, Result};
use crate::model::Model;

const MAGIC: u32 = 0x01C3ADF8;
const VERSION: u32 = 1;

const DEFAULT_COMPRESSION: &'static str = "brotli";
const DEFAULT_BROTLI_QUALITY: &'static str = "6";

#[derive(Clone, Copy)]
pub enum Compression {
    None = 0,
    Brotli = 1,
}

impl FromStr for Compression {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "none" => Ok(Compression::None),
            "brotli" => Ok(Compression::Brotli),
            _ => Err("unsupported compression"),
        }
    }
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
        name = "fm-brotli-quality",
        default_value = DEFAULT_BROTLI_QUALITY,
        long
    )]
    pub brotli_quality: u32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            compression: Compression::from_str(DEFAULT_COMPRESSION).unwrap(),
            brotli_quality: DEFAULT_BROTLI_QUALITY.parse::<u32>().unwrap(),
        }
    }
}

pub fn encode<W: Write>(
    model: &Model,
    params: &Params,
    mut writer: W,
) -> Result<()> {
    let _ = writer
        .write(&MAGIC.to_le_bytes())
        .res("failed to write .fm magic".to_string())?;
    let _ = writer
        .write(&VERSION.to_le_bytes())
        .res("failed to write .fm version".to_string())?;
    let _ = writer
        .write(&(params.compression as i32).to_le_bytes())
        .res("failed to write .fm compression".to_string())?;

    let mut buf = Vec::with_capacity(model.encoded_len());
    model.encode(&mut buf).unwrap();

    match params.compression {
        Compression::None => {
            let _ = writer
                .write(&buf)
                .res("failed to write .fm model".to_string())?;
        }
        Compression::Brotli => {
            let mut writer = brotli::CompressorWriter::new(
                writer,
                0,
                params.brotli_quality,
                0,
            );
            let _ = writer.write(&buf).res(
                "failed to write Brotli-compressed .fm model".to_string(),
            )?;
        }
    }

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
        .res("failed to read .fm magic".to_string())?;
    let val = u32::from_le_bytes(buf);
    if val != VERSION {
        return Err(Error::new(
            FeatureNotSupported,
            format!("unsupported .fm version '{}'", val),
        ));
    }

    reader
        .read(&mut buf)
        .res("failed to read .fm magic".to_string())?;
    let val = i32::from_le_bytes(buf);

    const COMPRESSION_NONE: i32 = Compression::None as i32;
    const COMPRESSION_BROTLI: i32 = Compression::Brotli as i32;
    let mut buf = vec![];

    match val {
        COMPRESSION_NONE => {
            reader
                .read_to_end(&mut buf)
                .res("failed to read .fm model".to_string())?;
        }
        COMPRESSION_BROTLI => {}
        _ => {
            let mut reader = brotli::Decompressor::new(reader, 0);
            reader.read_to_end(&mut buf).res(
                "failed to read Brotli-compressed .fm model".to_string(),
            )?;
        }
    }

    let model = Model::decode(buf.as_slice())
        .res("failed to decode .fm model".to_string())?;

    Ok(model)
}
