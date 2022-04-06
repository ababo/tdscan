use indexmap::IndexMap;
use kiddo::distance::squared_euclidean;
use kiddo::KdTree;
use rayon::prelude::*;

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
    background_dilations: &[f64],
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
    let background = BackgroundDetector::new(
        &image,
        background_color,
        background_deviation,
        background_dilations,
    );

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
            is_background: background.detect(pixel),
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
    background_dilations: &[f64],
) -> VertexAndFaceMetricsOfAllFrames {
    let mut vertex_metrics = vec![];
    let mut face_metrics = vec![];

    let results: Vec<(FrameMetrics, FrameMetrics)> = (0..scan_frames.len())
        .into_par_iter()
        .map(|frame_idx| {
            let frame = &scan_frames[frame_idx];
            let scan = scans.get(&frame.scan).unwrap();
            if let Some(m) = make_frame_metrics(
                scan,
                frame,
                mesh,
                background_color,
                background_deviation,
                background_dilations,
            ) {
                (Some(m.vertex_metrics), Some(m.face_metrics))
            } else {
                (None, None)
            }
        })
        .collect();
    for (vm, fm) in results {
        vertex_metrics.push(vm);
        face_metrics.push(fm);
    }
    VertexAndFaceMetricsOfAllFrames {
        vertex_metrics,
        face_metrics,
    }
}

pub fn build_cost_for_single_face(metrics: &Metrics) -> f64 {
    if metrics.within_bounds
        && !metrics.is_occluded
        // There is no `&& !metrics.is_background` clause.
        && metrics.depth > 0.0
        && metrics.dot_product > 0.0
    {
        1.0 / metrics.dot_product + metrics.ramp_penalty
    } else {
        f64::INFINITY
    }
}

fn build_costs_for_single_frame(
    metrics: &[Metrics],
    mesh: &Mesh,
    topo: &BasicMeshTopology,
    selection_corner_radius: usize,
) -> Vec<f64> {
    let mut costs: Vec<f64> =
        metrics.iter().map(build_cost_for_single_face).collect();

    // Avoid corners.
    for _ in 0..selection_corner_radius {
        costs = mesh_faces_spread_infinity(costs, mesh, topo);
    }

    costs
}

pub fn build_all_costs(
    metrics: &[FrameMetrics],
    mesh: &Mesh,
    topo: &BasicMeshTopology,
    selection_corner_radius: usize,
) -> Vec<Option<Vec<f64>>> {
    map_vec_option(metrics, &|single_frame_metrics| {
        build_costs_for_single_frame(
            single_frame_metrics,
            mesh,
            topo,
            selection_corner_radius,
        )
    })
}

pub fn select_cameras(
    all_costs: &[Option<Vec<f64>>],
    mesh: &Mesh,
    selection_cost_limit: f64,
) -> Vec<Option<usize>> {
    let mut chosen = vec![None; mesh.faces.len()];
    let mut costs = vec![f64::INFINITY; mesh.faces.len()];
    for (frame_idx, all_costs_option) in all_costs.iter().enumerate() {
        if let Some(alt_costs) = all_costs_option {
            for face_idx in 0..mesh.faces.len() {
                if alt_costs[face_idx] > selection_cost_limit {
                    // Skip option which is too expensive to be sensible.
                    continue;
                }
                if costs[face_idx] > alt_costs[face_idx] {
                    costs[face_idx] = alt_costs[face_idx];
                    chosen[face_idx] = Some(frame_idx);
                }
            }
        }
    }
    chosen
}

pub fn detect_background_static(
    pixel: Vector2,
    image: &RgbImage,
    background_color: Vector3,
    background_deviation: f64,
) -> bool {
    let diff3 = sample_pixel(pixel, image) - background_color;

    // Remove the grayscale component from the color difference vector.
    let gray = diff3.iter().sum::<f64>() / 3.0;
    let diff2 = diff3 - gray * Vector3::new(1.0, 1.0, 1.0);

    diff2.norm() < background_deviation
}

pub struct BackgroundDetector {
    image: RgbImage,
    bgmask: ImageMask,
}

impl BackgroundDetector {
    pub fn new(
        image: &RgbImage,
        background_color: Vector3,
        background_deviation: f64,
        background_dilations: &[f64],
    ) -> BackgroundDetector {
        let image = image.clone();
        let (w, h) = image.dimensions();
        let (w, h) = (Dim::from_usize(w as usize), Dim::from_usize(h as usize));
        let mut bgmask = ImageMask::from_element_generic(h, w, false);

        // Short-circuit when possible.
        if 0.0 < background_deviation {
            // Pixel-by-pixel detection.
            for i in 0..bgmask.nrows() {
                for j in 0..bgmask.ncols() {
                    let pixel =
                        ij_to_uv(Vector2::new(i as f64, j as f64), &image);
                    bgmask[(i, j)] = detect_background_static(
                        pixel,
                        &image,
                        background_color,
                        background_deviation,
                    );
                }
            }

            // Remove noise.
            for &r in background_dilations {
                if r > 0.0 {
                    bgmask = dilate(&bgmask, r);
                } else {
                    bgmask = erode(&bgmask, -r);
                }
            }
        }

        BackgroundDetector { bgmask, image }
    }

    pub fn detect(&self, pixel: Vector2) -> bool {
        let &[i, j] = uv_to_ij(pixel, &self.image).as_ref();
        self.bgmask.get((i as usize, j as usize)).cloned().unwrap_or(false)
    }
}

pub struct BackgroundDisqualificationParams {
    pub cost_limit: f64,
    pub consensus_threshold: f64,
    pub consensus_spread: usize,
}

pub fn disqualify_background_faces(
    chosen_cameras: &mut [Option<usize>],
    face_metrics: &[FrameMetrics],
    all_costs: &[Option<Vec<f64>>],
    mesh: &Mesh,
    topo: &BasicMeshTopology,
    params: BackgroundDisqualificationParams,
) {
    for face_idx in 0..mesh.faces.len() {
        if let Some(_frame_idx) = chosen_cameras[face_idx] {
            // Count how many reasonable frames say that the face is background.
            let mut bg_count_true = 0;
            let mut bg_count_false = 0;
            for other_frame_idx in 0..face_metrics.len() {
                if let Some(other_frame) =
                    face_metrics[other_frame_idx].as_ref()
                {
                    if all_costs[other_frame_idx].as_ref().unwrap()[face_idx]
                        < params.cost_limit
                    {
                        if other_frame[face_idx].is_background {
                            bg_count_true += 1;
                        } else {
                            bg_count_false += 1;
                        }
                    }
                }
            }

            // If a big enough proportion say that the face is indeed
            // background, disqualify it and a few surrounding faces.
            if bg_count_true as f64
                > params.consensus_threshold
                    * (bg_count_true + bg_count_false) as f64
            {
                set_mesh_face_value_with_radius(
                    chosen_cameras,
                    face_idx,
                    None,
                    params.consensus_spread,
                    topo,
                );
            }
        }
    }
}
