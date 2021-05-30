use glam::Vec3;

use base::model;

#[inline]
pub fn point3_to_vec3(point: &model::Point3) -> Vec3 {
    Vec3::new(point.x, point.y, point.z)
}

#[inline]
pub fn vec3_to_point3(vec: &Vec3) -> model::Point3 {
    model::Point3 {
        x: vec[0],
        y: vec[1],
        z: vec[2],
    }
}
