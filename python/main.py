from points_cloud import (
    build_points_cloud, get_scans_and_frames,
    PointsCloudParams, write_points_to_obj
)


if __name__ == '__main__':
    scans_source = 'vacuum.json'
    scans, frames = get_scans_and_frames(scans_source)
    params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(scans, frames, params, use_tqdm=True)
    write_points_to_obj(points, use_tqdm=True)
