use std::f64::INFINITY;

use indexmap::IndexMap;
use kiddo::distance::squared_euclidean;
use kiddo::KdTree;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;
use structopt::StructOpt;

use crate::misc::select_random;
use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::fm::scan_frame::DepthConfidence;
use base::util::cli::parse_key_val;

#[derive(Clone, StructOpt)]
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
        help = "Per-scan minimum point Z-coordinate",
        long = "scan-min-z",
        number_of_values = 1,
        parse(try_from_str = parse_key_val),
    )]
    pub scan_min_zs: Vec<(String, f32)>,

    #[structopt(
        help = "Maximum point Z-coordinate",
        long,
        short = "t",
        default_value = "inf"
    )]
    pub max_z: f32,

    #[structopt(
        help = "Per-scan maximum point Z-coordinate",
        long = "scan-max-z",
        number_of_values = 1,
        parse(try_from_str = parse_key_val),
    )]
    pub scan_max_zs: Vec<(String, f32)>,

    #[structopt(
        help = "Maximum point distance from Z axis",
        long,
        short = "d",
        default_value = "inf"
    )]
    pub max_z_distance: f32,

    #[structopt(
        help = "Number of neighbors for outlier removal",
        long,
        default_value = "20"
    )]
    pub outlier_num_neighbors: usize,

    #[structopt(
        help = "Standard deviation ratio for outlier removal",
        long,
        short = "u",
        default_value = "inf"
    )]
    pub outlier_std_ratio: f32,

    #[structopt(
        help = "Maximum number of points per frame",
        long,
        short = "p"
    )]
    pub max_num_frame_points: Option<usize>,
}

impl PointCloudParams {
    pub fn min_z(&self, scan: &str) -> f32 {
        self.scan_min_zs
            .iter()
            .find(|(s, _)| s == scan)
            .map(|p| p.1)
            .unwrap_or(self.min_z)
    }

    pub fn max_z(&self, scan: &str) -> f32 {
        self.scan_max_zs
            .iter()
            .find(|(s, _)| s == scan)
            .map(|p| p.1)
            .unwrap_or(self.max_z)
    }

    pub fn validate<'a, S>(&self, scans: S) -> Result<()>
    where
        S: Iterator<Item = &'a str> + Clone,
    {
        let validate = |zs: &Vec<_>| {
            let missing =
                zs.iter().find(|(s, _)| !scans.clone().any(|n| n == s));
            if let Some((name, _)) = missing {
                Err(Error::new(
                    InconsistentState,
                    format!("unknown scan '{}' specified", name),
                ))
            } else {
                Ok(())
            }
        };

        validate(&self.scan_min_zs)?;
        validate(&self.scan_max_zs)?;

        Ok(())
    }
}

pub type Point3 = nalgebra::Point3<f64>;
pub type Matrix4 = nalgebra::Matrix4<f64>;
pub type Vector3 = nalgebra::Vector3<f64>;
pub type Vector4 = nalgebra::Vector4<f64>;
type Quaternion = nalgebra::UnitQuaternion<f64>;

fn fm_point3_to_point3(p: &fm::Point3) -> Point3 {
    Point3::new(p.x as f64, p.y as f64, p.z as f64)
}

#[derive(Clone, Copy)]
pub struct PointNormal(pub Point3, pub Vector3);

