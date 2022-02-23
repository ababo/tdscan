// This file contains imports, typedefs and simple functionality that is common
// to all modules of the mesh texturing process.

pub use base::fm;
pub use crate::mesh::Mesh;
pub use crate::point_cloud::PointCloudParams;
pub use crate::texturing::input_projection::project_like_camera;
pub use crate::misc::extract_biggest_partition_component;
pub use crate::misc::vec_inv;

pub use image::RgbImage;
pub use image::Rgb;

pub use kiddo::KdTree;
pub use kiddo::distance::squared_euclidean;
pub use indexmap::map::IndexMap;
pub use petgraph::unionfind::UnionFind;
pub use std::collections::BinaryHeap;
pub use std::collections::HashMap;
pub use std::collections::HashSet;

pub use std::path::Path;
pub use std::io;
pub use std::io::prelude::*;
pub use std::fs::File;
pub use std::ffi::OsStr;

pub use std::time::{Duration, Instant};
pub use std::f64::consts::PI;
pub use std::convert::From;
pub use std::iter::FromIterator;

pub type VertexIdx = usize;
pub type FaceIdx = usize;
pub type Edge = [VertexIdx; 2];

pub type UV = Vector2;
pub type UVIdx = usize;
pub type FrameIdx = usize;

pub type Vector3 = nalgebra::Vector3<f64>;
pub type Point3 = nalgebra::Point3<f64>;
pub type Quaternion = nalgebra::UnitQuaternion<f64>;
pub type Matrix4 = nalgebra::Matrix4<f64>;

pub type Vector2 = nalgebra::Vector2<f64>;
pub type Matrix2 = nalgebra::Matrix2<f64>;

pub type Point2 = nalgebra::Vector2<f64>;
pub type Depth = f64;

pub use nalgebra::vector;
pub use nalgebra::Matrix3;
pub use nalgebra::Matrix;
pub use nalgebra::OMatrix;
pub use nalgebra::Dynamic;
pub use nalgebra::SVD;
pub use nalgebra::U3;

pub use std::ops::Sub;

use nalgebra::Const;
use nalgebra::ArrayStorage;
pub type Vector<const D: usize> =
    nalgebra::Vector<f64, Const<D>, ArrayStorage<f64, D, 1>>;

pub fn dominant_vector<const D: usize>(vs: &Vec<Vector<D>>) -> Vector<D> {
    // : OMatrix<f64, nalgebra::Const<D>, Dynamic>
    let mat = Matrix::from_columns(&vs);
    let svd = nalgebra::SVD::new(mat, true, false);
    let u = svd.u.unwrap();
    Vector::<D>::from(u.column(0))
}

pub fn fm_point3_to_point3(p: &fm::Point3) -> Point3 {
    Point3::new(p.x as f64, p.y as f64, p.z as f64)
}

pub fn split_option<T, U>(otu: Option<(T, U)>) -> (Option<T>, Option<U>) {
    if let Some((t, u)) = otu {
        (Some(t), Some(u))
    } else {
        (None, None)
    }
}

pub fn idxs_to_mask(len: usize, idxs: Vec<usize>) -> Vec<bool> {
    let mut mask = vec![false; len];
    for i in idxs {
        mask[i] = true;
    }
    mask
}

pub fn mask_to_idxs(mask: &Vec<bool>) -> Vec<usize> {
    mask.iter().enumerate().filter_map(|(i, &b)|
        if b { Some(i) } else { None }
    ).collect()
}

pub fn complement(u0: Vector3) -> (Vector3, Vector3) {
    let zero = vector![0.0, 0.0, 0.0];
    let m33 = Matrix3::from_columns(&[u0, zero, zero]);
    let svd = SVD::new(m33, true, false);
    let u = svd.u.unwrap();
    (Vector3::from(u.column(1)), Vector3::from(u.column(2)))
}

pub fn split_point2_depth(c: Vector3) -> (Point2, Depth) {
    (Point2::new(c[0], c[1]), c[2])
}

