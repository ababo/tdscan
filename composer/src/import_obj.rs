use std::io::{stdin, stdout, BufRead, BufReader, Read, Write};
use std::mem::take;
use std::path::{Path, PathBuf};

use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::model;
use base::util::fs;

const MAX_NUM_FACE_VERTICES: usize = 10;

#[derive(StructOpt)]
#[structopt(about = "Import data from Wavefront .obj file")]
pub struct ImportObjParams {
    #[structopt(help = "Input .obj file (STDIN if omitted)")]
    obj_path: Option<PathBuf>,
    #[structopt(help = "Element ID for imported data", long)]
    element_id: Option<String>,
    #[structopt(
        help = "Output .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    fm_path: Option<PathBuf>,
    #[structopt(flatten)]
    fm_write_params: fm::WriteParams,
}

pub fn import_obj_with_params(params: &ImportObjParams) -> Result<()> {
    let obj_reader = if let Some(path) = &params.obj_path {
        fs::open_file(path)
    } else {
        Ok(Box::new(stdin()) as Box<dyn Read>)
    }?;

    let fm_writer = if let Some(path) = &params.fm_path {
        fs::create_file(path)
    } else {
        Ok(Box::new(stdout()) as Box<dyn Write>)
    }?;

    let fm_writer =
        fm::Writer::from_writer(fm_writer, &params.fm_write_params)?;

    let element_id = if let Some(id) = &params.element_id {
        id.clone()
    } else if let Some(path) = &params.obj_path {
        path.file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_string()
    } else {
        String::default()
    };

    let mtl_dir = params
        .obj_path
        .as_deref()
        .unwrap_or(".".as_ref())
        .parent()
        .unwrap_or(".".as_ref());

    import_obj(
        obj_reader,
        |p| fs::read_file(p),
        mtl_dir,
        fm_writer,
        element_id.as_str(),
    )
}

pub fn import_obj<R: Read, F: Fn(&Path) -> Result<Vec<u8>>>(
    obj_reader: R,
    read_file: F,
    mtl_dir: &Path,
    mut fm_writer: fm::Writer,
    element_id: &str,
) -> Result<()> {
    let mut state = ImportState::default();

    for line_res in BufReader::new(obj_reader).lines() {
        if let Ok(line) = line_res {
            state.line += 1;

            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() > 0 {
                match parts[0] {
                    "f" => import_f(&mut state, &parts)?,
                    "mtllib" => {
                        import_mtllib(&read_file, mtl_dir, &mut state, &parts)?
                    }
                    "usemtl" => import_usemtl(&mut state, &parts)?,
                    "v" => import_v(&mut state, &parts)?,
                    "vn" => import_vn(&mut state, &parts)?,
                    "vt" => import_vt(&mut state, &parts)?,
                    _ => (),
                }
            }
        }
    }

    use model::record::Type;

    fm_writer.write_record(model::Record {
        r#type: Some(Type::Element(model::Element {
            id: element_id.to_string(),
            composite: String::default(),
        })),
    })?;

    fm_writer.write_record(model::Record {
        r#type: Some(Type::ElementView(take(&mut state.view))),
    })?;

    fm_writer.write_record(model::Record {
        r#type: Some(Type::ElementViewState(take(&mut state.view_state))),
    })?;

    Ok(())
}

#[derive(Default)]
struct ImportState {
    line: usize,
    normals: Vec<model::Point3>,
    view: model::ElementView,
    view_state: model::ElementViewState,
    mtl_line: usize,
    mtl_material: Option<String>,
    mtl_dir: PathBuf,
}

fn import_f(state: &mut ImportState, parts: &Vec<&str>) -> Result<()> {
    let num_vertices_err_res = |kind, prop| {
        let msg = "number of vertices in f-statement at line";
        Err(Error::new(kind, format!("{} {} {}", prop, msg, state.line)))
    };
    if parts.len() < 4 {
        return num_vertices_err_res(MalformedData, "bad");
    } else if parts.len() > MAX_NUM_FACE_VERTICES {
        return num_vertices_err_res(UnsupportedFeature, "unsupported");
    }

    let mut face_vertices = [(0, 0, 0); MAX_NUM_FACE_VERTICES];

    for (i, part) in parts[1..].iter().enumerate() {
        let mut iter = part.split("/");
        let vertex = parse_f_component(state.line, &mut iter, i + 1, false)?;
        let texture = parse_f_component(state.line, &mut iter, i + 1, true)?;
        let normal = parse_f_component(state.line, &mut iter, i + 1, false)?;
        if iter.next().is_some() {
            // Too many components, so emit the same error.
            parse_f_component(state.line, &mut iter, i + 1, false)?;
        }
        face_vertices[i] = (vertex, texture, normal);
    }

    let len = parts.len() - 1;
    for (i, (v, t, n)) in face_vertices[..len - 2].iter().cloned().enumerate() {
        let (v2, t2, n2) = face_vertices[i + 1];
        let (v3, t3, n3) = face_vertices[len - 1];

        add_normal(state, v, n)?;
        add_normal(state, v2, n2)?;
        add_normal(state, v3, n3)?;

        state.view.faces.push(model::element_view::Face {
            vertex1: v,
            vertex2: v2,
            vertex3: v3,
            texture1: t,
            texture2: t2,
            texture3: t3,
        })
    }

    Ok(())
}

