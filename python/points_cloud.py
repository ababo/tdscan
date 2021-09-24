import numpy as np
from numpy.linalg import norm
from scipy.spatial.transform.rotation import Rotation
from tqdm import tqdm

# frame keys
__depth_confidences = 'depth_confidences'
__depths = 'depths'
__scan = 'scan'
__time = 'time'

# scan keys
__camera_angle_of_view = 'camera_angle_of_view'
__camera_angular_velocity = 'camera_angular_velocity'
__camera_initial_position = 'camera_initial_position'
__camera_landscape_angle = 'camera_landscape_angle'
__camera_view_elevation = 'camera_view_elevation'
__depth_height = 'depth_height'
__depth_width = 'depth_width'
__sensor_plane_depth = 'sensor_plane_depth'

__eps = 1e-10


class PointsCloudParams:
    def __init__(
            self, min_depth_confidence=3, min_z=-np.inf,
            max_z=np.inf, max_z_distance=np.inf
    ):
        self.min_depth_confidence = min_depth_confidence
        self.min_z = min_z
        self.max_z = max_z
        self.max_z_distance = max_z_distance


def build_points_cloud(all_scans, all_frames, params=None, use_tqdm=False):
    init_output = __init_build_points_cloud(all_frames, params, use_tqdm)
    params, all_points, time_base, all_frames_to_iter = init_output
    for frame in all_frames_to_iter:
        scan = all_scans[frame[__scan]]
        points = __get_points(params, scan, frame, time_base)
        all_points.extend(points)
    all_points = np.array(all_points)
    return all_points


def __init_build_points_cloud(all_frames, params=None, use_tqdm=False):
    if params is None:
        params = PointsCloudParams()
    all_points = []
    time_base = all_frames[0][__time]
    if use_tqdm:
        all_frames_to_iter = tqdm(all_frames)
    else:
        all_frames_to_iter = all_frames
    return params, all_points, time_base, all_frames_to_iter


def __get_points(params, scan, frame, time_base):
    tan = __get_tan(scan)
    look, elev = __get_look_and_elev(scan)
    rot = __get_rot(scan, look)
    time_rot = __get_time_rot(scan, frame, time_base)
    i, j, depth_index = __get_indices(scan)
    depth = frame[__depths][depth_index]
    w, h = __get_w_and_h(i, j, scan)
    proj_square = __get_proj_square(w, h)
    if scan[__sensor_plane_depth]:
        depth = __get_depth_sensor_plane(scan, tan, depth, proj_square)
    x, y, z = __get_xyz(scan, tan, w, h, depth, proj_square)
    points = __get_unfiltered_points(x, y, z, look, elev, rot, time_rot)
    mask = __get_mask(params, frame, depth_index, points)
    points = points[mask]
    return points


def __get_tan(scan):
    angle = scan[__camera_angle_of_view]
    angle /= 2.0
    tan = np.tan(angle)
    return tan


def __get_look_and_elev(scan):
    eye = __get_eye(scan)
    elev = __get_elev(scan)
    look = eye - elev
    return look, elev


def __get_eye(scan):
    position = scan[__camera_initial_position]
    eye = [position[coord] for coord in ['x', 'y', 'z']]
    eye = np.array(eye)
    return eye


def __get_elev(scan):
    elev_z = scan[__camera_view_elevation]
    elev = [0.0, 0.0, elev_z]
    elev = np.array(elev)
    return elev


def __get_rot(scan, look):
    look_rot = __get_look_rot(look)
    landscape_rot = __get_landscape_rot(scan)
    rot = look_rot * landscape_rot
    return rot


def __get_look_rot(look):
    look_rot_axis = __get_look_rot_axis(look)
    look_angle = __get_look_angle(look)
    look_rot_vec = look_angle * look_rot_axis
    look_rot = Rotation.from_rotvec(look_rot_vec)
    return look_rot


def __get_look_rot_axis(look):
    if np.abs(look[1]) > __eps:
        slope = -look[0] / look[1]
        look_rot_axis_x = 1.0 / np.sqrt(1 + slope * slope)
        look_rot_axis_y = slope * look_rot_axis_x
    else:
        look_rot_axis_x = 0.0
        look_rot_axis_y = 1.0
    look_rot_axis = np.array([look_rot_axis_x, look_rot_axis_y, 0.0])
    return look_rot_axis


def __get_look_angle(look):
    tan = look[2] / norm(look[:2])
    arctan = np.arctan(tan)
    look_angle = arctan + np.pi / 2
    return look_angle


def __get_landscape_rot(scan):
    angle = scan[__camera_landscape_angle]
    landscape_rot = __rotation_from_euler_z(angle)
    return landscape_rot


def __rotation_from_euler_z(angle, degrees=False):
    return Rotation.from_euler('z', angle, degrees=degrees)


def __get_time_rot(scan, frame, time_base):
    timestamp = (frame[__time] - time_base) / 1e9
    camera_angle = timestamp * scan[__camera_angular_velocity]
    time_rot = __rotation_from_euler_z(camera_angle)
    return time_rot


def __get_indices(scan):
    i = np.arange(scan[__depth_height])
    j = np.arange(scan[__depth_width])
    j, i = np.meshgrid(j, i)
    depth_index = (i * scan[__depth_width] + j)
    return i, j, depth_index


def __get_confidence_mask(params, frame, depth_index):
    confidence = frame[__depth_confidences][depth_index]
    confidence_mask = confidence >= params.min_depth_confidence
    confidence_mask = np.ravel(confidence_mask)
    return confidence_mask


def __get_w_and_h(i, j, scan):
    w = j - scan[__depth_width] / 2.0
    h = i - scan[__depth_height] / 2.0
    return w, h


def __get_proj_square(w, h):
    w_square = w * w
    h_square = h * h
    proj_square = w_square + h_square
    return proj_square


def __get_depth_sensor_plane(scan, tan, depth, proj_square):
    fl = scan[__depth_width] / tan / 2.0
    sqrt_proj_square = np.sqrt(proj_square)
    angle = np.arctan(sqrt_proj_square / fl)
    output = depth / np.cos(angle)
    return output


def __get_xyz(scan, tan, w, h, depth, proj_square):
    denom = np.sqrt(
        scan[__depth_width] * scan[__depth_width]
        + 4.0 * proj_square * tan * tan
    )
    xy_factor = (2.0 * depth * tan) / denom
    x = w * xy_factor
    y = h * xy_factor
    z = depth * scan[__depth_width] / denom
    return x, y, z


def __get_unfiltered_points(x, y, z, look, elev, rot, time_rot):
    points = np.array([x, y, z])
    points = np.transpose(points, axes=[1, 2, 0])
    points = np.reshape(points, (-1, 3))
    points = rot.apply(points) + look + elev
    points = time_rot.apply(points)
    return points


def __get_mask(params, frame, depth_index, points):
    z_dist = norm(points[:, :2], axis=1)
    z_dist_mask = z_dist <= params.max_z_distance
    min_z_mask = points[:, 2] >= params.min_z
    max_z_mask = points[:, 2] <= params.max_z
    confidence_mask = __get_confidence_mask(params, frame, depth_index)
    final_mask = np.logical_and.reduce((
        confidence_mask, z_dist_mask, min_z_mask, max_z_mask
    ))
    return final_mask
