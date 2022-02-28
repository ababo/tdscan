mod input_selection;
mod output_patching;
mod textured_mesh;

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Cursor;
use std::ops::Sub;

use image::io::Reader as ImageReader;
use image::RgbImage;
use nalgebra::ArrayStorage;
use nalgebra::Const;
use nalgebra::Matrix;
use nalgebra::Matrix3;
use nalgebra::SVD;
use nalgebra::vector;

use crate::mesh::Mesh;
pub use crate::texture::input_selection::*;
pub use crate::texture::output_patching::*;
pub use crate::texture::textured_mesh::*;
use base::fm;

pub type Vector3 = nalgebra::Vector3<f64>;
pub type Point3 = nalgebra::Point3<f64>;
pub type Quaternion = nalgebra::UnitQuaternion<f64>;
pub type Matrix4 = nalgebra::Matrix4<f64>;
pub type Vector2 = nalgebra::Vector2<f64>;
pub type Matrix2 = nalgebra::Matrix2<f64>;
pub type Vector<const D: usize> =
    nalgebra::Vector<f64, Const<D>, ArrayStorage<f64, D, 1>>;

pub struct ProjectedPoint {
    pub point: Vector2,
    pub depth: f64,
}

pub fn sample_pixel(uv: Vector2, image: &RgbImage) -> Vector3 {
    let (w, h) = image.dimensions();
    let (i, j) = (
        uv[0].clamp(0.0, 1.0) * h as f64,
        uv[1].clamp(0.0, 1.0) * w as f64,
    );
    let (i0, i1) = ((i as u32).clamp(0, h - 1), (i as u32 + 1).clamp(0, h - 1));
    let (j0, j1) = ((j as u32).clamp(0, w - 1), (j as u32 + 1).clamp(0, w - 1));
    let (di, dj) = (i - i0 as f64, j - j0 as f64);
    let s00 = get_pixel_ij_as_vector3(i0, j0, image);
    let s01 = get_pixel_ij_as_vector3(i0, j1, image);
    let s10 = get_pixel_ij_as_vector3(i1, j0, image);
    let s11 = get_pixel_ij_as_vector3(i1, j1, image);
    let s0 = (1.0 - dj) * s00 + dj * s01;
    let s1 = (1.0 - dj) * s10 + dj * s11;
    (1.0 - di) * s0 + di * s1
}

pub fn load_frame_image(frame: &fm::ScanFrame) -> Option<image::RgbImage> {
    let img = ImageReader::new(Cursor::new(&frame.image.as_ref()?.data))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    Some(img.into_rgb8())
}

pub fn split_option<T, U>(otu: Option<(T, U)>) -> (Option<T>, Option<U>) {
    if let Some((t, u)) = otu {
        (Some(t), Some(u))
    } else {
        (None, None)
    }
}

pub fn get_pixel_ij_as_vector3(i: u32, j: u32, image: &RgbImage) -> Vector3 {
    let (x, y) = (j, i); // Beware: Transposing indices.
    let p = image.get_pixel(x, y);
    Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64)
}

#[derive(Debug, Clone)]
pub struct BasicMeshTopology {
    pub faces_around_vertex: Vec<HashSet<usize>>,
    pub faces_around_edge: HashMap<[usize; 2], Vec<usize>>,
    pub neighbouring_faces: Vec<HashSet<usize>>,
}

impl BasicMeshTopology {
    pub fn make(mesh: &Mesh) -> BasicMeshTopology {
        let mut faces_around_vertex = vec![HashSet::new(); mesh.vertices.len()];
        for (f_idx, &f) in mesh.faces.iter().enumerate() {
            for v in f {
                faces_around_vertex[v].insert(f_idx);
            }
        }

        let mut faces_around_edge = HashMap::new();
        for (f_idx, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
            for e in [[v0, v1], [v0, v2], [v1, v2]] {
                faces_around_edge.entry(ordered(e)).or_insert_with(Vec::new);
                faces_around_edge.get_mut(&ordered(e)).unwrap().push(f_idx);
            }
        }

        let mut neighbouring_faces = vec![HashSet::new(); mesh.faces.len()];
        for (f_idx, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
            for e in [[v0, v1], [v0, v2], [v1, v2]] {
                for f_idx_ in faces_around_edge[&ordered(e)].clone() {
                    if f_idx != f_idx_ {
                        neighbouring_faces[f_idx].insert(f_idx_);
                    }
                }
            }
        }
        BasicMeshTopology {
            faces_around_vertex,
            faces_around_edge,
            neighbouring_faces,
        }
    }
}

