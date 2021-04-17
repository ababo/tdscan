use std::fs::File;
use std::io::{BufRead, BufReader, Lines};

use crate::defs::Result;

pub fn read_lines<P>(filename: P) -> Result<Lines<BufReader<File>>>
where
    P: AsRef<std::path::Path>,
{
    let file = File::open(filename)?;
    Ok(BufReader::new(file).lines())
}
