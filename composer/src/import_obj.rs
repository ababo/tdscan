use clap::Clap;

use base::defs::Result;
use base::model;
use base::util;

#[derive(Clap)]
#[clap(about = "Import data from Wavefront .obj file")]
pub struct ImportObjParams {
    in_filename: String,
    out_filename: Option<String>,
}

pub fn import_obj(params: &ImportObjParams) -> Result<()> {
    for line in util::read_lines(&params.in_filename)? {
        if let Ok(ip) = line {
            println!("{}", ip);
        }
    }

    Ok(())
}
