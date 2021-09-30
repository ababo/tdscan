import numpy as np
from scipy.spatial.distance import cdist

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

    # # TODO DELETE THIS LATER
    # dummy_projs = np.zeros((points_count, 4, 4))
    # for i in tqdm(range(points_count)):
    #     point = points[i]
    #     plane_eq_0 = np.dot(point, sep_planes[0, :3]) + sep_planes[0, 3]
    #     plane_eq_1 = np.dot(point, sep_planes[1, :3]) + sep_planes[1, 3]
    #     plane_eq_2 = np.dot(point, sep_planes[2, :3]) + sep_planes[2, 3]
    #     plane_eq_3 = np.dot(point, sep_planes[3, :3]) + sep_planes[3, 3]
    #     if plane_eq_0 > 0:
    #         dummy_projs[i, 0] = projection_planes[0, 0]
    #     else:
    #         dummy_projs[i, 0] = projection_planes[0, 1]
    #     if plane_eq_1 > 0:
    #         dummy_projs[i, 1] = projection_planes[1, 0]
    #     else:
    #         dummy_projs[i, 1] = projection_planes[1, 1]
    #     if plane_eq_2 > 0:
    #         dummy_projs[i, 2] = projection_planes[2, 0]
    #     else:
    #         dummy_projs[i, 2] = projection_planes[2, 1]
    #     if plane_eq_3 > 0:
    #         dummy_projs[i, 3] = projection_planes[3, 0]
    #     else:
    #         dummy_projs[i, 3] = projection_planes[3, 1]
    # print(f'Diff {np.linalg.norm(projection_planes_mapped - dummy_projs)}')

    repeated_points = np.repeat(points, 4, axis=0)
    flatten_projection_planes = np.reshape(
        projection_planes_mapped, (4 * points_count, 4)
    )
    projections = project_points_to_planes(
        repeated_points, flatten_projection_planes
    )
    projections = np.reshape(projections, (points_count, 4, 3))
    # # TODO DELETE THIS LATER
    # dummy_planes_eqs = np.zeros((points_count, 4))
    # for i in tqdm(range(points_count)):
    #     point = points[i]
    #     plane_eq_0 = np.dot(point, sep_planes[0, :3]) + sep_planes[0, 3]
    #     plane_eq_1 = np.dot(point, sep_planes[1, :3]) + sep_planes[1, 3]
    #     plane_eq_2 = np.dot(point, sep_planes[2, :3]) + sep_planes[2, 3]
    #     plane_eq_3 = np.dot(point, sep_planes[3, :3]) + sep_planes[3, 3]
    #     if plane_eq_0 > 0:
    #         dummy_planes_eqs[i, 0] = np.dot(
    #             projections[i, 0], projection_planes[0, 0, :3]
    #         ) + projection_planes[0, 0, 3]
    #     else:
    #         dummy_planes_eqs[i, 0] = np.dot(
    #             projections[i, 0], projection_planes[0, 1, :3]
    #         ) + projection_planes[0, 1, 3]
    #     if plane_eq_1 > 0:
    #         dummy_planes_eqs[i, 1] = np.dot(
    #             projections[i, 1], projection_planes[1, 0, :3]
    #         ) + projection_planes[1, 0, 3]
    #     else:
    #         dummy_planes_eqs[i, 1] = np.dot(
    #             projections[i, 1], projection_planes[1, 1, :3]
    #         ) + projection_planes[1, 1, 3]
    #     if plane_eq_2 > 0:
    #         dummy_planes_eqs[i, 2] = np.dot(
    #             projections[i, 2], projection_planes[2, 0, :3]
    #         ) + projection_planes[2, 0, 3]
    #     else:
    #         dummy_planes_eqs[i, 2] = np.dot(
    #             projections[i, 2], projection_planes[2, 1, :3]
    #         ) + projection_planes[2, 1, 3]
    #     if plane_eq_3 > 0:
    #         dummy_planes_eqs[i, 3] = np.dot(
    #             projections[i, 3], projection_planes[3, 0, :3]
    #         ) + projection_planes[3, 0, 3]
    #     else:
    #         dummy_planes_eqs[i, 3] = np.dot(
    #             projections[i, 3], projection_planes[3, 1, :3]
    #         ) + projection_planes[3, 1, 3]
    # dummy_planes_eqs = np.ravel(dummy_planes_eqs)
    # print(f'Min {dummy_planes_eqs.min()}')
    # print(f'Max {dummy_planes_eqs.max()}')
    # print(f'Mean {dummy_planes_eqs.mean()}')

    projections = np.transpose(projections, axes=[1, 0, 2])

    dist = np.zeros(4)
    inds_for_dist = np.arange(points_count)
    np.random.shuffle(inds_for_dist)
    inds_for_dist = inds_for_dist[:dist_count]
    dist[0] = pairwise_dist_square_sum(projections[0, inds_for_dist])
    dist[1] = pairwise_dist_square_sum(projections[1, inds_for_dist])
    dist[2] = pairwise_dist_square_sum(projections[2, inds_for_dist])
    dist[3] = pairwise_dist_square_sum(projections[3, inds_for_dist])

    cost = np.sum(dist)

    return cost
