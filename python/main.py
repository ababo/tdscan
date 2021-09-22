import matplotlib.pyplot as plt
import numpy as np

from points_cloud import build_points_cloud, get_scans, PointsCloudParams


if __name__ == '__main__':
    scans_source = 'vacuum.json'
    scans, frames = get_scans(scans_source)
    params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(scans, frames, params, use_tqdm=True)

    print(points.shape[0])
    points_to_show = np.arange(points.shape[0])
    np.random.shuffle(points_to_show)
    points_to_show = points_to_show[:10000]

    figsize = (15, 7.5)
    fig = plt.figure(figsize=figsize)
    ax = fig.add_subplot(projection='3d')
    ax.scatter(
        points[points_to_show, 0],
        points[points_to_show, 1],
        points[points_to_show, 2],
        c='k', marker='o'
    )

    ax.set_xlabel('x')
    ax.set_ylabel('y')
    ax.set_zlabel('z')

    plt.show()
