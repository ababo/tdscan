use std::io::{stdin, stdout, BufRead, BufReader, Read, Write};
use std::mem::take;
use std::path::PathBuf;

use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::model;
use base::util::fs::{create_file, open_file, read_file};

const MAX_NUM_FACE_VERTICES: usize = 10;

#[derive(StructOpt)]
#[structopt(about = "Import data from Wavefront .obj file")]
pub struct ImportObjParams {
    #[structopt(about = "Input .obj filename (STDIN if omitted)")]
    obj_filename: Option<PathBuf>,
    #[structopt(about = "Input .mtl filename", long)]
    mtl_filename: Option<PathBuf>,
    #[structopt(about = "Output element ID", long)]
    element_id: Option<String>,
    #[structopt(
        about = "Output .fm filename (STDOUT if omitted)",
        long,
        short = "o"
    )]
    fm_filename: Option<PathBuf>,
    #[structopt(flatten)]
    fm_write_params: fm::WriteParams,
}

#[derive(Default)]
struct ImportState {
    line: usize,
    view: model::ElementView,
    view_state: model::ElementViewState,
    normals: Vec<model::Point3>,
    mtl_dir: PathBuf,
}

pub fn import_obj(params: &ImportObjParams) -> Result<()> {
    let reader = if let Some(filename) = &params.obj_filename {
        open_file(filename)
    } else {
        Ok(Box::new(stdin()) as Box<dyn Read>)
    }?;

    let writer = if let Some(filename) = &params.fm_filename {
        create_file(filename)
    } else {
        Ok(Box::new(stdout()) as Box<dyn Write>)
    }?;

    let mut writer = fm::Writer::from_writer(writer, &params.fm_write_params)?;

    let id = if let Some(id) = &params.element_id {
        id.clone()
    } else if let Some(filename) = &params.obj_filename {
        filename
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_string()
    } else {
        String::default()
    };

    let mut state = ImportState {
        view: model::ElementView {
            element: id.clone(),
            ..Default::default()
        },
        view_state: model::ElementViewState {
            element: id.clone(),
            ..Default::default()
        },
        ..Default::default()
    };

    for line_res in BufReader::new(reader).lines() {
        if let Ok(line) = line_res {
            state.line += 1;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 0 {
                match parts[0] {
                    "f" => import_f(&mut state, &parts)?,
                    "v" => import_v(&mut state, &parts)?,
                    "vn" => import_vn(&mut state, &parts)?,
                    "vt" => import_vt(&mut state, &parts)?,
                    _ => (),
                }
            }
        }
    }

    import_mtl(params, &mut state)?;

    use model::record::Type;

    writer.write_record(model::Record {
        r#type: Some(Type::Element(model::Element {
            id,
            composite: String::default(),
        })),
    })?;

    writer.write_record(model::Record {
        r#type: Some(Type::ElementView(take(&mut state.view))),
    })?;

    writer.write_record(model::Record {
        r#type: Some(Type::ElementViewState(take(&mut state.view_state))),
    })?;

    Ok(())
}

fn import_f(state: &mut ImportState, parts: &Vec<&str>) -> Result<()> {
    if parts.len() < 4 {
        return Err(Error::new(
            MalformedData,
            format!(
                "bad number of vertices in f-statement at line {}",
                state.line
            ),
        ));
    }

    let mut face_vertices = [(0, 0, 0); MAX_NUM_FACE_VERTICES];

    let whats = [
        "location number of f-statement",
        "texture number of f-statement",
        "normal number of f-statement",
    ];

    for (i, part) in parts[1..].iter().enumerate() {
        let mut nums: [u32; 3] = [0, 0, 0];
        for (j, istr) in part.split("/").enumerate() {
            if j > 2 {
                return Err(Error::new(
                    MalformedData,
                    format!(
                    "bad number of numbers of f-statement vertex {} at line {}",
                    i + 1,
                    state.line
                ),
                ));
            }
            if j != 1 || !istr.is_empty() {
                nums[j] = parse_num(whats[j], state.line, i + 1, istr)?;
            }
        }

        face_vertices[i] = (nums[0], nums[1], nums[2]);
    }

    let len = parts.len() - 1;
    for (i, (l, t, n)) in face_vertices[..len - 2].iter().cloned().enumerate() {
        let (l2, t2, n2) = face_vertices[i + 1];
        let (l3, t3, n3) = face_vertices[len - 1];

        add_normal(state, l, n)?;
        add_normal(state, l2, n2)?;
        add_normal(state, l3, n3)?;

        state.view.faces.push(model::element_view::Face {
            vertex1: l,
            vertex2: l2,
            vertex3: l3,
            texture1: t,
            texture2: t2,
            texture3: t3,
        })
    }

    Ok(())
}

