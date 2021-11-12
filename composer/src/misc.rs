use base::util::cli::{parse_key_val, Array as CliArray};
use std::collections::{BTreeMap, HashMap};
use std::io::{stdin, stdout};
use std::path::PathBuf;

use rlua::Value as LuaValue;
use serde_json::Value as JsonValue;
use structopt::StructOpt;

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
    Error::with_source(
        LuaError,
        "failed to evaluate expression".to_string(),
        err,
    )
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
        JsonValue::String(s) => ctx.create_string(s).map(LuaValue::String),
        JsonValue::Array(a) => {
            let mut vec = Vec::with_capacity(a.len());
            for e in a {
                let item = lua_table_from_json_val(ctx, e)?;
                vec.push(item);
            }
            ctx.create_sequence_from(vec).map(LuaValue::Table)
        }
        JsonValue::Object(o) => {
            let mut map = Vec::with_capacity(o.len());
            for (k, v) in o {
                let key = ctx.create_string(k)?;
                let val = lua_table_from_json_val(ctx, v)?;
                map.push((key, val));
            }
            ctx.create_table_from(map).map(LuaValue::Table)
        }
    }
}

#[derive(StructOpt)]
pub struct ScanParams {
    #[structopt(
        help = "Camera initial position to override with",
        long = "camera-initial-position",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "y"
    )]
    pub camera_initial_positions: Vec<(String, CliArray<f32, 3>)>,

    #[structopt(
        help = "Camera initial direction to override with",
        long = "camera-initial-direction",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "c"
    )]
    pub camera_initial_directions: Vec<(String, CliArray<f32, 3>)>,

    #[structopt(
        help = "Camera landscape angle to override with",
        long = "camera-landscape-angle",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "l"
    )]
    pub camera_up_angles: Vec<(String, f32)>,

    #[structopt(
        help = "Downsample factor",
        long = "downsample-factor",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "w"
    )]
    pub downsample_factors: Vec<(String, usize)>,

    #[structopt(
        help = "Drop scan depths",
        long = "drop-depths",
        number_of_values = 1,
    )]
    pub drop_depths: Vec<String>,

    #[structopt(
        help = "Drop scan images",
        long = "drop-images",
        number_of_values = 1,
    )]
    pub drop_images: Vec<String>,

    #[structopt(
        help = "Scan name to override with",
        long = "name",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
    )]
    pub names: Vec<(String, String)>,
}

pub fn read_scans(
    reader: &mut dyn fm::Read,
    scan_params: &ScanParams,
) -> Result<(BTreeMap<String, fm::Scan>, Vec<fm::ScanFrame>)> {
    let mut scans = BTreeMap::<String, fm::Scan>::new();
    let mut frames = Vec::<fm::ScanFrame>::new();
    let mut last_time = 0;

    loop {
        let rec = reader.read_record()?;
        if rec.is_none() {
            break;
        }

        use fm::record::Type::*;
        match rec.unwrap().r#type {
            Some(Scan(s)) => {
                if !frames.is_empty() {
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
                frames.push(f);
            }
            _ => (),
        }
    }

    let unknown_scan_err = |name| {
        let desc = format!(
            "unknown scan '{}' for camera initial position override",
            name
        );
        Err(Error::new(InconsistentState, desc))
    };

    for (name, eye) in scan_params.camera_initial_positions.iter() {
        if let Some(scan) = scans.get_mut(name) {
            scan.camera_initial_position = Some(fm::Point3 {
                x: eye.0[0],
                y: eye.0[1],
                z: eye.0[2],
            });
        } else {
            return unknown_scan_err(name);
        }
    }

    for (name, dir) in scan_params.camera_initial_directions.iter() {
        if let Some(scan) = scans.get_mut(name) {
            scan.camera_initial_direction = Some(fm::Point3 {
                x: dir.0[0],
                y: dir.0[1],
                z: dir.0[2],
            });
        } else {
            return unknown_scan_err(name);
        }
    }

    for (name, angle) in scan_params.camera_up_angles.iter() {
        if let Some(scan) = scans.get_mut(name) {
            scan.camera_up_angle = *angle;
        } else {
            return unknown_scan_err(name);
        }
    }

    for (name, _) in scan_params.downsample_factors.iter() {
        if scans.get_mut(name).is_none() {
            return unknown_scan_err(name);
        }
    }
    if !scan_params.downsample_factors.is_empty() {
        let factors = scan_params.downsample_factors.iter().cloned().collect();
        downsample_scan_frames(&factors, &mut frames);
    }

    for name in scan_params.drop_depths.iter() {
        if scans.get_mut(name).is_none() {
            return unknown_scan_err(name);
        }
    }
    for name in scan_params.drop_images.iter() {
        if scans.get_mut(name).is_none() {
            return unknown_scan_err(name);
        }
    }
    for frame in frames.iter_mut() {
        if scan_params
            .drop_depths
            .iter()
            .any(|name| name.as_str() == frame.scan)
        {
            frame.depths = vec![];
            frame.depth_confidences = vec![];
        }
        if scan_params
            .drop_images
            .iter()
            .any(|name| name.as_str() == frame.scan)
        {
            frame.image = None;
        }
    }

    for (name, new_name) in scan_params.names.iter() {
        if let Some(scan) = scans.get_mut(name) {
            scan.name = new_name.clone();
        } else {
            return unknown_scan_err(name);
        }
    }
    for frame in frames.iter_mut() {
        if let Some((_, new_name)) = scan_params
            .names
            .iter()
            .find(|(name, _)| name == &frame.scan)
        {
            frame.scan = new_name.clone();
        }
    }

    Ok((scans, frames))
}

