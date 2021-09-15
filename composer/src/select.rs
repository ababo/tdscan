use std::path::PathBuf;

use structopt::StructOpt;

use crate::misc::{fm_reader_from_file_or_stdin, fm_writer_to_file_or_stdout};
use crate::misc::{lua_err_to_err, lua_table_from_record};
use base::defs::Result;
use base::fm;
use base::util::fs;

#[derive(StructOpt)]
#[structopt(about = "Select records from .fm file")]
pub struct SelectParams {
    #[structopt(help = "Input .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,

    #[structopt(
        help = "Lua predicate expression",
        long,
        short = "p",
        conflicts_with = "predicate-path",
        required_unless = "predicate-path"
    )]
    predicate: Option<String>,

    #[structopt(
        help = "Input .lua file with predicate expression",
        long,
        short = "f"
    )]
    predicate_path: Option<PathBuf>,

    #[structopt(
        help = "Speed up by skipping record decoding",
        long,
        short = "n"
    )]
    no_rec_decoding: bool,

    #[structopt(
        help = concat!("Maximum length for string, array ",
            "or object (the rest of elements to be truncated)"),
        long,
        short = "t",
        conflicts_with = "no-rec-decoding",
    )]
    truncate_len: Option<usize>,

    #[structopt(
        help = "Output .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,

    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn select_with_params(params: &SelectParams) -> Result<()> {
    let mut reader = fm_reader_from_file_or_stdin(&params.in_path)?;

    let predicate = if let Some(path) = &params.predicate_path {
        fs::read_file_to_string(path)?
    } else {
        params.predicate.as_ref().unwrap().to_string()
    };

    let mut writer =
        fm_writer_to_file_or_stdout(&params.out_path, &params.fm_write_params)?;

    select(
        reader.as_mut(),
        &predicate,
        params.no_rec_decoding,
        params.truncate_len,
        writer.as_mut(),
    )
}

pub fn select(
    reader: &mut dyn fm::Read,
    predicate: &str,
    no_rec_decoding: bool,
    truncate_len: Option<usize>,
    writer: &mut dyn fm::Write,
) -> Result<()> {
    let lua = rlua::Lua::new();
    let mut num = 1;

    loop {
        match reader.read_raw_record()? {
            Some(raw) => {
                let rec = if no_rec_decoding {
                    None
                } else {
                    Some(raw.decode()?)
                };

                let res = lua
                    .context(|ctx| {
                        ctx.globals().set("n", num)?;
                        if let Some(rec) = rec {
                            let tbl =
                                lua_table_from_record(ctx, &rec, truncate_len)?;
                            ctx.globals().set("r", tbl)?;
                        }
                        ctx.load(predicate).eval()
                    })
                    .map_err(lua_err_to_err)?;

                if res {
                    writer.write_raw_record(&raw)?;
                }
            }
            None => break,
        }
        num += 1;
    }

    Ok(())
}
