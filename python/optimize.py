import numpy as np
from scipy.optimize import minimize, basinhopping

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
        (-2.0, 2.0),
        (-2.0, 2.0),
        (-2.0, 2.0),
        (-2.0, 2.0),
        (-np.pi, np.pi)
    ]

    scans, frames = get_scans_and_frames('vacuum_reduced5.json')
    scans_keys = ('vacuum',)
    build_points_cloud_params = PointsCloudParams(max_z_distance=0.4)
    points = build_points_cloud(scans, frames, build_points_cloud_params)
    base_points = select_partitions_mean_points(points, 5000, 10)
    args = scans, scans_keys, frames, build_points_cloud_params, base_points
    args = (args,)

    methods = ['CG', 'BFGS', 'SLSQP', 'TNC', 'COBYLA']
    for method in methods:
        print(f'Using method {method}.')
        nit = 1
        opt = minimize(
            local_plane_uniform_cost, x0, args,
            method=method,
            callback=minimize_callback
        )
        print(f'Optimize result {opt}')
        print(f'So that optimal x with this method is {opt.x}')

    print('Using method basinhopping.')
    opt = basinhopping(
        local_plane_uniform_cost, x0,
        minimizer_kwargs={'args': args}, disp=True
    )
    print(f'Optimize result {opt}')
    print(f'So that optimal x with this method is {opt.x}')
