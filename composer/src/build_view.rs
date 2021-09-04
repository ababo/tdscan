use std::collections::HashMap;
use std::f32::consts::PI;
use std::io::{stdin, stdout};
use std::path::PathBuf;

use glam::{Quat, Vec3};
use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::fm::scan_frame::DepthConfidence;
use base::util::cli::{parse_key_val, Array as CliArray};
use base::util::fs;
use base::util::glam::{point3_to_vec3, vec3_to_point3};

#[derive(StructOpt)]
#[structopt(about = "Build element view from scan .fm file")]
pub struct BuildViewParams {
    #[structopt(help = "Input scan .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,
    #[structopt(
        help = "Camera initial position to override with",
        long = "camera-initial-position",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "y"
    )]
    camera_initial_positions: Vec<(String, CliArray<f32, 3>)>,
    #[structopt(
        help = "Camera view elevation to override with",
        long = "camera-view-elevation",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "e"
    )]
    camera_view_elevations: Vec<(String, f32)>,
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

    let camera_initial_positions = params
        .camera_initial_positions
        .iter()
        .map(|d| (d.0.clone(), d.1 .0))
        .collect();
    let camera_view_elevations =
        params.camera_view_elevations.iter().cloned().collect();

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
        &camera_initial_positions,
        &camera_view_elevations,
        params.min_depth_confidence,
        params.min_z,
        params.max_z,
        params.max_z_distance,
        writer.as_mut(),
    )
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    camera_initial_positions: &HashMap<String, [f32; 3]>,
    camera_view_elevations: &HashMap<String, f32>,
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

        let unknown_scan_err = |name| {
            let desc = format!(
                "unknown scan '{}' for camera initial position override",
                name
            );
            return Err(Error::new(InconsistentState, desc));
        };

        for (name, eye) in camera_initial_positions {
            if let Some(scan) = scans.get_mut(name) {
                scan.camera_initial_position =
                    Some(vec3_to_point3(&Vec3::from(*eye)));
            } else {
                return unknown_scan_err(name);
            }
        }

        for (name, elev) in camera_view_elevations {
            if let Some(scan) = scans.get_mut(name) {
                scan.camera_view_elevation = *elev;
            } else {
                return unknown_scan_err(name);
            }
        }
    }

    let points = build_point_cloud(
        min_depth_confidence,
        min_z,
        max_z,
        max_z_distance,
        &scans,
        &scan_frames,
    );

    use std::io::Write;
    let mut file =
        std::fs::File::create("/Users/ababo/Desktop/foo.obj").unwrap();
    for p in points {
        file.write_all(
            format!("v {} {} {}\n", p[0], p[1], p[2])
                .into_bytes()
                .as_slice(),
        )
        .unwrap();
    }

    Ok(())
}

fn build_point_cloud(
    min_depth_confidence: DepthConfidence,
    min_z: f32,
    max_z: f32,
    max_z_distance: f32,
    scans: &HashMap<String, fm::Scan>,
    scan_frames: &Vec<fm::ScanFrame>,
) -> Vec<Vec3> {
    let mut points = Vec::new();
    let time_base = scan_frames.first().map(|f| f.time).unwrap_or_default();

    for frame in scan_frames {
        let scan = scans.get(&frame.scan).unwrap();

        let tan = (scan.camera_angle_of_view / 2.0).tan();

        let landscape_rot = Quat::from_rotation_z(scan.camera_landscape_angle);
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

        let timestamp = (frame.time - time_base) as f32 / 1E9;
        let camera_angle = timestamp * scan.camera_angular_velocity;
        let time_rot = Quat::from_rotation_z(camera_angle);

        for i in 0..scan.depth_height {
            for j in 0..scan.depth_width {
                let depth_index = (i * scan.depth_width + j) as usize;
                let confidence = frame.depth_confidences[depth_index];
                if confidence < min_depth_confidence as i32 {
                    continue;
                }

                let mut depth = frame.depths[depth_index];
                let depth_width = scan.depth_width as f32;
                let w = j as f32 - depth_width / 2.0;
                let h = i as f32 - scan.depth_height as f32 / 2.0;
                let proj_square = w * w + h + h;
                if scan.sensor_plane_depth {
                    let fl = depth_width / tan / 2.0;
                    depth /= (proj_square.sqrt() / fl).atan().cos();
                }

                let denom = (depth_width * depth_width
                    + 4.0 * proj_square * tan * tan)
                    .sqrt();
                let xy_factor = (2.0 * depth * tan) / denom;
                let (x, y) = (w * xy_factor, h * xy_factor);
                let z = depth * depth_width / denom;

                let point = rot.mul_vec3(Vec3::new(x, y, z)) + look + elev;
                let point = time_rot.mul_vec3(point);

                let z_dist = (point[0] * point[0] + point[1] * point[1]).sqrt();
                if point[2] < min_z
                    || point[2] > max_z
                    || z_dist > max_z_distance
                {
                    continue;
                }

                points.push(point);
            }
        }
    }

    points
}
