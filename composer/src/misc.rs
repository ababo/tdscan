use std::collections::BTreeMap;
use std::io::{stdin, stdout};
use std::path::PathBuf;

use rlua;
use rlua::Value as LuaValue;
use serde_json::Value as JsonValue;

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

pub fn lua_err_to_err(err: rlua::Error) -> Error {
    Error::with_source(LuaError, format!("failed to evaluate expression"), err)
}

pub fn lua_table_from_record<'a>(
    ctx: rlua::Context<'a>,
    record: &fm::Record,
    truncate_len: Option<usize>,
) -> rlua::Result<rlua::Table<'a>> {
    let mut val = serde_json::to_value(record).unwrap();
    if let Some(max_len) = truncate_len {
        truncate_json_value(&mut val, max_len);
    }
    lua_table_from_json_val(ctx, &val).map(|v| {
        if let rlua::Value::Table(t) = v {
            t
        } else {
            unreachable!()
        }
    })
}

fn lua_table_from_json_val<'a>(
    ctx: rlua::Context<'a>,
    val: &serde_json::Value,
) -> rlua::Result<rlua::Value<'a>> {
    match val {
        JsonValue::Null => Ok(LuaValue::Nil),
        JsonValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        JsonValue::Number(n) => Ok(LuaValue::Number(n.as_f64().unwrap())),
        JsonValue::String(s) => {
            ctx.create_string(s).map(|s| LuaValue::String(s))
        }
        JsonValue::Array(a) => {
            let mut vec = Vec::with_capacity(a.len());
            for e in a {
                let item = lua_table_from_json_val(ctx, e)?;
                vec.push(item);
            }
            ctx.create_sequence_from(vec).map(|t| LuaValue::Table(t))
        }
        JsonValue::Object(o) => {
            let mut map = Vec::with_capacity(o.len());
            for (k, v) in o {
                let key = ctx.create_string(k)?;
                let val = lua_table_from_json_val(ctx, v)?;
                map.push((key, val));
            }
            ctx.create_table_from(map).map(|t| LuaValue::Table(t))
        }
    }
}

pub fn read_scans(
    reader: &mut dyn fm::Read,
) -> Result<(BTreeMap<String, fm::Scan>, Vec<fm::ScanFrame>)> {
    let mut scans = BTreeMap::<String, fm::Scan>::new();
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

pub fn truncate_json_value(value: &mut JsonValue, max_len: usize) {
    match value {
        JsonValue::String(r#str) => {
            r#str.truncate(max_len);
        }
        JsonValue::Array(arr) => {
            arr.truncate(max_len);
            for mut e in arr {
                truncate_json_value(&mut e, max_len);
            }
        }
        JsonValue::Object(obj) => {
            let keys: Vec<_> = obj.keys().skip(max_len).cloned().collect();
            for k in keys {
                obj.remove(k.as_str());
            }

            for (_, mut v) in obj {
                truncate_json_value(&mut v, max_len);
            }
        }
        _ => {}
    };
}