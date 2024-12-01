use rand::{rngs::SmallRng, RngCore, SeedableRng};
use std::array;

use crate::resident_object_manager::partial_dirtiness_tracking::PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE;

use super::get_test_heap;

#[test]
fn test_heap_unload_vnv_list() {
    type TestType = [u8; 1000];

    fn rand_data(rand: &mut SmallRng) -> TestType {
        array::from_fn(|_| rand.next_u32() as u8)
    }

    let mut buffer = [0u8; 2000];
    let heap = get_test_heap("test_heap_persistency", 4 * 4096, &mut buffer, 2000, |_, _| {});
    const SEED: u64 = 5446535461589659585;

    let mut rand = SmallRng::seed_from_u64(SEED);

    let rand_obj = rand_data(&mut rand);
    let mut check_state= rand_obj.clone();
    let start_dirty_size = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;

    let mut list = heap.allocate_pd_array(rand_obj).unwrap();

    list.unload().unwrap();
    {
        assert_eq!(*list.get().unwrap(), check_state);
    }
    list.unload().unwrap();
    {
        let mut mut_ref = list.get_mut().unwrap();
        assert_eq!(*mut_ref, check_state);

        let prev_dirty_size = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;

        check_state[100] = rand.next_u32() as u8;
        mut_ref.set(100, check_state[100]).unwrap();
        assert_eq!(mut_ref.get(100), check_state[100]);

        let post_dirty_size = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;

        assert_eq!(prev_dirty_size, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE + post_dirty_size, "prev: {}, post: {}", prev_dirty_size, post_dirty_size);

        check_state[102] = rand.next_u32() as u8;
        mut_ref.set(102, check_state[102]).unwrap();
        assert_eq!(mut_ref.get(102), check_state[102]);
    }
    list.unload().unwrap();
    {
        assert_eq!(*list.get().unwrap(), check_state);
    }
    list.unload().unwrap();
    {
        let mut mut_ref = list.get_mut().unwrap();
        assert_eq!(*mut_ref, check_state);

        let prev_dirty_size = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;

        check_state[0] = rand.next_u32() as u8;
        mut_ref.set(0, check_state[0]).unwrap();
        assert_eq!(mut_ref.get(0), check_state[0]);

        let post_dirty_size = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;

        assert_eq!(prev_dirty_size, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE + post_dirty_size, "prev: {}, post: {}", prev_dirty_size, post_dirty_size);

        check_state[check_state.len() - 1] = rand.next_u32() as u8;
        mut_ref.set(check_state.len() - 1, check_state[check_state.len() - 1]).unwrap();
        assert_eq!(mut_ref.get(check_state.len() - 1), check_state[check_state.len() - 1]);
    }
    list.unload().unwrap();
    {
        assert_eq!(*list.get().unwrap(), check_state);
    }

    drop(list);

    let end_dirty_size = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;
    assert_eq!(start_dirty_size, end_dirty_size);
}


#[test]
fn test_heap_unload_vnv_object() {
    type TestType = [u8; 1000];

    fn rand_data(rand: &mut SmallRng) -> TestType {
        array::from_fn(|_| rand.next_u32() as u8)
    }

    let mut buffer = [0u8; 2000];
    let heap = get_test_heap("test_heap_persistency", 4 * 4096, &mut buffer, 1200, |_, _| {});
    const SEED: u64 = 5446535461589659585;

    let mut rand = SmallRng::seed_from_u64(SEED);

    let rand_obj = rand_data(&mut rand);
    let mut check_state= rand_obj.clone();
    let start_dirty_size = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;

    let mut obj = heap.allocate(rand_obj).unwrap();

    obj.unload().unwrap();
    {
        assert_eq!(*obj.get().unwrap(), check_state);
    }
    obj.unload().unwrap();
    {
        let mut mut_ref = obj.get_mut().unwrap();
        check_state[100] = rand.next_u32() as u8;
        mut_ref[100] = check_state[100];
        assert_eq!(mut_ref[100], check_state[100]);

        check_state[102] = rand.next_u32() as u8;
        mut_ref[102] = check_state[102];
        assert_eq!(mut_ref[102], check_state[102]);

    }
    obj.unload().unwrap();
    {
        assert_eq!(*obj.get().unwrap(), check_state);
    }
    obj.unload().unwrap();
    {

        let mut mut_ref = obj.get_mut().unwrap();
        check_state[0] = rand.next_u32() as u8;
        mut_ref[0] = check_state[0];
        assert_eq!(mut_ref[0], check_state[0]);

        check_state[check_state.len() - 1] = rand.next_u32() as u8;
        mut_ref[check_state.len() - 1] = check_state[check_state.len() - 1];
        assert_eq!(mut_ref[check_state.len() - 1], check_state[check_state.len() - 1]);

    }
    obj.unload().unwrap();
    {
        assert_eq!(*obj.get().unwrap(), check_state);
    }

    drop(obj);

    let end_dirty_size = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;
    assert_eq!(start_dirty_size, end_dirty_size);
}
