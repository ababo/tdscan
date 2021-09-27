import matplotlib.pyplot as plt
from numba import jit
import numpy as np
from tqdm import tqdm

from points_cloud import PointsCloudParams, build_points_cloud
from objectives import local_plane_uniform_cost
from utils import (
    get_scans_and_frames,
    select_partitions_mean_points
)

__eps = 1e-10


@jit(nopython=True)
def find_local_plane_points(i, xrange, yrange, zrange, base_points, plane):
    plane_points = np.zeros((0, 3))
    for xi in xrange:
        for yi in yrange:
            for zi in zrange:
                point = np.zeros(3)
                point[0] = xi
                point[1] = yi
                point[2] = zi
                dist = np.sqrt(np.sum((base_points - point) ** 2, 1))
                # dist = np.linalg.norm(
                #     base_points - point, axis=1
                # )
                nearest = np.argmin(dist)
                if nearest == i:
                    A, B, C, D = plane
                    if np.abs(A * xi + B * yi + C * zi + D) < 1e-2:
                        plane_points = np.vstack((
                            plane_points, np.expand_dims(point, 0)
                        ))
    return plane_points


if __name__ == '__main__':
    x = np.array([0.0, 0.85, 0.63, 0.39, -1.5707964])

    scans, frames = get_scans_and_frames('vacuum_reduced5.json')
    scans_keys = ('vacuum',)
    build_points_cloud_params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    base_points = select_partitions_mean_points(points, 5000, 10)
    args = scans, scans_keys, frames, build_points_cloud_params, base_points

    xmin, xmax = np.min(points[:, 0]) - __eps, np.max(points[:, 0]) + __eps
    ymin, ymax = np.min(points[:, 1]) - __eps, np.max(points[:, 1]) + __eps
    zmin, zmax = np.min(points[:, 2]) - __eps, np.max(points[:, 2]) + __eps

    xrange = np.linspace(xmin, xmax, 100)
    yrange = np.linspace(ymin, ymax, 100)
    zrange = np.linspace(zmin, zmax, 100)

    fig = plt.figure(figsize=(15.0, 7.5))
    ax = fig.add_subplot(projection='3d')
    ax.set_xlabel('x')
    ax.set_ylabel('y')
    ax.set_zlabel('z')

    planes = local_plane_uniform_cost(x, args)
    for i, plane in enumerate(tqdm(planes)):
        # plane_points = []
        # for xi in xrange:
        #     for yi in yrange:
        #         for zi in zrange:
        #             dist = np.linalg.norm(
        #                 base_points - np.array([xi, yi, zi]), axis=1
        #             )
        #             nearest = np.argmin(dist)
        #             if nearest == i:
        #                 A, B, C, D = plane
        #                 if np.abs(A * xi + B * yi + C * zi + D) < __eps:
        #                     plane_points.append(np.array([xi, yi, zi]))
        # plane_points = np.array(plane_points)
        plane_points = find_local_plane_points(i, xrange, yrange, zrange, base_points, plane)
        ax.scatter(
            plane_points[:, 0], plane_points[:, 1],
            plane_points[:, 2], c='b', marker='o'
        )
    plt.show()
