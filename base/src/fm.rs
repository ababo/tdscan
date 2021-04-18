use std::io::{Read, Write};

use brotli;
use prost::Message;

use crate::defs::Result;
use std::str::FromStr;

use crate::model::Model;

const VERSION: u32 = 1;
const MAGIC: u32 = 0x01C3ADF8;

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

pub struct Params {
    pub compression: Compression,
    pub brotli_quality: u32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            compression: Compression::Brotli,
            brotli_quality: 11,
        }
    }
}

pub fn encode<W: Write>(
    model: &Model,
    params: &Params,
    writer: &mut W,
) -> Result<()> {
    let _ = writer.write(&MAGIC.to_le_bytes())?;
    let _ = writer.write(&VERSION.to_le_bytes())?;
    let _ = writer.write(&(params.compression as i32).to_le_bytes())?;

    let mut buf = Vec::with_capacity(model.encoded_len());
    model.encode(&mut buf).unwrap();

    match params.compression {
        Compression::None => {
            let _ = writer.write(&buf)?;
        }
        Compression::Brotli => {
            let mut bwriter = brotli::CompressorWriter::new(
                writer,
                0,
                params.brotli_quality,
                0,
            );
            let _ = bwriter.write(&buf)?;
        }
    }

    Ok(())
}

pub fn decode<R: Read>(_reader: &mut R) -> Result<Model> {
    Ok(Model {
        ..Default::default()
    })
}
