import matplotlib.pyplot as plt
import numpy as np
from scipy.spatial.distance import cdist

from points_cloud import build_points_cloud
from utils import batch_dot, eval_dens

# scan keys
__camera_angle_of_view = 'camera_angle_of_view'
__camera_angular_velocity = 'camera_angular_velocity'
__camera_initial_position = 'camera_initial_position'
__camera_landscape_angle = 'camera_landscape_angle'
__camera_view_elevation = 'camera_view_elevation'
__depth_height = 'depth_height'
__depth_width = 'depth_width'
__sensor_plane_depth = 'sensor_plane_depth'
__x = 'x'
__y = 'y'
__z = 'z'

__eps = 1e-10


def local_plane_uniform_cost(x, args):
    scans, scans_keys, frames, build_points_cloud_params, base_points = args
    base_points_count = base_points.shape[0]
    for i, key in enumerate(scans_keys):
        scans[key][__camera_initial_position][__x] = x[5*i]
        scans[key][__camera_initial_position][__y] = x[5*i + 1]
        scans[key][__camera_initial_position][__z] = x[5*i + 2]
        scans[key][__camera_view_elevation] = x[5*i + 3]
        scans[key][__camera_landscape_angle] = x[5*i + 4]
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    points_count = points.shape[0]
    distances = cdist(base_points, points)
    nearest_base_points_inds = np.argmin(distances, axis=0)
    nearest_points_inds = np.argmin(distances, axis=1)
    nearest_points = points[nearest_points_inds]

    distances_without_nearest_points = distances.copy()
    distances_without_nearest_points[
        np.arange(base_points_count), nearest_points_inds
    ] = np.inf
    second_nearest_points_inds = np.argmin(
        distances_without_nearest_points, axis=1
    )
    second_nearest_points = points[second_nearest_points_inds]

    all_x_are_equal = __float_equals(base_points[:, 0], nearest_points[:, 0])
    all_x_are_equal = np.logical_and(
        __float_equals(nearest_points[:, 0], second_nearest_points[:, 0]),
        all_x_are_equal
    )
    all_x_are_equal = np.reshape(all_x_are_equal, (-1, 1))
    all_y_are_equal = __float_equals(base_points[:, 1], nearest_points[:, 1])
    all_y_are_equal = np.logical_and(
        __float_equals(nearest_points[:, 1], second_nearest_points[:, 1]),
        all_y_are_equal
    )
    all_y_are_equal = np.reshape(all_y_are_equal, (-1, 1))

    planes_equal_x = np.array([
        np.ones(base_points_count), np.zeros(base_points_count),
        np.zeros(base_points_count),
        np.zeros(base_points_count) - base_points[:, 0]
    ]).T
    planes_equal_y = np.array([
        np.zeros(base_points_count), np.ones(base_points_count),
        np.zeros(base_points_count),
        np.zeros(base_points_count) - base_points[:, 1]
    ]).T

    det = (
            base_points[:, 0] * nearest_points[:, 1]
            + nearest_points[:, 0] * second_nearest_points[:, 1]
            + second_nearest_points[:, 0] * base_points[:, 1]
            - second_nearest_points[:, 0] * nearest_points[:, 1]
            - nearest_points[:, 0] * base_points[:, 1]
            - base_points[:, 0] * second_nearest_points[:, 1]
    )
    other_planes_a = (
            base_points[:, 2] * nearest_points[:, 1]
            + nearest_points[:, 2] * second_nearest_points[:, 1]
            + second_nearest_points[:, 2] * base_points[:, 1]
            - second_nearest_points[:, 2] * nearest_points[:, 1]
            - nearest_points[:, 2] * base_points[:, 1]
            - base_points[:, 2] * second_nearest_points[:, 1]
    )
    other_planes_a /= det
    other_planes_b = (
            base_points[:, 0] * nearest_points[:, 2]
            + nearest_points[:, 0] * second_nearest_points[:, 2]
            + second_nearest_points[:, 0] * base_points[:, 2]
            - second_nearest_points[:, 0] * nearest_points[:, 2]
            - nearest_points[:, 0] * base_points[:, 2]
            - base_points[:, 0] * second_nearest_points[:, 2]
    )
    other_planes_b /= det
    other_planes_c = -np.ones(base_points_count)
    other_planes_d = (
        base_points[:, 0] * nearest_points[:, 1] * second_nearest_points[:, 2]
        + nearest_points[:, 0] * second_nearest_points[:, 1] * base_points[:, 2]
        + second_nearest_points[:, 0] * base_points[:, 1] * nearest_points[:, 2]
        - second_nearest_points[:, 0] * nearest_points[:, 1] * base_points[:, 2]
        - nearest_points[:, 0] * base_points[:, 1] * second_nearest_points[:, 2]
        - base_points[:, 0] * second_nearest_points[:, 1] * nearest_points[:, 2]
    )
    other_planes_d /= det
    other_planes = np.array([
        other_planes_a, other_planes_b, other_planes_c, other_planes_d
    ]).T

    planes = np.where(
        all_x_are_equal, planes_equal_x,
        np.where(all_y_are_equal, planes_equal_y, other_planes)
    )

    # TODO DELETE THIS TEST IN FUTURE
    # base_points_on_planes = planes[:, 0] * base_points[:, 0]\
    #                         + planes[:, 1] * base_points[:, 1]\
    #                         + planes[:, 2] * base_points[:, 2] + planes[:, 3]
    # base_points_on_planes = np.all(__float_equals(base_points_on_planes, 0.0, 1e-8))
    # nearest_points_on_planes = planes[:, 0] * nearest_points[:, 0]\
    #                            + planes[:, 1] * nearest_points[:, 1]\
    #                            + planes[:, 2] * nearest_points[:, 2]\
    #                            + planes[:, 3]
    # nearest_points_on_planes = np.all(__float_equals(nearest_points_on_planes, 0.0, 1e-8))
    # second_nearest_points_on_planes = planes[:, 0] * second_nearest_points[:, 0]\
    #                                   + planes[:, 1] * second_nearest_points[:, 1]\
    #                                   + planes[:, 2] * second_nearest_points[:, 2]\
    #                                   + planes[:, 3]
    # second_nearest_points_on_planes = np.all(__float_equals(second_nearest_points_on_planes, 0.0, 1e-8))
    # print(f'All points on planes: {base_points_on_planes and nearest_points_on_planes and second_nearest_points_on_planes}')

    planes_sum_square = planes[:, 0] ** 2 + planes[:, 1] ** 2 + planes[:, 2] ** 2
    det_planes_a_neq_zero = planes[:, 0] * planes_sum_square
    det_planes_b_neq_zero = planes[:, 1] * planes_sum_square
    det_planes_c_neq_zero = planes[:, 2] * planes_sum_square

    a_neq_zero = np.logical_not(__float_equals(planes[:, 0], 0.0, 1e-1))
    b_neq_zero = np.logical_not(__float_equals(planes[:, 1], 0.0, 1e-1))

    det_planes = np.where(
        a_neq_zero, det_planes_a_neq_zero,
        np.where(b_neq_zero, det_planes_b_neq_zero, det_planes_c_neq_zero)
    )
    A = planes[nearest_base_points_inds, 0]
    B = planes[nearest_base_points_inds, 1]
    C = planes[nearest_base_points_inds, 2]
    D = planes[nearest_base_points_inds, 3]

    projections_a_neq_zero = np.zeros((points_count, 3))
    projections_a_neq_zero[:, 0] = -A * A * D
    projections_a_neq_zero[:, 0] += A * C * (C*points[:, 0] - A*points[:, 2])
    projections_a_neq_zero[:, 0] += A * B * (B*points[:, 0] - A*points[:, 1])
    projections_a_neq_zero[:, 0] /= det_planes[nearest_base_points_inds]
    projections_a_neq_zero[:, 1] = B * C * (C*points[:, 0] - A*points[:, 2])
    projections_a_neq_zero[:, 1] += -(A*A + C*C) * (B*points[:, 0] - A*points[:, 1])
    projections_a_neq_zero[:, 1] += -A * B * D
    projections_a_neq_zero[:, 1] /= det_planes[nearest_base_points_inds]
    projections_a_neq_zero[:, 2] = B * C * (B*points[:, 0] - A*points[:, 1])
    projections_a_neq_zero[:, 2] += -(A*A + B*B) * (C*points[:, 0] - A*points[:, 2])
    projections_a_neq_zero[:, 2] += -A * C * D
    projections_a_neq_zero[:, 2] /= det_planes[nearest_base_points_inds]

    projections_b_neq_zero = np.zeros((points_count, 3))
    projections_b_neq_zero[:, 0] = (B*B + C*C) * (B*points[:, 0] - A*points[:, 1])
    projections_b_neq_zero[:, 0] += A * C * (C*points[:, 1] - B*points[:, 2])
    projections_b_neq_zero[:, 0] += A * B * D
    projections_b_neq_zero[:, 0] /= det_planes[nearest_base_points_inds]
    projections_b_neq_zero[:, 1] = B * C * (C*points[:, 1] - B*points[:, 2])
    projections_b_neq_zero[:, 1] += -A * B * (B*points[:, 0] - A*points[:, 1])
    projections_b_neq_zero[:, 1] += -B * B * D
    projections_b_neq_zero[:, 1] /= det_planes[nearest_base_points_inds]
    projections_b_neq_zero[:, 2] = -B * C * D
    projections_b_neq_zero[:, 2] += -(A*A + B*B) * (C*points[:, 1] - B*points[:, 2])
    projections_b_neq_zero[:, 2] += -A * C * (B*points[:, 0] - A*points[:, 1])
    projections_b_neq_zero[:, 2] /= det_planes[nearest_base_points_inds]

    projections_c_neq_zero = np.zeros((points_count, 3))
    projections_c_neq_zero[:, 0] = (B*B + C*C) * (C*points[:, 0] - A*points[:, 2])
    projections_c_neq_zero[:, 0] += -A * B * (C*points[:, 1] - B*points[:, 2])
    projections_c_neq_zero[:, 0] += -A * C * D
    projections_c_neq_zero[:, 0] /= det_planes[nearest_base_points_inds]
    projections_c_neq_zero[:, 1] = (A*A + C*C) * (C*points[:, 1] - B*points[:, 2])
    projections_c_neq_zero[:, 1] += -A * B * (C*points[:, 0] - A*points[:, 2])
    projections_c_neq_zero[:, 1] += -B * C * D
    projections_c_neq_zero[:, 1] /= det_planes[nearest_base_points_inds]
    projections_c_neq_zero[:, 2] = -C * C * D
    projections_c_neq_zero[:, 2] += -A * C * (C*points[:, 0] - A*points[:, 2])
    projections_c_neq_zero[:, 2] += -B * C * (C*points[:, 1] - B*points[:, 2])
    projections_c_neq_zero[:, 2] /= det_planes[nearest_base_points_inds]

    points_are_projections = __float_equals(
        A*points[:, 0] + B*points[:, 1] + C*points[:, 2] + D, 0.0
    )
    points_are_projections = np.reshape(points_are_projections, (-1, 1))
    a_neq_zero_mapped = a_neq_zero[nearest_base_points_inds].reshape(-1, 1)
    b_neq_zero_mapped = b_neq_zero[nearest_base_points_inds].reshape(-1, 1)
    projections = np.where(
        points_are_projections, points,
        np.where(
            a_neq_zero_mapped, projections_a_neq_zero,
            np.where(
                b_neq_zero_mapped,
                projections_b_neq_zero,
                projections_c_neq_zero
            )
        )
    )

    # TODO DELETE THIS TEST LATER
    # projections_on_planes = A*projections[:, 0] + B*projections[:, 1]\
    #                         + C*projections[:, 2] + D
    # print(np.min(projections_on_planes))
    # print(np.max(projections_on_planes))
    # projections_on_planes = np.all(
    #     __float_equals(projections_on_planes, 0.0, 1e-2)
    # )
    # print(f'All projections are on planes {projections_on_planes}')
    # vec = np.array([A, B, C]).T
    # vec = vec / np.linalg.norm(vec, axis=1).reshape(-1, 1)
    # dist_proj_points = np.linalg.norm(points - projections, axis=1).reshape(-1, 1)
    # almost_points = projections + dist_proj_points * vec
    # devi = np.linalg.norm(points - almost_points, axis=1)
    # print(np.min(devi))
    # print(np.max(devi))
    # print(np.median(devi))
    # max_devis = np.argsort(devi)[-10:]
    # for max_devi in max_devis:
    #     print(f'Deviation {devi[max_devi]}')
    #     print(f'Point {points[max_devi]}')
    #     print(f'Al point {almost_points[max_devi]}')
    #     print(f'Projection {projections[max_devi]}')
    #     print(f'A, B, C, D {A[max_devi], B[max_devi], C[max_devi], D[max_devi]}')
    # print(f'All projections are on axis {projections_on_axis}')

    quasi_dist_points_projections = np.sum((points - projections) ** 2)
    scale_pp = 1 / 20
    print(f'Quasi dist 1 {quasi_dist_points_projections * scale_pp}')

    new_basis_x = second_nearest_points - base_points
    new_basis_x /= np.linalg.norm(new_basis_x, axis=1, keepdims=True)
    new_basis_z = planes[:, :3]
    new_basis_z /= np.linalg.norm(new_basis_z, axis=1, keepdims=True)

    new_basis_y = np.array([
        new_basis_z[:, 2] * new_basis_x[:, 1] - new_basis_z[:, 1] * new_basis_x[:, 2],
        new_basis_z[:, 0] * new_basis_x[:, 2] - new_basis_z[:, 2] * new_basis_x[:, 0],
        new_basis_z[:, 1] * new_basis_x[:, 0] - new_basis_z[:, 0] * new_basis_x[:, 1]
    ]).T
    new_basis_y /= np.linalg.norm(new_basis_y, axis=1, keepdims=True)

    transition_matrices = np.array([
        [new_basis_x[:, 0], new_basis_y[:, 0], new_basis_z[:, 0]],
        [new_basis_x[:, 1], new_basis_y[:, 1], new_basis_z[:, 1]],
        [new_basis_x[:, 2], new_basis_y[:, 2], new_basis_z[:, 2]]
    ])
    transition_matrices = np.transpose(transition_matrices, [2, 0, 1])
    inv_transition_matrices = np.linalg.inv(transition_matrices)

    transformed_projections = batch_dot(
        inv_transition_matrices[nearest_base_points_inds],
        projections
    )
    plt.scatter(transformed_projections[:, 0], transformed_projections[:, 1], marker='x')

    dens_xmin, dens_xmax, dens_xcount = -0.6, 0.6, 121
    dens_xrange = np.linspace(dens_xmin, dens_xmax, dens_xcount)
    dens_ymin, dens_ymax, dens_ycount = -0.6, 0.6, 121
    dens_yrange = np.linspace(-0.6, 0.6, 121)

    dens_goal = np.ones((dens_ycount-1, dens_xcount-1))
    dens_goal /= (dens_xmax - dens_xmin) * (dens_ymax - dens_ymin)
    dens = eval_dens(transformed_projections[:, :2], dens_xrange, dens_yrange)
    quasi_dist_dens_goal = np.sum((dens - dens_goal) ** 2)
    scale_dg = 1 / 25000
    print(f'Quasi dist 2 {quasi_dist_dens_goal * scale_dg}')

    return quasi_dist_points_projections


def __float_equals(left, right, eps=__eps):
    return np.abs(left - right) < eps
