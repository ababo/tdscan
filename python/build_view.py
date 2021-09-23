import argparse
from ast import literal_eval as str_to_dict

import numpy as np

from points_cloud import (
    build_points_cloud, get_scans_and_frames,
    PointsCloudParams, write_points_to_obj
)

parser = argparse.ArgumentParser(
    description='Build a points cloud and save it to the .obj file'
)
parser.add_argument(
    '--scans_source',
    default='vacuum.json',
    type=str,
    help='File with LIDAR-data'
)
parser.add_argument(
    '--obj_path',
    default='foo.obj',
    type=str,
    help='File where a points cloud is saved'
)
parser.add_argument(
    '--use_progressbar',
    default=False,
    action='store_true',
    help='Show progressbar on long processes or not'
)
parser.add_argument(
    '--min_depth_confidence',
    default=3,
    type=int,
    help='Minimum depth confidence'
)
parser.add_argument(
    '--min_z',
    default=-np.inf,
    type=float,
    help='Minimum point Z-coordinate'
)
parser.add_argument(
    '--max_z',
    default=np.inf,
    type=float,
    help='Maximum point Z-coordinate'
)
parser.add_argument(
    '--max_z_distance',
    default=0.4,
    type=float,
    help='Maximum point distance from Z axis'
)
parser.add_argument(
    '--camera_initial_positions',
    default='{}',
    type=str,
    help='Camera initial position to override with'
)
parser.add_argument(
    '--camera_view_elevations',
    default='{}',
    type=str,
    help='Camera view elevation to override with'
)
parser.add_argument(
    '--camera_landscape_angles',
    default='{}',
    type=str,
    help='Camera landscape angle to override with'
)


def __parse_args():
    args = parser.parse_args()
    args.camera_initial_positions = str_to_dict(args.camera_initial_positions)
    args.camera_view_elevations = str_to_dict(args.camera_view_elevations)
    args.camera_landscape_angles = str_to_dict(args.camera_landscape_angles)
    return args


def __update_scans_with_args(scans, args):
    for scan_name in args.camera_initial_positions.keys():
        scans[scan_name]['camera_initial_position'] = \
            args.camera_initial_positions[scan_name]
    for scan_name in args.camera_view_elevations.keys():
        scans[scan_name]['camera_view_elevation'] = \
            args.camera_view_elevations[scan_name]
    for scan_name in args.camera_landscape_angles.keys():
        scans[scan_name]['camera_landscape_angle'] = \
            args.camera_landscape_angles[scan_name]


if __name__ == '__main__':
    args = __parse_args()
    scans, frames = get_scans_and_frames(
        args.scans_source, use_tqdm=args.use_progressbar
    )
    __update_scans_with_args(scans, args)
    params = PointsCloudParams(
        min_depth_confidence=args.min_depth_confidence,
        min_z=args.min_z, max_z=args.max_z, max_z_distance=args.max_z_distance
    )
    points = build_points_cloud(
        scans, frames, params, use_tqdm=args.use_progressbar
    )
    write_points_to_obj(
        points, obj_path=args.obj_path, use_tqdm=args.use_progressbar
    )
