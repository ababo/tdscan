use std::io::{BufRead, BufReader};

use clap::Clap;

use base::defs::{Error, Result};
use base::fm;
use base::model;
use base::util::fs;

#[derive(Clap)]
#[clap(about = "Import data from Wavefront .obj file")]
pub struct ImportObjParams {
    #[clap(about = "Input .obj filename (STDIN if omitted)")]
    in_filename: Option<String>,
    #[clap(about = "Output .fm filename (STDOUT if omitted)", long, short)]
    out_filename: Option<String>,
    #[clap(
        about = "Type of output data compression",
        default_value = "brotli",
        long
    )]
    compression: fm::Compression,
    #[clap(
        about = "Quality for Brotli compression",
        default_value = "11",
        long
    )]
    brotli_quality: u32,
}

#[derive(Default)]
struct ImportState {
    line: usize,
    num_vs: usize,
    num_vns: usize,
    num_vts: usize,
}

pub fn import_obj(params: &ImportObjParams) -> Result<()> {
    let mut model = model::Model {
        elements: vec![model::Element {
            ..Default::default()
        }],
        states: vec![model::State {
            elements: vec![model::ElementState {
                ..Default::default()
            }],
        }],
    };

    let mut import = ImportState {
        ..Default::default()
    };

    let read = fs::open_file_or_stdin(&params.in_filename.as_deref())?;
    for line_res in BufReader::new(read).lines() {
        if let Ok(line) = line_res {
            import.line += 1;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 0 {
                match parts[0] {
                    "v" => import_v(&mut model, &mut import, &parts)?,
                    "vn" => import_vn(&mut model, &mut import, &parts)?,
                    "vt" => import_vt(&mut model, &mut import, &parts)?,
                    _ => (),
                }
            }
        }
    }

    let mut writer = fs::open_file_or_stdout(&params.out_filename.as_deref())?;
    let fm_params = fm::Params {
        compression: params.compression,
        brotli_quality: params.brotli_quality,
    };
    fm::encode(&model, &fm_params, &mut writer)
}

fn import_v(
    model: &mut model::Model,
    import: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() < 4 || parts.len() > 5 {
        return Err(Error::MalformedData(format!(
            "malformed v-statement at line {}",
            import.line
        )));
    }

    let x = parse_float("x-coordinate of v-statement", import.line, parts[1])?;
    let y = parse_float("y-coordinate of v-statement", import.line, parts[2])?;
    let z = parse_float("z-coordinate of v-statement", import.line, parts[3])?;

    let vertex = get_vertex_state(model, import.num_vs);
    vertex.location = Some(model::Point3 { x, y, z });
    import.num_vs += 1;

    Ok(())
}

fn import_vn(
    model: &mut model::Model,
    import: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() != 4 {
        return Err(Error::MalformedData(format!(
            "malformed vn-statement at line {}",
            import.line
        )));
    }

    let x = parse_float("x-coordinate of vn-statement", import.line, parts[1])?;
    let y = parse_float("y-coordinate of vn-statement", import.line, parts[2])?;
    let z = parse_float("z-coordinate of vn-statement", import.line, parts[3])?;

    let vertex = get_vertex_state(model, import.num_vns);
    vertex.normal = Some(model::Point3 { x, y, z });
    import.num_vns += 1;

    Ok(())
}

fn import_vt(
    model: &mut model::Model,
    import: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() < 3 || parts.len() > 4 {
        return Err(Error::MalformedData(format!(
            "malformed vt-statement at line {}",
            import.line
        )));
    }

    let x = parse_float("x-coordinate of vt-statement", import.line, parts[1])?;
    let y = parse_float("y-coordinate of vt-statement", import.line, parts[2])?;

    let vertex = get_vertex(model, import.num_vts);
    vertex.texture = Some(model::Point2 { x, y });
    import.num_vts += 1;

    Ok(())
}

fn get_vertex_state<'a>(
    model: &'a mut model::Model,
    index: usize,
) -> &'a mut model::VertexState {
    let vertices = &mut model.states[0].elements[0].vertices;
    if vertices.len() <= index {
        vertices.resize(
            index + 1,
            model::VertexState {
                ..Default::default()
            },
        );
    }
    &mut vertices[index]
}

fn get_vertex<'a>(
    model: &'a mut model::Model,
    index: usize,
) -> &'a mut model::Vertex {
    let vertices = &mut model.elements[0].vertices;
    if vertices.len() <= index {
        vertices.resize(
            index + 1,
            model::Vertex {
                ..Default::default()
            },
        );
    }
    &mut vertices[index]
}

fn parse_float(what: &str, line: usize, str: &str) -> Result<f32> {
    match str.parse::<f32>() {
        Ok(val) => Ok(val),
        Err(_) => Err(Error::ParseFloatError(format!(
            "failed to parse {} at line {}",
            what, line
        ))),
    }
}
