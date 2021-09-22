from json import loads as json_to_dict

import numpy as np
from numpy.linalg import norm
from scipy.spatial.transform.rotation import Rotation
from tqdm import tqdm

# frame keys
__time = 'time'
__scan = 'scan'
__depth_confidences = 'depth_confidences'
__depths = 'depths'

# scan keys
__camera_angle_of_view = 'camera_angle_of_view'
__camera_angular_velocity = 'camera_angular_velocity'
__camera_initial_position = 'camera_initial_position'
__camera_landscape_angle = 'camera_landscape_angle'
__camera_view_elevation = 'camera_view_elevation'
__depth_height = 'depth_height'
__depth_width = 'depth_width'
__sensor_plane_depth = 'sensor_plane_depth'

# additional keys
__Scan = 'Scan'
__ScanFrame = 'ScanFrame'
__type = 'type'
__name = 'name'

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


def build_points_cloud(all_scans, all_frames, params=None, use_tqdm=True):
    if params is None:
        params = PointsCloudParams()

    points = []
    time_base = all_frames[0][__time]
    if use_tqdm:
        all_frames_to_iter = tqdm(all_frames)
    else:
        all_frames_to_iter = all_frames

    for frame in all_frames_to_iter:
        scan = all_scans[frame[__scan]]
        tan = np.tan(scan[__camera_angle_of_view] / 2.0)

        landscape_rot = __rotation_from_euler_z(scan[__camera_landscape_angle])
        eye = np.array([
            scan[__camera_initial_position][coord]
            for coord in ['x', 'y', 'z']
        ])
        elev = np.array([0.0, 0.0, scan[__camera_view_elevation]])
        look = eye - elev
        if np.abs(look[1]) > __eps:
            slope = -look[0] / look[1]
            look_rot_axis_x = 1.0 / np.sqrt(1 + slope * slope)
            look_rot_axis_y = slope * look_rot_axis_x
        else:
            look_rot_axis_x = 0.0
            look_rot_axis_y = 1.0
        look_rot_axis = np.array([look_rot_axis_x, look_rot_axis_y, 0.0])
        look_angle = np.arctan(look[2] / norm(look[:2])) + np.pi / 2
        look_rot_vec = look_angle * look_rot_axis
        look_rot = Rotation.from_rotvec(look_rot_vec)
        rot = look_rot * landscape_rot

        timestamp = (frame[__time] - time_base) / 1e9
        camera_angle = timestamp * scan[__camera_angular_velocity]
        time_rot = __rotation_from_euler_z(camera_angle)

        for i in range(0, scan[__depth_height]):
            for j in range(0, scan[__depth_width]):
                depth_index = (i * scan[__depth_width] + j)
                confidence = frame[__depth_confidences][depth_index]
                if confidence < params.min_depth_confidence:
                    continue

                depth = frame[__depths][depth_index]
                w = j - scan[__depth_width] / 2.0
                h = i - scan[__depth_height] / 2.0
                proj_square = w*w + h*h
                if scan[__sensor_plane_depth]:
                    fl = scan[__depth_width] / tan / 2.0
                    depth /= np.cos(np.arctan(np.sqrt(proj_square) / fl))

                denom = np.sqrt(
                    scan[__depth_width] * scan[__depth_width]
                    + 4.0 * proj_square * tan * tan
                )
                xy_factor = (2.0 * depth * tan) / denom
                x = w * xy_factor
                y = h * xy_factor
                z = depth * scan[__depth_width] / denom

                point = rot.apply(np.array([x, y, z])) + look + elev
                point = time_rot.apply(point)

                z_dist = norm(point[:2])
                point_pass = all([
                    point[2] >= params.min_z,
                    point[2] <= params.max_z,
                    z_dist <= params.max_z_distance
                ])
                if point_pass:
                    points.append(point)

    points = np.array(points)
    return points


def __rotation_from_euler_z(angle, degrees=False):
    return Rotation.from_euler('z', angle, degrees=degrees)


def get_scans(scans_source, sep='\n'):
    with open(scans_source, 'r') as src:
        src_data = src.read()
    scans_json = src_data.split(sep)[:-1]
    scans = {}
    frames = []
    for single_json_value in tqdm(scans_json):
        d = json_to_dict(single_json_value)
        if __Scan in d[__type].keys():
            name = d[__type][__Scan].pop(__name)
            scans[name] = d[__type][__Scan]
        elif __ScanFrame in d[__type].keys():
            frames.append(d[__type][__ScanFrame])
    return scans, frames