pub struct BarycentricCoordinateSystem {
    vs: [Vector2; 3],
    n22: nalgebra::QR<f64, nalgebra::U2, nalgebra::U2>,
}

impl BarycentricCoordinateSystem {
    pub fn try_new(vs: [Vector2; 3]) -> Option<Self> {
        let m22 = Matrix2::from_columns(&[vs[1] - vs[0], vs[2] - vs[0]]);
        let n22 = m22.qr();
        if n22.is_invertible() {
            Some(Self { vs, n22 })
        } else {
            None // Vectors v[0] and v[1] are parallel.
        }
    }

    // The functions 'infer' and 'apply' are mutually inverse.

    pub fn infer(&self, v: Vector2) -> Vector3 {
        let &[l1, l2] = self.n22.solve(&(v - self.vs[0])).unwrap().as_ref();
        Vector3::new(1.0 - l1 - l2, l1, l2)
    }

    // (Assuming the input 'u' sums to 1.0.)
    pub fn apply(&self, u: Vector3) -> Vector2 {
        u[0] * self.vs[0] + u[1] * self.vs[1] + u[2] * self.vs[2]
    }
}

pub fn all_nonneg(v: Vector3) -> bool {
    v.iter().all(|&c| c >= 0.0)
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Ord, PartialOrd, Copy)]
pub struct Rectangle<T> {
    pub pos: [T; 2],
    pub size: [T; 2],
}

impl<T> Rectangle<T> {
    pub fn bounding(ijs: &[[T; 2]]) -> Rectangle<T>
    where
        T: PartialOrd,
        T: Copy,
        T: Sub<Output = T>,
    {
        let imin = ijs
            .iter()
            .map(|ij| ij[0])
            .min_by(|p, q| p.partial_cmp(q).unwrap())
            .unwrap();
        let imax = ijs
            .iter()
            .map(|ij| ij[0])
            .max_by(|p, q| p.partial_cmp(q).unwrap())
            .unwrap();
        let jmin = ijs
            .iter()
            .map(|ij| ij[1])
            .min_by(|p, q| p.partial_cmp(q).unwrap())
            .unwrap();
        let jmax = ijs
            .iter()
            .map(|ij| ij[1])
            .max_by(|p, q| p.partial_cmp(q).unwrap())
            .unwrap();
        Rectangle {
            pos: [imin, jmin],
            size: [imax - imin, jmax - jmin],
        }
    }
}

pub fn dominant_vector<const D: usize>(vs: &[Vector<D>]) -> Vector<D> {
    // : OMatrix<f64, nalgebra::Const<D>, Dynamic>
    let mat = Matrix::from_columns(vs);
    let svd = nalgebra::SVD::new(mat, true, false);
    let u = svd.u.unwrap();
    Vector::<D>::from(u.column(0))
}

pub fn complement(u0: Vector3) -> (Vector3, Vector3) {
    let zero = vector![0.0, 0.0, 0.0];
    let m33 = Matrix3::from_columns(&[u0, zero, zero]);
    let svd = SVD::new(m33, true, false);
    let u = svd.u.unwrap();
    (Vector3::from(u.column(1)), Vector3::from(u.column(2)))
}

pub fn split_point2_depth(c: Vector3) -> ProjectedPoint {
    ProjectedPoint {
        point: Vector2::new(c[0], c[1]),
        depth: c[2],
    }
}

pub fn idxs_to_mask(len: usize, idxs: &[usize]) -> Vec<bool> {
    let mut mask = vec![false; len];
    for &i in idxs {
        mask[i] = true;
    }
    mask
}

pub fn mask_to_idxs(mask: &[bool]) -> Vec<usize> {
    mask.iter()
        .enumerate()
        .filter_map(|(i, &b)| if b { Some(i) } else { None })
        .collect()
}

pub fn ordered(e: [usize; 2]) -> [usize; 2] {
    if e[0] < e[1] {
        e
    } else {
        [e[1], e[0]]
    }
}
