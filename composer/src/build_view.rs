use std::collections::HashMap;
use std::path::PathBuf;

use glam::Vec3;
use structopt::StructOpt;

use crate::misc::{
    fm_reader_from_file_or_stdin, fm_writer_to_file_or_stdout, read_scans,
};
use crate::point_cloud::{build_point_clouds, PointCloudParams};
use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::util::cli::{parse_key_val, Array as CliArray};
use base::util::glam::vec3_to_point3;

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
        help = "Camera landscape angle to override with",
        long = "camera-landscape-angle",
            number_of_values = 1,
            parse(try_from_str = parse_key_val),
            short = "l"
    )]
    camera_landscape_angles: Vec<(String, f32)>,

    #[structopt(flatten)]
    point_cloud_params: PointCloudParams,

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
    let mut reader = fm_reader_from_file_or_stdin(&params.in_path)?;

    let camera_initial_positions = params
        .camera_initial_positions
        .iter()
        .map(|d| (d.0.clone(), d.1 .0))
        .collect();
    let camera_landscape_angles =
        params.camera_landscape_angles.iter().cloned().collect();
    let camera_view_elevations =
        params.camera_view_elevations.iter().cloned().collect();

    let mut writer =
        fm_writer_to_file_or_stdout(&params.out_path, &params.fm_write_params)?;

    build_view(
        reader.as_mut(),
        &camera_initial_positions,
        &camera_view_elevations,
        &camera_landscape_angles,
        &params.point_cloud_params,
        writer.as_mut(),
    )
}

pub fn build_view(
    reader: &mut dyn fm::Read,
    camera_initial_positions: &HashMap<String, [f32; 3]>,
    camera_view_elevations: &HashMap<String, f32>,
    camera_landscape_angles: &HashMap<String, f32>,
    point_cloud_params: &PointCloudParams,
    _writer: &mut dyn fm::Write,
) -> Result<()> {
    let (mut scans, scan_frames) = read_scans(reader)?;

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

    for (name, angle) in camera_landscape_angles {
        if let Some(scan) = scans.get_mut(name) {
            scan.camera_landscape_angle = *angle;
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

    let clouds = build_point_clouds(&scans, &scan_frames, point_cloud_params);

    use std::io::Write;
    let mut file =
        std::fs::File::create("/Users/ababo/Desktop/foo.obj").unwrap();
    for cloud in clouds {
        for point in cloud {
            file.write_all(
                format!("v {} {} {}\n", point[0], point[1], point[2])
                    .into_bytes()
                    .as_slice(),
            )
            .unwrap();
        }
    }

    Ok(())
}
