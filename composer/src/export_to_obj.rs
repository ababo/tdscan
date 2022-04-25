use std::io;
use std::path::Path;

use structopt::StructOpt;

use base::define_raw_output;
use base::defs::{Error, ErrorKind::*, IntoResult, Result};
use base::fm;
use base::util::cli;
use base::util::fs;

define_raw_output!(ObjOutput, "obj");

#[derive(StructOpt)]
#[structopt(about = "Export .fm file into OBJ")]
pub struct ExportToObjCommand {
    #[structopt(flatten)]
    input: cli::FmInput,

    #[structopt(flatten)]
    output: ObjOutput,

    #[structopt(help = "Skip texture output", long, short = "g")]
    skip_texture: bool,

    #[structopt(
        help = "Skip texture output",
        long,
        short = "m",
        conflicts_with = "skip-texture"
    )]
    material_name: Option<String>,
}

impl ExportToObjCommand {
    pub fn run(&self) -> Result<()> {
        let mut reader = self.input.get()?;
        let mut writer = self.output.get()?;

        let mtl_dir = self
            .output
            .path
            .as_deref()
            .unwrap_or_else(|| ".".as_ref())
            .parent()
            .unwrap_or_else(|| ".".as_ref());

        let material = if let Some(name) = &self.material_name {
            name.clone()
        } else {
            const DEFAULT: &str = "material";
            self.output
                .path
                .as_deref()
                .unwrap_or_else(|| DEFAULT.as_ref())
                .file_stem()
                .unwrap_or_else(|| DEFAULT.as_ref())
                .to_owned()
                .into_string()
                .unwrap_or_else(|_| DEFAULT.to_string())
        };

        let mtl = if self.skip_texture {
            None
        } else {
            Some(MtlParams {
                dir: mtl_dir,
                name: material.as_str(),
                write_file: |p, d| fs::write_file(p, d),
            })
        };

        export_to_obj(reader.as_mut(), &mut writer, mtl)
    }
}

pub struct MtlParams<'a, F: Fn(&Path, &[u8]) -> Result<()>> {
    pub dir: &'a Path,
    pub name: &'a str,
    pub write_file: F,
}

#[allow(dead_code)]
#[allow(clippy::type_complexity)]
pub const NO_MTL: Option<MtlParams<fn(&Path, &[u8]) -> Result<()>>> = None;

