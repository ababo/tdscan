use std::io;
use std::io::{stdin, stdout, BufRead, BufReader};
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
    element: Option<String>,
    #[structopt(
        help = "Output .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    fm_path: Option<PathBuf>,
    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn import_obj_with_params(params: &ImportObjParams) -> Result<()> {
    let mut obj_reader = if let Some(path) = &params.obj_path {
        Box::new(fs::open_file(path)?) as Box<dyn io::Read>
    } else {
        Box::new(stdin()) as Box<dyn io::Read>
    };

    let mut fm_writer = if let Some(path) = &params.fm_path {
        let writer =
            fm::Writer::new(fs::create_file(path)?, &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    } else {
        let writer = fm::Writer::new(stdout(), &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    };

    let element = if let Some(id) = &params.element {
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
        &mut obj_reader,
        |p| fs::read_file(p),
        mtl_dir,
        fm_writer.as_mut(),
        element.as_str(),
    )
}

pub fn import_obj<F: Fn(&Path) -> Result<Vec<u8>>>(
    obj_reader: &mut dyn io::Read,
    read_file: F,
    mtl_dir: &Path,
    fm_writer: &mut dyn fm::Write,
    element: &str,
) -> Result<()> {
    let mut state = ImportState {
        view: model::ElementView {
            element: element.to_string(),
            ..Default::default()
        },
        view_state: model::ElementViewState {
            element: element.to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

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
            id: element.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use base::fm::Read as _;
    use base::record_variant;
    use model::record::Type::*;

    fn face(
        v1: u32,
        v2: u32,
        v3: u32,
        t1: u32,
        t2: u32,
        t3: u32,
    ) -> model::element_view::Face {
        model::element_view::Face {
            vertex1: v1,
            vertex2: v2,
            vertex3: v3,
            texture1: t1,
            texture2: t2,
            texture3: t3,
        }
    }

    fn point3(x: f32, y: f32, z: f32) -> model::Point3 {
        model::Point3 { x, y, z }
    }

    #[test]
    fn test_valid_obj() {
        let obj = r#"
            mtllib foo.mtl
            usemtl bar

            # Vertices.
            v 0.00 0.01 0.02 0.03
            v 0.03 0.04 0.05
            v 0.06 0.07 0.08
            v 0.09 0.10 0.11
            v 0.12 0.13 0.14
            v 0.15 0.14 0.15

            # Normals.
            vn 0.20 0.21 0.22
            vn 0.23 0.24 0.25
            vn 0.26 0.27 0.28
            vn 0.29 0.30 0.31
            vn 0.32 0.33 0.34
            vn 0.35 0.36 0.37

            # Texture points.
            vt 0.40 0.41 0.42
            vt 0.42 0.43
            vt 0.44 0.45

            # Faces.
            f 1/1/6 2/2/5 3/3/4 4/1/3 5/2/2 6/3/1
            f 2/1/5 3/2/4 4/3/3 5/1/2
            f 3/2/4 4/3/3 5/1/2
        "#;

        let mtl = r#"
            # Foo materials.

            newmtl bar
            map_Ka bar.jpg
        "#;

        let mut obj_reader = obj.as_bytes();

        let mut fm_writer =
            fm::Writer::new(Vec::<u8>::new(), &fm::WriterParams::default())
                .unwrap();

        let read_file = |p: &Path| {
            if p == &Path::new("obj-path").join("foo.mtl") {
                Ok(mtl.as_bytes().to_vec())
            } else if p == &Path::new("obj-path").join("bar.jpg") {
                Ok(vec![1, 2, 3])
            } else {
                Err(Error::new(IoError, format!("bad file path")))
            }
        };

        import_obj(
            &mut obj_reader,
            read_file,
            "obj-path".as_ref(),
            &mut fm_writer,
            "buzz",
        )
        .unwrap();

        let fm_data = fm_writer.into_inner().unwrap();
        let mut fm_reader = fm_data.as_slice();
        let mut fm_reader = fm::Reader::new(&mut fm_reader).unwrap();

        let record = fm_reader.read_record().unwrap().unwrap();
        let element = record_variant!(Element, record);
        assert_eq!(element.id, format!("buzz"));
        assert!(element.composite.is_empty());

        let record = fm_reader.read_record().unwrap().unwrap();
        let view = record_variant!(ElementView, record);
        assert_eq!(view.element, format!("buzz"));

        let texture = view.texture.unwrap();
        assert_eq!(texture.r#type, model::image::r#Type::Jpeg as i32);
        assert_eq!(texture.data, vec![1, 2, 3]);

        let texture_points = view.texture_points;
        assert_eq!(texture_points.len(), 3);
        assert_eq!(texture_points[0], model::Point2 { x: 0.40, y: 0.41 });
        assert_eq!(texture_points[1], model::Point2 { x: 0.42, y: 0.43 });
        assert_eq!(texture_points[2], model::Point2 { x: 0.44, y: 0.45 });

        let faces = view.faces;
        assert_eq!(faces.len(), 7);
        assert_eq!(faces[0], face(1, 2, 6, 1, 2, 3));
        assert_eq!(faces[1], face(2, 3, 6, 2, 3, 3));
        assert_eq!(faces[2], face(3, 4, 6, 3, 1, 3));
        assert_eq!(faces[3], face(4, 5, 6, 1, 2, 3));
        assert_eq!(faces[4], face(2, 3, 5, 1, 2, 1));
        assert_eq!(faces[5], face(3, 4, 5, 2, 3, 1));
        assert_eq!(faces[6], face(3, 4, 5, 2, 3, 1));

        let record = fm_reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, record);
        assert_eq!(state.element, format!("buzz"));
        assert_eq!(state.time, 0);

        let vertices = state.vertices;
        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices[0], point3(0.00, 0.01, 0.02));
        assert_eq!(vertices[1], point3(0.03, 0.04, 0.05));
        assert_eq!(vertices[2], point3(0.06, 0.07, 0.08));
        assert_eq!(vertices[3], point3(0.09, 0.10, 0.11));
        assert_eq!(vertices[4], point3(0.12, 0.13, 0.14));
        assert_eq!(vertices[5], point3(0.15, 0.14, 0.15));

        let normals = state.normals;
        assert_eq!(normals.len(), 6);
        assert_eq!(normals[0], point3(0.35, 0.36, 0.37));
        assert_eq!(normals[1], point3(0.32, 0.33, 0.34));
        assert_eq!(normals[2], point3(0.29, 0.30, 0.31));
        assert_eq!(normals[3], point3(0.26, 0.27, 0.28));
        assert_eq!(normals[4], point3(0.23, 0.24, 0.25));
        assert_eq!(normals[5], point3(0.20, 0.21, 0.22));
    }
}
