use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::f32::INFINITY;

use glam::{Quat, Vec3};
use kdtree::distance::squared_euclidean;
use kdtree::KdTree;
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

    #[structopt(
        help = "Minimal distance to consider point as an outlier",
        long,
        short = "u",
        default_value = "0.01"
    )]
    pub outlier_distance: f32,
}

pub fn build_point_cloud(
    scan: &fm::Scan,
    frame: &fm::ScanFrame,
    params: &PointCloudParams,
) -> Vec<Vec3> {
    let mut points = Vec::new();

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
    let look_angle = (look[2] / (look[0] * look[0] + look[1] * look[1]).sqrt())
        .atan()
        + PI / 2.0;
    let look_rot = Quat::from_axis_angle(look_rot_axis, look_angle);
    let rot = look_rot.mul_quat(landscape_rot);

    let camera_angle = frame.time as f32 / 1E9 * scan.camera_angular_velocity;
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

    remove_outliers(&mut points, params.outlier_distance);

    points
}

pub fn clouds_distance(a: &[Vec3], b: &[Vec3]) -> Option<f32> {
    if a.len() == 0 || b.len() == 0 {
        return None;
    }

    let mut kdtree = KdTree::with_capacity(3, a.len());
    for p in a {
        kdtree.add(p.as_ref(), INFINITY).unwrap();
    }

    for p in b {
        let mut nearest = kdtree
            .iter_nearest_mut(p.as_ref(), &squared_euclidean)
            .unwrap();
        let (dist, min) = nearest.next().unwrap();
        if dist < *min {
            *min = dist;
        }
    }

    let mut max = -INFINITY;
    let all = kdtree
        .iter_nearest(&[0.0, 0.0, 0.0], &squared_euclidean)
        .unwrap();
    for (_, min) in all {
        if min.is_finite() && *min > max {
            max = *min;
        }
    }

    Some(max.sqrt())
}

fn remove_outliers(points: &mut Vec<Vec3>, distance: f32) {
    if distance.is_infinite() || points.len() < 2 {
        return;
    }

    let mut kdtree = KdTree::with_capacity(3, points.len());
    for point in points.iter() {
        kdtree.add(*point.as_ref(), ()).unwrap();
    }

    let distance_squared = distance * distance;

    let mut j = 0;
    for i in 0..points.len() {
        let mut nearest = kdtree
            .iter_nearest(points[i].as_ref(), &squared_euclidean)
            .unwrap();
        let _ = nearest.next(); // Skip itself.
        if nearest.next().unwrap().0 <= distance_squared {
            points.swap(i, j);
            j += 1;
        }
    }

    points.resize(j, Vec3::default());
}

pub fn build_point_clouds(
    scans: &BTreeMap<String, fm::Scan>,
    scan_frames: &Vec<fm::ScanFrame>,
    params: &PointCloudParams,
) -> Vec<Vec<Vec3>> {
    let mut clouds = Vec::new();
    for frame in scan_frames {
        let scan = scans.get(&frame.scan).unwrap();
        clouds.push(build_point_cloud(&scan, frame, params))
    }
    clouds
}

#[cfg(test)]
mod test {
    use super::*;

    use base::assert_eq_f32;

    #[test]
    fn test_clouds_distance() {
        assert_eq!(clouds_distance(&vec![], &vec![]), None);

        let a = vec![
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::new(9.0, 0.0, 0.0),
            Vec3::new(15.0, 0.0, 0.0),
        ];
        let b = vec![
            Vec3::new(6.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(21.0, 0.0, 0.0),
        ];
        assert_eq_f32!(clouds_distance(&a, &b).unwrap(), 6.0);
    }

    #[test]
    fn test_remove_outliers() {
        let mut points = vec![
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(7.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
        ];
        remove_outliers(&mut points, 1.0);
        assert_eq!(points.len(), 3);
        assert_eq!(points[0], Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(points[1], Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(points[2], Vec3::new(3.0, 0.0, 0.0));
    }
}
