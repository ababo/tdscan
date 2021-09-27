import numpy as np

from points_cloud import PointsCloudParams, build_points_cloud
from objectives import local_plane_uniform_cost
from utils import (
    find_function_time_metrics,
    get_scans_and_frames,
    select_partitions_mean_points
)


if __name__ == '__main__':
    x = np.array([0.0, 0.85, 0.63, 0.39, -1.5707964])

    scans, frames = get_scans_and_frames('vacuum_reduced5.json')
    scans_keys = ('vacuum',)
    build_points_cloud_params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    base_points = select_partitions_mean_points(points, 5000, 10)
    args = scans, scans_keys, frames, build_points_cloud_params, base_points

    cost_kwargs = {'x': x, 'args': args}
    times_metrics = find_function_time_metrics(
        local_plane_uniform_cost, cost_kwargs,
        simulations_number=1, use_tqdm=True
    )
    print(f'Average time {times_metrics["mean"]}')
    print(f'Times standard deviation {times_metrics["std"]}')
    print(f'Median time {times_metrics["median"]}')
    print(f'Min time {times_metrics["min"]}')
    print(f'Max time {times_metrics["max"]}')
