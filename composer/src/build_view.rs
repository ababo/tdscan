use std::collections::HashMap;
use std::io::{stdin, stdout};
use std::path::PathBuf;

use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::fm::scan_frame::DepthConfidence;
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
    #[structopt(
        help = "Minimum depth confidence",
        long,
        default_value = "high"
    )]
    min_depth_confidence: DepthConfidence,
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

    build_view(
        reader.as_mut(),
        &params.min_depth_confidence,
        writer.as_mut(),
    )
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    min_depth_confidence: &DepthConfidence,
    _writer: &mut dyn fm::Write,
) -> Result<()> {
    let mut scans = HashMap::<String, fm::Scan>::new();
    let mut scan_frames = Vec::<fm::ScanFrame>::new();

    loop {
        let rec = reader.read_record()?;
        if rec.is_none() {
            break;
        }

        use fm::record::Type::*;
        match rec.unwrap().r#type {
            Some(Scan(s)) => {
                if !scan_frames.is_empty() {
                    let desc = format!("scan '{}' after scan frame ", &s.name);
                    return Err(Error::new(InconsistentState, desc));
                }
                scans.insert(s.name.clone(), s);
            }
            Some(ScanFrame(f)) => {
                if !scans.contains_key(&f.scan) {
                    let desc = format!("frame for unknown scan '{}'", &f.scan);
                    return Err(Error::new(InconsistentState, desc));
                }
                scan_frames.push(f);
            }
            _ => (),
        }
    }

    let _points = build_point_cloud(&scans, &scan_frames, min_depth_confidence);

    Ok(())
}

fn build_point_cloud(
    _scans: &HashMap<String, fm::Scan>,
    _scan_frames: &Vec<fm::ScanFrame>,
    _min_depth_confidence: &DepthConfidence,
) -> Vec<fm::Point3> {
    vec![]
}
