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

//! Spiril is an implementation of a genetic algorithm for obtaining optimum
//! variables (genetics) for a task through mutation and natural selection.
//!
//! The API allows you to specify an initial group of units, which will act as
//! the original parents of all subsequent units.
//!
//! # Examples
//!
//! ```
//! extern crate spiril;
//! extern crate rand;
//!
//! use spiril::unit::Unit;
//! use spiril::population::Population;
//! use rand::{StdRng, SeedableRng, Rng};
//!
//! struct SudokuUnit {
//!     sudoku: Vec<usize>, // 9x9 grid
//!     answer: Vec<usize>, // 9x9 grid
//! }
//!
//! impl Unit for SudokuUnit {
//!     fn fitness(&self) -> f64 {
//!         let mut score = 1.0_f64;
//!
//!         for i in 0..9 {
//!             let mut seen_row: [usize; 9] = [0, 0, 0, 0, 0, 0, 0, 0, 0];
//!             let mut seen_col: [usize; 9] = [0, 0, 0, 0, 0, 0, 0, 0, 0];
//!             let mut seen_sqr: [usize; 9] = [0, 0, 0, 0, 0, 0, 0, 0, 0];
//!
//!             for j in 0..9 {
//!                 seen_row[self.answer[i * 9 + j] - 1] += 1;
//!                 seen_col[self.answer[i + 9 * j] - 1] += 1;
//!
//!                 let sqr_index = ((i % 3) * 3) + (((i / 3) % 3) * 27) + (9 * (j / 3)) + j % 3;
//!                 seen_sqr[self.answer[sqr_index] - 1] += 1;
//!             }
//!
//!             seen_row
//!                 .iter()
//!                 .chain(seen_col.iter())
//!                 .chain(seen_sqr.iter())
//!                 .map(|x| if *x == 0 {
//!                     // score -= (1.0 / 729.0);
//!                     score *= 0.9;
//!                 })
//!                 .last();
//!         }
//!
//!         score
//!     }
//!
//!     fn breed_with(&self, other: &SudokuUnit) -> SudokuUnit {
//!         // Even rows taken from self, odd rows taken from other.
//!         // Mutations applied at random.
//!         let mut new_unit: SudokuUnit = SudokuUnit {
//!             sudoku: self.sudoku.clone(),
//!             answer: self.answer.clone(),
//!         };
//!
//!         (0_usize..81_usize)
//!             .filter(|x| self.sudoku[*x] == 0)
//!             .map(|x| {
//!                 if rand::thread_rng().gen_range(0, 1) == 1 {
//!                     new_unit.answer[x] = other.answer[x];
//!                 }
//!                 new_unit.answer[x]
//!             })
//!             .last();
//!
//!         loop {
//!             let i = rand::thread_rng().gen_range(0, 81);
//!             if self.sudoku[i] == 0 {
//!                 new_unit.answer[i] = rand::thread_rng().gen_range(1, 10);
//!                 break;
//!             }
//!         }
//!
//!         new_unit
//!     }
//! }
//!
//! fn main() {
//!     let test_doku: Vec<usize> = vec![
//!         7, 2, 6,   0, 9, 3,   8, 1, 5,
//!         3, 0, 5,   7, 2, 8,   9, 0, 6,
//!         4, 8, 0,   6, 0, 1,   2, 3, 7,
//!
//!         8, 5, 2,   1, 4, 0,   6, 9, 3,
//!         0, 7, 3,   9, 8, 5,   1, 2, 4,
//!         9, 4, 1,   0, 6, 2,   0, 5, 8,
//!
//!         1, 9, 0,   8, 3, 0,   5, 7, 2,
//!         5, 6, 7,   2, 1, 4,   3, 8, 0,
//!         2, 0, 8,   5, 0, 9,   4, 6, 1,
//!     ];
//!
//!     let seed: &[_] = &[0];
//!     let mut init_rng: StdRng = SeedableRng::from_seed(seed);
//!     let units: Vec<SudokuUnit> = (0..1000)
//!         .map(|_| {
//!             SudokuUnit {
//!                 sudoku: test_doku.clone(),
//!                 answer: test_doku
//!                     .clone()
//!                     .iter()
//!                     .map(|x| if *x == 0 {
//!                         init_rng.gen_range(1, 10)
//!                     } else {
//!                         *x
//!                     })
//!                     .collect(),
//!             }
//!         })
//!         .collect();
//!
//!     assert_eq!(Population::new(units)
//!         .set_size(1000)
//!         .set_breed_factor(0.3)
//!         .set_survival_factor(1.0)
//!         .epochs(5000)
//!         .finish()
//!         .first()
//!         .unwrap()
//!         .fitness(), 1.0);
//! }
//! ```

extern crate crossbeam;
extern crate rand;

mod test;

pub mod population;
pub mod unit;
