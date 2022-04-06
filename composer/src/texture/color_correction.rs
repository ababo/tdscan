use image::RgbImage;

use crate::mesh::Mesh;
use crate::texture::*;

type CooMatrix = nalgebra_sparse::coo::CooMatrix<f64>;
type CsrMatrix = nalgebra_sparse::csr::CsrMatrix<f64>;
type DVector = nalgebra::DVector<f64>;
type Matrix3 = nalgebra::Matrix3<f64>;

const MINIMUM_TOLERATED_SIN_FOR_COTAN: f64 = 1e-12;

fn face_vertex_cotan(
    vertex_idx: usize,
    other_idxs: [usize; 2],
    mesh: &Mesh,
) -> f64 {
    let p = mesh.vertices[vertex_idx];
    let q1 = mesh.vertices[other_idxs[0]];
    let q2 = mesh.vertices[other_idxs[1]];
    let u = p - q1;
    let v = p - q2;
    let sin = u.cross(&v).norm();

    // Avoid division by zero (rare).
    let sin1 = f64::max(sin, MINIMUM_TOLERATED_SIN_FOR_COTAN);

    let cos = u.dot(&v).abs();
    cos / sin1
}

fn add_eq_to_minor(a: &mut Matrix3, i: usize, j: usize, s: f64) {
    a[(i, i)] += s;
    a[(i, j)] -= s;
    a[(j, i)] -= s;
    a[(j, j)] += s;
}

fn face_laplacian(face_idx: usize, mesh: &Mesh) -> Matrix3 {
    let [v0, v1, v2] = mesh.faces[face_idx];
    let mut a = Matrix3::zeros();
    add_eq_to_minor(&mut a, 0, 1, face_vertex_cotan(v2, [v0, v1], mesh));
    add_eq_to_minor(&mut a, 0, 2, face_vertex_cotan(v1, [v0, v2], mesh));
    add_eq_to_minor(&mut a, 1, 2, face_vertex_cotan(v0, [v1, v2], mesh));
    a
}

fn build_discontinuous_laplacian(mesh: &Mesh) -> CsrMatrix {
    let n = mesh.faces.len();
    let mut coo = CooMatrix::new(n * 3, n * 3);
    for face_idx in 0..n {
        coo.push_matrix(
            face_idx * 3,
            face_idx * 3,
            &face_laplacian(face_idx, mesh),
        );
    }
    CsrMatrix::from(&coo)
}

fn build_face_vertex_to_vertex_correspondence(
    mesh: &Mesh,
    topo: &BasicMeshTopology,
) -> CsrMatrix {
    let mut coo = CooMatrix::new(mesh.faces.len() * 3, mesh.vertices.len());
    for vertex_idx in 0..mesh.vertices.len() {
        for &face_idx in &topo.faces_around_vertex[vertex_idx] {
            let local_idx = mesh.faces[face_idx]
                .iter()
                .position(|&r| r == vertex_idx)
                .unwrap();
            coo.push(face_idx * 3 + local_idx, vertex_idx, 1.0);
        }
    }
    CsrMatrix::from(&coo)
}

fn build_color_sample_array(
    mesh: &Mesh,
    vertex_metrics: &[FrameMetrics],
    chosen_cameras: &[Option<usize>],
    images: &[Option<RgbImage>],
) -> Vec<[Vector3; 3]> {
    (0..mesh.faces.len())
        .map(|face_idx| {
            if let Some(frame_idx) = chosen_cameras[face_idx] {
                let uvs = uv_coords_from_metrics(
                    face_idx,
                    frame_idx,
                    vertex_metrics,
                    mesh,
                );
                let image = images[frame_idx].as_ref().unwrap();
                let f = |i| sample_pixel(uvs[i], image);
                [f(0), f(1), f(2)]
            } else {
                [Vector3::new(0.0, 0.0, 0.0); 3]
            }
        })
        .collect()
}

fn build_initial_guess_for_single_vertex(
    mesh: &Mesh,
    topo: &BasicMeshTopology,
    chosen_cameras: &[Option<usize>],
    color_samples: &[[Vector3; 3]],
    color_idx: usize,
    vertex_idx: usize,
) -> Option<f64> {
    // Choose an initial guess at the solution, based on a simple color average.
    let known_values: Vec<f64> = topo.faces_around_vertex[vertex_idx]
        .iter()
        .filter_map(|&face_idx| {
            if chosen_cameras[face_idx].is_some() {
                let local_idx = mesh.faces[face_idx]
                    .iter()
                    .position(|&r| r == vertex_idx)
                    .unwrap();
                Some(color_samples[face_idx][local_idx][color_idx])
            } else {
                None
            }
        })
        .collect();
    if !known_values.is_empty() {
        Some(known_values.iter().sum::<f64>() / known_values.len() as f64)
    } else {
        None
    }
}

