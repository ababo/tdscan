use std::cmp::{Eq, Ord, Ordering, Ordering::*, PartialEq, PartialOrd};
use std::collections::HashMap;
use std::io::stdout;
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::str::FromStr;

use glam::{EulerRot, Quat};
use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::model;
use base::util::cli::parse_key_val;
use base::util::fs;
use base::util::glam::{point3_to_vec3, vec3_to_point3};

#[derive(Default, Clone, Copy)]
pub struct Displacement {
    #[allow(dead_code)]
    dx: f32,
    #[allow(dead_code)]
    dy: f32,
    #[allow(dead_code)]
    dz: f32,
}

impl FromStr for Displacement {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let malformed_err = || {
            let desc = format!("malformed displacement '{}'", s);
            Error::new(MalformedData, desc)
        };

        let parse = |iter: &mut std::str::Split<&str>| {
            let part = iter.next().ok_or_else(|| malformed_err())?;
            if part.is_empty() {
                Ok(0.0)
            } else {
                part.parse::<f32>().or_else(|_| Err(malformed_err()))
            }
        };

        let mut iter = s.split(",");
        let dx = parse(&mut iter)?;
        let dy = parse(&mut iter)?;
        let dz = parse(&mut iter)?;

        if iter.next().is_some() {
            return Err(malformed_err());
        }

        Ok(Displacement { dx, dy, dz })
    }
}

#[derive(Default, Clone, Copy)]
pub struct Rotation {
    #[allow(dead_code)]
    around_x: f32,
    #[allow(dead_code)]
    around_y: f32,
    #[allow(dead_code)]
    around_z: f32,
}

impl FromStr for Rotation {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let malformed_err = || {
            let desc = format!("malformed rotation '{}'", s);
            Error::new(MalformedData, desc)
        };

        let parse = |iter: &mut std::str::Split<&str>| {
            let part = iter.next().ok_or_else(|| malformed_err())?;
            if part.is_empty() {
                Ok(0.0)
            } else {
                part.parse::<f32>().or_else(|_| Err(malformed_err()))
            }
        };

        let mut iter = s.split(",");
        let around_x = parse(&mut iter)?;
        let around_y = parse(&mut iter)?;
        let around_z = parse(&mut iter)?;

        if iter.next().is_some() {
            return Err(malformed_err());
        }

        Ok(Rotation {
            around_x,
            around_y,
            around_z,
        })
    }
}

