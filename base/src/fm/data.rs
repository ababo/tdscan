include!(concat!(env!("OUT_DIR"), "/base.fm.data.rs"));

use crate::defs::{Error, ErrorKind::*, Result};
use std::str::FromStr;

impl FromStr for scan_frame::DepthConfidence {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(Error::new(
                MalformedData,
                format!(concat!(
                    "unknown scan depth confidence ",
                    "(can be 'low', 'medium' or 'high')"
                )),
            )),
        }
    }
}
