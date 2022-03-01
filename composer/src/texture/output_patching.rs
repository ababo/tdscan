use std::f64::consts::PI;

use nalgebra::Matrix3;
use petgraph::unionfind::UnionFind;

use crate::misc::*;
use crate::mesh::Mesh;
use crate::texture::*;

// The following angle describes the maximal allowed deviation between a
// mesh face normal and the direction of projection. The particular value
// is the same one that Blender uses by default. Measured in radians.
const ANGLE_LIMIT: f64 = (PI / 180.0) * 66.0;

fn vertex_aligned(vertex_idx: usize, major_axis: Vector3, mesh: &Mesh) -> bool {
    // Assuming the major axis is normalized.
    mesh.normals[vertex_idx].dot(&major_axis) > ANGLE_LIMIT.cos()
}

fn face_aligned(face_idx: usize, major_axis: Vector3, mesh: &Mesh) -> bool {
    let [v0, v1, v2] = mesh.faces[face_idx];
    vertex_aligned(v0, major_axis, mesh)
        && vertex_aligned(v1, major_axis, mesh)
        && vertex_aligned(v2, major_axis, mesh)
}

fn count_aligned_faces(
    major_axis: Vector3,
    faces_mask: &[bool],
    mesh: &Mesh,
) -> usize {
    let mut count = 0;
    for (i, &b) in faces_mask.iter().enumerate() {
        if b && face_aligned(i, major_axis, mesh) {
            count += 1;
        }
    }
    count
}

fn face_normal(face_idx: usize, mesh: &Mesh) -> Vector3 {
    let [v0, v1, v2] = mesh.faces[face_idx];
    (mesh.normals[v0] + mesh.normals[v1] + mesh.normals[v2]).normalize()
}

fn get_major_axis(faces_mask: &[bool], mesh: &Mesh) -> Vector3 {
    // The mask must have at least one nonzero entry.
    let normals: Vec<Vector3> = (0..mesh.faces.len())
        .filter(|&i| faces_mask[i])
        .map(|i| face_normal(i, mesh))
        .collect();
    let mut major = dominant_vector(&normals);
    if count_aligned_faces(major, faces_mask, mesh)
        < count_aligned_faces(-major, faces_mask, mesh)
    {
        major = -major;
    }
    major
}

pub fn project_chunk_with_depths(
    faces_idx: &[usize],
    major_axis: Vector3,
    mesh: &Mesh,
) -> Vec<[ProjectedPoint; 3]> {
    // The (negative) major axis is included in the basis,
    // to extract (affine) depth data.
    let (_, ev) = complement(major_axis);
    let eu = ev.cross(&major_axis);
    let uvw_basis = Matrix3::from_columns(&[eu, ev, -major_axis]).transpose();

    // Project orthogonally (not perspective!), and keep depth data.
    let f =
        |v: usize| split_point2_depth(uvw_basis * mesh.vertices[v].coords);
    faces_idx
        .iter()
        .map(|&i| mesh.faces[i])
        .map(|[v0, v1, v2]| [f(v0), f(v1), f(v2)])
        .collect()
    // Skipping axis-aligning rotation.
    // Skipping normalization to [0,1]x[0,1] UV box.
}

fn visible_faces(
    faces_mask: &[bool],
    major_axis: Vector3,
    mesh: &Mesh,
) -> Vec<bool> {
    let faces_idx = mask_to_idxs(faces_mask);
    let uvws = project_chunk_with_depths(&faces_idx, major_axis, mesh);
    let uvs: Vec<[Vector2; 3]> = uvws
        .iter()
        .map(|[pd0, pd1, pd2]| [pd0.point, pd1.point, pd2.point])
        .collect();
    let depths: Vec<[f64; 3]> = uvws
        .iter()
        .map(|[pd0, pd1, pd2]| [pd0.depth, pd1.depth, pd2.depth])
        .collect();

    let mut faces_mask = faces_mask.to_owned();
    for (i_idx, &_i) in faces_idx.iter().enumerate() {
        if let Some(bcs) = BarycentricCoordinateSystem::new(uvs[i_idx]) {
            let di = Vector3::new(
                depths[i_idx][0], depths[i_idx][1], depths[i_idx][2]);
            for (j_idx, &j) in faces_idx.iter().enumerate() {
                for k in 0..3 {
                    let bary = bcs.infer(uvs[j_idx][k]);
                    let depth = bary.dot(&di);
                    // This tolerance (1mm) should probably be lowered later.
                    const TOL: f64 = 1e-3;
                    if all_nonneg(bary) && depth + TOL < depths[j_idx][k] {
                        faces_mask[j] = false;
                    }
                }
            }
        }
    }

    faces_mask
}

fn partition_faces(
    faces_mask: &[bool],
    _mesh: &Mesh,
    topo: &BasicMeshTopology,
) -> (UnionFind<usize>, Vec<usize>) {
    let carrier = mask_to_idxs(faces_mask);
    let carrier_inv = vec_inv(&carrier);
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
    faces_mask: &[bool],
    mesh: &Mesh,
    topo: &BasicMeshTopology,
    major_axis: Vector3,
) -> Vec<usize> {
    let mut faces_mask_subset = faces_mask.to_owned();
    for (i, b) in faces_mask_subset.iter_mut().enumerate() {
        *b &= face_aligned(i, major_axis, mesh)
    }

    let biggest: Vec<usize>;
    if faces_mask_subset.iter().map(|&b| b as usize).sum::<usize>() > 0 {
        let (partition, carrier) =
            partition_faces(&faces_mask_subset, mesh, topo);
        biggest = extract_biggest_partition_component(partition)
            .iter()
            .map(|&i| carrier[i])
            .collect();
    } else {
        // This happens toward the end when there are just
        // a few scattered faces left to choose from.
        let lone = faces_mask.iter().position(|&b| b).unwrap();
        biggest = vec![lone];
    }
    
    biggest
}

