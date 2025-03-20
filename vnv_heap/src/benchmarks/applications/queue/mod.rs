/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

 use std::hint::black_box;

use crate::benchmarks::Timer;

mod vnv_heap;
mod ram;
mod storage;
mod runner;
pub(crate) use runner::QueueBenchmarkRunner;

fn run_queue_application<const OBJ_SIZE: usize, Q: Queue<OBJ_SIZE>, T: Timer>(
    queue: &mut Q,
    queue_length: usize,
    iterations: usize,
) -> u32 {
    assert!(queue.consume().is_none(), "list should be empty");
    assert!(queue.capacity() >= queue_length + 1);


    let mut seed = 0;
    for _ in 0..queue_length {
        let mut obj = [0u8; OBJ_SIZE];
        rand_data(&mut obj, seed);
        seed += 1;

        queue.produce(obj);
    }

    let timer = T::start();

    for _ in 0..iterations {
        // first produce an object
        let mut obj = [0u8; OBJ_SIZE];
        rand_data(&mut obj, seed);
        seed += 1;

        queue.produce(obj);

        // then consume an other one
        black_box(queue.consume().unwrap());
    }

    let duration = timer.stop();

    for _ in 0..queue_length {
        black_box(queue.consume().unwrap());
    }


    duration
}

trait Queue<const OBJ_SIZE: usize> {
    fn produce(&mut self, data: [u8; OBJ_SIZE]);
    fn consume(&mut self) -> Option<[u8; OBJ_SIZE]>;
    fn capacity(&self) -> usize;
}

fn rand_data(arr: &mut [u8], seed: usize) {
    for i in 0..arr.len() {
        arr[i] = (((i * 7001 + 301 * seed) % 17) + ((seed) % (256 - 17))) as u8;
    }
}
