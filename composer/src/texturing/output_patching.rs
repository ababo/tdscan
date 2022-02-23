use crate::texturing::misc::*;

const ANGLE_LIMIT: f64 = (PI / 180.0) * 66.0;

fn vertex_aligned(i: VertexIdx, major_axis: Vector3, mesh: &Mesh) -> bool {
    // (assuming the major axis is normalized)
    mesh.normals[i].dot(&major_axis) > ANGLE_LIMIT.cos()
}

fn face_aligned(i: FaceIdx, major_axis: Vector3, mesh: &Mesh) -> bool {
    let [v0, v1, v2] = mesh.faces[i];
    vertex_aligned(v0, major_axis, mesh)
        && vertex_aligned(v1, major_axis, mesh)
        && vertex_aligned(v2, major_axis, mesh)
}

fn count_aligned_faces(
    major_axis: Vector3,
    faces_mask: &Vec<bool>,
    mesh: &Mesh
) -> usize {
    let mut count = 0;
    for i in 0..mesh.faces.len() {
        if faces_mask[i] && face_aligned(i, major_axis, mesh) {
            count += 1;
        }
    }
    count
}

fn face_normal(i: VertexIdx, mesh: &Mesh) -> Vector3 {
    let [v0, v1, v2] = mesh.faces[i];
    (mesh.normals[v0] + mesh.normals[v1] + mesh.normals[v2]).normalize()
}

// (the mask must have at least one nonzero entry)
fn get_major_axis(faces_mask: &Vec<bool>, mesh: &Mesh) -> Vector3 {
    let normals: Vec<Vector3> = (0..mesh.faces.len())
        .filter(|&i| faces_mask[i])
        .map(|i| face_normal(i, mesh))
        .collect();
    let mut major = dominant_vector(&normals);
    if count_aligned_faces(major, &faces_mask, mesh) <
        count_aligned_faces(-major, &faces_mask, mesh)
    {
        major = -major;
    }
    major
}

pub fn project_chunk_with_depths(
    chunk: &Vec<FaceIdx>,
    major_axis: Vector3,
    mesh: &Mesh,
) -> Vec<[(Point2, Depth); 3]> {
    // the (negative) major axis is included in the basis,
    // to extract (affine) depth data
    let (_, ev) = complement(major_axis);
    let eu = ev.cross(&major_axis);
    let uvw_basis = Matrix3::from_columns(&[eu, ev, -major_axis]).transpose();
    
    // project orthogonally (not perspective!), and keep depth data
    let f = |v: VertexIdx| split_point2_depth(
        uvw_basis * mesh.vertices[v].coords);
    let chunk_uvws = chunk.iter()
        .map(|&i| mesh.faces[i])
        .map(|[v0, v1, v2]| [f(v0), f(v1), f(v2)])
        .collect();
    
    // (skipping axis-aligning rotation)
    
    // (skipping normalization to [0,1]x[0,1] uv box)
    
    chunk_uvws
}

fn visible_faces(
    faces_mask: &Vec<bool>,
    major_axis: Vector3,
    mesh: &Mesh
) -> Vec<bool> {
    let faces_idx = mask_to_idxs(faces_mask);
    let uvws = project_chunk_with_depths(&faces_idx, major_axis, mesh);
    let uvs: Vec<[Vector2; 3]> = uvws
        .iter()
        .map(|&[(p0, _d0), (p1, _d1), (p2, _d2)]| [p0, p1, p2])
        .collect();
    let ws: Vec<[Depth; 3]> = uvws
        .iter()
        .map(|&[(_p0, d0), (_p1, d1), (_p2, d2)]| [d0, d1, d2])
        .collect();
    
    //let mut dbg_count = 0;
    
    let mut faces_mask = faces_mask.clone();
    for (i_idx, &_i) in faces_idx.iter().enumerate() {
        if let Some(bcs) = BarycentricCoordinateSystem::try_new(uvs[i_idx]) {
            let wsi = Vector3::new(ws[i_idx][0], ws[i_idx][1], ws[i_idx][2]);
            for (j_idx, &j) in faces_idx.iter().enumerate() {
                for k in 0..3 {
                    let bary = bcs.infer(uvs[j_idx][k]);
                    let depth = bary.dot(&wsi);
                    if all_nonneg(bary) && depth + 1e-3 < ws[j_idx][k] {
                        //if faces_mask[j] { dbg_count += 1; }
                        faces_mask[j] = false;
                    }
                }
            }
        }
    }
    
    if faces_idx.len() > 1000 {
        //println!("{dbg_count} out of {} faces are not visible",
        //         faces_idx.len());
    }
    
    faces_mask
}

