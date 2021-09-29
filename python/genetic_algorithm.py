from array import array
from deap import base, creator, tools
import numpy as np


def minimize_genetic(
        fun, x0, args, ind_size, pop_size, nb_gen,
        mutation_prob=0.2, crossover_prob=0.8, disp=False
):
    x0 = np.asarray(x0)
    # Initialization with deap
    creator.create('ModelFitness', base.Fitness, weights=(-1.0,))
    creator.create(
        'Individual', array, typecode='d', fitness=creator.ModelFitness
    )

    toolbox = base.Toolbox()
    # init_fun_seq = [
    #     lambda: np.random.randn() + x0[i] for i in range(ind_size)
    # ]
    # toolbox.register(
    #     'individual', tools.initCycle, creator.Individual, init_fun_seq, 1
    # )
    # toolbox.register(
    #     'population', tools.initRepeat, list, toolbox.individual, pop_size
    # )
    toolbox.register(
        'individual', tools.initRepeat, creator.Individual,
        lambda: np.random.rand() * np.pi, ind_size
    )
    toolbox.register(
        'population', tools.initRepeat, list, toolbox.individual, pop_size
    )
    toolbox.register('map', map)

    toolbox.register('mate', tools.cxOnePoint)
    toolbox.register(
        'mutate', tools.mutGaussian, mu=0.0, sigma=1.0, indpb=mutation_prob
    )
    toolbox.register('select', tools.selTournament)
    toolbox.register('evaluate', lambda ind: fun(ind, *args))

    # Creation of initial population
    population = toolbox.population()
    invalid_ind = [ind for ind in population if not ind.fitness.valid]
    fitnesses = toolbox.map(toolbox.evaluate, invalid_ind)
    for ind, fit in zip(invalid_ind, fitnesses):
        ind.fitness.values = fit,

    # Hall of fame is used to provide a principe of elitism
    hof = tools.HallOfFame(1)
    init_best_ind = toolbox.individual()
    for i in range(len(init_best_ind)):
        init_best_ind[i] = x0[i]
    init_best_ind.fitness.values = toolbox.evaluate(init_best_ind),
    hof.update(population + [init_best_ind])

    gens = np.arange(nb_gen)

    # Repeat self.ga_nb_gen times
    for gen in gens:
        offspring = list(toolbox.map(toolbox.clone, population))

        # Crossover
        for child1, child2 in zip(offspring[::2], offspring[1::2]):
            if np.random.rand() < crossover_prob:
                toolbox.mate(child1, child2)
                del child1.fitness.values
                del child2.fitness.values

        # Mutation
        for mutant in offspring:
            toolbox.mutate(mutant)
            del mutant.fitness.values

        for i in range(len(offspring)):
            for j in range(len(offspring[i])):
                if offspring[i][j] < 0 and j % 5 != 4:
                    offspring[i][j] = 0.0

        # Evaluate fitness for new individuals
        invalid_ind = [ind for ind in offspring if not ind.fitness.valid]
        fitnesses = toolbox.map(toolbox.evaluate, invalid_ind)
        for ind, fit in zip(invalid_ind, fitnesses):
            ind.fitness.values = fit,

        # Selection
        population[:] = toolbox.select(population + offspring, pop_size, 10)

        # Elitism: we always keep the best found individual in the population instead of the individual giving the
        # worst fitness
        hof.update(population + [hof[0]])

        best_ind = hof[0]
        best_fun = best_ind.fitness.values[0]
        if disp:
            xi_str = ', '.join([f'{best_ind[j]:.8f}' for j in range(ind_size)])
            xi_str = f'({xi_str})'
            print(f'Iter = {gen}, xi = {xi_str}, f(x) = {best_fun}')

    return np.array(hof[0]), hof[0].fitness.values[0]
