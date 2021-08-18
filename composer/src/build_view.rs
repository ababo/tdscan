use std::collections::HashMap;
use std::f32::consts::PI;
use std::io::{stdin, stdout};
use std::path::PathBuf;

use glam::{Quat, Vec3};
use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::fm::scan_frame::DepthConfidence;
use base::util::fs;
use base::util::glam::{point3_to_vec3, vec3_to_point3};

#[derive(StructOpt)]
#[structopt(about = "Build element view from scan .fm file")]
pub struct BuildViewParams {
    #[structopt(help = "Input scan .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,
    #[structopt(
        help = "Minimum depth confidence",
        long,
        default_value = "high"
    )]
    min_depth_confidence: DepthConfidence,
    #[structopt(
        help = "Minimum point Z-coordinate",
        long,
        short = "b",
        default_value = "-inf"
    )]
    min_z: f32,
    #[structopt(
        help = "Maximum point Z-coordinate",
        long,
        short = "t",
        default_value = "inf"
    )]
    max_z: f32,
    #[structopt(
        help = "Maximum point distance from Z axis",
        long,
        short = "d",
        default_value = "inf"
    )]
    max_z_distance: f32,
    #[structopt(
        help = "Output element view .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,
    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn build_view_with_params(params: &BuildViewParams) -> Result<()> {
    let mut reader = if let Some(path) = &params.in_path {
        let reader = fm::Reader::new(fs::open_file(path)?)?;
        Box::new(reader) as Box<dyn fm::Read>
    } else {
        let reader = fm::Reader::new(stdin())?;
        Box::new(reader) as Box<dyn fm::Read>
    };

    let mut writer = if let Some(path) = &params.out_path {
        let writer =
            fm::Writer::new(fs::create_file(path)?, &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    } else {
        let writer = fm::Writer::new(stdout(), &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    };

    build_view(
        reader.as_mut(),
        params.min_depth_confidence,
        params.min_z,
        params.max_z,
        params.max_z_distance,
        writer.as_mut(),
    )
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    min_depth_confidence: DepthConfidence,
    min_z: f32,
    max_z: f32,
    max_z_distance: f32,
    _writer: &mut dyn fm::Write,
) -> Result<()> {
    let mut scans = HashMap::<String, fm::Scan>::new();
    let mut scan_frames = Vec::<fm::ScanFrame>::new();
    let mut last_time = 0;

    loop {
        let rec = reader.read_record()?;
        if rec.is_none() {
            break;
        }

        use fm::record::Type::*;
        match rec.unwrap().r#type {
            Some(Scan(s)) => {
                if !scan_frames.is_empty() {
                    let desc = format!("scan '{}' after scan frame ", &s.name);
                    return Err(Error::new(InconsistentState, desc));
                }
                scans.insert(s.name.clone(), s);
            }
            Some(ScanFrame(f)) => {
                if !scans.contains_key(&f.scan) {
                    let desc = format!("frame for unknown scan '{}'", &f.scan);
                    return Err(Error::new(InconsistentState, desc));
                }
                if f.time < last_time {
                    let desc = format!(
                        "non-monotonic frame time for scan '{}'",
                        &f.scan
                    );
                    return Err(Error::new(InconsistentState, desc));
                }
                last_time = f.time;
                scan_frames.push(f);
            }
            _ => (),
        }
    }

    let _points = build_point_cloud(
        min_depth_confidence,
        min_z,
        max_z,
        max_z_distance,
        &scans,
        &scan_frames,
    );

    Ok(())
}

fn build_point_cloud(
    min_depth_confidence: DepthConfidence,
    min_z: f32,
    max_z: f32,
    max_z_distance: f32,
    scans: &HashMap<String, fm::Scan>,
    scan_frames: &Vec<fm::ScanFrame>,
) -> Vec<fm::Point3> {
    let mut points = Vec::new();
    // let time_base = scan_frames.first().map(|f| f.time).unwrap_or_default();

    let mut file =
        std::fs::File::create("/Users/ababo/Desktop/foo.obj").unwrap();

    for frame in scan_frames {
        let scan = scans.get(&frame.scan).unwrap();
        let tan = (scan.camera_angle_of_view as f32 / 2.0).tan();
        // let timestamp = (frame.time - time_base) as f32;
        // let camera_angle = timestamp * scan.camera_angular_velocity;

        let landscape_rot = Quat::from_rotation_z(-1.57079632679); //scan.camera_landscape_angle);
        let eye = scan.camera_initial_position.unwrap_or_default();
        let elev = Vec3::new(0.0, 0.0, scan.camera_view_elevation);
        let look = point3_to_vec3(&eye) - elev;
        let look_rot_axis = if look[1] != 0.0 {
            let slope = -look[0] / look[1];
            let x = 1.0 / (1.0 + slope * slope).sqrt();
            let y = slope * x;
            Vec3::new(x, y, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let look_angle =
            (look[2] / (look[0] * look[0] + look[1] * look[1]).sqrt()).atan()
                + PI / 2.0;
        let look_rot = Quat::from_axis_angle(look_rot_axis, look_angle);
        let rot = look_rot.mul_quat(landscape_rot);

        for i in 0..scan.depth_height {
            for j in 0..scan.depth_width {
                let confidence = frame.depth_confidences
                    [(i * scan.depth_width + j) as usize];
                if confidence < min_depth_confidence as i32 {
                    continue;
                }

                let depth = frame.depths[(i * scan.depth_width + j) as usize];
                let w = j as f32 - scan.depth_width as f32 / 2.0;
                let h = i as f32 - scan.depth_height as f32 / 2.0;
                let denom = (scan.depth_width as f32 * scan.depth_width as f32
                    + 4.0 * (h * h + w * w) * tan * tan)
                    .sqrt();
                let x = (2.0 * depth as f32 * w * tan) / denom;
                let y = (2.0 * depth as f32 * h * tan) / denom;
                let z = (depth as f32 * scan.depth_width as f32) / denom;
                let point = rot.mul_vec3(Vec3::new(x, y, z)) + look + elev;

                let z_dist = (point[0] * point[0] + point[1] * point[1]).sqrt();
                if point[2] < min_z
                    || point[2] > max_z
                    || z_dist > max_z_distance
                {
                    continue;
                }

                use std::io::Write;
                file.write_all(
                    format!("v {} {} {}\n", point[0], point[1], point[2])
                        .into_bytes()
                        .as_slice(),
                )
                .unwrap();

                points.push(vec3_to_point3(&point));
            }
        }

        break;
    }

    points
}