fn partition_faces(
    faces_mask: &Vec<bool>,
    _mesh: &Mesh,
    topo: &BasicMeshTopology,
) -> (UnionFind<usize>, Vec<FaceIdx>) {
    let carrier: Vec<FaceIdx> = mask_to_idxs(faces_mask);
    let carrier_inv: HashMap<FaceIdx, usize> = vec_inv(&carrier);
    let mut partition = UnionFind::new(carrier.len());

    for &f0 in &carrier {
        if let Some(&c0) = carrier_inv.get(&f0) {
            for &f1 in &topo.neighbouring_faces[f0] {
                if let Some(&c1) = carrier_inv.get(&f1) {
                    partition.union(c0, c1);
                }
            }
        }
    }

    (partition, carrier)
}

fn get_big_chunk_helper(
    faces_mask: &Vec<bool>,
    mesh: &Mesh,
    topo: &BasicMeshTopology,
    major_axis: Vector3
) -> Vec<FaceIdx> {
    let mut faces_mask_subset: Vec<bool> = faces_mask.clone();
    //let mut dbgy = 0;
    //let mut dbgn = 0;
    for i in 0..mesh.faces.len() {
        faces_mask_subset[i] &= face_aligned(i, major_axis, mesh);
        //if face_aligned(i, major_axis, mesh) { dbgy += 1 } else { dbgn += 1}
    }
    //dbg!(dbgy, dbgn);
    
    let biggest: Vec<FaceIdx>;
    if faces_mask_subset.iter().map(|&b| b as usize).sum::<usize>() > 0 {
        let (partition, carrier) =
            partition_faces(&faces_mask_subset, mesh, topo);
        biggest = extract_biggest_partition_component(partition)
            .iter().map(|&i| carrier[i]).collect();
        //println!("chunk has size {}", biggest.len());
    } else {
        // this happens toward the end when there are just
        // a few scattered faces left to choose from
        let lone: FaceIdx = faces_mask.iter().position(|&b| b).unwrap();
        biggest = vec![lone];
        //println!("fallback to singleton");
    }
    //dbg!(biggest.len());
    biggest
}

fn average_normal(
    faces_idx: &Vec<FaceIdx>,
    mesh: &Mesh
) -> Vector3 {
    faces_idx
        .iter()
        .map(|&i| mesh.faces[i])
        .flatten()
        .map(|i| mesh.normals[i])
        .sum::<Vector3>()
        .normalize()
}

fn get_big_chunk(
    faces_mask: &Vec<bool>,
    mesh: &Mesh,
    topo: &BasicMeshTopology,
) -> (Vec<FaceIdx>, Vector3, Vec<bool>) {
    // project uvs from a heuristically good direction
    let mut major_axis: Vector3 = get_major_axis(faces_mask, mesh);
    let mut biggest: Vec<FaceIdx> =
        get_big_chunk_helper(faces_mask, mesh, topo, major_axis);

    // improve the projection direction a little (not very important)
    for _ in 0..10 {
        major_axis = average_normal(&biggest, mesh);
        biggest = get_big_chunk_helper(faces_mask, mesh, topo, major_axis);
    }
    
    // fix glitches caused by uv self-overlap
    let biggest_mask = idxs_to_mask(mesh.faces.len(), biggest);
    let biggest_mask = visible_faces(&biggest_mask, major_axis, mesh);
    let biggest = mask_to_idxs(&biggest_mask);

    // remember the faces that were projected, to avoid duplication
    let mut faces_mask_remaining: Vec<bool> = faces_mask.clone();
    for &k in &biggest {
        if !(faces_mask_remaining[k]) { println!("emitting face {k} again"); }
        faces_mask_remaining[k] = false;
    }
        
    (biggest, major_axis, faces_mask_remaining)
}

