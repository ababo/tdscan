use std::collections::HashMap;
use std::io::stdout;
use std::path::PathBuf;
use std::str::FromStr;

use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::util::cli::parse_key_val;
use base::util::fs;

#[derive(Default, Clone, Copy)]
pub struct Displacement {
    #[allow(dead_code)]
    dx: f32,
    #[allow(dead_code)]
    dy: f32,
    #[allow(dead_code)]
    dz: f32,
}

impl FromStr for Displacement {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let malformed_err = || {
            let desc = format!("malformed displacement '{}'", s);
            Error::new(MalformedData, desc)
        };

        let parse = |iter: &mut std::str::Split<&str>| {
            let part = iter.next().ok_or_else(|| malformed_err())?;
            if part.is_empty() {
                Ok(0.0)
            } else {
                part.parse::<f32>().or_else(|_| Err(malformed_err()))
            }
        };

        let mut iter = s.split(",");
        let dx = parse(&mut iter)?;
        let dy = parse(&mut iter)?;
        let dz = parse(&mut iter)?;

        if iter.next().is_some() {
            return Err(malformed_err());
        }

        Ok(Displacement { dx, dy, dz })
    }
}

#[derive(StructOpt)]
#[structopt(about = "Combine multiple .fm files")]
pub struct CombineParams {
    #[structopt(help = "Input .fm files")]
    in_paths: Vec<PathBuf>,
    #[structopt(help="Element displacement in form 'element=dx,dy,dz'",
            long,
            parse(try_from_str = parse_key_val),
            short = "d"
        )]
    displacements: Vec<(String, Displacement)>,
    #[structopt(
        help = "Output .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,
    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn combine_with_params(params: &CombineParams) -> Result<()> {
    let mut reader_boxes = Vec::<Box<dyn fm::Read>>::new();
    for path in &params.in_paths {
        let file = fs::open_file(path)?;
        reader_boxes.push(Box::new(fm::Reader::new(file)?));
    }

    let mut readers: Vec<&mut dyn fm::Read> = Vec::new();
    for reader in &mut reader_boxes {
        readers.push(reader.as_mut());
    }

    let displacements = params.displacements.iter().cloned().collect();

    let mut writer = if let Some(path) = &params.out_path {
        let writer =
            fm::Writer::new(fs::create_file(path)?, &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    } else {
        let writer = fm::Writer::new(stdout(), &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    };

    combine(&readers, displacements, writer.as_mut())
}

pub fn combine(
    _readers: &[&mut dyn fm::Read],
    _displacements: HashMap<String, Displacement>,
    _writer: &mut dyn fm::Write,
) -> Result<()> {
    Ok(())
}