pub fn downsample_scan_frames(
    downsample_factors: &HashMap<String, usize>,
    frames: &mut Vec<fm::ScanFrame>,
) {
    let mut data = HashMap::<String, (fm::ScanFrame, usize)>::new();

    let mut j = 0;
    for i in 0..frames.len() {
        let factor = downsample_factors
            .get(&frames[i].scan)
            .cloned()
            .unwrap_or(1);
        if factor == 1 {
            frames.swap(i, j);
            j += 1;
            continue;
        }

        if let Some((acc, num)) = data.get_mut(&frames[i].scan) {
            for k in 0..acc.depths.len() {
                acc.depths[k] += &frames[i].depths[k];
                acc.depth_confidences[k] += &frames[i].depth_confidences[k];
            }
            *num += 1;

            if *num == factor {
                let (mut acc, num) = data.remove(&frames[i].scan).unwrap();
                for k in 0..acc.depths.len() {
                    acc.depths[k] /= num as f32;
                    acc.depth_confidences[k] =
                        (acc.depth_confidences[k] as f64 / num as f64).round()
                            as i32;
                }

                frames[j] = acc;
                j += 1;
            }
        } else {
            data.insert(frames[i].scan.clone(), (frames[i].clone(), 1));
        }
    }

    frames.truncate(j);
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

#[cfg(test)]
mod test {
    use super::*;

    use base::assert_eq_f32;

    fn new_scan_frame(
        scan: &str,
        time: fm::Time,
        depth: &[f32],
        depth_confidences: &[i32],
    ) -> fm::ScanFrame {
        fm::ScanFrame {
            scan: scan.to_string(),
            time,
            depths: depth.to_vec(),
            depth_confidences: depth_confidences.to_vec(),
            ..Default::default()
        }
    }

    fn new_downsample_factors(
        factors: &[(&str, usize)],
    ) -> HashMap<String, usize> {
        factors.iter().map(|f| (f.0.to_string(), f.1)).collect()
    }

    fn assert_eq_scan_frame(a: &fm::ScanFrame, b: &fm::ScanFrame) {
        assert_eq!(a.scan, b.scan);
        assert_eq!(a.time, b.time);
        assert_eq!(a.depths.len(), b.depths.len());
        for i in 0..a.depths.len() {
            assert_eq_f32!(a.depths[i], b.depths[i]);
        }
        assert_eq!(a.depth_confidences, b.depth_confidences);
    }

    #[test]
    fn test_downsample_scan_frames() {
        let mut frames = vec![
            new_scan_frame("a", 1, &[1.0, 2.0], &[1, 2]),
            new_scan_frame("a", 2, &[2.0, 1.0], &[2, 1]),
            new_scan_frame("b", 3, &[2.0, 3.0], &[2, 3]),
            new_scan_frame("a", 4, &[1.0, 2.0], &[1, 2]),
            new_scan_frame("b", 5, &[3.0, 2.0], &[3, 2]),
            new_scan_frame("a", 6, &[2.0, 1.0], &[2, 1]),
        ];

        let factors = new_downsample_factors(&[("a", 3)]);
        downsample_scan_frames(&factors, &mut frames);

        assert_eq!(frames.len(), 3);
        assert_eq_scan_frame(
            &frames[0],
            &new_scan_frame("b", 3, &[2.0, 3.0], &[2, 3]),
        );
        assert_eq_scan_frame(
            &frames[1],
            &new_scan_frame("a", 1, &[4.0 / 3.0, 5.0 / 3.0], &[1, 2]),
        );
        assert_eq_scan_frame(
            &frames[2],
            &new_scan_frame("b", 5, &[3.0, 2.0], &[3, 2]),
        );
    }
}
