// Copyright (c) 2017 Ashley Jeffs
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use unit::Unit;

use crossbeam::scope;

use rand::{SeedableRng, StdRng};
use rand::distributions::{IndependentSample, Range};

use std::mem;
use std::sync::{Arc, Mutex, Condvar};
use std::cmp::Ordering;
use std::sync::mpsc::sync_channel;

/// Wraps a unit within a struct that lazily evaluates its fitness to avoid
/// duplicate work.
struct LazyUnit<T: Unit> {
    unit: T,
    lazy_fitness: Option<f64>,
}

impl<T: Unit> LazyUnit<T> {
    fn from(unit: T) -> Self {
        LazyUnit {
            unit: unit,
            lazy_fitness: None,
        }
    }

    fn fitness(&mut self) -> f64 {
        match self.lazy_fitness {
            Some(x) => x,
            None => {
                let fitness = self.unit.fitness();
                self.lazy_fitness = Some(fitness);
                fitness
            }
        }
    }
}

/// Population is an abstraction that represents a collection of units. Each
/// unit is a combination of variables, which produces an overall fitness. Units
/// mate with other units to produce mutated offspring combining traits from
/// both units.
///
/// The population is responsible for iterating new generations of units by
/// mating fit units and killing unfit units.
pub struct Population<T: Unit> {
    units: Vec<T>,

    seed: usize,
    breed_factor: f64,
    survival_factor: f64,
    max_size: usize,
}

impl<T: Unit> Population<T> {
    /// Creates a new population, starts off with an empty population. If you
    /// wish to start with a preset population of units you can call
    /// `set_population` before calling epochs.
    pub fn new(init_pop: Vec<T>) -> Self {
        Population {
            units: init_pop,
            seed: 1,
            breed_factor: 0.5,
            survival_factor: 0.5,
            max_size: 100,
        }
    }

    //--------------------------------------------------------------------------

    /// Sets the random seed of the population.
    pub fn set_rand_seed(&mut self, seed: usize) -> &mut Self {
        self.seed = seed;
        self
    }

    /// Sets the maximum size of the population. If already populated with more
    /// than this amount a random section of the population is killed.
    pub fn set_size(&mut self, size: usize) -> &mut Self {
        self.units.truncate(size);
        self.max_size = size;
        self
    }

    /// Sets the breed_factor (0 < b <= 1) of the genetic algorithm, which is
    /// the percentage of the population that will be able to breed per epoch.
    /// Units that are more fit are preferred for breeding, and so a high
    /// breed_factor results in more poorly performing units being able to
    /// breed, which will slow the algorithm down but allow it to escape local
    /// peaks.
    pub fn set_breed_factor(&mut self, breed_factor: f64) -> &mut Self {
        assert!(breed_factor > 0.0 && breed_factor <= 1.0);
        self.breed_factor = breed_factor;
        self
    }

    /// Sets the survival_factor (0 <= b <= 1) of the genetic algorithm, which
    /// is the percentage of the breeding population that will survive each
    /// epoch. Units that are more fit are preferred for survival, and so a high
    /// survival rate results in more poorly performing units being carried into
    /// the next epoch.
    ///
    /// Note that this value is a percentage of the breeding population. So if
    /// your breeding factor is 0.5, and your survival factor is 0.9, the
    /// percentage of units that will survive the next epoch is:
    ///
    /// 0.5 * 0.9 * 100 = 45%
    ///
    pub fn set_survival_factor(&mut self, survival_factor: f64) -> &mut Self {
        assert!(survival_factor >= 0.0 && survival_factor <= 1.0);
        self.survival_factor = survival_factor;
        self
    }

    //--------------------------------------------------------------------------

    /// An epoch that allows units to breed and mutate without harsh culling.
    /// It's important to sometimes allow 'weak' units to produce generations
    /// that might escape local peaks in certain dimensions.
    fn epoch(&self, units: &mut Vec<LazyUnit<T>>, mut rng: StdRng) -> StdRng {
        assert!(units.len() > 0);

        // breed_factor dicates how large a percentage of the population will be
        // able to breed.
        let breed_up_to = (self.breed_factor * (units.len() as f64)) as usize;
        let mut breeders: Vec<LazyUnit<T>> = Vec::new();

        while let Some(unit) = units.pop() {
            breeders.push(unit);
            if breeders.len() == breed_up_to {
                break;
            }
        }
        units.clear();

        // The strongest half of our breeders will survive each epoch. Always at
        // least one.
        let surviving_parents = (breeders.len() as f64 * self.survival_factor).ceil() as usize;

        let pcnt_range = Range::new(0, breeders.len());
        for i in 0..self.max_size - surviving_parents {
            let rs = pcnt_range.ind_sample(&mut rng);
            units.push(LazyUnit::from(
                breeders[i % breeders.len()].unit.breed_with(
                    &breeders[rs].unit,
                ),
            ));
        }

        // Move our survivors into the new generation.
        units.append(&mut breeders.drain(0..surviving_parents).collect());

        rng
    }

