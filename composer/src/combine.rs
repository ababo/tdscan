use std::cmp::{Eq, Ord, Ordering, Ordering::*, PartialEq, PartialOrd};
use std::result::Result as StdResult;

use structopt::StructOpt;

use base::defs::Result;
use base::fm;
use base::util::cli;
use base::util::cli::{parse_key_val, Array as CliArray};

#[derive(StructOpt)]
#[structopt(about = "Combine multiple .fm files")]
pub struct CombineCommand {
    #[structopt(flatten)]
    inputs: cli::FmInputs,

    #[structopt(flatten)]
    output: cli::FmOutput,

    #[structopt(flatten)]
    params: CombineParams,
}

impl CombineCommand {
    pub fn run(&self) -> Result<()> {
        let mut readers = self.inputs.get()?;
        let mut writer = self.output.get()?;

        let mut reader_refs: Vec<&mut dyn fm::Read> = Vec::new();
        for reader in &mut readers {
            reader_refs.push(reader.as_mut());
        }

        combine(&mut reader_refs, writer.as_mut(), &self.params)
    }
}

#[derive(StructOpt)]
pub struct CombineParams {
    #[structopt(
        help="Element displacement in form 'element=dx,dy,dz'",
        long = "displacement",
        number_of_values = 1,
        parse(try_from_str = parse_key_val),
        short = "d"
    )]
    displacements: Vec<(String, CliArray<f32, 3>)>,

    #[structopt(
        help=concat!("Element rotation in form ",
            "'element=around_x,around_y,around_z' using radians"),
        long = "rotation",
        number_of_values = 1,
        parse(try_from_str = parse_key_val),
        short = "r")
    ]
    rotations: Vec<(String, CliArray<f32, 3>)>,

    #[structopt(
        help="Element scaling in form 'element=scale'",
        long = "scaling",
        number_of_values = 1,
        parse(try_from_str = parse_key_val),
        short = "s"
    )]
    scalings: Vec<(String, f32)>,
}

type Point3 = nalgebra::Point3<f32>;
type Quaternion = nalgebra::UnitQuaternion<f32>;
type Vector3 = nalgebra::Vector3<f32>;

