from json import loads as json_to_dict
from time import time

import matplotlib.pyplot as plt
from numba import jit
import numpy as np
from scipy.spatial.distance import cdist
from tqdm import tqdm


# frame keys
__data = 'data'
__depth_confidences = 'depth_confidences'
__depths = 'depths'
__image = 'image'
__scan = 'scan'
__time = 'time'

# additional keys
__Scan = 'Scan'
__ScanFrame = 'ScanFrame'
__type = 'type'
__name = 'name'

__eps = 1e-8


def float_equals(left, right, eps=__eps):
    return np.abs(left - right) < eps


def select_random_points(points, count):
    indices = select_random_points_indices(points, count)
    return points[indices]


def select_random_points_indices(points, count):
    indices = np.arange(points.shape[0])
    np.random.shuffle(indices)
    indices = indices[:count]
    return indices


def select_partitions_mean_points(points, count, min_points_count=100):
    # TODO REFACTOR THIS LATER MAYBE?
    xmin, xmax = np.min(points[:, 0]) - __eps, np.max(points[:, 0]) + __eps
    ymin, ymax = np.min(points[:, 1]) - __eps, np.max(points[:, 1]) + __eps
    zmin, zmax = np.min(points[:, 2]) - __eps, np.max(points[:, 2]) + __eps

    count_per_axis = int(np.power(count, 1/3))
    xrange = np.linspace(xmin, xmax, count_per_axis + 1)
    yrange = np.linspace(ymin, ymax, count_per_axis + 1)
    zrange = np.linspace(zmin, zmax, count_per_axis + 1)

    parition_mean_points = []

    for i in tqdm(range(len(xrange)-1)):
        for j in range(len(yrange)-1):
            for k in range(len(zrange)-1):
                partition_indicator = np.logical_and.reduce((
                    xrange[i] <= points[:, 0], points[:, 0] <= xrange[i+1],
                    yrange[j] <= points[:, 1], points[:, 1] <= yrange[j+1],
                    zrange[k] <= points[:, 2], points[:, 2] <= zrange[k+1],
                ))
                partition_vol = np.sum(partition_indicator)
                if partition_vol > min_points_count:
                    partition = points[partition_indicator]
                    mean_point = np.mean(partition, axis=0)
                    parition_mean_points.append(mean_point)
    return np.array(parition_mean_points)


def select_partition_mean_points_without_leaps(
        points, count, min_point_counts=100, allowed_max_leap=0.2,
        nearest_neighbors_count=5
):
    partition_mean_points = select_partitions_mean_points(
        points, count, min_point_counts
    )
    points_count = partition_mean_points.shape[0]
    dist_xy = cdist(partition_mean_points[:, :2], partition_mean_points[:, :2])
    rows_inds = __get_rows_inds_for_points_without_leaps(
        points_count, nearest_neighbors_count
    )
    cols_inds = __get_cols_inds_for_points_without_leaps(
        dist_xy, nearest_neighbors_count
    )
    dist_z_to_nearest_neighbors = __get_dist_z_to_nearest_neighbors(
        partition_mean_points, rows_inds, cols_inds
    )
    points_inds_to_delete = __get_points_inds_to_delete(
        dist_z_to_nearest_neighbors, allowed_max_leap, points_count
    )
    output = np.delete(partition_mean_points, points_inds_to_delete, axis=0)
    return output

def __get_rows_inds_for_points_without_leaps(points_count, neighbors_count):
    rows_inds = np.arange(points_count)
    rows_inds = rows_inds.reshape(-1, 1)
    rows_inds = np.repeat(rows_inds, neighbors_count, axis=1)
    return rows_inds


def __get_cols_inds_for_points_without_leaps(dist, neighbors_count):
    cols_inds = np.argsort(dist, axis=1)
    cols_inds = cols_inds[:, :neighbors_count]
    return cols_inds


def __get_dist_z_to_nearest_neighbors(points, rows_inds, cols_inds):
    partition_mean_points_z = points[:, 2].reshape(-1, 1)
    dist_z = cdist(partition_mean_points_z, partition_mean_points_z)
    dist_z_to_nearest_neighbors = dist_z[rows_inds, cols_inds]
    return dist_z_to_nearest_neighbors


def __get_points_inds_to_delete(dist, allowed_max_leap, points_count):
    inds = np.any(dist > allowed_max_leap, axis=1)
    inds = np.arange(points_count)[inds]
    return inds


def plot_view_matplotlib(
        points, count=1000, figszie=(15.0, 7.5),
        color='k', marker='o', immediately_show=False
):
    indices = select_random_points_indices(points, count)
    points_to_scatter = points[indices, :]
    ax = __create_ax(figszie)
    __scatter_points(ax, points_to_scatter, color, marker)
    if immediately_show:
        plt.show()
    return ax


def __create_ax(figsize):
    fig = plt.figure(figsize=figsize)
    ax = fig.add_subplot(projection='3d')
    ax.set_xlabel('x')
    ax.set_ylabel('y')
    ax.set_zlabel('z')
    return ax