#[derive(StructOpt)]
#[structopt(about = "Combine multiple .fm files")]
pub struct CombineParams {
    #[structopt(help = "Input .fm files")]
    in_paths: Vec<PathBuf>,
    #[structopt(help="Element displacement in form 'element=dx,dy,dz'",
            long = "displacement",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "d"
        )]
    displacements: Vec<(String, Displacement)>,
    #[structopt(
            help=concat!("Element rotation in form ",
                "'element=around_x,around_y,around_z' using radians"),
            long = "rotation",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "r"
        )]
    rotations: Vec<(String, Rotation)>,
    #[structopt(help="Element scaling in form 'element=scale'",
            long = "scaling",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "s"
        )]
    scalings: Vec<(String, f32)>,
    #[structopt(
        help = "Output .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,
    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn combine_with_params(params: &CombineParams) -> Result<()> {
    let mut readers = Vec::<Box<dyn fm::Read>>::new();
    for path in &params.in_paths {
        let file = fs::open_file(path)?;
        readers.push(Box::new(fm::Reader::new(file)?));
    }

    let mut reader_refs: Vec<&mut dyn fm::Read> = Vec::new();
    for reader in &mut readers {
        reader_refs.push(reader.as_mut());
    }

    let displacements = params.displacements.iter().cloned().collect();
    let rotations = params.rotations.iter().cloned().collect();
    let scalings = params.scalings.iter().cloned().collect();

    let mut writer = if let Some(path) = &params.out_path {
        let writer =
            fm::Writer::new(fs::create_file(path)?, &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    } else {
        let writer = fm::Writer::new(stdout(), &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    };

    combine(
        &mut reader_refs,
        &displacements,
        &rotations,
        &scalings,
        writer.as_mut(),
    )
}

pub fn combine(
    readers: &mut [&mut dyn fm::Read],
    displacements: &HashMap<String, Displacement>,
    rotations: &HashMap<String, Rotation>,
    scalings: &HashMap<String, f32>,
    writer: &mut dyn fm::Write,
) -> Result<()> {
    let mut items = Vec::new();
    for reader in readers.iter_mut() {
        items.push(Item(reader.read_record()?));
    }

    loop {
        let (i, _) = items.iter().enumerate().min_by_key(|i| i.1).unwrap();
        let item = &mut items[i]; // How to obtain &mut Item in above?
        if item.0.is_none() {
            break;
        }

        if let Some(model::record::Type::ElementViewState(state)) =
            &mut item.0.as_mut().unwrap().r#type
        {
            if let Some(disp) = displacements.get(&state.element) {
                for i in 0..state.vertices.len() {
                    state.vertices[i].x += disp.dx;
                    state.vertices[i].y += disp.dy;
                    state.vertices[i].z += disp.dz;
                }

                for i in 0..state.normals.len() {
                    state.normals[i].x += disp.dx;
                    state.normals[i].y += disp.dy;
                    state.normals[i].z += disp.dz;
                }
            }

            if let Some(rot) = rotations.get(&state.element) {
                let quat = Quat::from_euler(
                    EulerRot::XYZ,
                    rot.around_x,
                    rot.around_y,
                    rot.around_z,
                );

                for i in 0..state.vertices.len() {
                    state.vertices[i] = vec3_to_point3(
                        &quat.mul_vec3(point3_to_vec3(&state.vertices[i])),
                    );
                }

                for i in 0..state.normals.len() {
                    state.normals[i] = vec3_to_point3(
                        &quat.mul_vec3(point3_to_vec3(&state.normals[i])),
                    );
                }
            }

            if let Some(scale) = scalings.get(&state.element) {
                for i in 0..state.vertices.len() {
                    state.vertices[i].x *= scale;
                    state.vertices[i].y *= scale;
                    state.vertices[i].z *= scale;
                }

                for i in 0..state.normals.len() {
                    state.normals[i].x *= scale;
                    state.normals[i].y *= scale;
                    state.normals[i].z *= scale;
                }
            }
        }

        writer.write_record(item.0.as_ref().unwrap())?;

        items[i] = Item(readers[i].read_record()?);
    }

    Ok(())
}

struct Item(Option<model::Record>);

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        fn cmp_options<'a, T>(
            a: &'a Option<T>,
            b: &'a Option<T>,
        ) -> StdResult<(&'a T, &'a T), Ordering> {
            if a.is_none() {
                Err(if b.is_none() { Equal } else { Greater })
            } else if b.is_none() {
                Err(Less)
            } else {
                Ok((a.as_ref().unwrap(), b.as_ref().unwrap()))
            }
        }

        fn cmp_items(a: &Item, b: &Item) -> StdResult<(), Ordering> {
            let (this, that) = cmp_options(&a.0, &b.0)?;
            let (this, that) = cmp_options(&this.r#type, &that.r#type)?;

            use model::record::Type;
            Err(match this {
                Type::ElementView(_) => match that {
                    Type::ElementViewState(_) => Less,
                    _ => Equal,
                },
                Type::ElementViewState(this) => match that {
                    Type::ElementView(_) => Greater,
                    Type::ElementViewState(that) => this.time.cmp(&that.time),
                },
            })
        }

        cmp_items(self, other).unwrap_err()
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Item {}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::fm::{Read as _, Write as _};
    use base::util::test::{
        new_element_view_rec, new_element_view_state_rec, new_point3,
    };
    use base::{assert_eq_point3, record_variant};
    use model::record::Type::*;

    fn new_displacement(dx: f32, dy: f32, dz: f32) -> Displacement {
        Displacement { dx, dy, dz }
    }

    fn new_rotation(around_x: f32, around_y: f32, around_z: f32) -> Rotation {
        Rotation {
            around_x,
            around_y,
            around_z,
        }
    }

    fn new_simple_element_view_rec(element: &str) -> model::Record {
        new_element_view_rec(model::ElementView {
            element: format!("{}", element),
            ..Default::default()
        })
    }

    fn new_simple_element_view_state_rec(
        element: &str,
        time: i64,
    ) -> model::Record {
        new_element_view_state_rec(model::ElementViewState {
            element: format!("{}", element),
            time: time,
            vertices: vec![new_point3(0.1, 0.2, 0.3)],
            normals: vec![new_point3(0.2, 0.3, 0.4)],
            ..Default::default()
        })
    }

    #[test]
    fn test_combine_sanity() {
        fn new_data(element: &str, time: i64) -> Vec<u8> {
            let mut writer =
                fm::Writer::new(Vec::new(), &fm::WriterParams::default())
                    .unwrap();

            let rec = new_simple_element_view_rec(element);
            writer.write_record(&rec).unwrap();

            let rec = new_simple_element_view_state_rec(element, time);
            writer.write_record(&rec).unwrap();

            let rec = new_simple_element_view_state_rec(element, time + 2);
            writer.write_record(&rec).unwrap();

            writer.into_inner().unwrap()
        }

        let data1 = new_data("e1", 1);
        let data1_slice = &data1[..];
        let mut reader1 = fm::Reader::new(data1_slice).unwrap();

        let data2 = new_data("e2", 2);
        let data2_slice = &data2[..];
        let mut reader2 = fm::Reader::new(data2_slice).unwrap();

        let mut readers: [&mut dyn fm::Read; 2] = [&mut reader1, &mut reader2];

        let mut displacements = HashMap::new();
        displacements.insert(format!("e2"), new_displacement(0.3, 0.4, 0.5));

        let mut rotations = HashMap::new();
        rotations.insert(format!("e1"), new_rotation(0.6, 0.7, 0.8));

        let mut scales = HashMap::new();
        scales.insert(format!("e1"), 2.0);

        let mut writer =
            fm::Writer::new(Vec::new(), &fm::WriterParams::default()).unwrap();
        combine(
            &mut readers[..],
            &displacements,
            &rotations,
            &scales,
            &mut writer,
        )
        .unwrap();

        let data = writer.into_inner().unwrap();
        let data_slice = &data[..];
        let mut reader = fm::Reader::new(data_slice).unwrap();

        let rec = reader.read_record().unwrap().unwrap();
        let view = record_variant!(ElementView, rec);
        assert_eq!(view.element, format!("e1"));

        let rec = reader.read_record().unwrap().unwrap();
        let view = record_variant!(ElementView, rec);
        assert_eq!(view.element, format!("e2"));

        let rec = reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, rec);
        assert_eq!(state.element, format!("e1"));
        assert_eq!(state.time, 1);
        assert_eq!(state.vertices.len(), 1);
        assert_eq_point3!(
            state.vertices[0],
            new_point3(0.27363908, 0.0356109, 0.69559586)
        );
        assert_eq!(state.normals.len(), 1);
        assert_eq_point3!(
            state.normals[0],
            new_point3(0.39932388, 0.18115145, 0.9837299)
        );

        let rec = reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, rec);
        assert_eq!(state.element, format!("e2"));
        assert_eq!(state.time, 2);
        assert_eq!(state.vertices.len(), 1);
        assert_eq_point3!(state.vertices[0], new_point3(0.4, 0.6, 0.8));
        assert_eq!(state.normals.len(), 1);
        assert_eq_point3!(state.normals[0], new_point3(0.5, 0.7, 0.9));

        let rec = reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, rec);
        assert_eq!(state.element, format!("e1"));
        assert_eq!(state.time, 3);
        assert_eq!(state.vertices.len(), 1);
        assert_eq_point3!(state.vertices[0], new_point3(0.2, 0.4, 0.6));
        assert_eq!(state.normals.len(), 1);
        assert_eq_point3!(state.normals[0], new_point3(0.4, 0.6, 0.8));

        let rec = reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, rec);
        assert_eq!(state.element, format!("e2"));
        assert_eq!(state.time, 4);
        assert_eq!(state.vertices.len(), 1);
        assert_eq_point3!(state.vertices[0], new_point3(0.4, 0.6, 0.8));
        assert_eq!(state.normals.len(), 1);
        assert_eq_point3!(state.normals[0], new_point3(0.5, 0.7, 0.9));

        assert!(reader.read_record().unwrap().is_none());
    }
}