fn build_initial_guess(
    mesh: &Mesh,
    topo: &BasicMeshTopology,
    chosen_cameras: &[Option<usize>],
    color_samples: &[[Vector3; 3]],
    color_idx: usize,
) -> DVector {
    let guess: Vec<Option<f64>> = (0..mesh.vertices.len())
        .map(|vertex_idx| {
            build_initial_guess_for_single_vertex(
                mesh,
                topo,
                chosen_cameras,
                color_samples,
                color_idx,
                vertex_idx,
            )
        })
        .collect();

    // Fill missing data with nearby values to increase speed of convergence.
    let guess_total = mesh_fill(&guess, mesh, topo, 0.0);

    DVector::from_vec(guess_total)
}

fn conjugate_gradients_solve(
    a: &CsrMatrix,
    b: DVector,
    x0: DVector,
    steps: usize,
) -> DVector {
    // Solve the system ax = b, where a is a sparse positive definite matrix.
    // Following Wikipedia's "Example code in MATLAB / GNU Octave".
    assert!(
        a.nrows() == a.ncols()
            && a.nrows() == b.nrows()
            && a.nrows() == x0.nrows()
    );

    let mut x = x0;
    let mut r = b - a * &x;
    let mut p = r.clone();
    let mut rsold = r.dot(&r);

    for _ in 0..steps {
        let ap = a * &p;
        let alpha = rsold / (p.dot(&ap));
        x += alpha * p.clone();
        r -= alpha * ap;
        let rsnew = r.dot(&r);
        p = r.clone() + (rsnew / rsold) * p;
        rsold = rsnew;
    }

    x
}

pub struct ColorCorrection {
    // The implementation is hidden and may be changed to improve correction
    // resolution. Currently a piecewise linear correction is used.
    face_vertex_color_offsets: Vec<[Vector3; 3]>,
}

impl ColorCorrection {
    pub fn new(
        mesh: &Mesh,
        topo: &BasicMeshTopology,
        vertex_metrics: &[FrameMetrics],
        chosen_cameras: &[Option<usize>],
        images: &[Option<RgbImage>],
        color_correction_steps: usize,
        color_correction_final_offset: bool,
    ) -> ColorCorrection {
        if color_correction_steps == 0 {
            return ColorCorrection {
                face_vertex_color_offsets: vec![
                    [Vector3::zeros(); 3];
                    mesh.faces.len()
                ],
            };
        }

        // Formulate a system of linear equations to minimize the
        // surface integral of the squared norm of the correction gradient.
        let discontinuous_laplacian = build_discontinuous_laplacian(mesh);
        let face_vertex_to_vertex =
            build_face_vertex_to_vertex_correspondence(mesh, topo);
        let continuous_laplacian = &face_vertex_to_vertex.transpose()
            * &discontinuous_laplacian
            * &face_vertex_to_vertex;

        let color_samples = build_color_sample_array(
            mesh,
            vertex_metrics,
            chosen_cameras,
            images,
        );

        let mut face_vertex_color_offsets =
            vec![[Vector3::zeros(); 3]; mesh.faces.len()];
        for color_idx in 0..3 {
            // Solve the above defined system of linear equations.
            let discontinuous_pre_rhs = DVector::from_vec(
                color_samples
                    .iter()
                    .flat_map(|f| f.map(|c| c[color_idx]))
                    .collect(),
            );
            let discontinuous_rhs =
                &discontinuous_laplacian * discontinuous_pre_rhs;
            let continuous_rhs =
                face_vertex_to_vertex.transpose() * discontinuous_rhs;
            let x0 = build_initial_guess(
                mesh,
                topo,
                chosen_cameras,
                &color_samples,
                color_idx,
            );
            let continuous_lhs = conjugate_gradients_solve(
                &continuous_laplacian,
                continuous_rhs,
                x0,
                color_correction_steps - 1,
            );
            let discontinuous_lhs = &face_vertex_to_vertex * continuous_lhs;

            // Store the solution.
            for face_idx in 0..mesh.faces.len() {
                for local_idx in 0..3 {
                    face_vertex_color_offsets[face_idx][local_idx][color_idx] =
                        discontinuous_lhs[face_idx * 3 + local_idx]
                            - color_samples[face_idx][local_idx][color_idx];
                }
            }
        }

        // Offset the solution to be maximally consistent with the samples.
        if color_correction_final_offset {
            let known_color_offsets: Vec<Vector3> = face_vertex_color_offsets
                .iter()
                .enumerate()
                .filter_map(|(face_idx, &colors)| {
                    if chosen_cameras[face_idx].is_some() {
                        Some(colors)
                    } else {
                        None
                    }
                })
                .flatten()
                .collect();
            let average_color_offset =
                known_color_offsets.iter().sum::<Vector3>()
                    / f64::max(known_color_offsets.len() as f64, 1.0);
            let average_color_offset_grayscale = Vector3::from_element(
                average_color_offset.iter().sum::<f64>() / 3.0,
            );
            for i in face_vertex_color_offsets.iter_mut().flatten() {
                *i -= average_color_offset_grayscale;
            }
        }

        ColorCorrection {
            face_vertex_color_offsets,
        }
    }

    pub fn sample_color_offset(
        &self,
        face_idx: usize,
        barycentric_coordinates: Vector3,
    ) -> Vector3 {
        let face = self.face_vertex_color_offsets[face_idx];
        let co = barycentric_coordinates;
        face[0] * co[0] + face[1] * co[1] + face[2] * co[2]
    }
}
