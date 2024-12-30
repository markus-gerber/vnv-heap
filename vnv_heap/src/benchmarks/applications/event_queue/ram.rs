use std::mem::{size_of, MaybeUninit};

use crate::benchmarks::applications::event_queue::run_event_queue_application;
use serde::Serialize;

use super::{super::super::{Benchmark, Timer}, EventQueue};

#[derive(Serialize)]
pub struct EventQueueRAMBenchmarkOptions {
    object_size: usize,
    queue_length: usize,
    iterations: usize,
    buffer_size: usize,
    ram_overhead: usize
}

pub struct EventQueueRAMBenchmark<'a, const OBJ_SIZE: usize> {
    ring_buffer: RingBuffer<'a, OBJ_SIZE>,    
    queue_length: usize,
    iterations: usize,
}

impl<'a, const OBJ_SIZE: usize> EventQueueRAMBenchmark<'a, OBJ_SIZE>
{
    pub fn new(
        buffer: &'a mut [MaybeUninit<[u8; OBJ_SIZE]>],
        queue_length: usize,
        iterations: usize,
    ) -> Self {
        assert_eq!(buffer.len(), queue_length + 1);

        Self {
            ring_buffer: RingBuffer::new(buffer),
            queue_length,
            iterations,
        }
    }
}

impl<
        'a,
        const OBJ_SIZE: usize,
    > Benchmark<EventQueueRAMBenchmarkOptions>
    for EventQueueRAMBenchmark<'a, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "event_queue_ram"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        run_event_queue_application::<OBJ_SIZE, Self, T>(self, self.queue_length, self.iterations)
    }

    #[inline]
    fn get_bench_options(&self) -> EventQueueRAMBenchmarkOptions {
        EventQueueRAMBenchmarkOptions {
            object_size: OBJ_SIZE,
            iterations: self.iterations,
            queue_length: self.queue_length,
            buffer_size: self.ring_buffer.buffer.len() * size_of::<MaybeUninit<[u8; OBJ_SIZE]>>(),
            // next_in, next_out and buffer slice from ring buffer
            ram_overhead: 4 * size_of::<usize>()
        }
    }
}

impl<
        'a,
        const OBJ_SIZE: usize,
    > EventQueue<OBJ_SIZE> for EventQueueRAMBenchmark<'a, OBJ_SIZE>
{
    fn produce(&mut self, data: [u8; OBJ_SIZE]) {
        self.ring_buffer.push_front(data).unwrap();
    }

    fn consume(&mut self) -> Option<[u8; OBJ_SIZE]> {
        self.ring_buffer.pop_back()
    }
    
    fn capacity(&self) -> usize {
        self.ring_buffer.buffer.len()
    }
}


pub(super) struct RingBuffer<'a, const OBJ_SIZE: usize> {
    buffer: &'a mut [MaybeUninit<[u8; OBJ_SIZE]>],
    next_in: usize,
    next_out: usize,
    full: bool
}

impl<'a, const OBJ_SIZE: usize> RingBuffer<'a, OBJ_SIZE> {
    fn new(buffer: &'a mut [MaybeUninit<[u8; OBJ_SIZE]>]) -> Self {
        assert!(buffer.len() > 0);

        Self {
            buffer,
            next_in: 0,
            next_out: 0,
            full: false
        }
    }

    fn push_front(&mut self, obj: [u8; OBJ_SIZE]) -> Result<(), ()> {
        if self.full {
            return Err(());
        }

        self.buffer[self.next_in] = MaybeUninit::new(obj);
        self.next_in = (self.next_in + 1) % self.buffer.len();

        if self.next_in == self.next_out {
            self.full = true;
        }

        Ok(())
    }

    fn pop_back(&mut self) -> Option<[u8; OBJ_SIZE]> {
        if self.next_in == self.next_out && !self.full {
            return None;
        }

        let obj = unsafe { self.buffer[self.next_out].assume_init() };
        self.next_out = (self.next_out + 1) % self.buffer.len();
        self.full = false;

        Some(obj)
    }
}