def __scatter_points(ax, points_to_scatter, color, marker):
    ax.scatter(
        points_to_scatter[:, 0],
        points_to_scatter[:, 1],
        points_to_scatter[:, 2],
        c=color, marker=marker
    )


def find_function_time_metrics(
        func, kwargs, simulations_number=1000, use_tqdm=True
):
    time_metrics = {}
    times = np.zeros(simulations_number)
    simulations = range(simulations_number)
    if use_tqdm:
        simulations = tqdm(simulations)
    for i in simulations:
        t0 = time()
        _ = func(**kwargs)
        times[i] = time() - t0
    time_metrics['mean'] = np.mean(times)
    time_metrics['std'] = np.std(times)
    time_metrics['median'] = np.median(times)
    time_metrics['min'] = np.min(times)
    time_metrics['max'] = np.max(times)
    return time_metrics


def get_scans_and_frames(scans_source, sep='\n', use_tqdm=True):
    with open(scans_source, 'r') as src:
        src_data = src.read()
    scans_json = src_data.split(sep)[:-1]
    scans = {}
    frames = []
    scans_json_to_iter = tqdm(scans_json) if use_tqdm else scans_json
    for single_json_value in scans_json_to_iter:
        __update_scans_and_frames(scans, frames, single_json_value)
    return scans, frames


def __update_scans_and_frames(scans, frames, json_value):
    d = json_to_dict(json_value)
    d = d[__type]
    if __Scan in d.keys():
        d = d[__Scan]
        name = d.pop(__name)
        scans[name] = d
    elif __ScanFrame in d.keys():
        d = d[__ScanFrame]
        __transform_frame_lists_to_arrays(d)
        frames.append(d)


def __transform_frame_lists_to_arrays(frame):
    frame[__image][__data] = np.array(frame[__image][__data])
    frame[__depths] = np.array(frame[__depths])
    frame[__depth_confidences] = np.array(frame[__depth_confidences])


def write_points_to_obj(points, obj_path='foo.obj', use_tqdm=False):
    with open(obj_path, 'w') as obj_file:
        points_to_iter = tqdm(points) if use_tqdm else points
        for point in points_to_iter:
            obj_file.write(
                f'v {point[0]:.10f} {point[1]:.10f} {point[2]:.10f}\n'
            )


def batch_dot(matrices, vectors):
    matrices = np.asarray(matrices).astype('float')
    vectors = np.asarray(vectors).astype('float')
    return __batch_dot(matrices, vectors)


@jit(nopython=True)
def __batch_dot(matrices, vectors):
    N1, n1, m1 = matrices.shape
    N2, m2 = vectors.shape
    if N1 != N2 or m1 != m2:
        raise ValueError('Shape mismatch!')
    out = np.zeros((N1, n1))
    for i in np.arange(N1):
        out[i] = np.dot(matrices[i], vectors[i])
    return out


@jit(nopython=True)
def eval_dens(points, xrange, yrange):
    xcount = xrange.shape[0]
    ycount = yrange.shape[0]
    dens = np.zeros((ycount-1, xcount-1))
    vol = 0.0
    for i in np.arange(ycount-1):
        for j in np.arange(xcount-1):
            x_in_range_inds = __get_points_in_range(points[:, 0], xrange, j)
            y_in_range_inds = __get_points_in_range(points[:, 1], yrange, i)
            points_in_range_inds = np.logical_and(
                x_in_range_inds, y_in_range_inds
            ).astype(np.float_)
            dens[i, j] = np.sum(points_in_range_inds)
            local_vol = dens[i, j]
            local_vol *= xrange[j+1] - xrange[j]
            local_vol *= yrange[i+1] - yrange[i]
            vol += local_vol
    dens /= vol
    return dens


@jit(nopython=True)
def __get_points_in_range(points, points_range, index):
    out = np.logical_and(
        points_range[index] <= points,
        points < points_range[index + 1]
    )
    return out


def project_points_to_planes(points, planes):
    planes_neq_zero = __planes_neq_zero(planes)
    det_planes = __find_det_planes(planes, planes_neq_zero)
    projections = __find_projections(
        points, planes, planes_neq_zero, det_planes
    )
    return projections


def __planes_neq_zero(planes, eps=1e-2):
    planes_eq_zero = float_equals(planes, 0.0, eps)
    planes_neq_zero = np.logical_not(planes_eq_zero)
    return planes_neq_zero


def __find_det_planes(planes, planes_neq_zero):
    planes_sum_square = np.sum(planes[:, :3] ** 2, axis=1)
    det_planes_a_neq_zero = planes[:, 0] * planes_sum_square
    det_planes_b_neq_zero = planes[:, 1] * planes_sum_square
    det_planes_c_neq_zero = planes[:, 2] * planes_sum_square
    a_neq_zero, b_neq_zero, _, _ = __get_neq_zero_coords(planes_neq_zero)
    det_planes = np.where(
        a_neq_zero, det_planes_a_neq_zero,
        np.where(b_neq_zero, det_planes_b_neq_zero, det_planes_c_neq_zero)
    )
    return det_planes


