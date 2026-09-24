use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

fn note_allocation() {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get().saturating_add(1)));
        }
    });
}

// Test-only transparent system allocator; counters use non-dropping TLS.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        // SAFETY: forward the allocator caller's unchanged layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        // SAFETY: forward the allocator caller's unchanged layout.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        note_allocation();
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

pub(in crate::topology::tests) fn counted<R>(operation: impl FnOnce() -> R) -> (R, usize) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            ENABLED.with(|enabled| enabled.set(false));
        }
    }
    ENABLED.with(|enabled| assert!(!enabled.replace(true)));
    let stop = Stop;
    ALLOCATIONS.with(|count| count.set(0));
    let result = operation();
    drop(stop);
    (result, ALLOCATIONS.with(Cell::get))
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