pub fn build_point_cloud(
    scan: &fm::Scan,
    frame: &fm::ScanFrame,
    params: &PointCloudParams,
) -> Vec<PointNormal> {
    if frame.depths.is_empty() {
        return vec![];
    }

    let depth_width = scan.depth_width as usize;
    let depth_height = scan.depth_height as usize;

    // Normal calculation is based on deltas.
    if depth_width < 2 || depth_height < 2 {
        return vec![];
    }

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
        Matrix4::look_at_rh(&eye, &dir, &Vector3::new(0.0, 0.0, 1.0));
    let view_rot = look_rot.try_inverse().unwrap() * Matrix4::from(up_rot);

    let camera_angle =
        frame.time as f64 / 1E9 * scan.camera_angular_velocity as f64;
    let time_rot =
        Quaternion::from_axis_angle(&Vector3::z_axis(), camera_angle);

    let (min_z, max_z) = (params.min_z(&scan.name), params.max_z(&scan.name));

    let mut points = Vec::with_capacity(depth_height * depth_width);
    for i in 0..depth_height {
        for j in 0..depth_width {
            let depth_index = i * depth_width + j;
            let mut depth = frame.depths[depth_index] as f64;
            let depth_width_f64 = depth_width as f64;
            let w = j as f64 - depth_width_f64 / 2.0;
            let h = i as f64 - scan.depth_height as f64 / 2.0;

            let u = w / (depth_width_f64 / 2.0) * tan;
            let v = h / (depth_width_f64 / 2.0) * tan;

            // If depth sensor measures distance rather than depth.
            if !scan.sensor_plane_depth {
                depth /= (1.0 + u * u + v * v).sqrt();
            }

            let focus_to_object = depth * Vector4::new(u, -v, -1.0, 0.0);
            let point = (view_rot * focus_to_object).xyz() + eye.coords;
            points.push(Point3::from(time_rot * point));
        }
    }

    let mut point_normals = Vec::new();
    for i in 0..depth_height {
        for j in 0..depth_width {
            let depth_index = i * depth_width + j;
            let depth = frame.depths[depth_index];
            if depth.is_nan() || depth.is_infinite() {
                continue;
            }

            let confidence = frame.depth_confidences[depth_index];
            if confidence < params.min_depth_confidence as i32 {
                continue;
            }

            let point = points[depth_index];
            if !validate_point_bounds(
                &point,
                min_z,
                max_z,
                params.max_z_distance,
            ) {
                continue;
            }

            let vdiff = if i > 0 {
                points[depth_index] - points[depth_index - depth_width]
            } else {
                points[depth_index + depth_width] - points[depth_index]
            };
            let hdiff = if j > 0 {
                points[depth_index] - points[depth_index - 1]
            } else {
                points[depth_index + 1] - points[depth_index]
            };
            let normal = vdiff.cross(&hdiff).normalize();
            if !normal.iter().cloned().any(f64::is_nan) {
                point_normals.push(PointNormal(point, normal));
            }
        }
    }

    point_normals
}

pub fn build_frame_clouds(
    scans: &IndexMap<String, fm::Scan>,
    scan_frames: &[fm::ScanFrame],
    params: &PointCloudParams,
) -> Vec<Vec<PointNormal>> {
    let mut clouds = Vec::new();
    for frame in scan_frames {
        let scan = scans.get(&frame.scan).unwrap();
        clouds.push(build_point_cloud(scan, frame, params))
    }

    if let Some(max_num_frame_points) = params.max_num_frame_points {
        select_random_points(&mut clouds, max_num_frame_points);
    }

    remove_outliers(
        &mut clouds,
        params.outlier_num_neighbors,
        params.outlier_std_ratio as f64,
    );

    clouds
}

pub fn distance_between_point_clouds(
    a: &[PointNormal],
    b: &[PointNormal],
) -> Option<f64> {
    if a.is_empty() || b.is_empty() {
        return None;
    }

    let mut kdtree = KdTree::new();
    for (i, p) in a.iter().enumerate() {
        kdtree.add(p.0.coords.as_ref(), i).unwrap();
    }

    let mut dists = vec![INFINITY; a.len()];
    for p in b {
        let (dist, i) = kdtree
            .nearest(p.0.coords.as_ref(), 1, &squared_euclidean)
            .unwrap()[0];
        if dist < dists[*i] {
            dists[*i] = dist;
        }
    }

    // Filter out infinities.
    let mut j = 0;
    for i in 0..dists.len() {
        if dists[i].is_finite() {
            dists.swap(i, j);
            j += 1;
        }
    }
    dists.truncate(j);

    // Discard 5% of biggest contributors (probable outliers).
    dists.sort_by(|a, b| a.partial_cmp(b).unwrap());
    dists.truncate(dists.len() * 95 / 100);

    if dists.is_empty() {
        None
    } else {
        let sum = dists.iter().map(|d| d.sqrt()).sum::<f64>();
        Some(sum / dists.len() as f64)
    }
}

