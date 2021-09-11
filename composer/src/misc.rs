use std::collections::HashMap;
use std::io::{stdin, stdout};
use std::path::PathBuf;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::util::fs;

pub fn fm_reader_from_file_or_stdin(
    path: &Option<PathBuf>,
) -> Result<Box<dyn fm::Read>> {
    if let Some(path) = path {
        let reader = fm::Reader::new(fs::open_file(path)?)?;
        Ok(Box::new(reader) as Box<dyn fm::Read>)
    } else {
        let reader = fm::Reader::new(stdin())?;
        Ok(Box::new(reader) as Box<dyn fm::Read>)
    }
}

pub fn fm_writer_to_file_or_stdout(
    path: &Option<PathBuf>,
    params: &fm::WriterParams,
) -> Result<Box<dyn fm::Write>> {
    if let Some(path) = path {
        let writer = fm::Writer::new(fs::create_file(path)?, params)?;
        Ok(Box::new(writer) as Box<dyn fm::Write>)
    } else {
        let writer = fm::Writer::new(stdout(), params)?;
        Ok(Box::new(writer) as Box<dyn fm::Write>)
    }
}

pub fn read_scans(
    reader: &mut dyn fm::Read,
) -> Result<(HashMap<String, fm::Scan>, Vec<fm::ScanFrame>)> {
    let mut scans = HashMap::<String, fm::Scan>::new();
    let mut scan_frames = Vec::<fm::ScanFrame>::new();
    let mut last_time = 0;

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
                if f.time < last_time {
                    let desc = format!(
                        "non-monotonic frame time for scan '{}'",
                        &f.scan
                    );
                    return Err(Error::new(InconsistentState, desc));
                }
                last_time = f.time;
                scan_frames.push(f);
            }
            _ => (),
        }
    }

    Ok((scans, scan_frames))
}
