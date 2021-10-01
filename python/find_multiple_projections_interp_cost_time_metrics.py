import numpy as np
import scipy.interpolate as interp

from points_cloud import build_points_cloud, PointsCloudParams
from objectives import multiple_projections_interp_cost
from utils import (
    get_scans_and_frames,
    find_function_time_metrics,
    select_partitions_mean_points
)


if __name__ == '__main__':
    # x = np.array([0.0, 0.9, 0.63, 0.39, -1.5707964])
    # x = np.array([0.61836404, 0.78357694, 0.50819665, 1.15025058, -2.50332919])
    x = np.array([-0.00001532, 0.85035854, 0.62806972, 0.38964167, -1.64893864])

    scans, frames = get_scans_and_frames('vacuum_reduced5.json')
    scans_keys = ('vacuum',)
    build_points_cloud_params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(
        scans, frames, build_points_cloud_params, use_tqdm=False
    )
    base_points = select_partitions_mean_points(points, 5000, 10)
    ip = interp.RBFInterpolator(
        base_points[:, :2], base_points[:, 2], smoothing=0.0125
    )
    dist_count = 1000
    args = scans, scans_keys, frames, build_points_cloud_params, dist_count, ip

    cost_kwargs = {'x': x, 'args': args}
    times_metrics = find_function_time_metrics(
        multiple_projections_interp_cost, cost_kwargs,
        simulations_number=1, use_tqdm=True
    )
    print(f'Average time {times_metrics["mean"]}')
    print(f'Times standard deviation {times_metrics["std"]}')
    print(f'Median time {times_metrics["median"]}')
    print(f'Min time {times_metrics["min"]}')
    print(f'Max time {times_metrics["max"]}')