def __get_neq_zero_coords(planes_neq_zero):
    a_neq_zero = planes_neq_zero[:, 0]
    b_neq_zero = planes_neq_zero[:, 1]
    c_neq_zero = planes_neq_zero[:, 2]
    d_neq_zero = planes_neq_zero[:, 3]
    return a_neq_zero, b_neq_zero, c_neq_zero, d_neq_zero


def __find_projections(points, planes, planes_neq_zero, det_planes):
    points_count = points.shape[0]
    a, b, c, d = __get_planes_coords(planes)
    x, y, z = __get_points_coords(points)
    a_neq_zero, b_neq_zero, _, _ = __get_neq_zero_coords(planes_neq_zero)
    a_neq_zero = np.reshape(a_neq_zero, (-1, 1))
    b_neq_zero = np.reshape(b_neq_zero, (-1, 1))
    projections_a_neq_zero = __find_projections_a_neq_zero(
        (x, y, z), points_count, (a, b, c, d), det_planes
    )
    projections_b_neq_zero = __find_projections_b_neq_zero(
        (x, y, z), points_count, (a, b, c, d), det_planes
    )
    projections_c_neq_zero = __find_projections_c_neq_zero(
        (x, y, z), points_count, (a, b, c, d), det_planes
    )
    points_are_projections = __find_points_are_projections(
        (x, y, z), (a, b, c, d)
    )
    projections_different_from_points = np.where(
        a_neq_zero, projections_a_neq_zero,
        np.where(b_neq_zero, projections_b_neq_zero, projections_c_neq_zero)
    )
    projections = np.where(
        points_are_projections, points, projections_different_from_points
    )
    return projections


def __get_planes_coords(planes):
    a = planes[:, 0]
    b = planes[:, 1]
    c = planes[:, 2]
    d = planes[:, 3]
    return a, b, c, d


def __get_points_coords(points):
    x = points[:, 0]
    y = points[:, 1]
    z = points[:, 2]
    return x, y, z


def __find_projections_a_neq_zero(points, points_count, planes, det_planes):
    x, y, z = points
    a, b, c, d = planes
    cx_minus_az = c*x - a*z
    bx_minus_ay = b*x - a*y
    projections = np.zeros((points_count, 3))
    projections[:, 0] = -a * a * d
    projections[:, 0] += a * c * cx_minus_az
    projections[:, 0] += a * b * bx_minus_ay
    projections[:, 0] /= det_planes
    projections[:, 1] = b * c * cx_minus_az
    projections[:, 1] += -(a*a + c*c) * bx_minus_ay
    projections[:, 1] += -a * b * d
    projections[:, 1] /= det_planes
    projections[:, 2] = b * c * bx_minus_ay
    projections[:, 2] += -(a*a + b*b) * cx_minus_az
    projections[:, 2] += -a * c * d
    projections[:, 2] /= det_planes
    return projections


def __find_projections_b_neq_zero(points, points_count, planes, det_planes):
    x, y, z = points
    a, b, c, d = planes
    bx_minus_ay = b*x - a*y
    cy_minus_bz = c*y - b*z
    projections = np.zeros((points_count, 3))
    projections[:, 0] = (b*b + c*c) * bx_minus_ay
    projections[:, 0] += a * c * cy_minus_bz
    projections[:, 0] += a * b * d
    projections[:, 0] /= det_planes
    projections[:, 1] = b * c * cy_minus_bz
    projections[:, 1] += -a * b * bx_minus_ay
    projections[:, 1] += -b * b * d
    projections[:, 1] /= det_planes
    projections[:, 2] = -b * c * d
    projections[:, 2] += -(a*a + b*b) * cy_minus_bz
    projections[:, 2] += -a * c * bx_minus_ay
    projections[:, 2] /= det_planes
    return projections


def __find_projections_c_neq_zero(points, points_count, planes, det_planes):
    x, y, z = points
    a, b, c, d = planes
    cx_minus_az = c*x - a*z
    cy_minus_bz = c*y - b*z
    projections = np.zeros((points_count, 3))
    projections[:, 0] = (b*b + c*c) * cx_minus_az
    projections[:, 0] += -a * b * cy_minus_bz
    projections[:, 0] += -a * c * d
    projections[:, 0] /= det_planes
    projections[:, 1] = (a*a + c*c) * cy_minus_bz
    projections[:, 1] += -a * b * cx_minus_az
    projections[:, 1] += -b * c * d
    projections[:, 1] /= det_planes
    projections[:, 2] = -c * c * d
    projections[:, 2] += -a * c * cx_minus_az
    projections[:, 2] += -b * c * cy_minus_bz
    projections[:, 2] /= det_planes
    return projections


def __find_points_are_projections(points, planes):
    x, y, z = points
    a, b, c, d = planes
    plane_eq = a*x + b*y + c*z + d
    points_are_projections = float_equals(plane_eq, 0.0)
    points_are_projections = np.reshape(points_are_projections, (-1, 1))
    return points_are_projections


@jit(nopython=True)
def pairwise_dist_square_sum(points):
    sum = 0.0
    points_count = points.shape[0]
    for i in np.arange(1, points_count):
        for j in np.arange(i+1, points_count):
            sum += np.sum((points[i] - points[j]) ** 2)
    return sum
