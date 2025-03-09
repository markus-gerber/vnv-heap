use std::mem::size_of;

use crate::{benchmarks::applications::queue::run_queue_application, modules::persistent_storage::PersistentStorageModule};
use serde::Serialize;

use super::{super::super::{Benchmark, Timer}, Queue};

#[derive(Serialize)]
pub struct QueueStorageBenchmarkOptions {
    object_size: usize,
    queue_length: usize,
    iterations: usize,
    ram_overhead: usize,
}

pub struct QueueStorageBenchmark<'a, const OBJ_SIZE: usize, S: PersistentStorageModule> {
    ring_buffer: StorageRingBuffer<'a, OBJ_SIZE, S>,
    queue_length: usize,
    iterations: usize,
}

impl<'a, const OBJ_SIZE: usize, S: PersistentStorageModule> QueueStorageBenchmark<'a, OBJ_SIZE, S>
{
    pub fn new(
        storage: &'a mut S,
        queue_length: usize,
        iterations: usize,
    ) -> Self {
        let capacity = storage.get_max_size() / (size_of::<[u8; OBJ_SIZE]>());

        assert!(capacity >= queue_length);

        Self {
            ring_buffer: StorageRingBuffer::new(storage, capacity),
            queue_length,
            iterations,
        }
    }
}

impl<
        'a,
        const OBJ_SIZE: usize,
        S: PersistentStorageModule
    > Benchmark<QueueStorageBenchmarkOptions>
    for QueueStorageBenchmark<'a, OBJ_SIZE, S>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "queue_storage"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        run_queue_application::<OBJ_SIZE, Self, T>(self, self.queue_length, self.iterations)
    }

    #[inline]
    fn get_bench_options(&self) -> QueueStorageBenchmarkOptions {
        QueueStorageBenchmarkOptions {
            object_size: OBJ_SIZE,
            iterations: self.iterations,
            queue_length: self.queue_length,

            // next_in, next_out and buffer size from ring buffer and storage module
            ram_overhead: 3 * size_of::<usize>() + size_of::<S>()
        }
    }
}

impl<
        'a,
        const OBJ_SIZE: usize,
        S: PersistentStorageModule
    > Queue<OBJ_SIZE> for QueueStorageBenchmark<'a, OBJ_SIZE, S>
{
    fn produce(&mut self, data: [u8; OBJ_SIZE]) {
        self.ring_buffer.push_front(data).unwrap();
    }

    fn consume(&mut self) -> Option<[u8; OBJ_SIZE]> {
        self.ring_buffer.pop_back()
    }

    fn capacity(&self) -> usize {
        self.ring_buffer.capacity
    }
}


struct StorageRingBuffer<'a, const OBJ_SIZE: usize, S: PersistentStorageModule> {
    storage: &'a mut S,
    next_in: usize,
    next_out: usize,
    capacity: usize
}

impl<'a, const OBJ_SIZE: usize, S: PersistentStorageModule> StorageRingBuffer<'a, OBJ_SIZE, S> {
    fn new(storage: &'a mut S, capacity: usize) -> Self {
        assert!(storage.get_max_size() >= capacity * size_of::<[u8; OBJ_SIZE]>());

        Self {
            storage,
            next_in: 0,
            next_out: 0,
            capacity
        }
    }

    fn push_front(&mut self, obj: [u8; OBJ_SIZE]) -> Result<(), ()> {
        let new_next_in = (self.next_in + size_of::<[u8; OBJ_SIZE]>()) % (self.capacity * size_of::<[u8; OBJ_SIZE]>());
        if new_next_in == self.next_out {
            return Err(());
        }

        self.storage.write(self.next_in, &obj)?;
        self.next_in = new_next_in;

        Ok(())
    }

    fn pop_back(&mut self) -> Option<[u8; OBJ_SIZE]> {
        if self.next_in == self.next_out {
            return None;
        }

        let mut obj = [0u8; OBJ_SIZE];
        self.storage.read(self.next_out, &mut obj).unwrap();
        self.next_out = (self.next_out + size_of::<[u8; OBJ_SIZE]>()) % (self.capacity * size_of::<[u8; OBJ_SIZE]>());

        Some(obj)
    }
}
