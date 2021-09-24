from points_cloud import build_points_cloud,  PointsCloudParams
from utils import find_function_time_metrics, get_scans_and_frames


if __name__ == '__main__':
    scans_source = 'vacuum.json'
    scans, frames = get_scans_and_frames(scans_source)
    params = PointsCloudParams(max_z_distance=0.4)
    build_kwargs = {
        'all_scans': scans, 'all_frames': frames,
        'params': params, 'use_tqdm': False
    }
    times_metrics = find_function_time_metrics(
        build_points_cloud, build_kwargs,
        simulations_number=100, use_tqdm=True
    )
    print(f'Average time {times_metrics["mean"]}')
    print(f'Times standard deviation {times_metrics["std"]}')
    print(f'Median time {times_metrics["median"]}')
    print(f'Min time {times_metrics["min"]}')
    print(f'Max time {times_metrics["max"]}')
