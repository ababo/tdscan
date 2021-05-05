use std::io::{ErrorKind::UnexpectedEof, Read};

use flate2;
use prost::Message;

use crate::defs::{Error, ErrorKind::*, IntoResult, Result};
use crate::fm::{Compression, MAGIC, VERSION};
use crate::model::Record;

pub struct Reader {
    reader: Box<dyn Read>,
    buffer: Vec<u8>,
}

impl Reader {
    pub fn from_reader<R: Read + 'static>(mut reader: R) -> Result<Self> {
        let mut buf = [0; 4];

        reader
            .read_exact(&mut buf)
            .res(|| format!("failed to read .fm magic"))?;
        let val = u32::from_le_bytes(buf);
        if val != MAGIC {
            return Err(Error::new(
                MalformedData,
                format!("bad .fm magic '{:#X}'", val),
            ));
        }

        reader
            .read_exact(&mut buf)
            .res(|| format!("failed to read .fm version"))?;
        let val = u32::from_le_bytes(buf);
        if val != VERSION {
            return Err(Error::new(
                UnsupportedFeature,
                format!("unsupported .fm version '{}'", val),
            ));
        }

        reader
            .read_exact(&mut buf)
            .res(|| format!("failed to read .fm compression"))?;
        let val = i32::from_le_bytes(buf);

        const COMPRESSION_NONE: i32 = Compression::None as i32;
        const COMPRESSION_GZIP: i32 = Compression::Gzip as i32;

        let dec_reader: Box<dyn Read>;

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

        Ok(Self {
            reader: dec_reader,
            buffer: Vec::<u8>::with_capacity(0),
        })
    }

    pub fn read_record(&mut self) -> Result<Option<Record>> {
        let mut buf = [0; 4];
        match self.reader.read_exact(&mut buf) {
            Err(e) => {
                return if e.kind() == UnexpectedEof {
                    Ok(None)
                } else {
                    Err(Error::with_source(
                        MalformedData,
                        format!("failed to read .fm record size"),
                        e,
                    ))
                }
            }
            Ok(()) => (),
        }

        let size = u32::from_le_bytes(buf) as usize;
        self.buffer.resize(size, 0);

        self.reader
            .read_exact(&mut self.buffer)
            .res(|| format!("failed to read .fm record"))?;

        let rec = Record::decode(self.buffer.as_slice())
            .res(|| format!("failed to decode .fm record"))?;

        Ok(Some(rec))
    }
}
