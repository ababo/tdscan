from time import time

import numpy as np
from tqdm import tqdm

from points_cloud import (
    build_points_cloud, get_scans_and_frames,
    PointsCloudParams
)


if __name__ == '__main__':
    scans_source = 'vacuum.json'
    scans, frames = get_scans_and_frames(scans_source)
    params = PointsCloudParams(max_z_distance=0.4)
    simulation_number = 1000
    times = np.zeros(simulation_number)
    for i in tqdm(range(simulation_number)):
        t0 = time()
        points = build_points_cloud(scans, frames, params, use_tqdm=False)
        times[i] = time() - t0
    print(f'Average time {np.mean(times)}')
    print(f'Times standard deviation {np.std(times)}')
    print(f'Median time {np.median(times)}')
    print(f'Min time {np.min(times)}')
    print(f'Max time {np.max(times)}')
