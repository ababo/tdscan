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
//pub use kiddo::distance::squared_euclidean;
//pub use kiddo::KdTree;
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