fn remove_outliers(
    clouds: &mut [Vec<PointNormal>],
    num_neighbors: usize,
    std_ratio: f64,
) {
    if std_ratio.is_infinite() {
        return;
    }

    let num_points = clouds.iter().map(Vec::len).sum::<usize>();
    if num_points < 1 + num_neighbors {
        return;
    }

    let mut kdtree = KdTree::with_capacity(200).unwrap();
    for point in clouds.iter().flatten() {
        kdtree.add(point.0.coords.as_ref(), ()).unwrap();
    }

    let local_deviation = |point: &PointNormal| {
        let nearest = kdtree
            .nearest(
                point.0.coords.as_ref(),
                1 + num_neighbors,
                &squared_euclidean,
            )
            .unwrap();

        nearest.iter().map(|p| p.0.sqrt()).sum::<f64>() / num_neighbors as f64
    };
    let avgs: Vec<f64> = (0..clouds.len())
        .into_par_iter()
        .map(|i| clouds[i].iter().map(local_deviation).collect::<Vec<f64>>())
        .flatten()
        .collect();

    let avg = avgs.iter().sum::<f64>() / avgs.len() as f64;
    let std = (avgs.iter().map(|d| (d - avg) * (d - avg)).sum::<f64>()
        / (num_points - 1) as f64)
        .sqrt();

    let mut k = 0;
    let threshold = std_ratio * std;
    for points in clouds.iter_mut() {
        let mut j = 0;
        for i in 0..points.len() {
            if avgs[k] < avg + threshold {
                points.swap(i, j);
                j += 1;
            }
            k += 1;
        }
        points.truncate(j);
    }
}

fn select_random_points(
    clouds: &mut [Vec<PointNormal>],
    max_num_frame_points: usize,
) {
    // Use deterministic generator to maintain consistency while optimizing.
    let mut rng = StdRng::seed_from_u64(0);

    for points in clouds.iter_mut() {
        select_random(points, max_num_frame_points, &mut rng);
    }
}

#[inline]
pub fn validate_point_bounds(
    point: &Point3,
    min_z: f32,
    max_z: f32,
    max_z_distance: f32,
) -> bool {
    point.z >= min_z as f64
        && point.z <= max_z as f64
        && (point.x * point.x + point.y * point.y).sqrt()
            <= max_z_distance as f64
}

#[cfg(test)]
mod test {
    use super::*;

    use base::assert_approx_eq;

    fn new_point_normal(x: f64, y: f64, z: f64) -> PointNormal {
        PointNormal(Point3::new(x, y, z), Vector3::new(0.0, 0.0, 0.0))
    }

    #[test]
    fn test_distance_between_point_clouds() {
        assert_eq!(distance_between_point_clouds(&vec![], &vec![]), None);

        let a = vec![
            new_point_normal(1.0, 0.0, 0.0),
            new_point_normal(5.0, 0.0, 0.0),
            new_point_normal(9.0, 0.0, 0.0),
            new_point_normal(15.0, 0.0, 0.0),
        ];
        let b = vec![
            new_point_normal(6.0, 0.0, 0.0),
            new_point_normal(10.0, 0.0, 0.0),
            new_point_normal(21.0, 0.0, 0.0),
        ];
        assert_approx_eq!(distance_between_point_clouds(&a, &b).unwrap(), 1.0);
    }
}
