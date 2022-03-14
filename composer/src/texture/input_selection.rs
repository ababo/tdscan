use indexmap::IndexMap;
use kiddo::distance::squared_euclidean;
use kiddo::KdTree;

use crate::mesh::Mesh;
use crate::texture::*;
use base::fm;

pub fn project_like_camera(
    scan: &fm::Scan,
    frame: &fm::ScanFrame,
    points: &[Point3],
) -> Vec<ProjectedPoint> {
    let tan = (scan.camera_angle_of_view as f64 / 2.0).tan();

    let eye = scan.camera_initial_position.unwrap_or_default();
    let eye = Point3::new(eye.x as f64, eye.y as f64, eye.z as f64);
    let dir = scan.camera_initial_direction.unwrap_or_default();
    let dir = Point3::new(dir.x as f64, dir.y as f64, dir.z as f64);

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

    let view_rot_3x3_inv = view_rot.fixed_slice::<3, 3>(0, 0).transpose();
    let time_rot_3x3_inv = Matrix4::from(time_rot)
        .fixed_slice::<3, 3>(0, 0)
        .transpose();

    let depth_width = scan.depth_width as f64;
    let depth_height = scan.depth_height as f64;

    points
        .iter()
        .map(|point3d| {
            // Undo rigid 3d transformations.
            let frame_real = time_rot_3x3_inv * point3d;
            let frame = view_rot_3x3_inv * (frame_real - eye);

            // Redo camera screen projection.
            let depth = -frame.z;
            let u = frame.x / depth;
            let v = -frame.y / depth;

            // Apply camera field of view.
            let w = u * (depth_width / 2.0) / tan;
            let h = v * (depth_width / 2.0) / tan;

            // Standardize to the interval [0, 1].
            let i = (h + depth_height / 2.0) / depth_height;
            let j = (w + depth_width / 2.0) / depth_width;

            ProjectedPoint {
                point: Vector2::new(i, j),
                depth,
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub pixel: Vector2,
    pub depth: f64,
    pub dot_product: f64,
    pub within_bounds: bool,
    pub is_occluded: bool,
    pub is_background: bool,
    pub ramp_penalty: f64,
}

pub type FrameMetrics = Option<Vec<Metrics>>; // Either by vertex, or by face.

fn summarize_metrics(ms: &[Metrics]) -> Metrics {
    Metrics {
        pixel: ms.iter().map(|m| m.pixel).sum::<Vector2>() / ms.len() as f64,
        depth: ms.iter().map(|m| m.depth).sum::<f64>() / ms.len() as f64,
        dot_product: ms.iter().map(|m| m.dot_product).sum::<f64>()
            / ms.len() as f64,
        within_bounds: ms.iter().all(|m| m.within_bounds),
        is_occluded: ms.iter().any(|m| m.is_occluded),
        is_background: ms.iter().any(|m| m.is_background),
        ramp_penalty: ms.iter().map(|m| m.ramp_penalty).sum::<f64>(),
    }
}

fn orientation(v0: Vector2, v1: Vector2, v2: Vector2) -> f64 {
    (v1[0] * v2[1] - v1[1] * v2[0])
        + (v2[0] * v0[1] - v2[1] * v0[0])
        + (v0[0] * v1[1] - v0[1] * v1[0])
}

fn containment_check(v: Vector2, f: [Vector2; 3]) -> bool {
    let [v0, v1, v2] = f;
    if v == v0 || v == v1 || v == v2 {
        return false;
    }
    let s0 = orientation(v, v1, v2);
    let s1 = orientation(v0, v, v2);
    let s2 = orientation(v0, v1, v);
    s0 > 0.0 && s1 > 0.0 && s2 > 0.0
}

fn max(a: [f64; 3]) -> f64 {
    *a.iter().max_by(|p, q| p.partial_cmp(q).unwrap()).unwrap()
}

fn compute_occlusion_for_all_vertices(
    vertices_proj: &[ProjectedPoint],
    mesh: &Mesh,
) -> Vec<bool> {
    // Build 2d kdtree of all vertices.
    let mut kdtree = KdTree::new();
    for (i, v) in vertices_proj.iter().enumerate() {
        kdtree.add(v.point.as_ref(), i).unwrap();
    }

    // Set all vertices to visible initially.
    let mut occluded = vec![false; vertices_proj.len()];

    // For each triangle, occlude nearby vertices.
    for face in &mesh.faces {
        let ProjectedPoint {
            point: v0,
            depth: d0,
        } = vertices_proj[face[0]];
        let ProjectedPoint {
            point: v1,
            depth: d1,
        } = vertices_proj[face[1]];
        let ProjectedPoint {
            point: v2,
            depth: d2,
        } = vertices_proj[face[2]];
        if d0 < 0.0 && d1 < 0.0 && d2 < 0.0 {
            continue;
        }
        let v = (v0 + v1 + v2) / 3.0;
        let radius = 1.1
            * max([
                (v0 - v).norm_squared(),
                (v1 - v).norm_squared(),
                (v2 - v).norm_squared(),
            ]);
        for (_dist, &i) in kdtree
            .within_unsorted(v.as_ref(), radius, &squared_euclidean)
            .unwrap()
        {
            let ProjectedPoint {
                point: vi,
                depth: di,
            } = vertices_proj[i];
            if d0 < di
                && d1 < di
                && d2 < di
                && containment_check(vi, [v0, v1, v2])
            {
                occluded[i] = true;
            }
        }
    }

    occluded
}

struct VertexAndFaceMetricsOfSingleFrame {
    pub vertex_metrics: Vec<Metrics>,
    pub face_metrics: Vec<Metrics>,
}

fn make_frame_metrics(
    scan: &fm::Scan,
    frame: &fm::ScanFrame,
    mesh: &Mesh,
    background_color: Vector3,
    background_deviation: f64,
) -> Option<VertexAndFaceMetricsOfSingleFrame> {
    let image = load_frame_image(frame)?;

    let vertices_proj = project_like_camera(scan, frame, &mesh.vertices);

    let camera_angle =
        frame.time as f64 / 1E9 * scan.camera_angular_velocity as f64;
    let time_rot =
        Quaternion::from_axis_angle(&Vector3::z_axis(), camera_angle);
    let eye = scan.camera_initial_position.unwrap_or_default();
    let eye = Point3::new(eye.x as f64, eye.y as f64, eye.z as f64);
    let camera = time_rot * eye;

    let occlusions = compute_occlusion_for_all_vertices(&vertices_proj, mesh);

    let mut vertex_metrics = vec![];
    for i in 0..mesh.vertices.len() {
        let ProjectedPoint {
            point: pixel,
            depth,
        } = vertices_proj[i];
        vertex_metrics.push(Metrics {
            pixel,
            depth,
            dot_product: (camera - mesh.vertices[i]).dot(&mesh.normals[i]),
            within_bounds: depth > 0.0
                && 0.01 <= pixel[0]
                && pixel[0] <= 0.99
                && 0.01 <= pixel[1]
                && pixel[1] <= 0.99,
            is_occluded: occlusions[i],
            is_background: evaluate_background_predicate(
                pixel,
                &image,
                background_color,
                background_deviation,
            ),
            ramp_penalty: 0.0,
        });
    }
    let face_metrics = mesh
        .faces
        .iter()
        .map(|&[v0, v1, v2]| {
            let ms =
                [vertex_metrics[v0], vertex_metrics[v1], vertex_metrics[v2]];
            summarize_metrics(&ms)
        })
        .collect();
    Some(VertexAndFaceMetricsOfSingleFrame {
        vertex_metrics,
        face_metrics,
    })
}

pub struct VertexAndFaceMetricsOfAllFrames {
    pub vertex_metrics: Vec<FrameMetrics>,
    pub face_metrics: Vec<FrameMetrics>,
}

pub fn make_all_frame_metrics(
    scans: &IndexMap<String, fm::Scan>,
    scan_frames: &[fm::ScanFrame],
    mesh: &Mesh,
    background_color: Vector3,
    background_deviation: f64,
) -> VertexAndFaceMetricsOfAllFrames {
    let mut vertex_metrics = vec![];
    let mut face_metrics = vec![];
    for frame in scan_frames {
        let scan = scans.get(&frame.scan).unwrap();
        let (vm, fm) = if let Some(m) = make_frame_metrics(
            scan,
            frame,
            mesh,
            background_color,
            background_deviation,
        ) {
            (Some(m.vertex_metrics), Some(m.face_metrics))
        } else {
            (None, None)
        };
        vertex_metrics.push(vm);
        face_metrics.push(fm);
    }
    VertexAndFaceMetricsOfAllFrames {
        vertex_metrics,
        face_metrics,
    }
}

fn build_costs_for_single_frame(
    frame_idx: usize,
    metrics: &[FrameMetrics],
    mesh: &Mesh,
) -> Vec<f64> {
    let mut costs = vec![f64::INFINITY; mesh.faces.len()];
    if let Some(mets) = &metrics[frame_idx] {
        for face_idx in 0..mesh.faces.len() {
            let met: Metrics = mets[face_idx];
            let mut cost = f64::INFINITY;
            if met.within_bounds && !met.is_occluded && !met.is_background {
                cost = 1.0 / met.dot_product + met.ramp_penalty;
            }
            costs[face_idx] = cost;
        }
    }
    costs
}

pub fn select_cameras(
    metrics: &[FrameMetrics],
    mesh: &Mesh,
    selection_cost_limit: f64,
) -> Vec<Option<usize>> {
    let mut chosen = vec![None; mesh.faces.len()];
    let mut costs = vec![f64::INFINITY; mesh.faces.len()];
    for frame_idx in 0..metrics.len() {
        let alt_costs = build_costs_for_single_frame(frame_idx, metrics, mesh);
        for face_idx in 0..mesh.faces.len() {
            if alt_costs[face_idx] > selection_cost_limit {
                continue; // Skip option which is too expensive to be sensible.
            }
            if costs[face_idx] > alt_costs[face_idx] {
                costs[face_idx] = alt_costs[face_idx];
                chosen[face_idx] = Some(frame_idx);
            }
        }
    }
    chosen
}

pub fn evaluate_background_predicate(
    pixel: Vector2,
    image: &RgbImage,
    background_color: Vector3,
    background_deviation: f64,
) -> bool {
    let diff3 = sample_pixel(pixel, image) - background_color;

    // Remove the grayscale component from the color difference vector.
    let diff2 = Vector2::new(diff3[0] - diff3[1], diff3[0] - diff3[2]);

    diff2.norm() < background_deviation
}
