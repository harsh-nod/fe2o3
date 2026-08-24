//! Bounded device memory operations with checked Rust-side obligations.
//!
//! These functions are semantic identities consumed by the fe2o3 backend. The
//! ordinary Rust implementations validate bounds for host tests and define the
//! source behavior that verified target lowering must preserve.

use crate::DisjointSlice;

/// Computes the signed element distance between two positions in one slice.
///
/// # Panics
///
/// Panics when either index is beyond one past the end of `allocation` or `T`
/// is zero-sized.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_offset_from_v1"]
pub fn offset_from<T>(allocation: &[T], pointer_index: usize, origin_index: usize) -> isize {
    assert!(
        core::mem::size_of::<T>() != 0,
        "zero-sized values have no device-memory distance"
    );
    assert!(
        pointer_index <= allocation.len(),
        "pointer index is outside the allocation"
    );
    assert!(
        origin_index <= allocation.len(),
        "origin index is outside the allocation"
    );
    let pointer_index =
        isize::try_from(pointer_index).expect("a valid nonzero-sized Rust slice extent fits isize");
    let origin_index =
        isize::try_from(origin_index).expect("a valid nonzero-sized Rust slice extent fits isize");
    pointer_index - origin_index
}

/// Performs one volatile load from a Rust allocation.
///
/// # Panics
///
/// Panics when `index` is outside `allocation`.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_volatile_load_v1"]
pub fn volatile_load<T: Copy>(allocation: &[T], index: usize) -> T {
    assert!(
        index < allocation.len(),
        "volatile load index is outside the allocation"
    );
    // SAFETY: a checked element of a shared slice is initialized, aligned, and readable.
    unsafe { core::ptr::read_volatile(allocation.as_ptr().add(index)) }
}

/// Performs one volatile store into an exclusive device slice.
///
/// # Panics
///
/// Panics when `index` is outside `allocation`.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_volatile_store_v1"]
pub fn volatile_store<T: Copy>(allocation: &mut DisjointSlice<T>, index: usize, value: T) {
    assert!(
        index < allocation.len,
        "volatile store index is outside the allocation"
    );
    // SAFETY: the checked element is valid and writable by the DisjointSlice contract.
    unsafe { core::ptr::write_volatile(allocation.ptr.add(index), value) }
}

/// Copies `count` elements between non-overlapping device regions.
///
/// # Panics
///
/// Panics when either selected range is outside its allocation.
///
/// A valid `DisjointSlice` excludes a live source alias to its elements, so
/// the selected source and destination regions cannot overlap.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_copy_nonoverlapping_v1"]
pub fn copy_nonoverlapping<T: Copy>(
    source: &[T],
    source_index: usize,
    destination: &mut DisjointSlice<T>,
    destination_index: usize,
    count: usize,
) {
    let source_end = source_index
        .checked_add(count)
        .expect("source range extent overflows usize");
    assert!(
        source_end <= source.len(),
        "source range is outside the allocation"
    );
    let destination_end = destination_index
        .checked_add(count)
        .expect("destination range extent overflows usize");
    assert!(
        destination_end <= destination.len,
        "destination range is outside the allocation"
    );
    // SAFETY: checked slice ranges are valid and aligned. A valid DisjointSlice
    // excludes a simultaneously live source alias, so positive-byte overlap is
    // impossible.
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
        assert_eq!(offset_from(&source, 1, 3), -2);
        assert_eq!(volatile_load(&source, 2), 30);

        let mut destination = [0_u32; 4];
        // SAFETY: This host test has exclusive ownership of `destination`.
        let mut device =
            unsafe { DisjointSlice::from_raw_parts(destination.as_mut_ptr(), destination.len()) };
        volatile_store(&mut device, 1, 77);
        assert_eq!(destination, [0, 77, 0, 0]);
    }

    #[test]
    fn host_copy_uses_element_counts_and_offsets() {
        let source = [1_u32, 2, 3, 4, 5];
        let mut destination = [9_u32; 6];
        // SAFETY: This host test has exclusive ownership of `destination`.
        let mut device =
            unsafe { DisjointSlice::from_raw_parts(destination.as_mut_ptr(), destination.len()) };
        copy_nonoverlapping(&source, 1, &mut device, 2, 3);
        assert_eq!(destination, [9, 9, 2, 3, 4, 9]);
    }

    #[test]
    fn host_memory_operations_reject_out_of_bounds_ranges() {
        let source = [1_u32, 2];
        let mut destination = [0_u32; 2];
        // SAFETY: This host test has exclusive ownership of `destination`.
        let mut device =
            unsafe { DisjointSlice::from_raw_parts(destination.as_mut_ptr(), destination.len()) };

        assert!(std::panic::catch_unwind(|| volatile_load(&source, 2)).is_err());
        assert!(std::panic::catch_unwind(|| offset_from(&source, 3, 0)).is_err());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                volatile_store(&mut device, 2, 3)
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                copy_nonoverlapping(&source, 1, &mut device, 0, 2)
            }))
            .is_err()
        );
    }
}
