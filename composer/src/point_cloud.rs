use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::f32::INFINITY;

use glam::{Quat, Vec3};
use kdtree::distance::squared_euclidean;
use kdtree::KdTree;
use nalgebra::DMatrix;
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

pub fn clouds_distance(
    a: &[Vec3],
    b: &[Vec3],
    num_neighbours: usize,
) -> Option<f32> {
    if num_neighbours < 3 {
        return None;
    }

    let mut a_tree = KdTree::with_capacity(3, a.len());
    for (i, p) in a.iter().enumerate() {
        a_tree.add(p.as_ref(), i).unwrap();
    }

    let mut b_tree = KdTree::with_capacity(3, b.len());
    for (i, p) in b.iter().enumerate() {
        b_tree.add(p.as_ref(), i).unwrap();
    }

    let mut neighbours = Vec::with_capacity(num_neighbours);
    let mut max = -INFINITY;

    for a_point in a {
        let mut b_nearest = b_tree
            .iter_nearest(a_point.as_ref(), &squared_euclidean)
            .unwrap();
        let b_point = if let Some(p) = b_nearest.next() {
            b[*p.1]
        } else {
            return None;
        };

        neighbours.clear();
        let mut a_nearest = a_tree
            .iter_nearest(a_point.as_ref(), &squared_euclidean)
            .unwrap();
        let _ = a_nearest.next(); // Skip itself.
        neighbours.extend(a_nearest.map(|p| a[*p.1]).take(num_neighbours));
        let a_plane = if let Some(plane) =
            compute_best_fitting_plane(neighbours.as_slice())
        {
            plane
        } else {
            continue;
        };

        let dist = (*a_point - b_point).dot(a_plane.n).abs();
        if dist > max {
            max = dist;
        }
    }

    if max.is_finite() {
        Some(max)
    } else {
        None
    }
}

// Plane defined in Hessian normal form.
struct Plane {
    n: Vec3,
    #[allow(dead_code)]
    p: f32,
}

fn compute_best_fitting_plane(points: &[Vec3]) -> Option<Plane> {
    let data = points.iter().map(|p| p.as_ref()).flatten().cloned();
    let points = DMatrix::from_iterator(points.len(), 3, data);
    let means = points.row_mean();
    let mut points = points.transpose();
    points.row_mut(0).add_scalar_mut(-means[0]);
    points.row_mut(1).add_scalar_mut(-means[1]);
    points.row_mut(2).add_scalar_mut(-means[2]);
    if let Some(u) = points.svd(true, false).u {
        let normal = u.column(0);
        let normal = Vec3::new(normal[0], normal[1], normal[2]);
        let centroid = Vec3::new(means[0], means[1], means[2]);
        Some(Plane {
            n: normal,
            p: -normal.dot(centroid),
        })
    } else {
        None
    }
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
