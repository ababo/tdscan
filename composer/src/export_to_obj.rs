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

        export_to_obj(
            reader.as_mut(),
            &mut writer,
            |p, d| fs::write_file(p, d),
            mtl_dir,
            self.skip_texture,
        )
    }
}

pub fn export_to_obj<F: Fn(&Path, &[u8]) -> Result<()>>(
    reader: &mut dyn fm::Read,
    writer: &mut dyn io::Write,
    write_file: F,
    mtl_dir: &Path,
    skip_texture: bool,
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

    let textured = view.texture.is_some() && !skip_texture;
    if textured {
        use fm::image::Type::*;
        let ext = match view.texture.as_ref().unwrap().r#type() {
            Png => "png",
            Jpeg => "jpg",
            None => panic!("unsupported texture image type"),
        };
        let txr_filename = mtl_dir.join(&view.element).with_extension(ext);
        write_file(&txr_filename, &view.texture.unwrap().data)?;

        let mtl_filename = mtl_dir.join(&view.element).with_extension("mtl");
        let mut mtl_content = format!("newmtl {}\n", &view.element);
        mtl_content += format!("map_Ka {}.{}\n", &view.element, ext).as_str();
        mtl_content += format!("map_Kd {}.{}\n", &view.element, ext).as_str();
        write_file(&mtl_filename, mtl_content.as_bytes())?;

        writeln!(writer, "mtllib {}.mtl", &view.element)
            .into_result(write_err)?;
        writeln!(writer, "usemtl {}", &view.element).into_result(write_err)?;

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
            if p == &PathBuf::from("/some/path/element.mtl") {
                assert_eq!(
                    str::from_utf8(d).unwrap(),
                    "newmtl element\nmap_Ka element.jpg\nmap_Kd element.jpg\n"
                );
            } else if p == &PathBuf::from("/some/path/element.jpg") {
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
            write_file,
            &PathBuf::from("/some/path"),
            false,
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
mtllib element.mtl
usemtl element
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

        let write_file = |_p: &Path, _d: &[u8]| {
            panic!("unexpected write_file call");
        };

        let mut writer = Vec::new();
        export_to_obj(
            &mut reader,
            &mut writer,
            write_file,
            &PathBuf::from("/some/path"),
            true,
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
f 1//1 2//2 3//3
f 1//1 2//2 4//4
"#;
        assert_eq!(str::from_utf8(&writer).unwrap(), expected);
    }
}
