use crate::texturing::misc::*;

const COST_LIMIT: f64 = 10.0;

#[derive(Debug, Copy, Clone)]
pub struct Metrics {
    pub pixel: Point2,
    pub depth: f64,
    pub dot_product: f64,
    pub within_bounds: bool,
    pub is_occluded: bool,
    pub is_green: bool,
    pub ramp_penalty: f64,
}

pub type FrameMetrics = Option<Vec<Metrics>>;  // either by vertex, or by face

fn summarize_metrics(ms: Vec<Metrics>) -> Metrics {
    Metrics {
        pixel: ms.iter().map(|m| m.pixel).sum::<Point2>() / ms.len() as f64,
        depth: ms.iter().map(|m| m.depth).sum::<f64>() / ms.len() as f64,
        dot_product:
            ms.iter().map(|m| m.dot_product).sum::<f64>() / ms.len() as f64,
        within_bounds: ms.iter().all(|m| m.within_bounds),
        is_occluded: ms.iter().any(|m| m.is_occluded),
        is_green: ms.iter().any(|m| m.is_green),
        ramp_penalty: ms.iter().map(|m| m.ramp_penalty).sum::<f64>(),
    }
}

fn orientation(v0: Vector2, v1: Vector2, v2: Vector2) -> f64 {
    (v1[0]*v2[1] - v1[1]*v2[0])
        + (v2[0]*v0[1] - v2[1]*v0[0])
        + (v0[0]*v1[1] - v0[1]*v1[0])
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
    *a.iter().max_by(|p, q| p.partial_cmp(&q).unwrap()).unwrap()
    /*let mut ret = f64::NEG_INFINITY;
    for ak in a {
        if ret < ak {
            ret = ak;
        }
    }
    ret*/
}

fn compute_occlusion_for_all_vertices(
    vertices_proj: &Vec<(Point2, Depth)>,
    mesh: &Mesh,
) -> Vec<bool> {
    
    // build 2d kdtree of all vertices
    let mut kdtree = KdTree::new();
    for (i, (v, _d)) in vertices_proj.iter().enumerate() {
        kdtree.add(v.as_ref(), i).unwrap();
    }
    
    // set all vertices to visible initially
    let mut occluded: Vec<bool> = vec![false; vertices_proj.len()];
    
    // for each triangle, occlude nearby vertices
    for face in &mesh.faces {
        let (v0, d0) = vertices_proj[face[0]];
        let (v1, d1) = vertices_proj[face[1]];
        let (v2, d2) = vertices_proj[face[2]];
        if d0 < 0.0 && d1 < 0.0 && d2 < 0.0 { continue; }
        let v = (v0 + v1 + v2) / 3.0;
        let radius = 1.1 * max([(v0 - v).norm_squared(),
                                (v1 - v).norm_squared(),
                                (v2 - v).norm_squared()]);
        for (_dist, &i) in kdtree.within_unsorted(
            v.as_ref(),
            radius,
            &squared_euclidean
        ).unwrap() {
            let (vi, di) = vertices_proj[i];
            if d0 < di && d1 < di && d2 < di
                && containment_check(vi, [v0, v1, v2])
            {
                occluded[i] = true;
            }
        }
    }
    
    occluded
}

fn evaluate_green_screen_predicate(pixel: Point2, image: &RgbImage) -> bool {
    let &[red, green, _blue] = sample_pixel(pixel, &image).as_ref();
    green > red - 10.0 && green > 20.0  // this could be made configurable too
}

