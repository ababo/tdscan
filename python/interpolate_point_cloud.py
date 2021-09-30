import numpy as np
import scipy.interpolate as interp
import matplotlib.pyplot as plt

from points_cloud import PointsCloudParams, build_points_cloud
from utils import (
    get_scans_and_frames, plot_view_matplotlib, select_partitions_mean_points
)


if __name__ == '__main__':
    scans_source = 'vacuum_reduced5.json'
    scans, frames = get_scans_and_frames(scans_source)
    params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(scans, frames, params, use_tqdm=True)
    base_points = select_partitions_mean_points(points, 5000, 10)

    # interpolation = interp.RBFInterpolator(
    #     base_points[:, :2], base_points[:, 2], kernel='gaussian', epsilon=10
    # )
    interpolation = interp.RBFInterpolator(
        base_points[:, :2], base_points[:, 2], smoothing=0.0125
    )
    # interpolation = interp.LinearNDInterpolator(
    #     base_points[:, :2], base_points[:, 2]
    # )
    x, y = np.linspace(-0.5, 0.5, 51), np.linspace(-0.5, 0.5, 51)
    x, y = np.meshgrid(x, y)
    xravel, yravel = np.ravel(x), np.ravel(y)
    z = interpolation(np.array([xravel, yravel]).T).reshape(51, 51)
    # z = interpolation(np.array([xravel, yravel]).T)

    # ax = plot_view_matplotlib(
    #     base_points,
    #     count=base_points.shape[0],
    #     immediately_show=False
    # )

    # ax = plot_view_matplotlib(
    #     points,
    #     count=2000,
    #     immediately_show=False
    # )

    fig = plt.figure(figsize=(15.0, 7.5))
    ax = fig.add_subplot(projection='3d')
    ax.set_xlabel('x')
    ax.set_ylabel('y')
    ax.set_zlabel('z')

    # ax.scatter(x, y, z, c='b', marker='x')
    ax.plot_wireframe(x, y, z)

    plt.show()
