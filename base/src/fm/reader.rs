use std::io;
use std::io::Read as _;

use flate2::read::GzDecoder;

use crate::defs::{Error, ErrorKind::*, IntoResult, Result};
use crate::fm::{Compression, RawRecord, Record, MAGIC, VERSION};

pub trait Read {
    fn read_raw_record<'a>(&'a mut self) -> Result<Option<RawRecord<'a>>>;
    fn read_record(&mut self) -> Result<Option<Record>>;
}

#[derive(Debug)]
enum RawReader<R: io::Read> {
    Plain(R),
    Gzip(GzDecoder<R>),
}

impl<R: io::Read> io::Read for RawReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            RawReader::Plain(inner) => inner.read(buf),
            RawReader::Gzip(decoder) => decoder.read(buf),
        }
    }
}

#[derive(Debug)]
pub struct Reader<R: io::Read> {
    reader: RawReader<R>,
    buffer: Vec<u8>,
}

impl<R: io::Read> Reader<R> {
    pub fn new(mut inner: R) -> Result<Self> {
        let mut buf = [0; 4];

        inner
            .read_exact(&mut buf)
            .into_result(|| format!("failed to read .fm magic"))?;
        let val = u32::from_le_bytes(buf);
        if val != MAGIC {
            return Err(Error::new(
                MalformedData,
                format!("bad .fm magic '{:#X}'", val),
            ));
        }

        inner
            .read_exact(&mut buf)
            .into_result(|| format!("failed to read .fm version"))?;
        let val = u32::from_le_bytes(buf);
        if val != VERSION {
            return Err(Error::new(
                UnsupportedFeature,
                format!("unsupported .fm version '{}'", val),
            ));
        }

        inner
            .read_exact(&mut buf)
            .into_result(|| format!("failed to read .fm compression"))?;
        let val = i32::from_le_bytes(buf);

        const COMPRESSION_NONE: i32 = Compression::None as i32;
        const COMPRESSION_GZIP: i32 = Compression::Gzip as i32;

        let reader = match val {
            COMPRESSION_NONE => Ok(RawReader::Plain(inner)),
            COMPRESSION_GZIP => Ok(RawReader::Gzip(GzDecoder::new(inner))),
            _ => Err(Error::new(
                UnsupportedFeature,
                format!("unsupported compression '{}'", val),
            )),
        }?;

        Ok(Self {
            reader,
            buffer: Vec::<u8>::with_capacity(0),
        })
    }
}

impl<R: io::Read> Read for Reader<R> {
    fn read_raw_record<'a>(&'a mut self) -> Result<Option<RawRecord<'a>>> {
        let mut buf = [0; 4];
        match self.reader.read_exact(&mut buf) {
            Err(e) => {
                return if e.kind() == io::ErrorKind::UnexpectedEof {
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
            .into_result(|| format!("failed to read .fm record"))?;

        Ok(Some(RawRecord(&self.buffer)))
    }

    fn read_record(&mut self) -> Result<Option<Record>> {
        Ok(if let Some(rec) = self.read_raw_record()? {
            Some(rec.decode()?)
        } else {
            None
        })
    }
}
