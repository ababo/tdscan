import numpy as np

from utils import batch_dot, find_function_time_metrics


if __name__ == '__main__':
    matrices = np.random.randint(0, 10, (150000, 3, 3))
    vectors = np.random.randint(0, 10, (150000, 3))
    sim_num = 1000
    kwargs = {'matrices': matrices, 'vectors': vectors}

    numba_time_metrics = find_function_time_metrics(
        batch_dot, kwargs, sim_num
    )
    print(f'Numba metrics: {numba_time_metrics}')

    numba_free_time_metrics = find_function_time_metrics(
        batch_dot_numba_free, kwargs, sim_num
    )
    print(f'Numba free metrics: {numba_free_time_metrics}')
