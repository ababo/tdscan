import numpy as np

from points_cloud import PointsCloudParams
from objectives import points_in_neighborhood_cost
from utils import get_scans_and_frames, find_function_time_metrics


if __name__ == '__main__':
    x = np.array([0.0, 0.9, 0.63, 0.39, -1.5707964])
    # x = np.array([0.05296124, 0.891831, 0.62812217, 1.28069815, -1.73551659])
    # x = np.array([0.61836404, 0.78357694, 0.50819665, 1.15025058, -2.50332919])

    scans, frames = get_scans_and_frames('vacuum_reduced5.json')
    scans_keys = ('vacuum',)
    build_points_cloud_params = PointsCloudParams(max_z_distance=0.4)
    dist_count = 1000
    args = scans, scans_keys, frames, build_points_cloud_params, dist_count

    cost_kwargs = {'x': x, 'args': args}
    times_metrics = find_function_time_metrics(
        points_in_neighborhood_cost, cost_kwargs,
        simulations_number=1, use_tqdm=True
    )
    print(f'Average time {times_metrics["mean"]}')
    print(f'Times standard deviation {times_metrics["std"]}')
    print(f'Median time {times_metrics["median"]}')
    print(f'Min time {times_metrics["min"]}')
    print(f'Max time {times_metrics["max"]}')