pub fn choose_uv_patches(
    mesh: &Mesh,
    topo: &BasicMeshTopology
) -> Vec<(Vec<FaceIdx>, Vector3)> {
    let mut faces_mask: Vec<bool> = vec![true; mesh.faces.len()];
    // (^ to make it faster, maybe replace this mask by a set of indices)
    let mut result = vec![];
    
    let mut i = 0;
    while faces_mask.iter().map(|&b| b as usize).sum::<usize>() > 0 {
        
        if i < 5 || i % 10 == 0 {
            //println!("deflation, step {}", i);
        }
        
        let (chunk, major_axis, faces_mask_remaining) =
            get_big_chunk(&faces_mask, mesh, topo);
        
        faces_mask = faces_mask_remaining;
        
        if i < 5 || i % 10 == 0 {
            //println!("chunk size {}", chunk.len());
        }
        
        result.push((chunk, major_axis));
        i += 1;
        
        //if i == 100 { break; }
        /*if i == 5 { break; }*/
    }
    
    result
}

fn average_uv3(vec: &Vec<[UV; 3]>) -> UV {
    let mut sum: UV = UV::zeros();
    let mut num = 0;
    for uvs in vec {
        for uv in uvs {
            sum += uv;
            num += 1;
        }
    }
    sum / num as f64
}

#[derive(Debug, PartialEq, Clone, PartialOrd)]
pub struct LocalPatch {
    pub chunk: Vec<FaceIdx>,  // indices into the list of mesh faces
    pub size: [f64; 2],       // actual physical size of patch, in meters
    pub uvs: Vec<[UV; 3]>,    // coordinates normalized to [0,1]x[0,1]
}

impl LocalPatch {
    pub fn calculate_from(
        chunk: &Vec<FaceIdx>,
        major_axis: Vector3,
        mesh: &Mesh,
    ) -> LocalPatch {
        // (similar to project_chunk_with_depths -> Vec<[(Point2, Depth); 3]>)
        
        // fix the orientation (not really necessary, but is nice)
        let (_, ev) = complement(major_axis);
        let eu = ev.cross(&major_axis);
        let uv_basis = nalgebra::Matrix3x2::from_columns(&[eu, ev]).transpose();

        // project orthogonally (not perspective!)
        let f = |v: VertexIdx| uv_basis * mesh.vertices[v].coords;
        let chunk_uvs: Vec<[UV; 3]> = chunk.iter()
            .map(|&i| mesh.faces[i])
            .map(|[v0, v1, v2]| [f(v0), f(v1), f(v2)])
            .collect();

        // rotate to align with axes
        // (not really necessary either, but may reduce file size)
        let avg: UV = average_uv3(&chunk_uvs);
        let f = |uv: &UV| uv - avg;
        let ev = dominant_vector(
            &chunk_uvs.iter().flatten().map(f).collect());
        let eu = Vector2::new(ev[1], -ev[0]);
        let uv_basis = Matrix2::from_columns(&[eu, ev]).transpose();
        let f = |uv|  uv_basis * uv;
        let chunk_uvs: Vec<[UV; 3]> = chunk_uvs.iter()
            .map(|[uv0, uv1, uv2]| [f(uv0), f(uv1), f(uv2)])
            .collect();

        // normalize to [0,1]x[0,1]
        let f = |uv: &UV| [uv[0], uv[1]];
        let uv_rect = Rectangle::<f64>::bounding(
            &chunk_uvs.iter().flatten().map(f).collect());
        let [u_min, v_min] = uv_rect.pos;
        let [u_size, v_size] = uv_rect.size;
        
        let f = |uv: UV| Vector2
            ::new((uv[0] - u_min) / u_size, (uv[1] - v_min) / v_size);
        let uvs: Vec<[UV; 3]> = chunk_uvs.iter()
            .map(|&[uv0, uv1, uv2]| [f(uv0), f(uv1), f(uv2)])
            .collect();
        let size = [u_size, v_size];
        
        LocalPatch { chunk: chunk.clone(), uvs, size }
    }
    
    pub fn to_global_coords(&self, rect: Rectangle<f64>) -> Vec<[UV; 3]> {
        let f = |uv: UV| UV::new(
            rect.pos[0] + rect.size[0]*uv[0],
            rect.pos[1] + rect.size[1]*uv[1]
        );
        self.uvs
            .iter()
            .map(|&[uv0, uv1, uv2]| [f(uv0), f(uv1), f(uv2)])
            .collect()
    }
}
