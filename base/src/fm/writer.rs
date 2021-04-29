use std::io::Write;

use prost::Message;

use crate::defs::{IntoResult, Result};
use crate::fm::{Compression, WriteParams, MAGIC, VERSION};
use crate::model::Record;

pub struct Writer {
    writer: Box<dyn Write>,
    buffer: Vec<u8>,
}

impl Writer {
    pub fn from_writer<W: Write + 'static>(
        mut writer: W,
        params: &WriteParams,
    ) -> Result<Self> {
        writer
            .write_all(&MAGIC.to_le_bytes())
            .res(|| format!("failed to write .fm magic"))?;
        writer
            .write_all(&VERSION.to_le_bytes())
            .res(|| format!("failed to write .fm version"))?;
        writer
            .write_all(&(params.compression as i32).to_le_bytes())
            .res(|| format!("failed to write .fm compression"))?;

        let enc_writer: Box<dyn Write>;

        match params.compression {
            Compression::None => {
                enc_writer = Box::new(writer);
            }
            Compression::Gzip => {
                let compression = flate2::Compression::new(params.gzip_level);
                let encoder =
                    flate2::write::GzEncoder::new(writer, compression);
                enc_writer = Box::new(encoder);
            }
        }

        Ok(Self {
            writer: enc_writer,
            buffer: Vec::<u8>::with_capacity(0),
        })
    }

    pub fn write_record(&mut self, record: Record) -> Result<()> {
        let size = record.encoded_len();
        self.writer
            .write_all(&(size as u32).to_le_bytes())
            .res(|| format!("failed to write .fm record size"))?;

        self.buffer.resize(0, 0);
        self.buffer.reserve(size);
        record.encode(&mut self.buffer).unwrap();

        self.writer
            .write_all(&self.buffer)
            .res(|| format!("failed to write .fm record"))
    }
}
