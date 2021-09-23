use std::collections::BTreeMap;
use std::f32::consts::PI;

use glam::{Quat, Vec3};
use structopt::StructOpt;

use base::fm;
use base::fm::scan_frame::DepthConfidence;
use base::util::glam::point3_to_vec3;

#[derive(Clone, Copy, StructOpt)]
pub struct PointCloudParams {
    #[structopt(
        help = "Minimum depth confidence",
        long,
        default_value = "high"
    )]
    pub min_depth_confidence: DepthConfidence,

    #[structopt(
        help = "Minimum point Z-coordinate",
        long,
        short = "b",
        default_value = "-inf"
    )]
    pub min_z: f32,

    #[structopt(
        help = "Maximum point Z-coordinate",
        long,
        short = "t",
        default_value = "inf"
    )]
    pub max_z: f32,

    #[structopt(
        help = "Maximum point distance from Z axis",
        long,
        short = "d",
        default_value = "inf"
    )]
    pub max_z_distance: f32,
}

pub fn build_point_cloud(
    scans: &BTreeMap<String, fm::Scan>,
    scan_frames: &Vec<fm::ScanFrame>,
    params: &PointCloudParams,
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
                if confidence < params.min_depth_confidence as i32 {
                    continue;
                }

                let mut depth = frame.depths[depth_index];
                let depth_width = scan.depth_width as f32;
                let w = j as f32 - depth_width / 2.0;
                let h = i as f32 - scan.depth_height as f32 / 2.0;
                let proj_square = w * w + h * h;
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
                if point[2] < params.min_z
                    || point[2] > params.max_z
                    || z_dist > params.max_z_distance
                {
                    continue;
                }

                points.push(point);
            }
        }
    }
    eprint!("Number of points {}\n", points.len());
    points
}
