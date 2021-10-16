use std::collections::BTreeMap;
use std::f64::INFINITY;

use kdtree::distance::squared_euclidean;
use kdtree::KdTree;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use structopt::StructOpt;

use base::fm;
use base::fm::scan_frame::DepthConfidence;

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
        default_value = "inf"
    )]
    pub outlier_distance: f32,

    #[structopt(
        help = "Number of points per frame cloud limit",
        long,
        short = "p"
    )]
    pub max_num_points: Option<usize>,
}

type Point3 = nalgebra::Point3<f64>;
type Vector3 = nalgebra::Vector3<f64>;
type Quaternion = nalgebra::UnitQuaternion<f64>;
type Matrix4 = nalgebra::Matrix4<f64>;

fn fm_point3_to_point3(p: &fm::Point3) -> Point3 {
    Point3::new(p.x as f64, p.y as f64, p.z as f64)
}

pub fn build_point_cloud(
    scan: &fm::Scan,
    frame: &fm::ScanFrame,
    params: &PointCloudParams,
) -> Vec<Point3> {
    let mut points = Vec::new();

    let tan = (scan.camera_angle_of_view as f64 / 2.0).tan();

    let eye =
        fm_point3_to_point3(&scan.camera_initial_position.unwrap_or_default());
    let dir =
        fm_point3_to_point3(&scan.camera_initial_direction.unwrap_or_default());
    let up_rot = Quaternion::from_axis_angle(
        &Vector3::z_axis(),
        scan.camera_up_angle as f64,
    );
    let look_rot =
        Matrix4::look_at_lh(&eye, &dir, &Vector3::new(0.0, 0.0, 1.0));
    let view_rot = look_rot.try_inverse().unwrap() * Matrix4::from(up_rot);

    let camera_angle =
        frame.time as f64 / 1E9 * scan.camera_angular_velocity as f64;
    let time_rot =
        Quaternion::from_axis_angle(&Vector3::z_axis(), camera_angle);

    for i in 0..scan.depth_height {
        for j in 0..scan.depth_width {
            let depth_index = (i * scan.depth_width + j) as usize;
            let confidence = frame.depth_confidences[depth_index];
            if confidence < params.min_depth_confidence as i32 {
                continue;
            }

            let mut depth = frame.depths[depth_index] as f64;
            let depth_width = scan.depth_width as f64;
            let w = j as f64 - depth_width / 2.0;
            let h = i as f64 - scan.depth_height as f64 / 2.0;

            let u = w / (depth_width / 2.0) * tan;
            let v = h / (depth_width / 2.0) * tan;

            // If depth sensor measures distance rather than depth.
            if !scan.sensor_plane_depth {
                depth /= (1.0 + u * u + v * v).sqrt();
            }

            let focus_to_object =
                depth * nalgebra::Vector4::new(u, v, 1.0, 0.0);

            let point = (view_rot * focus_to_object).xyz() + eye.coords;
            let point = time_rot * point;

            let z_dist = (point[0] * point[0] + point[1] * point[1]).sqrt();
            if point[2] < params.min_z as f64
                || point[2] > params.max_z as f64
                || z_dist > params.max_z_distance as f64
            {
                continue;
            }

            points.push(Point3::from(point));
        }
    }

    if let Some(max_num_points) = params.max_num_points {
        select_random_points(&mut points, max_num_points);
    }

    remove_outliers(&mut points, params.outlier_distance as f64);

    points
}

pub fn build_frame_clouds(
    scans: &BTreeMap<String, fm::Scan>,
    scan_frames: &Vec<fm::ScanFrame>,
    params: &PointCloudParams,
) -> Vec<Vec<Point3>> {
    let mut clouds = Vec::new();
    for frame in scan_frames {
        let scan = scans.get(&frame.scan).unwrap();
        clouds.push(build_point_cloud(&scan, frame, params))
    }
    clouds
}

pub fn distance_between_point_clouds(
    a: &[Point3],
    b: &[Point3],
) -> Option<f64> {
    let mut kdtree = KdTree::with_capacity(3, a.len());
    for p in a {
        kdtree.add(p.coords.as_ref(), INFINITY).unwrap();
    }

    for p in b {
        let mut nearest = kdtree
            .iter_nearest_mut(p.coords.as_ref(), &squared_euclidean)
            .unwrap();
        let (dist, min) = nearest.next().unwrap();
        if dist < *min {
            *min = dist;
        }
    }

    let mut dists = Vec::with_capacity(a.len());

    let all = kdtree
        .iter_nearest(&[0.0, 0.0, 0.0], &squared_euclidean)
        .unwrap();
    for (_, min) in all {
        if min.is_finite() {
            dists.push(min.sqrt());
        }
    }

    // Discard 5% of biggest contributors (probable outliers).
    dists.sort_by(|a, b| a.partial_cmp(b).unwrap());
    dists.resize(dists.len() * 95 / 100, 0.0);

    if dists.is_empty() {
        None
    } else {
        Some(dists.iter().sum::<f64>() / dists.len() as f64)
    }
}

fn remove_outliers(points: &mut Vec<Point3>, distance: f64) {
    if distance.is_infinite() || points.len() < 2 {
        return;
    }

    let mut kdtree = KdTree::with_capacity(3, points.len());
    for point in points.iter() {
        kdtree.add(*point.coords.as_ref(), ()).unwrap();
    }

    let distance_squared = distance * distance;

    let mut j = 0;
    for i in 0..points.len() {
        let mut nearest = kdtree
            .iter_nearest(points[i].coords.as_ref(), &squared_euclidean)
            .unwrap();
        let _ = nearest.next(); // Skip itself.
        if nearest.next().unwrap().0 <= distance_squared {
            points.swap(i, j);
            j += 1;
        }
    }

    points.resize(j, Point3::origin());
}

fn select_random_points(points: &mut Vec<Point3>, num: usize) {
    if num >= points.len() {
        return;
    }

    // Use deterministic generator to maintain consistency while optimizing.
    let mut rng = StdRng::seed_from_u64(0);

    for i in 0..num {
        let j = rng.gen_range(i..points.len());
        points.swap(i, j);
    }

    points.resize(num, Point3::origin());
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_remove_outliers() {
        let mut points = vec![
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(7.0, 0.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
        ];
        remove_outliers(&mut points, 1.0);
        assert_eq!(points.len(), 3);
        assert_eq!(points[0], Point3::new(1.0, 0.0, 0.0));
        assert_eq!(points[1], Point3::new(2.0, 0.0, 0.0));
        assert_eq!(points[2], Point3::new(3.0, 0.0, 0.0));
    }
}
