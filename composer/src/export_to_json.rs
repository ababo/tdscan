use std::io;
use std::io::{stdin, stdout};
use std::path::PathBuf;

use serde::ser::Serialize;
use serde_json::{to_value, to_writer, to_writer_pretty, Value};
use structopt::StructOpt;

use base::defs::{IntoResult, Result};
use base::fm;
use base::util::fs;

#[derive(StructOpt)]
#[structopt(about = "Export .fm file into JSON")]
pub struct ExportToJsonParams {
    #[structopt(help = "Input .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,
    #[structopt(
        help = "Output .json file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,
    #[structopt(
        help = concat!("Maximum length for string, array ",
            "or object (the rest of elements to be truncated)"),
        long,
        short = "m"
    )]
    max_len: Option<usize>,
    #[structopt(help = "Prettify JSON output", long, short = "p")]
    pretty: bool,
}

pub fn export_to_json_with_params(params: &ExportToJsonParams) -> Result<()> {
    let mut reader = if let Some(path) = &params.in_path {
        let reader = fm::Reader::new(fs::open_file(path)?)?;
        Box::new(reader) as Box<dyn fm::Read>
    } else {
        let reader = fm::Reader::new(stdin())?;
        Box::new(reader) as Box<dyn fm::Read>
    };

    let mut writer = if let Some(path) = &params.out_path {
        Box::new(fs::create_file(path)?) as Box<dyn io::Write>
    } else {
        Box::new(stdout()) as Box<dyn io::Write>
    };

    export_to_json(reader.as_mut(), &mut writer, params.max_len, params.pretty)
}

pub fn export_to_json(
    reader: &mut dyn fm::Read,
    writer: &mut dyn io::Write,
    max_len: Option<usize>,
    pretty: bool,
) -> Result<()> {
    loop {
        match reader.read_record()? {
            Some(rec) => {
                if let Some(max) = max_len {
                    let mut val = to_value(rec).into_result(|| {
                        format!("failed to convert record into JSON value")
                    })?;
                    truncate_len(&mut val, max);
                    write_record(writer, val, pretty)?;
                } else {
                    write_record(writer, rec, pretty)?;
                }
            }
            None => break,
        }
    }

    Ok(())
}

fn truncate_len(value: &mut Value, max: usize) {
    match value {
        Value::String(r#str) => {
            r#str.truncate(max);
        }
        Value::Array(arr) => {
            arr.truncate(max);
            for mut e in arr {
                truncate_len(&mut e, max);
            }
        }
        Value::Object(obj) => {
            let keys: Vec<_> = obj.keys().skip(max).cloned().collect();
            for k in keys {
                obj.remove(k.as_str());
            }

            for (_, mut v) in obj {
                truncate_len(&mut v, max);
            }
        }
        _ => {}
    };
}

fn write_record<T: Serialize>(
    writer: &mut dyn io::Write,
    record: T,
    pretty: bool,
) -> Result<()> {
    if pretty {
        to_writer_pretty(&mut *writer, &record)
    } else {
        to_writer(&mut *writer, &record)
    }
    .into_result(|| format!("failed to write record JSON"))?;
    writer
        .write_all("\n".as_bytes())
        .into_result(|| format!("failed to write end-of-line"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::util::test::*;

    fn export(max_len: Option<usize>, pretty: bool) -> String {
        let mut reader = create_reader_with_records(&vec![
            new_element_view_rec(fm::ElementView {
                element: format!("element"),
                texture_points: vec![
                    new_point2(1.0, 2.0),
                    new_point2(3.0, 4.0),
                ],
                ..Default::default()
            }),
            new_element_view_state_rec(fm::ElementViewState {
                element: format!("element"),
                vertices: vec![
                    new_point3(5.0, 6.0, 7.0),
                    new_point3(8.0, 9.0, 10.0),
                    new_point3(11.0, 12.0, 13.0),
                ],
                ..Default::default()
            }),
        ]);

        let mut json = Vec::new();
        export_to_json(&mut reader, &mut json, max_len, pretty).unwrap();
        format!("\n{}", from_utf8(json.as_slice()).unwrap())
    }

    #[test]
    fn test_export_to_json_plain() {
        assert_eq!(
            export(None, false),
            r#"
{"type":{"ElementView":{"element":"element","texture":null,"texture_points":[{"x":1.0,"y":2.0},{"x":3.0,"y":4.0}],"faces":[]}}}
{"type":{"ElementViewState":{"element":"element","time":0,"vertices":[{"x":5.0,"y":6.0,"z":7.0},{"x":8.0,"y":9.0,"z":10.0},{"x":11.0,"y":12.0,"z":13.0}],"normals":[]}}}
"#
        );
    }

    #[test]
    fn test_export_to_json_pretty() {
        assert_eq!(
            export(None, true),
            r#"
{
  "type": {
    "ElementView": {
      "element": "element",
      "texture": null,
      "texture_points": [
        {
          "x": 1.0,
          "y": 2.0
        },
        {
          "x": 3.0,
          "y": 4.0
        }
      ],
      "faces": []
    }
  }
}
{
  "type": {
    "ElementViewState": {
      "element": "element",
      "time": 0,
      "vertices": [
        {
          "x": 5.0,
          "y": 6.0,
          "z": 7.0
        },
        {
          "x": 8.0,
          "y": 9.0,
          "z": 10.0
        },
        {
          "x": 11.0,
          "y": 12.0,
          "z": 13.0
        }
      ],
      "normals": []
    }
  }
}
"#
        );
    }

    #[test]
    fn test_export_to_json_truncated() {
        assert_eq!(
            export(Some(2), true),
            r#"
{
  "type": {
    "ElementView": {
      "element": "el",
      "faces": []
    }
  }
}
{
  "type": {
    "ElementViewState": {
      "element": "el",
      "normals": []
    }
  }
}
"#
        );
    }
}
