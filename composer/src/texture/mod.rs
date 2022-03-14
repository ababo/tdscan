mod input_selection;
mod output_baking;
mod output_packing;
mod output_patching;
mod textured_mesh;

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::ops::Sub;

use image::io::Reader as ImageReader;
use image::{Rgb, RgbImage};
use nalgebra::{ArrayStorage, Const, Matrix, Matrix3, SVD, vector};

use crate::mesh::Mesh;
pub use crate::texture::{
    input_selection::*,
    output_baking::*,
    output_packing::*,
    output_patching::*,
    textured_mesh::*,
};
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

pub fn load_all_frame_images(
    frames: &[fm::ScanFrame],
) -> Vec<Option<image::RgbImage>> {
    frames.iter().map(load_frame_image).collect()
}

pub fn get_pixel_ij_as_vector3(i: u32, j: u32, image: &RgbImage) -> Vector3 {
    let (x, y) = (j, i); // Beware: Transposing indices.
    let p = image.get_pixel(x, y);
    Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64)
}

pub fn set_pixel_ij_as_vector3(
    i: u32,
    j: u32,
    color: Vector3,
    image: &mut RgbImage,
) {
    let (x, y) = (j, i); // Beware: Transposing indices.
    let [r, g, b] = color.as_ref();
    let r1 = r.clamp(0.0, 255.0).round() as u8;
    let g1 = g.clamp(0.0, 255.0).round() as u8;
    let b1 = b.clamp(0.0, 255.0).round() as u8;
    image.put_pixel(x, y, Rgb([r1, g1, b1]));
}

#[derive(Debug, Clone)]
pub struct BasicMeshTopology {
    pub faces_around_vertex: Vec<HashSet<usize>>,
    pub faces_around_edge: HashMap<[usize; 2], Vec<usize>>,
    pub neighbouring_faces: Vec<HashSet<usize>>,
}

impl BasicMeshTopology {
    pub fn new(mesh: &Mesh) -> BasicMeshTopology {
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
    pub fn new(vs: [Vector2; 3]) -> Option<Self> {
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

    // Assuming the input 'u' sums to 1.0.
    pub fn apply(&self, u: Vector3) -> Vector2 {
        u[0] * self.vs[0] + u[1] * self.vs[1] + u[2] * self.vs[2]
    }
}

pub fn all_nonneg(v: Vector3) -> bool {
    v.iter().all(|&c| c >= 0.0)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Rectangle<T> {
    pub pos: [T; 2],
    pub size: [T; 2],
}

type Comparator<T> = fn(&T, &T) -> Ordering;

pub fn extremum<T: Copy+PartialOrd+Sub<Output = T>, I: Iterator<Item = T>>(
    it: I,
    f: fn(I, Comparator<T>) -> Option<T>,
) -> T {
    f(it, |p, q| p.partial_cmp(q).unwrap()).unwrap()
}

impl<T> Rectangle<T> {
    pub fn bounding(ijs: &[[T; 2]]) -> Rectangle<T>
    where
        T: Copy+PartialOrd+Sub<Output = T>
    {    
        let ijs_coord = |k: usize| ijs.iter().map(move |ij| ij[k]);
        
        let imin = extremum(ijs_coord(0), Iterator::min_by);
        let imax = extremum(ijs_coord(0), Iterator::max_by);
        let jmin = extremum(ijs_coord(1), Iterator::min_by);
        let jmax = extremum(ijs_coord(1), Iterator::max_by);
        
        Rectangle {
            pos: [imin, jmin],
            size: [imax - imin, jmax - jmin],
        }
    }
}

pub fn dominant_vector<const D: usize>(vs: &[Vector<D>]) -> Vector<D> {
    // This has type OMatrix<f64, nalgebra::Const<D>, Dynamic>.
    let mat = Matrix::from_columns(vs);
    let svd = nalgebra::SVD::new(mat, true, false);
    let u = svd.u.unwrap();
    Vector::from(u.column(0))
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

// Assuming f(bounds[0]) fails (typically), and f(bounds[1]) succeeds.
// The bounds are allowed to come in any order.
pub fn bisect<T>(
    f: impl Fn(f64) -> Option<T>,
    bounds: [f64; 2],
    rtol: f64,
) -> (f64, T) {
    let tol = rtol * f64::max(bounds[0].abs(), bounds[1].abs());

    let [mut fails, mut succeeds] = bounds;
    let mut best_result = f(bounds[1]).unwrap();

    while (fails - succeeds).abs() > tol {
        let next = (fails + succeeds) / 2.0;
        if let Some(new_best_result) = f(next) {
            best_result = new_best_result;
            succeeds = next;
        } else {
            fails = next;
        }
    }

    (succeeds, best_result)
}

pub fn uv_to_ij(uv: Vector2, img: &RgbImage) -> Vector2 {
    let (dimx, dimy) = img.dimensions(); // Beware that x comes before y here.
    let [u, v] = uv.as_ref();
    Vector2::new(
        dimy as f64 * u.clamp(0.0, 1.0),
        dimx as f64 * v.clamp(0.0, 1.0),
    )
}

pub fn ij_to_uv(ij: Vector2, img: &RgbImage) -> Vector2 {
    let (dimx, dimy) = img.dimensions(); // Beware that x comes before y here.
    let [i, j] = ij.as_ref();
    Vector2::new(i / dimy as f64, j / dimx as f64)
}
