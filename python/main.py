from points_cloud import build_points_cloud, PointsCloudParams
from utils import (
    get_scans_and_frames, plot_view_matplotlib,
    select_partitions_mean_points, write_points_to_obj
)


if __name__ == '__main__':
    scans_source = 'vacuum_reduced5.json'
    scans, frames = get_scans_and_frames(scans_source)
    params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(scans, frames, params, use_tqdm=True)
    partitions_mean_points = select_partitions_mean_points(points, 5000, 10)
    partitions_mean_points_count = partitions_mean_points.shape[0]
    print(partitions_mean_points_count)

    # write_points_to_obj(points, use_tqdm=True)
    plot_view_matplotlib(
        partitions_mean_points,
        count=partitions_mean_points_count,
        immediately_show=True
    )