pub fn combine(
    readers: &mut [&mut dyn fm::Read],
    writer: &mut dyn fm::Write,
    params: &CombineParams,
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

        if let Some(fm::record::Type::ElementViewState(state)) =
            &mut item.0.as_mut().unwrap().r#type
        {
            if let Some((_, disp)) = params
                .displacements
                .iter()
                .find(|(e, _)| e == &state.element)
            {
                for i in 0..state.vertices.len() {
                    state.vertices[i].x += disp.0[0];
                    state.vertices[i].y += disp.0[1];
                    state.vertices[i].z += disp.0[2];
                }

                for i in 0..state.normals.len() {
                    state.normals[i].x += disp.0[0];
                    state.normals[i].y += disp.0[1];
                    state.normals[i].z += disp.0[2];
                }
            }

            if let Some((_, rot)) =
                params.rotations.iter().find(|(e, _)| e == &state.element)
            {
                let x_quat =
                    Quaternion::from_axis_angle(&Vector3::x_axis(), rot.0[0]);
                let y_quat =
                    Quaternion::from_axis_angle(&Vector3::y_axis(), rot.0[1]);
                let z_quat =
                    Quaternion::from_axis_angle(&Vector3::z_axis(), rot.0[2]);
                let quat = x_quat * y_quat * z_quat;

                for i in 0..state.vertices.len() {
                    let p = state.vertices[i];
                    let p = quat * Point3::new(p.x, p.y, p.z);
                    state.vertices[i] = point3_to_fm_point3(&p);
                }

                for i in 0..state.normals.len() {
                    let p = state.normals[i];
                    let p = quat * Point3::new(p.x, p.y, p.z);
                    state.normals[i] = point3_to_fm_point3(&p);
                }
            }

            if let Some((_, scale)) =
                params.scalings.iter().find(|(e, _)| e == &state.element)
            {
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

fn point3_to_fm_point3(p: &Point3) -> fm::Point3 {
    fm::Point3 {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}

struct Item(Option<fm::Record>);

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

        use fm::record::Type;

        fn type_prio(r#type: &Type) -> i8 {
            match r#type {
                Type::ElementView(_) => 0,
                Type::ElementViewState(_) => 1,
                Type::Scan(_) => 0,
                Type::ScanFrame(_) => 1,
            }
        }

        fn type_time(r#type: &Type) -> fm::Time {
            match r#type {
                Type::ElementViewState(fm::ElementViewState {
                    time, ..
                })
                | Type::ScanFrame(fm::ScanFrame { time, .. }) => *time,
                _ => 0,
            }
        }

        fn cmp_items(a: &Item, b: &Item) -> StdResult<(), Ordering> {
            let (this, that) = cmp_options(&a.0, &b.0)?;
            let (this, that) = cmp_options(&this.r#type, &that.r#type)?;
            let ordering = type_prio(this).cmp(&type_prio(that));
            Err(if ordering == Equal {
                type_time(this).cmp(&type_time(that))
            } else {
                ordering
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
    use base::util::test::*;
    use base::{assert_eq_point3, record_variant};
    use fm::record::Type::*;
    use fm::Read as _;

    fn new_simple_element_view_rec(element: &str) -> fm::Record {
        new_element_view_rec(fm::ElementView {
            element: format!("{}", element),
            ..Default::default()
        })
    }

    fn new_simple_element_view_state_rec(
        element: &str,
        time: i64,
    ) -> fm::Record {
        new_element_view_state_rec(fm::ElementViewState {
            element: format!("{}", element),
            time: time,
            vertices: vec![new_point3(0.1, 0.2, 0.3)],
            normals: vec![new_point3(0.2, 0.3, 0.4)],
            ..Default::default()
        })
    }

    #[test]
    fn test_combine_sanity() {
        let new_reader = |element, time| {
            create_reader_with_records(&vec![
                new_simple_element_view_rec(element),
                new_simple_element_view_state_rec(element, time),
                new_simple_element_view_state_rec(element, time + 2),
            ])
        };

        let mut reader1 = new_reader("e1", 1);
        let mut reader2 = new_reader("e2", 2);
        let mut readers: [&mut dyn fm::Read; 2] = [&mut reader1, &mut reader2];

        let params = &CombineParams {
            displacements: vec![("e2".to_string(), [0.3, 0.4, 0.5].into())],
            rotations: vec![("e1".to_string(), [0.6, 0.7, 0.8].into())],
            scalings: vec![("e1".to_string(), 2.0)],
        };
        let mut writer = create_writer();
        combine(&mut readers[..], &mut writer, &params).unwrap();

        let mut reader = writer_to_reader(writer);

        let rec = reader.read_record().unwrap().unwrap();
        let view = record_variant!(ElementView, rec);
        assert_eq!(view.element.as_str(), "e1");

        let rec = reader.read_record().unwrap().unwrap();
        let view = record_variant!(ElementView, rec);
        assert_eq!(view.element.as_str(), "e2");

        let rec = reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, rec);
        assert_eq!(state.element.as_str(), "e1");
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
        assert_eq!(state.element.as_str(), "e2");
        assert_eq!(state.time, 2);
        assert_eq!(state.vertices.len(), 1);
        assert_eq_point3!(state.vertices[0], new_point3(0.4, 0.6, 0.8));
        assert_eq!(state.normals.len(), 1);
        assert_eq_point3!(state.normals[0], new_point3(0.5, 0.7, 0.9));

        let rec = reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, rec);
        assert_eq!(state.element.as_str(), "e1");
        assert_eq!(state.time, 3);
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
        assert_eq!(state.element.as_str(), "e2");
        assert_eq!(state.time, 4);
        assert_eq!(state.vertices.len(), 1);
        assert_eq_point3!(state.vertices[0], new_point3(0.4, 0.6, 0.8));
        assert_eq!(state.normals.len(), 1);
        assert_eq_point3!(state.normals[0], new_point3(0.5, 0.7, 0.9));

        assert!(reader.read_record().unwrap().is_none());
    }
}
