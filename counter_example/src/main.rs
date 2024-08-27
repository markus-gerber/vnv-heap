use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule, object_management::DefaultObjectManagementModule, persistent_storage::FilePersistentStorageModule
    }, vnv_persist_all, VNVConfig, VNVHeap
};

struct Counter {
    val: u32
}

impl Counter {
    fn new(initial_value: u32) -> Self { Self { val: initial_value } }
    fn increase(&mut self) { self.val += 1; }
    fn increase_by(&mut self, inc: u32) { self.val += inc; }
    fn get_val(&self) -> u32 { self.val }
}


fn main() {
    let storage = FilePersistentStorageModule::new("test.data".to_string(), 4096).unwrap();
    let config = VNVConfig {
        max_dirty_bytes: 1024,
    };
    let mut buffer = [0u8; 2048];
    let heap = LinkedListAllocatorModule::new();

    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<16>,
        DefaultObjectManagementModule,
        FilePersistentStorageModule
    > = VNVHeap::new(&mut buffer, storage, heap, config, |_, _| {}).unwrap();

    {
        // allocate new counter
        let mut obj = heap.allocate::<Counter>(Counter::new(0)).unwrap();

        {
            // print current value
            let obj_ref = obj.get().unwrap();
            println!("counter: {}", obj_ref.get_val());
        } // implicit drop of immutable reference: object could be unloaded

        // do something in between...

        {
            // increase the value by 100
            let mut mut_ref = obj.get_mut().unwrap();
            mut_ref.increase();
            println!("counter: {}", mut_ref.get_val());
            mut_ref.increase_by(100);
        } // implicit drop of mutable reference: object could be synchronized/unloaded

        // do something in between...

        {
            // print current value
            let obj_ref = obj.get().unwrap();
            println!("counter: {}", obj_ref.get_val());
        } // implicit drop of immutable reference: object could be unloaded

    } // implicit drop of obj1: free memory
}

// called once power failure is imminent
// persist the state of all vNVHeaps in this application
#[allow(unused)]
fn persist() {
    // unsafe: this should not be called if another
    // call of this function did not finish yet
    unsafe { vnv_persist_all() };
}