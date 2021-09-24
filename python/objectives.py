from time import time

import numpy as np
from scipy.spatial.distance import cdist

from points_cloud import build_points_cloud

# scan keys
__camera_angle_of_view = 'camera_angle_of_view'
__camera_angular_velocity = 'camera_angular_velocity'
__camera_initial_position = 'camera_initial_position'
__camera_landscape_angle = 'camera_landscape_angle'
__camera_view_elevation = 'camera_view_elevation'
__depth_height = 'depth_height'
__depth_width = 'depth_width'
__sensor_plane_depth = 'sensor_plane_depth'
__x = 'x'
__y = 'y'
__z = 'z'


def local_plane_uniform_cost(x, args):
    scans, scans_keys, frames, build_points_cloud_params, base_points = args
    for i, key in enumerate(scans_keys):
        scans[key][__camera_initial_position][__x] = x[5*i]
        scans[key][__camera_initial_position][__y] = x[5*i + 1]
        scans[key][__camera_initial_position][__z] = x[5*i + 2]
        scans[key][__camera_view_elevation] = x[5*i + 3]
        scans[key][__camera_landscape_angle] = x[5*i + 4]
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    # TODO ADD HERE RANDOM POINTS SELECTION
    distances = cdist(base_points, points)
    nearest_base_points = np.argmin(distances, axis=0)
    return distances