pub fn make_frame_metrics(
    scan: &fm::Scan,
    frame: &fm::ScanFrame,
    //params: &PointCloudParams,
    mesh: &Mesh,
) -> Option<(Vec<Metrics>, Vec<Metrics>)> {
    let image: RgbImage = try_load_frame_image(&frame)?;
    
    let vertices_proj: Vec<(Point2, Depth)> =
        project_like_camera(scan, frame, &mesh.vertices);
    
    let camera_angle =
        frame.time as f64 / 1E9 * scan.camera_angular_velocity as f64;
    let time_rot =
        Quaternion::from_axis_angle(&Vector3::z_axis(), camera_angle);
    let eye =
        fm_point3_to_point3(&scan.camera_initial_position.unwrap_or_default());
    let camera: Point3 = time_rot * eye;
    
    let occlusions: Vec<bool> =
        compute_occlusion_for_all_vertices(&vertices_proj, &mesh);
    
    let mut vertex_metrics: Vec<Metrics> = vec![];
    for i in 0..mesh.vertices.len() {
        let (pixel, depth) = vertices_proj[i];
        vertex_metrics.push(Metrics {
            pixel,
            depth,
            dot_product: (camera - mesh.vertices[i]).dot(&mesh.normals[i]),
            within_bounds: depth > 0.0
                && 0.01 <= pixel[0] && pixel[0] <= 0.99
                && 0.01 <= pixel[1] && pixel[1] <= 0.99,
            is_occluded: occlusions[i],
            is_green: evaluate_green_screen_predicate(pixel, &image),
            ramp_penalty: 0.0  // TODO (used for limiting camera to
                               //       "its" part of the mesh)
        });
    }
    let mut face_metrics: Vec<Metrics> = vec![];
    for &[v0, v1, v2] in &mesh.faces {
        let ms = vec![vertex_metrics[v0],
                      vertex_metrics[v1],
                      vertex_metrics[v2]];
        face_metrics.push(summarize_metrics(ms));
    }
    Some((vertex_metrics, face_metrics))
}

pub fn make_all_frame_metrics(
    scans: &IndexMap<String, fm::Scan>,
    scan_frames: &Vec<fm::ScanFrame>,
    mesh: &Mesh,
) -> (Vec<FrameMetrics>, Vec<FrameMetrics>) {
    let mut vertex_metrics: Vec<FrameMetrics> = vec![];
    let mut face_metrics: Vec<FrameMetrics> = vec![];
    for frame in scan_frames {
        let scan = scans.get(&frame.scan).unwrap();
        let (vm, fm) =
            split_option(make_frame_metrics(scan, &frame, &mesh));
        vertex_metrics.push(vm);
        face_metrics.push(fm);
    }
    (vertex_metrics, face_metrics)
}


fn build_costs_for_single_frame(
    frame_idx: FrameIdx,
    metrics: &Vec<FrameMetrics>,
    mesh: &Mesh,
) -> Vec<f64> {
    let mut costs = vec![f64::INFINITY; mesh.faces.len()];
    if let Some(mets) = &metrics[frame_idx] {
        for face_idx in 0..mesh.faces.len() {
            let met: Metrics = mets[face_idx];
            let mut cost = f64::INFINITY;
            if met.within_bounds && !met.is_occluded && !met.is_green {
                cost = 1.0/met.dot_product + met.ramp_penalty;
            }
            costs[face_idx] = cost;
        }
    }
    costs
}

pub fn select_cameras(
    metrics: &Vec<FrameMetrics>,
    mesh: &Mesh
) -> Vec<Option<FrameIdx>> {
    let mut chosen: Vec<Option<FrameIdx>> = vec![None; mesh.faces.len()];
    let mut costs: Vec<f64> = vec![f64::INFINITY; mesh.faces.len()];
    for frame_idx in 0..metrics.len() {
        let alt_costs =
            build_costs_for_single_frame(frame_idx, &metrics, &mesh);
        for face_idx in 0..mesh.faces.len() {
            if alt_costs[face_idx] > COST_LIMIT {
                continue;  // skip option which is too expensive to be sensible
            }
            if costs[face_idx] > alt_costs[face_idx] {
                costs[face_idx] = alt_costs[face_idx];
                chosen[face_idx] = Some(frame_idx);
            }
        }
    }
    chosen
}
