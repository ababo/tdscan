use std::io::{stdin, stdout, BufRead, BufReader, Read, Write};
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
    #[structopt(
        about = "Output .fm filename (STDOUT if omitted)",
        long,
        short = "o"
    )]
    fm_filename: Option<PathBuf>,
    #[structopt(flatten)]
    fm_params: fm::Params,
}

#[derive(Default)]
struct ImportState {
    line: usize,
    normals: Vec<model::Point3>,
    mtl_dir: PathBuf,
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

    let reader = if let Some(filename) = &params.obj_filename {
        open_file(filename)
    } else {
        Ok(Box::new(stdin()) as Box<dyn Read>)
    }?;

    for line_res in BufReader::new(reader).lines() {
        if let Ok(line) = line_res {
            import.line += 1;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 0 {
                match parts[0] {
                    "f" => import_f(&mut model, &mut import, &parts)?,
                    "v" => import_v(&mut model, &mut import, &parts)?,
                    "vn" => import_vn(&mut model, &mut import, &parts)?,
                    "vt" => import_vt(&mut model, &mut import, &parts)?,
                    _ => (),
                }
            }
        }
    }

    import_mtl(params, &mut model, &mut import)?;

    let mut writer = if let Some(filename) = &params.fm_filename {
        create_file(filename)
    } else {
        Ok(Box::new(stdout()) as Box<dyn Write>)
    }?;

    fm::encode(&model, &params.fm_params, &mut writer)
}

fn import_f(
    model: &mut model::Model,
    import: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() < 4 {
        return Err(Error::new(
            MalformedData,
            format!(
                "bad number of vertices in f-statement at line {}",
                import.line
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
                    import.line
                ),
                ));
            }
            if j != 1 || !istr.is_empty() {
                nums[j] = parse_num(whats[j], import.line, i + 1, istr)?;
            }
        }

        face_vertices[i] = (nums[0], nums[1], nums[2]);
    }

    let len = parts.len() - 1;
    for (i, (l, t, n)) in face_vertices[..len - 2].iter().cloned().enumerate() {
        let (l2, t2, n2) = face_vertices[i + 1];
        let (l3, t3, n3) = face_vertices[len - 1];

        add_normal(model, import, l, n)?;
        add_normal(model, import, l2, n2)?;
        add_normal(model, import, l3, n3)?;

        model.elements[0].faces.push(model::Face {
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

fn import_v(
    model: &mut model::Model,
    import: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() < 4 || parts.len() > 5 {
        return Err(Error::new(
            MalformedData,
            format!("malformed v-statement at line {}", import.line),
        ));
    }

    let x = parse_coord("x-coordinate of v-statement", import.line, parts[1])?;
    let y = parse_coord("y-coordinate of v-statement", import.line, parts[2])?;
    let z = parse_coord("z-coordinate of v-statement", import.line, parts[3])?;

    model.states[0].elements[0]
        .vertices
        .push(model::Point3 { x, y, z });

    Ok(())
}

fn import_vn(
    _model: &mut model::Model,
    import: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() != 4 {
        return Err(Error::new(
            MalformedData,
            format!("malformed vn-statement at line {}", import.line),
        ));
    }

    let x = parse_coord("x-coordinate of vn-statement", import.line, parts[1])?;
    let y = parse_coord("y-coordinate of vn-statement", import.line, parts[2])?;
    let z = parse_coord("z-coordinate of vn-statement", import.line, parts[3])?;

    import.normals.push(model::Point3 { x, y, z });

    Ok(())
}

fn import_vt(
    model: &mut model::Model,
    import: &mut ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() < 3 || parts.len() > 4 {
        return Err(Error::new(
            MalformedData,
            format!("malformed vt-statement at line {}", import.line),
        ));
    }

    let x = parse_coord("x-coordinate of vt-statement", import.line, parts[1])?;
    let y = parse_coord("y-coordinate of vt-statement", import.line, parts[2])?;

    model.elements[0]
        .texture_points
        .push(model::Point2 { x, y });

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

fn add_normal(
    model: &mut model::Model,
    import: &mut ImportState,
    vertex: u32,
    normal: u32,
) -> Result<()> {
    let vi = (vertex - 1) as usize;
    if model.states[0].elements[0].vertices.len() <= vi {
        return Err(Error::new(
            MalformedData,
            format!(
                "mention of unknown vertex {} at line {}",
                vertex, import.line
            ),
        ));
    }

    const ZERO: model::Point3 = model::Point3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    let normals = &mut model.states[0].elements[0].normals;
    if normals.len() <= vi {
        normals.resize(vi + 1, ZERO);
    }

    let ni = (normal - 1) as usize;
    if normals[vi] == ZERO {
        normals[vi] = import.normals[ni].clone();
    } else if normals[vi] != import.normals[ni] {
        return Err(Error::new(
            MalformedData,
            format!(
                "more than one normal for vertex {} at line {}",
                vertex, import.line
            ),
        ));
    }

    Ok(())
}

fn import_mtl(
    params: &ImportObjParams,
    model: &mut model::Model,
    import: &mut ImportState,
) -> Result<()> {
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

    import.line = 0;
    import.mtl_dir = path.parent().unwrap().to_path_buf();

    for line_res in BufReader::new(reader).lines() {
        if let Ok(line) = line_res {
            import.line += 1;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if !parts.is_empty() {
                match parts[0] {
                    "map_Ka" => import_map_ka(model, import, &parts)?,
                    _ => (),
                }
            }
        }
    }

    Ok(())
}

fn import_map_ka(
    model: &mut model::Model,
    import: &ImportState,
    parts: &Vec<&str>,
) -> Result<()> {
    if parts.len() != 2 {
        return Err(Error::new(
            MalformedData,
            format!("malformed map_Ka-statement at line {}", import.line),
        ));
    }

    let path = import.mtl_dir.join(parts[1]);

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
                    import.line
                ),
            ));
        }
    };

    let texture = model::Image {
        r#type: image_type as i32,
        data: read_file(path)?,
    };
    model.elements[0].texture = Some(texture);

    Ok(())
}