    /// Runs a number of epochs where fitness is calculated across n parallel
    /// processes. This is useful when the fitness calcuation is an expensive
    /// operation.
    pub fn epochs_parallel(&mut self, n_epochs: u32, n_processes: u32) -> &mut Self {
        scope(|scope| {
            let cvar_pair = Arc::new((Mutex::new(0), Condvar::new()));

            let (tx, rx) = sync_channel(0);
            let process_queue = Arc::new(Mutex::new(rx));

            let processed_stack = Arc::new(Mutex::new(Vec::new()));

            for _ in 0..n_processes {
                let cvar_pair_clone = cvar_pair.clone();
                let processed_stack_clone = processed_stack.clone();
                let process_queue_clone = process_queue.clone();

                scope.spawn(move || {
                    let &(ref lock, ref cvar) = &*cvar_pair_clone;
                    loop {
                        let mut l_unit: LazyUnit<T> =
                            match process_queue_clone.lock().ok().unwrap().recv() {
                                Ok(u) => u,
                                Err(_) => return,
                            };
                        l_unit.fitness();
                        processed_stack_clone.lock().ok().unwrap().push(l_unit);
                        {
                            let mut processed = lock.lock().unwrap();
                            *processed += 1;
                            cvar.notify_all();
                        }
                    }
                });
            }

            let &(ref lock, ref cvar) = &*cvar_pair;
            let mut active_stack = Vec::new();

            while let Some(unit) = self.units.pop() {
                active_stack.push(LazyUnit::from(unit));
            }

            let seed: &[_] = &[self.seed];
            let mut rng: StdRng = SeedableRng::from_seed(seed);

            for i in 0..(n_epochs + 1) {
                let jobs_total = active_stack.len();

                while let Some(unit) = active_stack.pop() {
                    tx.send(unit).unwrap();
                }

                let mut jobs_processed = lock.lock().unwrap();
                while *jobs_processed != jobs_total {
                    jobs_processed = cvar.wait(jobs_processed).unwrap();
                }
                *jobs_processed = 0;

                // Swap the full processed_stack with the active stack.
                mem::swap(&mut active_stack, &mut processed_stack.lock().ok().unwrap());

                // We want to sort such that highest fitness units are at the
                // end.
                active_stack.sort_by(|a, b| {
                    a.lazy_fitness
                        .unwrap_or(0.0)
                        .partial_cmp(&b.lazy_fitness.unwrap_or(0.0))
                        .unwrap_or(Ordering::Equal)
                });

                // If we have the perfect solution then break early.
                if active_stack.last().unwrap().lazy_fitness.unwrap_or(0.0) == 1.0 {
                    break;
                }

                if i != n_epochs {
                    rng = self.epoch(&mut active_stack, rng);
                }
            }

            // Reverse the order of units such that the first unit is the
            // strongest candidate.
            while let Some(unit) = active_stack.pop() {
                self.units.push(unit.unit);
            }
        });

        self
    }

    /// Runs a number of epochs on a single process.
    pub fn epochs(&mut self, n_epochs: u32) -> &mut Self {
        let mut processed_stack = Vec::new();
        let mut active_stack = Vec::new();

        while let Some(unit) = self.units.pop() {
            active_stack.push(LazyUnit::from(unit));
        }

        let seed: &[_] = &[self.seed];
        let mut rng: StdRng = SeedableRng::from_seed(seed);

        for i in 0..(n_epochs + 1) {
            while let Some(mut unit) = active_stack.pop() {
                unit.fitness();
                processed_stack.push(unit);
            }

            // Swap the full processed_stack with the active stack.
            mem::swap(&mut active_stack, &mut processed_stack);

            // We want to sort such that highest fitness units are at the
            // end.
            active_stack.sort_by(|a, b| {
                a.lazy_fitness
                    .unwrap_or(0.0)
                    .partial_cmp(&b.lazy_fitness.unwrap_or(0.0))
                    .unwrap_or(Ordering::Equal)
            });

            // If we have the perfect solution then break early.
            if active_stack.last().unwrap().lazy_fitness.unwrap_or(0.0) == 1.0 {
                break;
            }

            if i != n_epochs {
                rng = self.epoch(&mut active_stack, rng);
            }
        }

        // Reverse the order of units such that the first unit is the
        // strongest candidate.
        while let Some(unit) = active_stack.pop() {
            self.units.push(unit.unit);
        }

        self
    }

    //--------------------------------------------------------------------------

    /// Returns the full population of units, ordered such that the first
    /// element is the strongest candidate. This collection can be used to
    /// create a new population.
    pub fn finish(&mut self) -> Vec<T> {
        let mut empty_units: Vec<T> = Vec::new();
        mem::swap(&mut empty_units, &mut self.units);
        empty_units
    }
}
