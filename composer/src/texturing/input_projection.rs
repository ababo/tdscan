use crate::texturing::misc::*;

pub fn project_like_camera(
    scan: &fm::Scan,
    frame: &fm::ScanFrame,
    points: &Vec<Point3>,
) -> Vec<(Point2, Depth)> {
    let depth_width = scan.depth_width as usize;
    let depth_height = scan.depth_height as usize;
    
    let tan = (scan.camera_angle_of_view as f64 / 2.0).tan();
    
    let eye =
        fm_point3_to_point3(&scan.camera_initial_position
                            .unwrap_or_default());
    let dir =
        fm_point3_to_point3(&scan.camera_initial_direction
                            .unwrap_or_default());
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
    
    let view_rot_3x3_inv =
        view_rot.fixed_slice::<3, 3>(0, 0).transpose();
    let time_rot_3x3_inv =
        Matrix4::from(time_rot).fixed_slice::<3, 3>(0, 0).transpose();
    
    let mut projected = Vec::with_capacity(points.len());
    for point3d in points {
        // undo rigid 3d transformations
        let frame_real = time_rot_3x3_inv * point3d;
        let frame = view_rot_3x3_inv * (frame_real - eye);
        
        // redo camera screen projection
        let depth = -frame.z;
        let u = frame.x / depth;
        let v = -frame.y / depth;
        
        // the following is disabled for the sake of standardization,
        // i.e. to avoid making texturing needlessly complicated
        /*
        // If depth sensor measures distance rather than depth.
        if !scan.sensor_plane_depth {
            depth *= (1.0 + u * u + v * v).sqrt();
        }
         */
        
        // apply camera field of view
        let w = u * (depth_width as f64 / 2.0) / tan;
        let h = v * (depth_width as f64 / 2.0) / tan;
        
        // standardize to the interval [0,1)
        let i = (h + depth_height as f64 / 2.0) / depth_height as f64;
        let j = (w + depth_width as f64 / 2.0) / depth_width as f64;
        let point2d = Point2::new(i, j);
        
        projected.push((point2d, depth));
    }
    projected
}
