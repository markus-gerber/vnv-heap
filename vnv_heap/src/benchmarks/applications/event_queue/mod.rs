use std::hint::black_box;

use crate::benchmarks::Timer;

mod implementation;
mod ram;
mod storage;
mod runner;
pub(crate) use runner::EventQueueBenchmarkRunner;

fn run_event_queue_application<const OBJ_SIZE: usize, Q: EventQueue<OBJ_SIZE>, T: Timer>(
    queue: &mut Q,
    queue_length: usize,
    iterations: usize,
) -> u32 {
    assert!(queue.consume().is_none(), "list should be empty");
    assert!(queue.capacity() >= queue_length + 1);

    let timer = T::start();

    let mut seed = 0;

    for _ in 0..1 {
        for _ in 0..queue_length {
            let mut obj = [0u8; OBJ_SIZE];
            rand_data(&mut obj, seed);
            seed += 1;
    
            queue.produce(obj);
        }    

        for _ in 0..iterations {
            // first produce an object
            let mut obj = [0u8; OBJ_SIZE];
            rand_data(&mut obj, seed);
            seed += 1;
    
            queue.produce(obj);
    
            // then consume an other one
            black_box(queue.consume().unwrap());
        }
    
        for _ in 0..queue_length {
            black_box(queue.consume().unwrap());
        }
    }

    let duration = timer.stop();
    duration
}

trait EventQueue<const OBJ_SIZE: usize> {
    fn produce(&mut self, data: [u8; OBJ_SIZE]);
    fn consume(&mut self) -> Option<[u8; OBJ_SIZE]>;
    fn capacity(&self) -> usize;
}

fn rand_data(arr: &mut [u8], seed: usize) {
    for i in 0..arr.len() {
        arr[i] = (((i * 7001 + 301 * seed) % 17) + ((seed) % (256 - 17))) as u8;
    }
}
