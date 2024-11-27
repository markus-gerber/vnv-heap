use std::{
    array, mem,
    ptr::{null_mut, slice_from_raw_parts_mut}, time::Instant,
};

use env_logger::{Builder, Env};
use rand::{rngs::SmallRng, RngCore, SeedableRng};
use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule,
        persistent_storage::FilePersistentStorageModule,
    },
    vnv_persist_all, VNVConfig, VNVHeap,
};

static mut PERSIST_TIMER: Option<DesktopTimer> = None;


extern "C" fn signal_handler(_sig: libc::c_int) {
    unsafe { PERSIST_TIMER = Some(DesktopTimer::start()) };
    unsafe { vnv_persist_all() };
}

fn setup_handler() {
    let mut new: libc::sigaction = unsafe { mem::zeroed() };
    new.sa_sigaction = signal_handler as usize;

    if unsafe { libc::sigaction(libc::SIGUSR1, &new, null_mut()) } != 0 {
        panic!("failed to execute sigaction");
    }
}

fn main() {
    Builder::from_env(Env::default())
        .filter_level(log::LevelFilter::Warn)
        .format_module_path(false)
        .init();

    setup_handler();

    let storage = FilePersistentStorageModule::new("/tmp/vnv_desktop_persist.data".to_string(), 4096 * 4).unwrap();
    let config = VNVConfig {
        max_dirty_bytes: 500,
    };
    let mut buffer = [0u8; 2000];
    let heap = LinkedListAllocatorModule::new();

    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<16>,
        DefaultObjectManagementModule,
        FilePersistentStorageModule
    > = VNVHeap::new(&mut buffer, storage, heap, config, |base_ptr, size| {
        let latency = unsafe { PERSIST_TIMER.take().unwrap().stop() };

        {
            let text = "persist finished in ";
            unsafe { libc::write(libc::STDOUT_FILENO, text.as_ptr() as *const libc::c_void, text.len()) };

            // convert latency to string
            let mut curr = latency;
            let mut buf = ['0'; 32];
            let mut pos: usize = 32;
            while curr != 0 {
                pos -= 1;

                buf[pos] =  char::from_digit(curr % 10, 10).unwrap();
                curr /= 10;
            }
            if pos == 32 {
                // special case: would not print any character otherwise.
                pos -= 1;
                buf[pos] = '0';
            }

            unsafe { libc::write(libc::STDOUT_FILENO, ((&buf[pos]) as *const char) as *const libc::c_void, 32 - pos) };

            let text = "ns\n";
            unsafe { libc::write(libc::STDOUT_FILENO, text.as_ptr() as *const libc::c_void, text.len()) };
        }
        
        let buffer = unsafe { slice_from_raw_parts_mut(base_ptr, size).as_mut() }.unwrap();
        buffer.fill(0);

        // this will be called from our signal handler, so do not use print
        {
            let text = "finished clearing buffer\n";
            unsafe { libc::write(libc::STDOUT_FILENO, text.as_ptr() as *const libc::c_void, text.len()) };
        }
    })
    .unwrap();

    type TestType = [u8; 10];

    fn rand_data(rand: &mut SmallRng) -> TestType {
        array::from_fn(|_| rand.next_u32() as u8)
    }

    const SEED: u64 = 5446535461589659585;
    const OBJECT_COUNT: usize = 100;

    let mut rand = SmallRng::seed_from_u64(SEED);

    loop {
        println!("new iteration");
        let mut objects = vec![];
        let mut check_states = vec![];

        macro_rules! allocate {
            () => {
                let data = rand_data(&mut rand);

                objects.push(heap.allocate(data.clone()).unwrap());
                check_states.push(data);
            };
        }

        macro_rules! single_test {
            () => {
                let i = rand.next_u32() as usize % objects.len();
                let test_type = rand.next_u32() % 10;
                if test_type == 0 {
                    // get mut and change data
                    let mut mut_ref = objects[i].get_mut().unwrap();
                    assert_eq!(*mut_ref, check_states[i]);

                    let data = rand_data(&mut rand);
                    *mut_ref = data;
                    check_states[i] = data;
                } else if test_type < 2 {
                    // get mut and dont change data
                    let mut_ref = objects[i].get_mut().unwrap();
                    assert_eq!(*mut_ref, check_states[i]);
                } else {
                    // get ref
                    let immut_ref = objects[i].get().unwrap();
                    assert_eq!(*immut_ref, check_states[i]);
                }
            };
        }

        // start allocating some first objects
        for _ in 0..OBJECT_COUNT / 3 {
            allocate!();
        }

        // start testing
        for _ in 0..10_000 {
            single_test!();
        }

        let mut open_ref_obj = vec![];
        let mut open_refs = vec![];
        let mut open_muts = vec![];
        for _ in 0..10 {
            open_ref_obj.push(heap.allocate(rand_data(&mut rand)).unwrap());
        }
        for (i, obj) in open_ref_obj.iter_mut().enumerate() {
            if i % 2 == 0 {
                open_refs.push(obj.get().unwrap());
            } else {
                open_muts.push(obj.get_mut().unwrap());
            }
        }

        // test again
        for _ in 0..10_000 {
            single_test!();
        }

        // drop open refs
        drop(open_refs);

        // test again
        for _ in 0..10_000 {
            single_test!();
        }

        drop(open_muts);
        drop(open_ref_obj);

        // test again
        for _ in 0..100_000 {
            single_test!();
        }

        // start allocating last objects
        for _ in 0..(OBJECT_COUNT - objects.len()) {
            allocate!();
        }

        // test again
        for _ in 0..1_000_000 {
            single_test!();
        }
    }
}


struct DesktopTimer {
    start_time: Instant,
}

impl DesktopTimer {

    #[inline]
    fn start() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    #[inline]
    fn stop(self) -> u32 {
        (Instant::now() - self.start_time).subsec_nanos()
    }
}