// TODO: Rewrite when Rust issue #53667 is resolved.
#[allow(clippy::unnecessary_unwrap)]
pub fn export_to_obj<F: Fn(&Path, &[u8]) -> Result<()>>(
    reader: &mut dyn fm::Read,
    writer: &mut dyn io::Write,
    mtl_params: Option<MtlParams<F>>,
) -> Result<()> {
    let (view, state) = read_element(reader)?;

    let write_err = || "failed to write OBJ-file".to_string();

    for v in state.vertices {
        writeln!(writer, "v {} {} {}", v.x, v.y, v.z).into_result(write_err)?;
    }

    for n in state.normals {
        writeln!(writer, "vn {} {} {}", n.x, n.y, n.z)
            .into_result(write_err)?;
    }

    if view.texture.is_some() && mtl_params.is_some() {
        let mtl = mtl_params.unwrap();
        let ext =
            fm::image_type_extension(view.texture.as_ref().unwrap().r#type());
        let txr_filename = mtl.dir.join(mtl.name).with_extension(ext);
        (mtl.write_file)(&txr_filename, &view.texture.unwrap().data)?;

        let mtl_filename = mtl.dir.join(mtl.name).with_extension("mtl");
        let mut mtl_content = format!("newmtl {}\n", mtl.name);
        mtl_content += format!("map_Ka {}.{}\n", mtl.name, ext).as_str();
        mtl_content += format!("map_Kd {}.{}\n", mtl.name, ext).as_str();
        (mtl.write_file)(&mtl_filename, mtl_content.as_bytes())?;

        writeln!(writer, "mtllib {}.mtl", mtl.name)
            .into_result(write_err)?;
        writeln!(writer, "usemtl {}", mtl.name).into_result(write_err)?;

        for p in view.texture_points {
            writeln!(writer, "vt {} {}", p.x, 1.0 - p.y)
                .into_result(write_err)?;
        }

        for f in view.faces {
            #[rustfmt::skip]
            writeln!(writer, "f {}/{}/{} {}/{}/{} {}/{}/{}",
                f.vertex1, f.texture1, f.normal1,
                f.vertex2, f.texture2, f.normal2,
                f.vertex3, f.texture3, f.normal3,
            ).into_result(write_err)?;
        }
    } else {
        for f in view.faces {
            #[rustfmt::skip]
            writeln!(writer, "f {}//{} {}//{} {}//{}",
                f.vertex1, f.normal1,
                f.vertex2, f.normal2,
                f.vertex3, f.normal3,
            ).into_result(write_err)?;
        }
    }

    Ok(())
}

fn read_element(
    reader: &mut dyn fm::Read,
) -> Result<(fm::ElementView, fm::ElementViewState)> {
    let mut view: Option<fm::ElementView> = None;
    let mut state: Option<fm::ElementViewState> = None;

    loop {
        let rec = reader.read_record()?;
        if rec.is_none() {
            break;
        }

        use fm::record::Type::*;
        match rec.unwrap().r#type {
            Some(ElementView(v)) => {
                if view.is_some() {
                    return Err(Error::new(
                        UnsupportedFeature,
                        "multiple element views are not supported".to_string(),
                    ));
                }
                view = Some(v);
            }
            Some(ElementViewState(s)) => {
                if state.is_some() {
                    return Err(Error::new(
                        UnsupportedFeature,
                        "multiple element view states are not supported"
                            .to_string(),
                    ));
                }
                if view.is_none() || s.element != view.as_ref().unwrap().element
                {
                    return Err(Error::new(
                        InconsistentState,
                        format!("unknown view state element {}", s.element),
                    ));
                }
                state = Some(s);
            }
            _ => {}
        }
    }

    if state.is_none() {
        return Err(Error::new(
            InconsistentState,
            "missing element state".to_string(),
        ));
    }

    Ok((view.unwrap(), state.unwrap()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::str;

    use super::*;
    use base::util::test::*;

    #[test]
    fn test_read_missing_element_state() {
        let mut reader =
            create_reader_with_records(&vec![new_element_view_rec(
                fm::ElementView {
                    element: "element".to_string(),
                    ..Default::default()
                },
            )]);
        let res = read_element(&mut reader);
        let err = res.unwrap_err();
        assert_eq!(err.kind, InconsistentState);
        assert_eq!(&err.description, "missing element state");
    }

    #[test]
    fn test_read_multiple_element_views() {
        let mut reader = create_reader_with_records(&vec![
            new_element_view_rec(fm::ElementView {
                element: "element".to_string(),
                ..Default::default()
            }),
            new_element_view_rec(fm::ElementView {
                element: "element2".to_string(),
                ..Default::default()
            }),
        ]);
        let res = read_element(&mut reader);
        let err = res.err().unwrap();
        assert_eq!(err.kind, UnsupportedFeature);
        assert_eq!(
            &err.description,
            "multiple element views are not supported"
        );
    }

    #[test]
    fn test_read_multiple_element_view_states() {
        let mut reader = create_reader_with_records(&vec![
            new_element_view_rec(fm::ElementView {
                element: "element".to_string(),
                ..Default::default()
            }),
            new_element_view_state_rec(fm::ElementViewState {
                element: "element".to_string(),
                ..Default::default()
            }),
            new_element_view_state_rec(fm::ElementViewState {
                element: "element".to_string(),
                ..Default::default()
            }),
        ]);
        let res = read_element(&mut reader);
        let err = res.err().unwrap();
        assert_eq!(err.kind, UnsupportedFeature);
        assert_eq!(
            &err.description,
            "multiple element view states are not supported"
        );
    }

    #[test]
    fn test_read_unknown_view_state_element() {
        let mut reader =
            create_reader_with_records(&vec![new_element_view_state_rec(
                fm::ElementViewState {
                    element: "element".to_string(),
                    ..Default::default()
                },
            )]);
        let res = read_element(&mut reader);
        let err = res.err().unwrap();
        assert_eq!(err.kind, InconsistentState);
        assert_eq!(&err.description, "unknown view state element element");
    }

    fn create_element() -> fm::Reader<io::Cursor<Vec<u8>>> {
        create_reader_with_records(&vec![
            new_element_view_rec(fm::ElementView {
                element: "element".to_string(),
                texture_points: vec![
                    new_point2(1.0, 2.0),
                    new_point2(3.0, 4.0),
                    new_point2(5.0, 6.0),
                    new_point2(7.0, 8.0),
                ],
                #[cfg_attr(rustfmt, rustfmt_skip)]
                faces: vec![
                    new_ev_face(1, 2, 3, 1, 2, 3, 1, 2, 3),
                    new_ev_face(1, 2, 4, 1, 2, 4, 1, 2, 4),
                ],
                texture: Some(fm::Image {
                    r#type: fm::image::Type::Jpeg as i32,
                    data: vec![1, 2, 3],
                }),
                ..Default::default()
            }),
            new_element_view_state_rec(fm::ElementViewState {
                element: "element".to_string(),
                vertices: vec![
                    new_point3(1.0, 2.0, 3.0),
                    new_point3(2.0, 3.0, 4.0),
                    new_point3(3.0, 4.0, 5.0),
                    new_point3(4.0, 5.0, 6.0),
                ],
                normals: vec![
                    new_point3(2.0, 3.0, 4.0),
                    new_point3(3.0, 4.0, 5.0),
                    new_point3(4.0, 5.0, 6.0),
                    new_point3(5.0, 6.0, 7.0),
                ],
                ..Default::default()
            }),
        ])
    }

    #[test]
    fn test_export_textured_element() {
        let mut reader = create_element();

        let write_file = |p: &Path, d: &[u8]| {
            if p == &PathBuf::from("/some/path/abc.mtl") {
                assert_eq!(
                    str::from_utf8(d).unwrap(),
                    "newmtl abc\nmap_Ka abc.jpg\nmap_Kd abc.jpg\n"
                );
            } else if p == &PathBuf::from("/some/path/abc.jpg") {
                assert_eq!(d, &[1, 2, 3]);
            } else {
                panic!("unexpected write_file path");
            }
            Ok(())
        };

        let mut writer = Vec::new();
        export_to_obj(
            &mut reader,
            &mut writer,
            Some(MtlParams {
                dir: &PathBuf::from("/some/path"),
                name: "abc",
                write_file,
            }),
        )
        .unwrap();

        let expected = r#"v 1 2 3
v 2 3 4
v 3 4 5
v 4 5 6
vn 2 3 4
vn 3 4 5
vn 4 5 6
vn 5 6 7
mtllib abc.mtl
usemtl abc
vt 1 -1
vt 3 -3
vt 5 -5
vt 7 -7
f 1/1/1 2/2/2 3/3/3
f 1/1/1 2/2/2 4/4/4
"#;
        assert_eq!(str::from_utf8(&writer).unwrap(), expected);
    }

    #[test]
    fn test_export_non_textured_element() {
        let mut reader = create_element();

        let mut writer = Vec::new();
        export_to_obj(&mut reader, &mut writer, NO_MTL).unwrap();

        let expected = r#"v 1 2 3
v 2 3 4
v 3 4 5
v 4 5 6
vn 2 3 4
vn 3 4 5
vn 4 5 6
vn 5 6 7
f 1//1 2//2 3//3
f 1//1 2//2 4//4
"#;
        assert_eq!(str::from_utf8(&writer).unwrap(), expected);
    }
}