fn average_normal(faces_idx: &[usize], mesh: &Mesh) -> Vector3 {
    faces_idx
        .iter()
        .map(|&i| mesh.faces[i])
        .flatten()
        .map(|i| mesh.normals[i])
        .sum::<Vector3>()
        .normalize()
}

fn get_big_chunk(
    faces_mask: &[bool],
    mesh: &Mesh,
    topo: &BasicMeshTopology,
) -> (Vec<usize>, Vector3, Vec<bool>) {
    // Project UVs from a heuristically good direction.
    let mut major_axis = get_major_axis(faces_mask, mesh);
    let mut biggest = get_big_chunk_helper(faces_mask, mesh, topo, major_axis);

    // Improve the projection direction a little (not very important).
    for _ in 0..10 {
        major_axis = average_normal(&biggest, mesh);
        biggest = get_big_chunk_helper(faces_mask, mesh, topo, major_axis);
    }

    // Fix glitches caused by UV self-overlap.
    let biggest_mask = idxs_to_mask(mesh.faces.len(), &biggest);
    let biggest_mask = visible_faces(&biggest_mask, major_axis, mesh);
    let biggest = mask_to_idxs(&biggest_mask);

    // Remember the faces that were projected, to avoid duplication.
    let mut faces_mask_remaining = faces_mask.to_owned();
    for &k in &biggest {
        faces_mask_remaining[k] = false;
    }

    (biggest, major_axis, faces_mask_remaining)
}

pub fn choose_uv_patches(
    mesh: &Mesh,
    topo: &BasicMeshTopology,
) -> Vec<(Vec<usize>, Vector3)> {
    let mut faces_mask = vec![true; mesh.faces.len()];
    // ^ To make it faster, maybe replace this mask by a set of indices.
    let mut result = vec![];

    while faces_mask.iter().map(|&b| b as usize).sum::<usize>() > 0 {
        let (faces_idx_taken, major_axis, faces_mask_remaining) =
            get_big_chunk(&faces_mask, mesh, topo);

        faces_mask = faces_mask_remaining;

        result.push((faces_idx_taken, major_axis));
    }

    result
}

fn average_uv3(vec: &[[Vector2; 3]]) -> Vector2 {
    let mut sum = Vector2::zeros();
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
    pub chunk: Vec<usize>,       // Indices into the list of mesh faces.
    pub size: [f64; 2],          // Actual physical size of patch, in meters.
    pub uvs: Vec<[Vector2; 3]>,  // Coordinates normalized to [0,1]x[0,1].
}

impl LocalPatch {
    pub fn calculate_from(
        faces_idx: &[usize],
        major_axis: Vector3,
        mesh: &Mesh,
    ) -> LocalPatch {
        // Similar to project_chunk_with_depths -> Vec<[ProjectedPoint; 3]>.

        // Fix the orientation (not really necessary, but is nice).
        let (_, ev) = complement(major_axis);
        let eu = ev.cross(&major_axis);
        let uv_basis = nalgebra::Matrix3x2::from_columns(&[eu, ev]).transpose();

        // Project orthogonally (not perspective!).
        let f = |v: usize| uv_basis * mesh.vertices[v].coords;
        let uvs: Vec<[Vector2; 3]> = faces_idx
            .iter()
            .map(|&i| mesh.faces[i])
            .map(|[v0, v1, v2]| [f(v0), f(v1), f(v2)])
            .collect();

        // Rotate to align with axes
        // (not really necessary either, but may reduce file size).
        let avg = average_uv3(&uvs);
        let f = |uv: &Vector2| uv - avg;
        let ev = dominant_vector(
            &uvs.iter().flatten().map(f).collect::<Vec<_>>(),
        );
        let eu = Vector2::new(ev[1], -ev[0]);
        let uv_basis = Matrix2::from_columns(&[eu, ev]).transpose();
        let f = |uv| uv_basis * uv;
        let uvs: Vec<[Vector2; 3]> = uvs
            .iter()
            .map(|[uv0, uv1, uv2]| [f(uv0), f(uv1), f(uv2)])
            .collect();

        // Normalize to [0,1]x[0,1].
        let f = |uv: &Vector2| [uv[0], uv[1]];
        let uv_rect = Rectangle::<f64>::bounding(
            &uvs.iter().flatten().map(f).collect::<Vec<_>>(),
        );
        let [u_min, v_min] = uv_rect.pos;
        let [u_size, v_size] = uv_rect.size;

        let f = |uv: Vector2| {
            Vector2::new((uv[0] - u_min) / u_size, (uv[1] - v_min) / v_size)
        };
        let uvs: Vec<[Vector2; 3]> = uvs
            .iter()
            .map(|&[uv0, uv1, uv2]| [f(uv0), f(uv1), f(uv2)])
            .collect();
        let size = [u_size, v_size];

        LocalPatch {
            chunk: faces_idx.to_owned(),
            uvs,
            size,
        }
    }

    pub fn to_global_coords(&self, rect: Rectangle<f64>) -> Vec<[Vector2; 3]> {
        let f = |uv: Vector2| {
            Vector2::new(
                rect.pos[0] + rect.size[0] * uv[0],
                rect.pos[1] + rect.size[1] * uv[1],
            )
        };
        self.uvs
            .iter()
            .map(|&[uv0, uv1, uv2]| [f(uv0), f(uv1), f(uv2)])
            .collect()
    }
}
