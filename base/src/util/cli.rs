use std::error::Error as StdError;
use std::result::Result as StdResult;
use std::str::FromStr;

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