fn parse_f_component(
    line: usize,
    iter: &mut std::str::Split<&str>,
    vnum: usize,
    tex: bool,
) -> Result<u32> {
    let component: &str = iter.next().unwrap_or_default();
    if component.is_empty() && tex {
        return Ok(0);
    }

    let num = component.parse::<u32>().unwrap_or_default();
    if num != 0 {
        Ok(num)
    } else {
        let desc = format!(
            "malformed vertex {} in f-statement at line {}",
            vnum, line
        );
        Err(Error::new(MalformedData, desc))
    }
}

fn import_mtllib<F: Fn(&Path) -> Result<Vec<u8>>>(
    read_file: &F,
    mtl_dir: &Path,
    state: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    let num_filenames_err_res = |kind, prop| {
        let msg = "number of filenames in mtllib-statement at line";
        Err(Error::new(kind, format!("{} {} {}", prop, msg, state.line)))
    };
    if parts.len() < 2 {
        return num_filenames_err_res(MalformedData, "bad");
    } else if parts.len() > 2 {
        return num_filenames_err_res(UnsupportedFeature, "unsupported");
    }

    let mtl_data = read_file(&mtl_dir.join(parts[1]))?;
    for line_res in BufReader::new(mtl_data.as_slice()).lines() {
        if let Ok(line) = line_res {
            state.mtl_line += 1;

            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if !parts.is_empty() {
                match parts[0] {
                    "map_Ka" => {
                        import_mtl_map_ka(&read_file, mtl_dir, state, &parts)?
                    }
                    "newmtl" => import_mtl_newmtl(state, &parts)?,
                    _ => (),
                }
            }
        }
    }

    Ok(())
}

fn import_usemtl(state: &mut ImportState, parts: &Vec<&str>) -> Result<()> {
    if parts.len() != 2 {
        let desc = format!("malformed usemtl-statement at line {}", state.line);
        return Err(Error::new(MalformedData, desc));
    }

    if Some(parts[1]) != state.mtl_material.as_deref() {
        let desc = format!(
            "unknown material in usemtl-statement at line {}",
            state.line
        );
        return Err(Error::new(InconsistentState, desc));
    }

    Ok(())
}

fn import_mtl_map_ka<F: Fn(&Path) -> Result<Vec<u8>>>(
    read_file: &F,
    mtl_dir: &Path,
    state: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() != 2 {
        let desc =
            format!("malformed map_Ka-statement at line {}", state.mtl_line);
        return Err(Error::new(MalformedData, desc));
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
            let desc = format!(
                concat!(
                    "missing or unsupported file extension ",
                    "in map_Ka-statement at line {}"
                ),
                state.line
            );
            return Err(Error::new(UnsupportedFeature, desc));
        }
    };

    let texture = model::Image {
        r#type: image_type as i32,
        data: read_file(&mtl_dir.join(path))?,
    };
    state.view.texture = Some(texture);

    Ok(())
}

fn import_mtl_newmtl(state: &mut ImportState, parts: &Vec<&str>) -> Result<()> {
    if state.mtl_material.is_some() {
        let desc = format!(
            "multiple materials are not supported, found at line {}",
            state.mtl_line
        );
        return Err(Error::new(UnsupportedFeature, desc));
    }
    state.mtl_material = Some(parts[1].to_string());
    Ok(())
}

fn add_normal(state: &mut ImportState, vertex: u32, normal: u32) -> Result<()> {
    let vi = (vertex - 1) as usize;
    if state.view_state.vertices.len() <= vi {
        let desc = format!(
            "reference to unknown vertex {} in f-statement at line {}",
            vertex, state.line
        );
        return Err(Error::new(InconsistentState, desc));
    }

    let normals = &mut state.view_state.normals;
    if normals.len() <= vi {
        normals.resize(vi + 1, model::Point3::default());
    }

    let ni = (normal - 1) as usize;
    if normals[vi] == model::Point3::default() {
        normals[vi] = state.normals[ni].clone();
    } else if normals[vi] != state.normals[ni] {
        let desc = format!(
            "multiple normals for vertex {} in f-statement at line {}",
            vertex, state.line
        );
        return Err(Error::new(InconsistentState, desc));
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
