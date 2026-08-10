//! Bounded device memory operations with explicit Rust unsafe obligations.
//!
//! These functions are semantic identities consumed by the fe2o3 backend. The
//! ordinary Rust implementations remain useful for host tests and define the
//! source behavior that target lowering must preserve.

use crate::DisjointSlice;

/// Computes the signed element distance between two positions in one slice.
///
/// # Safety
///
/// Both indices must be in bounds or one past the end of `allocation`. The
/// resulting byte distance must fit in `isize`. Zero-sized `T` is unsupported.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_offset_from_v1"]
pub unsafe fn offset_from<T>(allocation: &[T], pointer_index: usize, origin_index: usize) -> isize {
    let base = allocation.as_ptr();
    // SAFETY: The caller establishes the complete `offset_from` contract.
    unsafe { base.add(pointer_index).offset_from(base.add(origin_index)) }
}

/// Performs one volatile load from a Rust allocation.
///
/// # Safety
///
/// `index` must select an initialized, aligned, readable element of
/// `allocation`, and the volatile access must not trap.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_volatile_load_v1"]
pub unsafe fn volatile_load<T: Copy>(allocation: &[T], index: usize) -> T {
    // SAFETY: The caller establishes pointer validity, alignment, and access.
    unsafe { core::ptr::read_volatile(allocation.as_ptr().add(index)) }
}

/// Performs one volatile store into an exclusive device slice.
///
/// # Safety
///
/// `index` must select an aligned, writable element of `allocation`, the
/// volatile access must not trap, and the `DisjointSlice` construction
/// contract must remain valid across all GPU invocations and aliases.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_volatile_store_v1"]
pub unsafe fn volatile_store<T: Copy>(allocation: &mut DisjointSlice<T>, index: usize, value: T) {
    // SAFETY: The caller establishes pointer validity, alignment, and access.
    unsafe { core::ptr::write_volatile(allocation.ptr.add(index), value) }
}

/// Copies `count` elements between non-overlapping device regions.
///
/// # Safety
///
/// `source_index..source_index + count` must be readable and contained in
/// `source`; `destination_index..destination_index + count` must be writable
/// and contained in `destination`; both starting pointers must be aligned even
/// when `count == 0`; the byte count must fit in `usize`; and the positive-byte
/// source and destination regions must not overlap.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_copy_nonoverlapping_v1"]
pub unsafe fn copy_nonoverlapping<T: Copy>(
    source: &[T],
    source_index: usize,
    destination: &mut DisjointSlice<T>,
    destination_index: usize,
    count: usize,
) {
    // SAFETY: The caller establishes both ranges, alignment, and non-overlap.
    unsafe {
        core::ptr::copy_nonoverlapping(
            source.as_ptr().add(source_index),
            destination.ptr.add(destination_index),
            count,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_nonoverlapping, offset_from, volatile_load, volatile_store};
    use crate::DisjointSlice;

    #[test]
    fn host_semantics_preserve_signed_distance_and_volatile_access() {
        let source = [10_u32, 20, 30, 40];
        // SAFETY: Both indices select positions in the same allocation.
        assert_eq!(unsafe { offset_from(&source, 1, 3) }, -2);
        // SAFETY: Index two is initialized, aligned, readable, and nontrapping.
        assert_eq!(unsafe { volatile_load(&source, 2) }, 30);

        let mut destination = [0_u32; 4];
        // SAFETY: This host test has exclusive ownership of `destination`.
        let mut device =
            unsafe { DisjointSlice::from_raw_parts(destination.as_mut_ptr(), destination.len()) };
        // SAFETY: Index one is aligned, writable, and exclusively owned.
        unsafe { volatile_store(&mut device, 1, 77) };
        assert_eq!(destination, [0, 77, 0, 0]);
    }

    #[test]
    fn host_copy_uses_element_counts_and_offsets() {
        let source = [1_u32, 2, 3, 4, 5];
        let mut destination = [9_u32; 6];
        // SAFETY: This host test has exclusive ownership of `destination`.
        let mut device =
            unsafe { DisjointSlice::from_raw_parts(destination.as_mut_ptr(), destination.len()) };
        // SAFETY: The selected ranges are valid, aligned, and non-overlapping.
        unsafe { copy_nonoverlapping(&source, 1, &mut device, 2, 3) };
        assert_eq!(destination, [9, 9, 2, 3, 4, 9]);
    }
}
