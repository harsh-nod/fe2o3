use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static FAIL_AFTER: Cell<Option<usize>> = const { Cell::new(None) };
    static FAILED: Cell<bool> = const { Cell::new(false) };
}

struct CountingAllocator;

fn note_allocation() -> bool {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get().saturating_add(1)));
        }
    });
    FAIL_AFTER
        .try_with(|remaining| match remaining.get() {
            Some(1) => {
                remaining.set(None);
                let _ = FAILED.try_with(|failed| failed.set(true));
                true
            }
            Some(count) => {
                remaining.set(Some(count - 1));
                false
            }
            None => false,
        })
        .unwrap_or(false)
}

// Tests may refuse one allocation on this thread; all other calls reach System.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if note_allocation() {
            return std::ptr::null_mut();
        }
        // SAFETY: forward the allocator caller's unchanged layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if note_allocation() {
            return std::ptr::null_mut();
        }
        // SAFETY: forward the allocator caller's unchanged layout.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if note_allocation() {
            return std::ptr::null_mut();
        }
        // SAFETY: the original pointer/layout and requested size are unchanged.
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: this pointer was allocated by the same System allocator.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

pub(crate) struct AllocationCounter(std::marker::PhantomData<*mut ()>);

impl AllocationCounter {
    pub(crate) fn begin() -> Self {
        ENABLED.with(|enabled| assert!(!enabled.replace(true)));
        ALLOCATIONS.with(|count| count.set(0));
        Self(std::marker::PhantomData)
    }

    pub(crate) fn finish(self) -> usize {
        drop(self);
        ALLOCATIONS.with(Cell::get)
    }
}

impl Drop for AllocationCounter {
    fn drop(&mut self) {
        ENABLED.with(|enabled| enabled.set(false));
    }
}

pub(in crate::topology::tests) fn counted<R>(operation: impl FnOnce() -> R) -> (R, usize) {
    let counter = AllocationCounter::begin();
    let result = operation();
    (result, counter.finish())
}

pub(in crate::topology::tests) fn fail_one<R>(
    ordinal: usize,
    operation: impl FnOnce() -> R,
) -> (R, bool) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            FAIL_AFTER.with(|remaining| remaining.set(None));
        }
    }
    assert!(ordinal > 0);
    FAIL_AFTER.with(|remaining| {
        assert!(remaining.replace(None).is_none());
        remaining.set(Some(ordinal));
    });
    FAILED.with(|failed| failed.set(false));
    let stop = Stop;
    let result = operation();
    drop(stop);
    (result, FAILED.with(Cell::get))
}

#[test]
fn counter_observes_allocations_and_resets_after_unwind() {
    let (bytes, calls) = counted(|| std::hint::black_box(vec![7_u8; 4096]));
    assert_eq!(bytes.len(), 4096);
    assert!(calls >= 1);
    assert_eq!(counted(|| std::hint::black_box(7)).1, 0);
    assert!(std::panic::catch_unwind(|| counted(|| panic!("counter unwind"))).is_err());
    assert_eq!(counted(|| std::hint::black_box(7)).1, 0);
}

#[test]
fn allocation_refusal_is_one_shot_thread_local_and_preserves_realloc_input() {
    for ordinal in [1, 2] {
        let ((first, second), failed) = fail_one(ordinal, || {
            let first = Vec::<u8>::new().try_reserve_exact(4096);
            let second = Vec::<u8>::new().try_reserve_exact(4096);
            (first, second)
        });
        assert!(failed);
        assert_eq!(first.is_err(), ordinal == 1);
        assert_eq!(second.is_err(), ordinal == 2);
    }
    let mut original = vec![7_u8; 4096];
    let pointer = original.as_ptr();
    let capacity = original.capacity();
    let (result, failed) = fail_one(1, || original.try_reserve_exact(8192));
    assert!(failed && result.is_err());
    assert_eq!(
        (original.as_ptr(), original.capacity()),
        (pointer, capacity)
    );
    assert!(original.iter().all(|byte| *byte == 7));
    assert!(
        std::panic::catch_unwind(|| fail_one(usize::MAX, || panic!("fault scope unwind"))).is_err()
    );
    assert!(Vec::<u8>::new().try_reserve_exact(4096).is_ok());
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let state = Arc::new((AtomicBool::new(false), AtomicBool::new(false)));
    let child_state = Arc::clone(&state);
    let child = std::thread::spawn(move || {
        while !child_state.0.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        let result = Vec::<u8>::new().try_reserve_exact(4096).is_ok();
        child_state.1.store(true, Ordering::Release);
        result
    });
    let (result, failed) = fail_one(1, || {
        state.0.store(true, Ordering::Release);
        while !state.1.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        Vec::<u8>::new().try_reserve_exact(4096)
    });
    assert!(failed && result.is_err());
    assert!(child.join().unwrap());
}
