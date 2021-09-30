import numpy as np

from points_cloud import PointsCloudParams
from objectives import multiple_projections_cost
from utils import get_scans_and_frames, find_function_time_metrics


if __name__ == '__main__':
    x = np.array([0.0, 0.9, 0.63, 0.39, -1.5707964])

    scans, frames = get_scans_and_frames('vacuum_reduced5.json')
    scans_keys = ('vacuum',)
    build_points_cloud_params = PointsCloudParams(max_z_distance=0.4)
    dist_count = 1000
    args = scans, scans_keys, frames, build_points_cloud_params, dist_count

    cost_kwargs = {'x': x, 'args': args}
    times_metrics = find_function_time_metrics(
        multiple_projections_cost, cost_kwargs,
        simulations_number=1, use_tqdm=True
    )
    print(f'Average time {times_metrics["mean"]}')
    print(f'Times standard deviation {times_metrics["std"]}')
    print(f'Median time {times_metrics["median"]}')
    print(f'Min time {times_metrics["min"]}')
    print(f'Max time {times_metrics["max"]}')
