use rand_xoshiro::{
    rand_core::{RngCore, SeedableRng},
    Xoshiro128StarStar,
};

use crate::benchmarks::Timer;

mod bench;
mod page_wise;
mod runner;
mod vnv_heap;
pub(crate) use runner::KVSBenchmarkRunner;

fn random_array<const SIZE: usize>(rng: &mut Xoshiro128StarStar) -> [u8; SIZE] {
    [rng.next_u32() as u8; SIZE]
}

fn run_kvs_application_equiv_obj_len<
    const OBJ_SIZE: usize,
    InternalPointer,
    I: KeyValueStoreImpl<InternalPointer>,
    T: Timer,
>(
    internal: &mut I,
    value_cnt: usize,
    iterations: usize,
) -> u32 {
    let mut kvs = KeyValueStore::new(internal);

    const DATA_SEED: [u8; 16] = [
        17, 47, 137, 149, 21, 154, 201, 98, 148, 76, 203, 156, 140, 247, 234, 183,
    ];
    const CONTROL_SEED: [u8; 16] = [
        149, 228, 163, 172, 175, 184, 104, 86, 131, 185, 95, 73, 18, 58, 248, 111,
    ];

    // deterministic random number generators for data and control flow
    let mut data_rng = Xoshiro128StarStar::from_seed(DATA_SEED);
    let mut control_rng = Xoshiro128StarStar::from_seed(CONTROL_SEED);

    for i in 0..value_cnt {
        kvs.insert(i as u32, random_array::<OBJ_SIZE>(&mut data_rng))
            .unwrap();
        kvs.flush::<OBJ_SIZE>(i as u32).unwrap();
    }

    let timer = T::start();

    for _ in 0..iterations {
        let access_index = control_rng.next_u32();
        let index = access_index % value_cnt as u32;
        kvs.update(index, random_array::<OBJ_SIZE>(&mut data_rng))
            .unwrap();
    }

    let duration = timer.stop();
    for i in 0..value_cnt {
        kvs.remove::<OBJ_SIZE>(i as u32).unwrap();
    }

    duration
}

struct KeyValuePair<InternalPointer> {
    key: u32,
    value: InternalPointer,
}

struct KeyValueStore<'a, I: KeyValueStoreImpl<InternalPointer>, InternalPointer> {
    implementation: &'a mut I,

    // we simplify our implementation with using a Vec, in the real world you would not do this
    key_value_pairs: Vec<KeyValuePair<InternalPointer>>,
}

impl<'a, I: KeyValueStoreImpl<InternalPointer>, InternalPointer>
    KeyValueStore<'a, I, InternalPointer>
{
    fn new(implementation: &'a mut I) -> Self {
        Self {
            implementation,
            key_value_pairs: Vec::new(),
        }
    }

    fn insert<const SIZE: usize>(&mut self, key: u32, value: [u8; SIZE]) -> Result<(), ()> {
        let ptr = self.implementation.allocate::<[u8; SIZE]>(value)?;

        self.key_value_pairs.push(KeyValuePair { key, value: ptr });

        Ok(())
    }

    #[allow(unused)]
    fn get<const SIZE: usize>(&mut self, key: u32) -> Result<[u8; SIZE], ()> {
        let pair = self
            .key_value_pairs
            .iter()
            .find(|kvp| kvp.key == key)
            .ok_or(())?;

        Ok(self.implementation.get(&pair.value)?)
    }
    fn update<const SIZE: usize>(&mut self, key: u32, value: [u8; SIZE]) -> Result<(), ()> {
        let pair = self
            .key_value_pairs
            .iter()
            .find(|kvp| kvp.key == key)
            .ok_or(())?;

        self.implementation.update(&pair.value, value)
    }

    #[allow(unused)]
    fn remove<const SIZE: usize>(&mut self, key: u32) -> Result<(), ()> {
        let pair = self
            .key_value_pairs
            .iter()
            .find(|kvp| kvp.key == key)
            .ok_or(())?;

        self.implementation.deallocate::<[u8; SIZE]>(&pair.value);
        Ok(())
    }

    #[allow(unused)]
    fn flush<const SIZE: usize>(&mut self, key: u32) -> Result<(), ()> {
        let pair = self
            .key_value_pairs
            .iter()
            .find(|kvp| kvp.key == key)
            .ok_or(())?;

        self.implementation.flush::<[u8; SIZE]>(&pair.value);
        Ok(())
    }
}

trait KeyValueStoreImpl<InternalPointer> {
    fn allocate<T>(&self, data: T) -> Result<InternalPointer, ()>;
    fn deallocate<T>(&self, ptr: &InternalPointer);
    fn get<T: Copy>(&mut self, ptr: &InternalPointer) -> Result<T, ()>;
    fn update<T>(&mut self, ptr: &InternalPointer, data: T) -> Result<(), ()>;
    fn flush<T>(&mut self, ptr: &InternalPointer) -> Result<(), ()>;
}
