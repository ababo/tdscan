use std::io::{stdin, stdout};
use std::path::PathBuf;

use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::util::fs;

#[derive(StructOpt)]
#[structopt(about = "Build element view from scan .fm file")]
pub struct BuildViewParams {
    #[structopt(help = "Input scan .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,
    #[structopt(
        help = "Output element view .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,
    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn build_view_with_params(params: &BuildViewParams) -> Result<()> {
    let mut reader = if let Some(path) = &params.in_path {
        let reader = fm::Reader::new(fs::open_file(path)?)?;
        Box::new(reader) as Box<dyn fm::Read>
    } else {
        let reader = fm::Reader::new(stdin())?;
        Box::new(reader) as Box<dyn fm::Read>
    };

    let mut writer = if let Some(path) = &params.out_path {
        let writer =
            fm::Writer::new(fs::create_file(path)?, &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    } else {
        let writer = fm::Writer::new(stdout(), &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    };

    build_view(reader.as_mut(), writer.as_mut())
}

pub fn build_view(
    _reader: &mut dyn fm::Read,
    _writer: &mut dyn fm::Write,
) -> Result<()> {
    Ok(())
}