pub struct BarycentricCoordinateSystem {
    vs: [Vector2; 3],
    n22: nalgebra::QR<f64, nalgebra::U2, nalgebra::U2>
}

impl BarycentricCoordinateSystem {
    pub fn try_new(vs: [Vector2; 3]) -> Option<Self> {
        let m22 = Matrix2::from_columns(&[vs[1]-vs[0], vs[2]-vs[0]]);
        let n22 = m22.qr();
        if n22.is_invertible() {
            Some(Self { vs, n22 })
        } else {
            None  // vectors v[0] and v[1] are parallel
        }
    }
    
    // 'infer' and 'apply' are mutually inverse
    
    pub fn infer(&self, v: Vector2) -> Vector3 {
        let &[l1, l2] = self.n22.solve(&(v - self.vs[0])).unwrap().as_ref();
        Vector3::new(1.0-l1-l2, l1, l2)
    }

    // (assuming the input 'u' sums to 1.0)
    pub fn apply(&self, u: Vector3) -> Vector2 {
        u[0]*self.vs[0] + u[1]*self.vs[1] + u[2]*self.vs[2]
    }
}

pub fn all_nonneg(v: Vector3) -> bool {
    v.iter().all(|&c| c >= 0.0)
}

pub fn try_load_frame_image(sf: &fm::ScanFrame) -> Option<image::RgbImage> {
    let im = sf.image.clone()?;
    
    use std::io::Cursor;
    use image::io::Reader as ImageReader;
    let img = ImageReader::new(Cursor::new(im.data))
        .with_guessed_format().ok()?
        .decode().ok()?;
    Some(img.into_rgb8())
}

pub fn try_load_all_frame_images(
    sfs: &Vec<fm::ScanFrame>
) -> Vec<Option<image::RgbImage>> {
    sfs.iter().map(|sf| try_load_frame_image(sf)).collect()
}

pub fn ordered(e: Edge) -> Edge {
    if e[0] < e[1] { e } else { [e[1], e[0]] }
}

pub fn get_pixel_ij_as_vector3(i: u32, j: u32, image: &RgbImage) -> Vector3 {
    let (x, y) = (j, i);  // beware: transposing indices
    let p = image.get_pixel(x, y);
    Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64)
}

pub fn set_pixel_ij_as_vector3(
    i: u32,
    j: u32,
    color: Vector3,
    image: &mut RgbImage
) {
    let (x, y) = (j, i);  // beware: transposing indices
    let [r, g, b] = color.as_ref();
    let r1 = r.clamp(0.0, 255.0).round() as u8;
    let g1 = g.clamp(0.0, 255.0).round() as u8;
    let b1 = b.clamp(0.0, 255.0).round() as u8;
    image.put_pixel(x, y, Rgb([r1, g1, b1]));
}

pub fn sample_pixel(uv: Point2, image: &RgbImage) -> Vector3 {
    let (w, h) = image.dimensions();
    let (i, j) = (uv[0].clamp(0.0, 1.0) * h as f64,
                  uv[1].clamp(0.0, 1.0) * w as f64);
    let (i0, i1) = ((i as u32).clamp(0, h-1), (i as u32 + 1).clamp(0, h-1));
    let (j0, j1) = ((j as u32).clamp(0, w-1), (j as u32 + 1).clamp(0, w-1));
    let (di, dj) = (i - i0 as f64, j - j0 as f64);
    let s00 = get_pixel_ij_as_vector3(i0, j0, &image);
    let s01 = get_pixel_ij_as_vector3(i0, j1, &image);
    let s10 = get_pixel_ij_as_vector3(i1, j0, &image);
    let s11 = get_pixel_ij_as_vector3(i1, j1, &image);
    let s0_ = (1.0 - dj)*s00 + dj*s01;
    let s1_ = (1.0 - dj)*s10 + dj*s11;
    (1.0 - di)*s0_ + di*s1_
}

