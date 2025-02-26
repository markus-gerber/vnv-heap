use std::array::from_fn;

use rand_xoshiro::{
    rand_core::{RngCore, SeedableRng},
    Xoshiro128StarStar,
};
use serde::Serialize;

use crate::benchmarks::Timer;

mod bench;
mod page_wise;
mod runner;
mod vnv_heap;
pub(crate) use runner::KVSBenchmarkRunner;

fn random_array<const SIZE: usize>(rng: &mut Xoshiro128StarStar) -> [u8; SIZE] {
    [rng.next_u32() as u8; SIZE]
}

#[derive(Serialize, Copy, Clone)]
enum AccessType {
    /// totally random access over the keys
    Random,
    /// sequential access over the keys
    Sequential,
    /// chose random random partition (range of keys), random access within the partition
    ///
    /// if there exists a rest partition with a partition size > partition_size, then it is simply ignored
    Partitioned {
        /// size of the partition in keys
        partition_size: usize,
        /// access count within the partition
        ///
        /// decreases the amount of total partition iterations
        access_count: usize,

        curr_partition: u32,
    },
    // Distributed,
}

impl AccessType {
    fn next_key(
        &mut self,
        iteration: usize,
        _total_iterations: usize,
        value_cnt: usize,
        control_rng: &mut Xoshiro128StarStar,
    ) -> u32 {
        match self {
            AccessType::Random => {
                let rand = control_rng.next_u32();
                rand % value_cnt as u32
            }
            AccessType::Sequential => (iteration as u32) % value_cnt as u32,
            AccessType::Partitioned {
                partition_size,
                access_count,
                curr_partition,
            } => {
                debug_assert!(*partition_size > 0);
                debug_assert!(*access_count > 0);

                if iteration % *access_count == 0 {
                    // choose next partition
                    *curr_partition =
                        control_rng.next_u32() % (value_cnt as u32 / *partition_size as u32);
                }
                let partition_start = (*curr_partition) * (*partition_size as u32);
                partition_start + (control_rng.next_u32() % (*partition_size as u32))
            }
            // AccessType::Distributed => {
            //     todo!()
            // }
        }
    }
}

pub(super) const KVS_APP_DIVERSE_OBJ_LEN_OBJ_VALUES: usize = 4;
pub(super) const KVS_APP_DIVERSE_OBJ_LEN_OBJ_SIZES: [usize; KVS_APP_DIVERSE_OBJ_LEN_OBJ_VALUES] =
    [32, 128, 256, 512];
pub(super) const KVS_APP_DIVERSE_OBJ_LEN_OBJ_COUNT_DISTRIBUTION: [usize;
    KVS_APP_DIVERSE_OBJ_LEN_OBJ_VALUES] = [20, 20, 10, 10];

fn run_kvs_application_diverse_obj_len<
    InternalPointer,
    I: KeyValueStoreImpl<InternalPointer>,
    T: Timer,
