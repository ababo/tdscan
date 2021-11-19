use std::io;
use std::io::{BufWriter, stdout};
use std::path::PathBuf;

use serde::ser::Serialize;
use serde_json::{to_value, to_writer, to_writer_pretty};
use structopt::StructOpt;

use crate::misc::{fm_reader_from_file_or_stdin, truncate_json_value};
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
        short = "t"
    )]
    truncate_len: Option<usize>,

    #[structopt(help = "Prettify JSON output", long, short = "p")]
    pretty: bool,
}

pub fn export_to_json_with_params(params: &ExportToJsonParams) -> Result<()> {
    let mut reader = fm_reader_from_file_or_stdin(&params.in_path)?;

    let mut writer = if let Some(path) = &params.out_path {
        let writer = BufWriter::new(fs::create_file(path)?);
        Box::new(writer) as Box<dyn io::Write>
    } else {
        Box::new(stdout()) as Box<dyn io::Write>
    };

    export_to_json(
        reader.as_mut(),
        &mut writer,
        params.truncate_len,
        params.pretty,
    )
}

pub fn export_to_json(
    reader: &mut dyn fm::Read,
    writer: &mut dyn io::Write,
    truncate_len: Option<usize>,
    pretty: bool,
) -> Result<()> {
    while let Some(rec) = reader.read_record()? {
        if let Some(max_len) = truncate_len {
            let mut val = to_value(rec).into_result(|| {
                "failed to convert record into JSON value".to_string()
            })?;
            truncate_json_value(&mut val, max_len);
            write_record(writer, val, pretty)?;
        } else {
            write_record(writer, rec, pretty)?;
        }
    }

    Ok(())
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
    .into_result(|| "failed to write record JSON".to_string())?;
    writer
        .write_all("\n".as_bytes())
        .into_result(|| "failed to write end-of-line".to_string())
}

#[cfg(test)]
mod tests {
    use std::str::from_utf8;

    use super::*;
    use base::util::test::*;

    fn export(truncate_len: Option<usize>, pretty: bool) -> String {
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
        export_to_json(&mut reader, &mut json, truncate_len, pretty).unwrap();
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
