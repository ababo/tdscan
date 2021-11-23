use std::error::Error as StdError;
use std::fmt::Debug;
use std::io::{stdin, stdout};
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::str::FromStr;

use arrayvec::ArrayVec;
use structopt::StructOpt;

use crate::defs::{Error, ErrorKind::*, Result};
use crate::fm;
use crate::util::fs;

pub struct Array<T: FromStr, const N: usize>(pub [T; N]);

impl<T: Debug + Default + FromStr, const N: usize> FromStr for Array<T, N> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let malformed_err = || {
            let desc = format!("malformed value array '{}'", s);
            Error::new(MalformedData, desc)
        };

        let parse = |iter: &mut std::str::Split<char>| {
            let part = iter.next().ok_or_else(malformed_err)?;
            if part.is_empty() {
                Ok(T::default())
            } else {
                part.parse::<T>().map_err(|_| malformed_err())
            }
        };

        let mut iter = s.split(',');
        let mut vec = ArrayVec::<T, N>::new();

        for _ in 0..N {
            vec.push(parse(&mut iter)?);
        }

        if iter.next().is_some() {
            return Err(malformed_err());
        }

        Ok(Array(vec.into_inner().unwrap()))
    }
}

impl<T: FromStr, const N: usize> From<[T; N]> for Array<T, N> {
    fn from(array: [T; N]) -> Self {
        Self(array)
    }
}

pub fn parse_key_val<T, U>(s: &str) -> StdResult<(T, U), Box<dyn StdError>>
where
    T: FromStr,
    T::Err: StdError + 'static,
    U: FromStr,
    U::Err: StdError + 'static,
{
    let err_func = || format!("malformed 'key=value' pair '{}'", s);
    let pos = s.find('=').ok_or_else(err_func)?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[derive(StructOpt)]
pub struct FmInput {
    #[structopt(help = "Input .fm file (STDIN if omitted)", name="in-file")]
    pub path: Option<PathBuf>,
}

impl FmInput {
    pub fn get(&self) -> Result<Box<dyn fm::Read>> {
        if let Some(path) = &self.path {
            let reader = fm::Reader::new(fs::open_file(path)?)?;
            Ok(Box::new(reader) as Box<dyn fm::Read>)
        } else {
            let reader = fm::Reader::new(stdin())?;
            Ok(Box::new(reader) as Box<dyn fm::Read>)
        }
    }
}

#[derive(StructOpt)]
pub struct FmInputs {
    #[structopt(help = "Input .fm files (STDIN if omitted)", name="in-files")]
    pub paths: Vec<PathBuf>,
}

impl FmInputs {
    pub fn get(&self) -> Result<Vec<Box<dyn fm::Read>>> {
        let mut readers = Vec::<Box<dyn fm::Read>>::new();
        for path in &self.paths {
            let file = fs::open_file(path)?;
            readers.push(Box::new(fm::Reader::new(file)?));
        }
        if readers.is_empty() {
            let reader = fm::Reader::new(stdin())?;
            readers.push(Box::new(reader) as Box<dyn fm::Read>);
        }
        Ok(readers)
    }
}

#[derive(StructOpt)]
pub struct FmOutput {
    #[structopt(
        help = "Output .fm file (STDOUT if omitted)",
        long = "out-file",
        short = "o"
    )]
    pub path: Option<PathBuf>,

    #[structopt(flatten)]
    pub fm_params: fm::WriterParams,
}

impl FmOutput {
    pub fn get(&self) -> Result<Box<dyn fm::Write>> {
        if let Some(path) = &self.path {
            let writer =
                fm::Writer::new(fs::create_file(path)?, &self.fm_params)?;
            Ok(Box::new(writer) as Box<dyn fm::Write>)
        } else {
            let writer = fm::Writer::new(stdout(), &self.fm_params)?;
            Ok(Box::new(writer) as Box<dyn fm::Write>)
        }
    }
}

#[macro_export]
macro_rules! define_raw_input {
    ($name: ident, $ext: expr) => {
        #[derive(StructOpt)]
        pub struct $name {
            #[structopt(
                help = concat!("Output .", $ext, " file (STDOUT if omitted)"),
                name = "in-file"
            )]
            pub path: Option<PathBuf>,
        }
        impl $name {
            pub fn get(&self) -> Result<Box<dyn io::Read>> {
                Ok(if let Some(path) = &self.path {
                    Box::new(fs::open_file(path)?) as Box<dyn io::Read>
                } else {
                    Box::new(stdin()) as Box<dyn io::Read>
                })
            }
        }
    }
}

#[macro_export]
macro_rules! define_raw_output {
    ($name: ident, $ext: expr) => {
        use std::path::PathBuf;
        use std::io::{stdout, BufWriter};
        use base::util::fs;

        #[derive(StructOpt)]
        pub struct $name {
            #[structopt(
                help = concat!("Output .", $ext, " file (STDOUT if omitted)"),
                long = "out-file",
                short = "o"
            )]
            pub path: Option<PathBuf>,
        }

        impl JsonOutput {
            pub fn get(&self) -> Result<Box<dyn io::Write>> {
                Ok(if let Some(path) = &self.path {
                    let writer = BufWriter::new(fs::create_file(path)?);
                    Box::new(writer) as Box<dyn io::Write>
                } else {
                    Box::new(stdout()) as Box<dyn io::Write>
                })
            }
        }
    };
}