>(
    internal: &mut I,
    value_cnt: usize,
    iterations: usize,
    mut access_type: AccessType,
) -> u32 {
    const DEBUG: bool = false;
    const OBJ_VALUES: usize = KVS_APP_DIVERSE_OBJ_LEN_OBJ_VALUES;
    const OBJ_SIZES: [usize; OBJ_VALUES] = KVS_APP_DIVERSE_OBJ_LEN_OBJ_SIZES;
    const OBJ_COUNT_DISTRIBUTION: [usize; OBJ_VALUES] =
        KVS_APP_DIVERSE_OBJ_LEN_OBJ_COUNT_DISTRIBUTION;

    // calculate absolute object count for each object size
    let total_object_distribution_count: usize = OBJ_COUNT_DISTRIBUTION.iter().sum();
    let mut object_count: [usize; OBJ_VALUES] =
        from_fn(|i| (value_cnt * OBJ_COUNT_DISTRIBUTION[i]) / total_object_distribution_count);
    let object_count_sum: usize = object_count.iter().sum();
    let object_count_diff: usize = value_cnt - object_count_sum;

    if object_count_diff > 0 {
        // put the remaining objects to the object with the highest value in OBJ_COUNT_DISTRIBUTION
        let index = OBJ_COUNT_DISTRIBUTION
            .iter()
            .enumerate()
            .max_by_key(|(_, &val)| val)
            .unwrap()
            .0;
        object_count[index] += object_count_diff;
    }

    debug_assert_eq!(object_count.iter().sum::<usize>(), value_cnt);

    macro_rules! for_obj_size_impl {
        ($index: ident, $value: expr, { $($inner: stmt)* }) => {
            //static_assertions::const_assert_eq!(OBJ_SIZES.len(), $value);
            seq_macro::seq!(I in 0..$value {
                $($inner)*
            });
        };
    }

    macro_rules! for_obj_size {
        ($index: ident, { $($inner: stmt)* }) => {
            for_obj_size_impl!($index, 4, { $($inner)* })
        };
    }

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

    // initialize objects
    for_obj_size!(I, {
        // helper variables for generics: OBJ_SIZE_0, ..., OBJ_SIZE_n
        paste::paste! {
            const [<OBJ_SIZE_ I>]: usize = OBJ_SIZES[I];
        }
        // list of keys for each object size: objects0, ..., objectsn
        paste::paste! {
            let mut [<objects I>]: Vec<u32> = vec![];
        }
    });

    {
        let mut remaining_cnts = object_count.clone();

        // insert values randomly so that the values are not order by their size

        // NOTE: as choosing the next value to be inserted is uniformly distributed
        // the object that have a small amount of instances will normally be inserted
        // more at the beginning. Towards the end of this loop only objects with a higher
        // amount of instances will be inserted.
        // -> its probably good to avoid big differences in the object distribution
        for curr_key in 0..value_cnt {
            let mut obj_size_index = control_rng.next_u32() as usize % OBJ_VALUES;
            while remaining_cnts[obj_size_index] == 0 {
                obj_size_index = (obj_size_index + 1) % OBJ_VALUES;
            }
            remaining_cnts[obj_size_index] -= 1;

            for_obj_size!(I, {
                if I == obj_size_index {
                    if DEBUG {
                        paste::paste! {
                            println!("{}: Inserting object of size {}", curr_key, [<OBJ_SIZE_ I>]);
                        };
                    }

                    paste::paste! {
                        let arr = random_array::<[<OBJ_SIZE_ I>]>(&mut data_rng);
                    };
                    kvs.insert(curr_key as u32, arr).unwrap();
                    paste::paste! {
                        [<objects I>].push(curr_key as u32);
                    }
                }
            });
        }
    }

    if DEBUG {
        for_obj_size!(I, {
            paste::paste! {
                println!("{:?}", [<objects I>]);
            }
        });
    }

    // flush all objects
    for_obj_size!(I, {
        paste::paste! {
            for &key in [<objects I>].iter() {
                kvs.flush::<[<OBJ_SIZE_ I>]>(key as u32).unwrap();
            }
        }
    });

    // create a object size look up table: [KEY] -> [INDEX FOR OBJ_SIZES]
    let mut obj_size_lookup = vec![0; value_cnt];
    for_obj_size!(I, {
        paste::paste! {
            for &key in [<objects I>].iter() {
                obj_size_lookup[key as usize] = I;
            }
        }
    });

    macro_rules! lookup_obj_size {
        ($index: ident, $key: expr, { $($inner: stmt)* }) => {
            {
                let obj_size_index = obj_size_lookup[$key];
                for_obj_size!($index, {
                    if $index == obj_size_index {
                        $($inner)*
                    }
                });
            }
        };
    }

    let timer = T::start();

    for i in 0..iterations {
        let access_index = access_type.next_key(i, iterations, value_cnt, &mut control_rng);
        lookup_obj_size!(I, access_index as usize, {
            if DEBUG {
                paste::paste! {
                    println!("[{}] {} -> {}", i, access_index, [<OBJ_SIZE_ I>]);
                }
            }
            paste::paste! {
                kvs.update(access_index, random_array::<[<OBJ_SIZE_ I>]>(&mut data_rng)).unwrap();
            }
        });
    }

    let duration = timer.stop();

    for_obj_size!(I, {
        paste::paste! {
            for &key in [<objects I>].iter() {
                kvs.remove::<[<OBJ_SIZE_ I>]>(key as u32).unwrap();
            }
        }
    });

    duration
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
    mut access_type: AccessType,
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
        let arr = random_array::<OBJ_SIZE>(&mut data_rng);
        kvs.insert(i as u32, arr).unwrap();
    }

    for i in 0..value_cnt {
        kvs.flush::<OBJ_SIZE>(i as u32).unwrap();
    }

    let timer = T::start();

    for i in 0..iterations {
        let access_index = access_type.next_key(i, iterations, value_cnt, &mut control_rng);
        kvs.update(access_index, random_array::<OBJ_SIZE>(&mut data_rng))
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

// #[cfg(test)]
// mod test {

//     use std::time::Instant;

//     use crate::{
//         benchmarks::Timer,
//         modules::{
//             allocator::LinkedListAllocatorModule, persistent_storage::test::get_test_storage,
//         },
//     };

//     use super::{
//         page_wise::PagedKeyValueStoreImplementation, run_kvs_application_distributed_obj_len,
//         AccessType,
//     };

//     #[test]
//     fn test() {
//         let mut storage = get_test_storage("test", 100000);

//         let mut pages = [[0u8; 4096]; 20];
//         let mut i = PagedKeyValueStoreImplementation::new(
//             &mut storage,
//             LinkedListAllocatorModule::new(),
//             20,
//             &mut pages,
//         );
//         run_kvs_application_distributed_obj_len::<_, _, T>(
//             &mut i,
//             25,
//             100,
//             AccessType::Partitioned {
//                 partition_size: 5,
//                 access_count: 20,
//                 curr_partition: 0,
//             },
//         );
//     }

//     struct T {
//         start_time: Instant,
//     }

//     impl Timer for T {
//         fn get_ticks_per_ms() -> u32 {
//             1000
//         }

//         #[inline]
//         fn start() -> Self {
//             Self {
//                 start_time: Instant::now(),
//             }
//         }

//         #[inline]
//         fn stop(self) -> u32 {
//             (Instant::now() - self.start_time).subsec_micros()
//         }
//     }
// }