pub fn uv_to_ij(uv: UV, img: &RgbImage) -> Vector2 {
    let (dimx, dimy) = img.dimensions();  // beware that x comes before y here
    let [u, v] = uv.as_ref();
    Vector2::new(dimy as f64 * u.clamp(0.0, 1.0),
                 dimx as f64 * v.clamp(0.0, 1.0))
}

pub fn ij_to_uv(ij: Vector2, img: &RgbImage) -> Vector2 {
    let (dimx, dimy) = img.dimensions();  // beware that x comes before y here
    let [i, j] = ij.as_ref();
    UV::new(i / dimy as f64,
            j / dimx as f64)
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Ord, PartialOrd, Copy)]
pub struct Rectangle<T> {
    pub pos: [T; 2],
    pub size: [T; 2]
}

impl<T> Rectangle<T> {
    pub fn bounding(ijs: &Vec<[T; 2]>) -> Rectangle<T>
        where T: PartialOrd, T: Copy, T: Sub<Output = T>
    {
        let imin = ijs.iter().map(|ij| ij[0])
            .min_by(|p, q| p.partial_cmp(&q).unwrap()).unwrap();
        let imax = ijs.iter().map(|ij| ij[0])
            .max_by(|p, q| p.partial_cmp(&q).unwrap()).unwrap();
        let jmin = ijs.iter().map(|ij| ij[1])
            .min_by(|p, q| p.partial_cmp(&q).unwrap()).unwrap();
        let jmax = ijs.iter().map(|ij| ij[1])
            .max_by(|p, q| p.partial_cmp(&q).unwrap()).unwrap();
        Rectangle { pos: [imin, jmin], size: [imax - imin, jmax - jmin] }
    }
}

impl Rectangle<u32> {
    pub fn to_f64(&self) -> Rectangle<f64> {
        Rectangle {
            pos: [self.pos[0] as f64, self.pos[1] as f64],
            size: [self.size[0] as f64, self.size[1] as f64]
        }
    }
}

impl Rectangle<f64> {
    pub fn to_ij(&self, img: &RgbImage) -> Rectangle<usize> {
        let pos_uv = UV::new(self.pos[0], self.pos[1]);
        let pos_ij = uv_to_ij(pos_uv , img);
        let size_uv = UV::new(self.size[0], self.size[1]);
        let size_ij = uv_to_ij(size_uv, img);
        Rectangle {
            pos: [pos_ij[0] as usize, pos_ij[1] as usize],
            size: [size_ij[0] as usize, size_ij[1] as usize]
        }
    }
}

#[derive(Debug, Clone)]
pub struct BasicMeshTopology {
    pub faces_around_vertex: Vec<HashSet<usize>>,
    pub faces_around_edge: HashMap<Edge, Vec<FaceIdx>>,
    pub neighbouring_faces: Vec<HashSet<usize>>
}

impl BasicMeshTopology {
    pub fn make(mesh: &Mesh) -> BasicMeshTopology {
        let mut faces_around_vertex = vec![HashSet::new(); mesh.vertices.len()];
        for (f_idx, &f) in mesh.faces.iter().enumerate() {
            for v in f {
                faces_around_vertex[v].insert(f_idx);
            }
        }

        let mut faces_around_edge: HashMap<Edge, Vec<FaceIdx>> = HashMap::new();
        for (f_idx, &[v0, v1, v2]) in mesh.faces.iter().enumerate() {
            for e in [[v0, v1], [v0, v2], [v1, v2]] {
                faces_around_edge.entry(ordered(e)).or_insert(vec![]);
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
            neighbouring_faces
        }
    }
}

// Assuming f(bounds[0]) fails (typically), and f(bounds[1]) succeeds.
// The bounds are allowed to come in any order.
pub fn bisect<T>(
    f: impl Fn(f64) -> Option<T>,
    bounds: [f64; 2],
    rtol: f64
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

    return (succeeds, best_result)
}
