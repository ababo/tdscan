mod input_selection;
mod textured_mesh;

use std::io::Cursor;

use image::io::Reader as ImageReader;
use image::RgbImage;

pub use crate::texture::input_selection::*;
pub use crate::texture::textured_mesh::*;
use base::fm;

pub type Vector3 = nalgebra::Vector3<f64>;
pub type Point3 = nalgebra::Point3<f64>;
pub type Quaternion = nalgebra::UnitQuaternion<f64>;
pub type Matrix4 = nalgebra::Matrix4<f64>;
pub type Vector2 = nalgebra::Vector2<f64>;

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
    let s0_ = (1.0 - dj) * s00 + dj * s01;
    let s1 = (1.0 - dj) * s10 + dj * s11;
    (1.0 - di) * s0_ + di * s1
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
