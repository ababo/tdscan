import numpy as np
from scipy.optimize import (
    minimize, basinhopping, differential_evolution, dual_annealing, shgo
)

from genetic_algorithm import minimize_genetic
from objectives import local_plane_uniform_cost
from points_cloud import build_points_cloud, PointsCloudParams
from utils import get_scans_and_frames, select_partitions_mean_points

nit = 1


def minimize_callback(xi, state=None):
    if state is not None:
        xi_str = ', '.join([f'{xi[j]:.8f}' for j in range(xi.shape[0])])
        xi_str = f'({xi_str})'
        print(f'Iter = {state.nit}, xi = {xi_str}, f(xi) = {state.fun}')
    else:
        global nit
        xi_str = ', '.join([f'{xi[j]:.8f}' for j in range(xi.shape[0])])
        xi_str = f'({xi_str})'
        print(f'Iter = {nit}, xi = {xi_str}')
        nit += 1
    return False


if __name__ == '__main__':
    x0 = np.array([0.0, 0.85, 0.63, 0.39, -1.5707964])
    bounds = [
        (0.0, 2.0),
        (0.0, 2.0),
        (0.0, 2.0),
        (0.0, 2.0),
        (-np.pi, np.pi)
    ]

    scans, frames = get_scans_and_frames('vacuum_reduced5.json')
    scans_keys = ('vacuum',)
    build_points_cloud_params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    base_points = select_partitions_mean_points(points, 5000, 10)
    args = scans, scans_keys, frames, build_points_cloud_params, base_points
    args = (args,)

    # methods = ['CG', 'BFGS', 'SLSQP', 'TNC', 'COBYLA']
    # for method in methods:
    #     print(f'Using method {method}.')
    #     nit = 1
    #     try:
    #         opt = minimize(
    #             local_plane_uniform_cost, x0, args,
    #             method=method,
    #             callback=minimize_callback
    #         )
    #         print(f'Optimize result {opt}')
    #         print(f'So that optimal x with this method is {opt.x}')
    #     except Exception as e:
    #         e_msg = ''.join([
    #             f'Method {method} did not work ',
    #             f'due to the following exception:\n{e}\n'
    #         ])
    #         print(e_msg)
    #
    # print('Using method basinhopping.')
    # try:
    #     opt = basinhopping(
    #         local_plane_uniform_cost, x0,
    #         minimizer_kwargs={'args': args}, disp=True
    #     )
    #     print(f'Optimize result {opt}')
    #     print(f'So that optimal x with this method is {opt.x}')
    # except Exception as e:
    #     e_msg = ''.join([
    #         f'Method basinhopping did not work ',
    #         f'due to the following exception:\n{e}\n'
    #     ])
    #     print(e_msg)

    # print('Using genetic algorithm.')
    # try:
    #     x, cost = minimize_genetic(
    #         local_plane_uniform_cost, x0, args, 5, 50, 1000,
    #         disp=True
    #     )
    #     print(f'Optimal x with this method is {x}')
    #     print(f'Optimal cost with this method is {cost}')
    # except Exception as e:
    #     e_msg = ''.join([
    #         f'Genetic algorithm did not work ',
    #         f'due to the following exception:\n{e}\n'
    #     ])
    #     print(e_msg)

    print('Using method dual annealing.')
    try:
        opt = dual_annealing(local_plane_uniform_cost, bounds, args, x0=x0)
        print(f'Optimize result {opt}')
        print(f'So that optimal x with this method is {opt.x}')
    except Exception as e:
        e_msg = ''.join([
            f'Method dual annealing did not work ',
            f'due to the following exception:\n{e}\n'
        ])
        print(e_msg)

    print('Using method differential evolution.')
    try:
        opt = differential_evolution(
            local_plane_uniform_cost, bounds, args,
            workers=1, x0=x0, disp=True
        )
        print(f'Optimize result {opt}')
        print(f'So that optimal x with this method is {opt.x}')
    except Exception as e:
        e_msg = ''.join([
            f'Method differential evolution did not work ',
            f'due to the following exception:\n{e}\n'
        ])
        print(e_msg)
