use std::io;
use std::path::Path;

use structopt::StructOpt;

use base::define_raw_output;
use base::defs::Result;
use base::fm;
use base::util::cli;
use base::util::fs;

define_raw_output!(ObjOutput, "obj");

#[derive(StructOpt)]
#[structopt(about = "Export .fm file into OBJ")]
pub struct ExportToObjCommand {
    #[structopt(flatten)]
    input: cli::FmInput,

    #[structopt(flatten)]
    output: ObjOutput,
}

impl ExportToObjCommand {
    pub fn run(&self) -> Result<()> {
        let mut reader = self.input.get()?;
        let mut writer = self.output.get()?;

        let mtl_dir = self
            .output
            .path
            .as_deref()
            .unwrap_or_else(|| ".".as_ref())
            .parent()
            .unwrap_or_else(|| ".".as_ref());

        export_to_obj(
            reader.as_mut(),
            &mut writer,
            |p, d| fs::write_file(p, d),
            mtl_dir,
        )
    }
}

pub fn export_to_obj<F: Fn(&Path, &[u8]) -> Result<()>>(
    _reader: &mut dyn fm::Read,
    _writer: &mut dyn io::Write,
    _write_file: F,
    _mtl_dir: &Path,
) -> Result<()> {
    Ok(())
}
