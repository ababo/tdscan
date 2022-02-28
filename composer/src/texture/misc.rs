// This file contains imports, typedefs and simple functionality that is common
// to all modules of the mesh texturing process.

//pub use std::ffi::OsStr;
pub use std::fs::File;
pub use std::io;
pub use std::io::prelude::*;
pub use std::path::Path;

//pub use std::collections::BinaryHeap;
//pub use std::collections::HashMap;
//pub use std::collections::HashSet;

//pub use std::convert::From;
//pub use std::f64::consts::PI;
//pub use std::iter::FromIterator;
//pub use std::time::{Duration, Instant};

//pub use std::ops::Sub;


pub use image::Rgb;
pub use image::RgbImage;

pub use indexmap::map::IndexMap;
pub use kiddo::distance::squared_euclidean;
pub use kiddo::KdTree;
//pub use petgraph::unionfind::UnionFind;

//pub use nalgebra::vector;
//pub use nalgebra::Dynamic;
//pub use nalgebra::Matrix;
//pub use nalgebra::Matrix3;
//pub use nalgebra::OMatrix;
//pub use nalgebra::SVD;
//pub use nalgebra::U3;

//use nalgebra::ArrayStorage;
//use nalgebra::Const;
//pub type Vector<const D: usize> =
//    nalgebra::Vector<f64, Const<D>, ArrayStorage<f64, D, 1>>;


pub use crate::mesh::Mesh;
//pub use crate::point_cloud::PointCloudParams;
pub use base::fm;
//pub use crate::misc::extract_biggest_partition_component;
//pub use crate::misc::vec_inv;


pub type Vector3 = nalgebra::Vector3<f64>;
pub type Point3 = nalgebra::Point3<f64>;
pub type Quaternion = nalgebra::UnitQuaternion<f64>;
pub type Matrix4 = nalgebra::Matrix4<f64>;

pub type Vector2 = nalgebra::Vector2<f64>;
//pub type Matrix2 = nalgebra::Matrix2<f64>;

pub struct ProjectedPoint {
    pub point: Vector2,
    pub depth: f64,
}


pub fn fm_point3_to_point3(p: &fm::Point3) -> Point3 {
    Point3::new(p.x as f64, p.y as f64, p.z as f64)
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
    let s0_ = (1.0 - dj) * s00 + dj * s01;
    let s1_ = (1.0 - dj) * s10 + dj * s11;
    (1.0 - di) * s0_ + di * s1_
}

pub fn try_load_frame_image(sf: &fm::ScanFrame) -> Option<image::RgbImage> {
    let im = sf.image.clone()?;

    use image::io::Reader as ImageReader;
    use std::io::Cursor;
    let img = ImageReader::new(Cursor::new(im.data))
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
