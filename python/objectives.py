import numpy as np
from scipy.spatial.distance import cdist
import matplotlib.pyplot as plt

from points_cloud import build_points_cloud
from utils import (
    batch_dot, eval_dens, float_equals,
    pairwise_dist_square_sum, project_points_to_planes
)

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


def local_plane_uniform_cost(x, args):
    # Setup
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
    if points_count == 0 or base_points_count == 0:
        return 1e+8

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

    # Find planes
    all_x_are_equal = float_equals(base_points[:, 0], nearest_points[:, 0])
    all_x_are_equal = np.logical_and(
        float_equals(nearest_points[:, 0], second_nearest_points[:, 0]),
        all_x_are_equal
    )
    all_x_are_equal = np.reshape(all_x_are_equal, (-1, 1))
    all_y_are_equal = float_equals(base_points[:, 1], nearest_points[:, 1])
    all_y_are_equal = np.logical_and(
        float_equals(nearest_points[:, 1], second_nearest_points[:, 1]),
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

    # Find projections
    planes_sum_square = planes[:, 0] ** 2 + planes[:, 1] ** 2 + planes[:, 2] ** 2
    det_planes_a_neq_zero = planes[:, 0] * planes_sum_square
    det_planes_b_neq_zero = planes[:, 1] * planes_sum_square
    det_planes_c_neq_zero = planes[:, 2] * planes_sum_square

    a_neq_zero = np.logical_not(float_equals(planes[:, 0], 0.0, 1e-1))
    b_neq_zero = np.logical_not(float_equals(planes[:, 1], 0.0, 1e-1))

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

    points_are_projections = float_equals(
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

    # Find quasi-distance between points and their projections
    quasi_dist_points_projections = np.sum((points - projections) ** 2)
    scale_pp = 1 / 20

    # Find transformed projections
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

    # Find transformed projections 2d density
    dens_xmin, dens_xmax, dens_xcount = -0.6, 0.6, 121
    dens_xrange = np.linspace(dens_xmin, dens_xmax, dens_xcount)
    dens_ymin, dens_ymax, dens_ycount = -0.6, 0.6, 121
    dens_yrange = np.linspace(-0.6, 0.6, 121)

    dens_goal = np.ones((dens_ycount-1, dens_xcount-1))
    dens_goal /= (dens_xmax - dens_xmin) * (dens_ymax - dens_ymin)
    dens = eval_dens(transformed_projections[:, :2], dens_xrange, dens_yrange)

    # Find quasi-distance between real density and target density
    quasi_dist_dens_goal = np.sum((dens - dens_goal) ** 2)
    scale_dg = 1 / 25000

    # Find an output
    output = scale_pp * quasi_dist_points_projections
    output += scale_dg * quasi_dist_dens_goal
    print(output)

    return output


def multiple_projections_cost(x, args):
    scans, scans_keys, frames, build_points_cloud_params, dist_count = args
    for i, key in enumerate(scans_keys):
        scans[key][__camera_initial_position][__x] = x[5*i]
        scans[key][__camera_initial_position][__y] = x[5*i + 1]
        scans[key][__camera_initial_position][__z] = x[5*i + 2]
        scans[key][__camera_view_elevation] = x[5*i + 3]
        scans[key][__camera_landscape_angle] = x[5*i + 4]
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    points_count = points.shape[0]
    if points_count == 0:
        return 1e+6

    projections, _ = __find_projections_on_different_views(
        points, points_count
    )

    dist = np.zeros(4)
    inds_for_dist = np.arange(points_count)
    np.random.shuffle(inds_for_dist)
    inds_for_dist = inds_for_dist[:dist_count]
    dist[0] = pairwise_dist_square_sum(projections[0, inds_for_dist])
    dist[1] = pairwise_dist_square_sum(projections[1, inds_for_dist])
    dist[2] = pairwise_dist_square_sum(projections[2, inds_for_dist])
    dist[3] = pairwise_dist_square_sum(projections[3, inds_for_dist])

    cost = np.sum(dist)

    return cost / points_count


def __find_projections_on_different_views(points, points_count):
    sep_planes = np.array([
        [1.0, 0.0, 0.0, 0.0],
        [-1.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0, 0.0]
    ])
    projection_planes = np.repeat(sep_planes, 2, axis=0)
    projection_planes[::2, 3] -= 2
    projection_planes[1::2, 3] += 2
    projection_planes = np.reshape(projection_planes, (4, 2, 4))

    points_additional_col = np.ones((points_count, 1))
    extended_points = np.hstack((points, points_additional_col))
    sep_planes_equations_on_points = np.dot(extended_points, sep_planes.T)
    points_sep_groups = sep_planes_equations_on_points > 0
    points_sep_groups = np.expand_dims(points_sep_groups, 2)

    projection_planes_mapped = np.where(
        points_sep_groups,
        np.expand_dims(projection_planes[:, 0], 0),
        np.expand_dims(projection_planes[:, 1], 0)
    )

    repeated_points = np.repeat(points, 4, axis=0)
    flatten_projection_planes = np.reshape(
        projection_planes_mapped, (4 * points_count, 4)
    )
    projections = project_points_to_planes(
        repeated_points, flatten_projection_planes
    )
    projections = np.reshape(projections, (points_count, 4, 3))
    projections = np.transpose(projections, axes=[1, 0, 2])

    return projections, points_sep_groups


def points_in_neighborhood_cost(x, args):
    scans, scans_keys, frames, build_points_cloud_params, dist_count, ip = args
    for i, key in enumerate(scans_keys):
        scans[key][__camera_initial_position][__x] = x[5*i]
        scans[key][__camera_initial_position][__y] = x[5*i + 1]
        scans[key][__camera_initial_position][__z] = x[5*i + 2]
        scans[key][__camera_view_elevation] = x[5*i + 3]
        scans[key][__camera_landscape_angle] = x[5*i + 4]
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    points_count = points.shape[0]
    if points_count == 0:
        return 1e+6

    projections, points_sep_groups = __find_projections_on_different_views(
        points, points_count
    )

    transformed_projections = np.zeros((4, points_count, 2))
    transformed_projections[0] = projections[:, :, [1, 2]][0]
    transformed_projections[2] = projections[:, :, [0, 2]][2]

    rot_mat = np.ones((2, 2)) * np.sqrt(2) / 2
    rot_mat[1, 0] *= -1
    transformed_projections[1, :, 0] = np.dot(
        projections[1, :, :2], rot_mat
    )[:, 1]
    transformed_projections[1, :, 1] = projections[1, :, 2]

    rot_mat = rot_mat.T
    transformed_projections[3, :, 0] = np.dot(
        projections[3, :, :2], rot_mat
    )[:, 1]
    transformed_projections[3, :, 1] = projections[3, :, 2]

    # # TODO DELETE THIS DUMMY TEST LATER
    # sep_planes = np.array([
    #     [1.0, 0.0, 0.0, 0.0],
    #     [-1.0, 1.0, 0.0, 0.0],
    #     [0.0, 1.0, 0.0, 0.0],
    #     [1.0, 1.0, 0.0, 0.0]
    # ])
    # projection_planes = np.repeat(sep_planes, 2, axis=0)
    # projection_planes[::2, 3] -= 2
    # projection_planes[1::2, 3] += 2
    # for i in range(4):
    #     plt.figure(figsize=(15.0, 7.5))
    #     plt.title(f'Plane {projection_planes[2*i]}')
    #     points_to_plot = transformed_projections[i, points_sep_groups[:, i, 0]]
    #     plt.plot(points_to_plot[:, 0], points_to_plot[:, 1], 'kx')
    #     # ideal_points_x, ideal_points_y = get_ideal_points_on_projection_planes(
    #     #     i, 0
    #     # )
    #     # plt.plot(ideal_points_x, ideal_points_y, 'b-')
    #     x = np.linspace(-0.4, 0.4, 801)
    #     y = ip[i][0](x)
    #     plt.plot(x, y, 'b-')
    #     xgrid = np.linspace(-0.5, 0.5, 101)
    #     ygrid = np.linspace(-0.05, 0.65, 71)
    #     xx, yy = np.meshgrid(xgrid, ygrid)
    #     contour_x = np.logical_and(-0.4 <= xx, xx <= 0.4)
    #     contour_y = np.logical_and(0.0 <= yy, yy <= ip[i][0](xx))
    #     contour = np.logical_and(contour_x, contour_y)
    #     print(contour.shape)
    #     plt.contourf(xx, yy, contour.astype('float'))
    #
    #     plt.figure(figsize=(15.0, 7.5))
    #     plt.title(f'Plane {projection_planes[2*i + 1]}')
    #     points_to_plot = transformed_projections[
    #         i, np.logical_not(points_sep_groups[:, i, 0])
    #     ]
    #     plt.plot(points_to_plot[:, 0], points_to_plot[:, 1], 'kx')
    #     # ideal_points_x, ideal_points_y = get_ideal_points_on_projection_planes(
    #     #     i, 1
    #     # )
    #     # plt.plot(ideal_points_x, ideal_points_y, 'b-')
    #     x = np.linspace(-0.4, 0.4, 801)
    #     y = ip[i][1](x)
    #     plt.plot(x, y, 'b-')
    #     xgrid = np.linspace(-0.5, 0.5, 101)
    #     ygrid = np.linspace(-0.05, 0.65, 71)
    #     xx, yy = np.meshgrid(xgrid, ygrid)
    #     contour_x = np.logical_and(-0.4 <= xx, xx <= 0.4)
    #     contour_y = np.logical_and(0.0 <= yy, yy <= ip[i][1](xx))
    #     contour = np.logical_and(contour_x, contour_y)
    #     plt.contourf(xx, yy, contour.astype('float'))
    # plt.show()

    sep_planes = np.array([
        [1.0, 0.0, 0.0, 0.0],
        [-1.0, 1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0, 0.0]
    ])
    projection_planes = np.repeat(sep_planes, 2, axis=0)
    projection_planes[::2, 3] -= 2
    projection_planes[1::2, 3] += 2

    xmin, xmax, xcount = -0.5, 0.5, 101
    xstep = (xmax - xmin) / (xcount - 1)
    xgrid = np.linspace(xmin, xmax, xcount)
    ymin, ymax, ycount = -0.05, 0.65, 71
    ystep = (ymax - ymin) / (ycount - 1)
    ygrid = np.linspace(ymin, ymax, ycount)
    xx, yy = np.meshgrid(xgrid, ygrid)
    contour_x = np.logical_and(-0.4 <= xx, xx <= 0.4)
    cost = 0.0
    for i in range(4):
        for j in range(2):
            if j == 0:
                projs_inds = points_sep_groups[:, i, 0]
            else:
                projs_inds = np.logical_not(points_sep_groups[:, i, 0])
            projs = transformed_projections[i, projs_inds]
            contour_y = np.logical_and(0.0 <= yy, yy <= ip[i][j](xx))
            contour = np.logical_and(contour_x, contour_y)
            goal_dens = np.where(contour[:-1, :-1], 1.0, 0.0)
            vol = np.sum(goal_dens * xstep * ystep)
            goal_dens /= vol
            real_dens = eval_dens(projs, xgrid, ygrid)
            cost += np.sum((goal_dens - real_dens) ** 2)
    return cost / points_count


def multiple_projections_interp_cost(x, args):
    scans, scans_keys, frames, build_points_cloud_params, dist_count, ip = args
    for i, key in enumerate(scans_keys):
        scans[key][__camera_initial_position][__x] = x[5*i]
        scans[key][__camera_initial_position][__y] = x[5*i + 1]
        scans[key][__camera_initial_position][__z] = x[5*i + 2]
        scans[key][__camera_view_elevation] = x[5*i + 3]
        scans[key][__camera_landscape_angle] = x[5*i + 4]
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    points_count = points.shape[0]
    if points_count == 0:
        return 1e+6

    mult_proj_args = \
        scans, scans_keys, frames, build_points_cloud_params, dist_count
    mult_proj_cost = multiple_projections_cost(x, mult_proj_args)

    interp_points_z = ip(points[:, :2])
    interp_cost = np.sum((interp_points_z - points[:, 2]) ** 2)

    mult_proj_scale = 1.0
    interp_scale = 1 / 2000

    cost = mult_proj_scale * mult_proj_cost + interp_scale * interp_cost

    return cost


def get_ideal_points_on_projection_planes(plane_index, plane_subindex):
    if plane_index == 0:
        if plane_subindex == 0:
            ideal_points_x = np.array([
                -0.5, -0.4, -0.4, -0.385, -0.34, -0.2, -0.16, -0.156, -0.153,
                -0.135, -0.09, -0.075, 0.047, 0.056, 0.09, 0.11, 0.134, 0.147,
                0.147, 0.34, 0.4, 0.4, 0.5
            ])
            ideal_points_y = np.array([
                0.0, 0.0, 0.06, 0.067, 0.13, 0.15, 0.15, 0.467, 0.51, 0.55,
                0.592, 0.628, 0.622, 0.57, 0.56, 0.53, 0.46, 0.334, 0.132,
                0.131, 0.06, 0.0, 0.0
            ])
        else:
            ideal_points_x = np.array([
                -0.5, -0.4, -0.4, -0.35, -0.34, -0.185, -0.145, -0.143, -0.139,
                -0.117, -0.114, -0.094, -0.07, -0.061, 0.058, 0.063, 0.1,
                0.133, 0.148, 0.148, 0.21, 0.27, 0.34, 0.4, 0.4, 0.5
            ])
            ideal_points_y = np.array([
                0.0, 0.0, 0.09, 0.11, 0.13, 0.13, 0.135, 0.34, 0.485, 0.55,
                0.556, 0.56, 0.59, 0.628, 0.628, 0.59, 0.55, 0.473, 0.345,
                0.15, 0.15, 0.135, 0.13, 0.06, 0.0, 0.0
            ])
    elif plane_index == 1:
        if plane_subindex == 0:
            ideal_points_x = np.array([
                -0.5, -0.4, -0.4, -0.375, -0.35, -0.34, -0.33, -0.2, -0.182,
                -0.168, -0.15, -0.11, -0.076, 0.04, 0.057, 0.07, 0.1, 0.12,
                0.134, 0.151, 0.151, 0.34, 0.4, 0.4, 0.5
            ])
            ideal_points_y = np.array([
                0.0, 0.0, 0.06, 0.06, 0.11, 0.125, 0.13, 0.13, 0.463, 0.51,
                0.54, 0.58, 0.625, 0.625, 0.6, 0.571, 0.56, 0.53, 0.48, 0.366,
                0.13, 0.13, 0.06, 0.0, 0.0
            ])
        else:
            ideal_points_x = np.array([
                -0.5, -0.4, -0.4, -0.386, -0.337, -0.32, -0.17, -0.166, -0.15,
                -0.128, -0.085, -0.05, 0.09, 0.146, 0.16, 0.17, 0.17, 0.34,
                0.38, 0.4, 0.4, 0.5
            ])
            ideal_points_y = np.array([
                0.0, 0.0, 0.06, 0.07, 0.127, 0.13, 0.13, 0.43, 0.53, 0.555,
                0.6, 0.63, 0.61, 0.46, 0.41, 0.35, 0.134, 0.13, 0.06, 0.06,
                0.0, 0.0
            ])
    elif plane_index == 2:
        if plane_subindex == 0:
            ideal_points_x = np.array([
                -0.5, -0.4, -0.4, -0.38, -0.34, -0.3, -0.26, -0.18, -0.17,
                -0.163, -0.154, -0.113, -0.08, 0.06, 0.1, 0.125, 0.14, 0.155,
                0.162, 0.167, 0.17, 0.332, 0.39, 0.4, 0.4, 0.5
            ])
            ideal_points_y = np.array([
                0.0, 0.0, 0.06, 0.06, 0.125, 0.137, 0.14, 0.13, 0.44, 0.5, 0.55,
                0.61, 0.62, 0.62, 0.61, 0.58, 0.54, 0.49, 0.45, 0.416, 0.132,
                0.131, 0.06, 0.06, 0.0, 0.0
            ])
        else:
            ideal_points_x = np.array([
                -0.5, -0.4, -0.4, -0.34, -0.33, -0.22, -0.196, -0.185, -0.181,
                -0.168, -0.155, -0.114, -0.09, 0.02, 0.1, 0.133, 0.147, 0.16,
                0.167, 0.17, 0.23, 0.34, 0.383, 0.4, 0.4, 0.5
            ])
            ideal_points_y = np.array([
                0.0, 0.0, 0.06, 0.12, 0.126, 0.135, 0.155, 0.354, 0.48, 0.532,
                0.56, 0.614, 0.628, 0.626, 0.61, 0.588, 0.558, 0.5, 0.423,
                0.149, 0.13, 0.13, 0.065, 0.06, 0.0, 0.0
            ])
    else:
        if plane_subindex == 0:
            ideal_points_x = np.array([
                -0.5, -0.4, -0.4, -0.34, -0.26, -0.193, -0.186, -0.18, -0.17,
                -0.11, -0.05, 0.04, 0.07, 0.08, 0.085, 0.11, 0.125, 0.145,
                0.16, 0.162, 0.24, 0.29, 0.31, 0.34, 0.355, 0.37, 0.39, 0.4,
                0.4, 0.5
            ])
            ideal_points_y = np.array([
                0.0, 0.0, 0.06, 0.127, 0.135, 0.131, 0.45, 0.5, 0.55, 0.6,
                0.625, 0.625, 0.617, 0.6, 0.569, 0.563, 0.53, 0.458, 0.34,
                0.14, 0.15, 0.145, 0.13, 0.127, 0.113, 0.11, 0.1, 0.06, 0.0,
                0.0
            ])
        else:
            ideal_points_x = np.array([
                -0.5, -0.4, -0.4, -0.34, -0.32, -0.29, -0.21, -0.195, -0.175,
                -0.132, -0.09, 0.08, 0.1, 0.13, 0.155, 0.165, 0.165, 0.26,
                0.35, 0.4, 0.4, 0.5
            ])
            ideal_points_y = np.array([
                0.0, 0.0, 0.08, 0.13, 0.135, 0.15, 0.15, 0.5, 0.55, 0.6, 0.63,
                0.63, 0.61, 0.53, 0.41, 0.3, 0.14, 0.14, 0.12, 0.06, 0.0, 0.0
            ])
    return ideal_points_x, ideal_points_y
