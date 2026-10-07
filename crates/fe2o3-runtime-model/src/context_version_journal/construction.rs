use super::ContextVersionJournalErrorV1;
use alloc::vec::Vec;

#[allow(unused_macros)]
#[macro_use]
mod templates {
    include!("constructor_bodies.rs");
}

macro_rules! constructor_rust_expr {
    ($body:expr) => {
        $body
    };
}

pub(crate) trait ConstructorAllocatorV1 {
    fn reserve<T>(&mut self, site: u8, capacity: usize, storage: &mut Vec<T>) -> bool;
}

pub(crate) struct NativeConstructorAllocatorV1;

impl ConstructorAllocatorV1 for NativeConstructorAllocatorV1 {
    fn reserve<T>(&mut self, _site: u8, capacity: usize, storage: &mut Vec<T>) -> bool {
        storage.try_reserve_exact(capacity).is_ok()
    }
}

pub(crate) fn vacant<T>(
    capacity: usize,
    site: u8,
    allocator: &mut impl ConstructorAllocatorV1,
) -> Result<Vec<Option<T>>, ContextVersionJournalErrorV1> {
    constructor_vacant_body!(
        constructor_rust_expr,
        T,
        capacity,
        site,
        allocator,
        slots,
        [],
        [],
        []
    )
}

pub(crate) fn free(
    capacity: usize,
    site: u8,
    allocator: &mut impl ConstructorAllocatorV1,
) -> Result<Vec<usize>, ContextVersionJournalErrorV1> {
    constructor_free_body!(
        constructor_rust_expr,
        capacity,
        site,
        allocator,
        free,
        next,
        [],
        [],
        []
    )
}

pub(crate) fn zero(
    capacity: usize,
    site: u8,
    allocator: &mut impl ConstructorAllocatorV1,
) -> Result<Vec<usize>, ContextVersionJournalErrorV1> {
    constructor_zero_body!(
        constructor_rust_expr,
        capacity,
        site,
        allocator,
        counts,
        [],
        [],
        []
    )
}
