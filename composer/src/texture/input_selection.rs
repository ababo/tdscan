use crate::texture::misc::*;

pub fn project_like_camera(
    scan: &fm::Scan,
    frame: &fm::ScanFrame,
    points: &[Point3],
) -> Vec<ProjectedPoint> {
    let tan = (scan.camera_angle_of_view as f64 / 2.0).tan();

    let eye =
        fm_point3_to_point3(&scan.camera_initial_position.unwrap_or_default());
    let dir =
        fm_point3_to_point3(&scan.camera_initial_direction.unwrap_or_default());
    let up_rot = Quaternion::from_axis_angle(
        &Vector3::z_axis(),
        scan.camera_up_angle as f64,
    );
    let look_rot =
        Matrix4::look_at_rh(&eye, &dir, &Vector3::new(0.0, 0.0, 1.0));
    let view_rot = look_rot.try_inverse().unwrap() * Matrix4::from(up_rot);

    let camera_angle =
        frame.time as f64 / 1E9 * scan.camera_angular_velocity as f64;
    let time_rot =
        Quaternion::from_axis_angle(&Vector3::z_axis(), camera_angle);

    let view_rot_3x3_inv = view_rot.fixed_slice::<3, 3>(0, 0).transpose();
    let time_rot_3x3_inv = Matrix4::from(time_rot)
        .fixed_slice::<3, 3>(0, 0)
        .transpose();

    let depth_width = scan.depth_width as f64;
    let depth_height = scan.depth_height as f64;

    points
        .iter()
        .map(|point3d| {
            // Undo rigid 3d transformations.
            let frame_real = time_rot_3x3_inv * point3d;
            let frame = view_rot_3x3_inv * (frame_real - eye);

            // Redo camera screen projection.
            let depth = -frame.z;
            let u = frame.x / depth;
            let v = -frame.y / depth;

            // Apply camera field of view.
            let w = u * (depth_width / 2.0) / tan;
            let h = v * (depth_width / 2.0) / tan;

            // Standardize to the interval [0, 1].
            let i = (h + depth_height / 2.0) / depth_height;
            let j = (w + depth_width / 2.0) / depth_width;

            ProjectedPoint {
                point: Vector2::new(i, j),
                depth,
            }
        })
        .collect()
}

// More code to come below...
