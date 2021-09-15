use std::io;
use std::io::Write as _;
use std::result;

use flate2::write::GzEncoder;
use prost::Message;

use crate::defs::{Error, IntoResult, Result};
use crate::fm::{Compression, RawRecord, Record, WriterParams, MAGIC, VERSION};

pub trait Write {
    fn write_raw_record<'a>(&mut self, record: &RawRecord<'a>) -> Result<()>;
    fn write_record(&mut self, record: &Record) -> Result<()>;
}

#[derive(Debug)]
pub enum RawWriter<W: io::Write> {
    Plain(W),
    Gzip(GzEncoder<W>),
}

impl<W: io::Write> RawWriter<W> {
    pub fn into_inner(self) -> result::Result<W, (Self, Error)> {
        match self {
            RawWriter::Plain(inner) => Ok(inner),
            RawWriter::Gzip(mut encoder) => {
                if let Err(err) = encoder
                    .try_finish()
                    .into_result(|| format!("failed to finish encoding"))
                {
                    return Err((RawWriter::Gzip(encoder), err));
                }
                Ok(encoder.finish().unwrap())
            }
        }
    }
}

impl<W: io::Write> io::Write for RawWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            RawWriter::Plain(inner) => inner.write(buf),
            RawWriter::Gzip(encoder) => encoder.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            RawWriter::Plain(inner) => inner.flush(),
            RawWriter::Gzip(encoder) => encoder.flush(),
        }
    }
}

#[derive(Debug)]
pub struct Writer<W: io::Write> {
    writer: RawWriter<W>,
    buffer: Vec<u8>,
}

impl<W: io::Write> Writer<W> {
    pub fn new(mut inner: W, params: &WriterParams) -> Result<Self> {
        inner
            .write_all(&MAGIC.to_le_bytes())
            .into_result(|| format!("failed to write .fm magic"))?;
        inner
            .write_all(&VERSION.to_le_bytes())
            .into_result(|| format!("failed to write .fm version"))?;
        inner
            .write_all(&(params.compression as i32).to_le_bytes())
            .into_result(|| format!("failed to write .fm compression"))?;

        let writer = match params.compression {
            Compression::None => RawWriter::Plain(inner),
            Compression::Gzip => {
                let compression = flate2::Compression::new(params.gzip_level);
                RawWriter::Gzip(GzEncoder::new(inner, compression))
            }
        };

        Ok(Self {
            writer,
            buffer: Vec::<u8>::with_capacity(0),
        })
    }

    pub fn into_inner(self) -> result::Result<W, (Self, Error)> {
        match self.writer.into_inner() {
            Ok(inner) => Ok(inner),
            Err((writer, err)) => Err((
                Self {
                    writer,
                    buffer: self.buffer,
                },
                err,
            )),
        }
    }
}

impl<W: io::Write> Write for Writer<W> {
    fn write_raw_record<'a>(&mut self, record: &RawRecord<'a>) -> Result<()> {
        self.writer
            .write_all(&(record.0.len() as u32).to_le_bytes())
            .into_result(|| format!("failed to write .fm record size"))?;

        self.writer
            .write_all(record.0)
            .into_result(|| format!("failed to write .fm record"))
    }

    fn write_record(&mut self, record: &Record) -> Result<()> {
        let size = record.encoded_len();
        self.buffer.resize(0, 0);
        self.buffer.reserve(size);
        record.encode(&mut self.buffer).unwrap();

        // The borrow checker doesn't allow to reuse write_raw_record().
        self.writer
            .write_all(&(size as u32).to_le_bytes())
            .into_result(|| format!("failed to write .fm record size"))?;

        self.writer
            .write_all(&self.buffer)
            .into_result(|| format!("failed to write .fm record"))
    }
}