fn import_v(state: &mut ImportState, parts: &Vec<&str>) -> Result<()> {
    if parts.len() < 4 || parts.len() > 5 {
        return Err(Error::new(
            MalformedData,
            format!("malformed v-statement at line {}", state.line),
        ));
    }

    let x = parse_coord("x-coordinate of v-statement", state.line, parts[1])?;
    let y = parse_coord("y-coordinate of v-statement", state.line, parts[2])?;
    let z = parse_coord("z-coordinate of v-statement", state.line, parts[3])?;

    state.view_state.vertices.push(model::Point3 { x, y, z });

    Ok(())
}

fn import_vn(state: &mut ImportState, parts: &Vec<&str>) -> Result<()> {
    if parts.len() != 4 {
        return Err(Error::new(
            MalformedData,
            format!("malformed vn-statement at line {}", state.line),
        ));
    }

    let x = parse_coord("x-coordinate of vn-statement", state.line, parts[1])?;
    let y = parse_coord("y-coordinate of vn-statement", state.line, parts[2])?;
    let z = parse_coord("z-coordinate of vn-statement", state.line, parts[3])?;

    state.normals.push(model::Point3 { x, y, z });

    Ok(())
}

fn import_vt(state: &mut ImportState, parts: &Vec<&str>) -> Result<()> {
    if parts.len() < 3 || parts.len() > 4 {
        return Err(Error::new(
            MalformedData,
            format!("malformed vt-statement at line {}", state.line),
        ));
    }

    let x = parse_coord("x-coordinate of vt-statement", state.line, parts[1])?;
    let y = parse_coord("y-coordinate of vt-statement", state.line, parts[2])?;

    state.view.texture_points.push(model::Point2 { x, y });

    Ok(())
}

fn parse_coord(what: &str, line: usize, str: &str) -> Result<f32> {
    match str.parse::<f32>() {
        Ok(val) => Ok(val),
        Err(_) => Err(Error::new(
            MalformedData,
            format!("failed to parse {} at line {}", what, line),
        )),
    }
}

fn parse_num(what: &str, line: usize, vertex: usize, str: &str) -> Result<u32> {
    match str.parse::<u32>() {
        Ok(val) => {
            if val > 0 {
                Ok(val)
            } else {
                Err(Error::new(
                    MalformedData,
                    format!("zero {} vertex {} at line {}", what, vertex, line),
                ))
            }
        }
        Err(_) => Err(Error::new(
            MalformedData,
            format!(
                "failed to parse {} vertex {} at line {}",
                what, vertex, line
            ),
        )),
    }
}

fn add_normal(state: &mut ImportState, vertex: u32, normal: u32) -> Result<()> {
    let vi = (vertex - 1) as usize;
    if state.view_state.vertices.len() <= vi {
        return Err(Error::new(
            MalformedData,
            format!(
                "mention of unknown vertex {} at line {}",
                vertex, state.line
            ),
        ));
    }

    const ZERO: model::Point3 = model::Point3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    let normals = &mut state.view_state.normals;
    if normals.len() <= vi {
        normals.resize(vi + 1, ZERO);
    }

    let ni = (normal - 1) as usize;
    if normals[vi] == ZERO {
        normals[vi] = state.normals[ni].clone();
    } else if normals[vi] != state.normals[ni] {
        return Err(Error::new(
            MalformedData,
            format!(
                "more than one normal for vertex {} at line {}",
                vertex, state.line
            ),
        ));
    }

    Ok(())
}

fn import_mtl(params: &ImportObjParams, state: &mut ImportState) -> Result<()> {
    let path = if let Some(filename) = &params.mtl_filename {
        filename.clone()
    } else if let Some(filename) = &params.obj_filename {
        let mut filename = filename.clone();
        filename.set_extension("mtl");
        if filename.exists() {
            filename
        } else {
            return Ok(());
        }
    } else {
        return Ok(());
    };

    let reader = open_file(&path)?;

    state.line = 0;
    state.mtl_dir = path.parent().unwrap().to_path_buf();

    for line_res in BufReader::new(reader).lines() {
        if let Ok(line) = line_res {
            state.line += 1;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if !parts.is_empty() {
                match parts[0] {
                    "map_Ka" => import_map_ka(state, &parts)?,
                    _ => (),
                }
            }
        }
    }

    Ok(())
}

fn import_map_ka(state: &mut ImportState, parts: &Vec<&str>) -> Result<()> {
    if parts.len() != 2 {
        return Err(Error::new(
            MalformedData,
            format!("malformed map_Ka-statement at line {}", state.line),
        ));
    }

    let path = state.mtl_dir.join(parts[1]);

    let ext = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap()
        .to_lowercase();

    let image_type = match ext.as_str() {
        "png" => model::image::Type::Png,
        "jpg" => model::image::Type::Jpeg,
        _ => {
            return Err(Error::new(
                FeatureNotSupported,
                format!(
                    "unknown type of file '{}' in map_Ka-statement at line {}",
                    path.to_str().unwrap(),
                    state.line
                ),
            ));
        }
    };

    let texture = model::Image {
        r#type: image_type as i32,
        data: read_file(path)?,
    };
    state.view.texture = Some(texture);

    Ok(())
}
