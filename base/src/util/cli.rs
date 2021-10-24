use std::error::Error as StdError;
use std::fmt::Debug;
use std::result::Result as StdResult;
use std::str::FromStr;

use arrayvec::ArrayVec;

use crate::defs::{Error, ErrorKind::*, Result};

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
