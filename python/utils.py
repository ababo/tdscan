from json import loads as json_to_dict
from time import time

import matplotlib.pyplot as plt
from numba import jit
import numpy as np
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
