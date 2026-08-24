//! Bounded device memory operations implemented by a reviewed provider.
//!
//! These functions are semantic identities consumed by the fe2o3 backend. The
//! safe API exposes only operation-specific access through Rust slices and
//! ownership witnesses; it does not expose raw pointers. The explicitly unsafe
//! expert operation retains the general range-copy contract. These ordinary
//! Rust implementations define the source behavior that target lowering must
//! preserve.

use crate::{DisjointIndex, DisjointSlice};

/// Computes the signed element distance between two positions in one slice.
///
/// Panics if either index is outside `allocation` (one-past-the-end is
/// accepted) or if `T` is zero-sized.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_offset_from_v1"]
pub fn offset_from<T>(allocation: &[T], pointer_index: usize, origin_index: usize) -> isize {
    assert_supported_element::<T>();
    assert_position(allocation.len(), pointer_index);
    assert_position(allocation.len(), origin_index);

    let base = allocation.as_ptr();
    // SAFETY: Both positions are within this one valid slice allocation, and
    // valid non-ZST Rust slices cannot exceed `isize::MAX` bytes.
    unsafe { base.add(pointer_index).offset_from(base.add(origin_index)) }
}

/// Performs one volatile load from a Rust allocation.
///
/// Panics if `index` is outside `allocation` or if `T` is zero-sized.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_volatile_load_v1"]
pub fn volatile_load<T: Copy>(allocation: &[T], index: usize) -> T {
    assert_supported_element::<T>();
    assert_element(allocation.len(), index);

    // SAFETY: The checked index selects an initialized, aligned element of the
    // valid shared slice. The reviewed provider performs exactly one load.
    unsafe { core::ptr::read_volatile(allocation.as_ptr().add(index)) }
}

/// Performs one volatile store selected by disjoint write authority.
///
/// Panics if the witnessed index is outside `allocation` or if `T` is
/// zero-sized. `IndexSpace` must match so safe code cannot use authority for
/// one invocation-to-element mapping with a differently partitioned slice.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_volatile_store_v1"]
pub fn volatile_store<T: Copy, IndexSpace>(
    allocation: &mut DisjointSlice<T, IndexSpace>,
    index: &DisjointIndex<IndexSpace>,
    value: T,
) {
    assert_supported_element::<T>();
    let index = index.get();
    assert_element(allocation.len, index);

    // SAFETY: The mapping-matched witness proves that this invocation owns the
    // selected element. `DisjointSlice` construction establishes pointer
    // validity, and the checked index remains inside the allocation.
    unsafe { core::ptr::write_volatile(allocation.ptr.add(index), value) }
}

/// Copies one element into a position selected by disjoint write authority.
///
/// Panics if either element is outside its allocation, if the source and
/// destination elements overlap, or if `T` is zero-sized. A `DisjointIndex`
/// owns one mapped element, so this safe operation deliberately has no free
/// element count.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_copy_one_nonoverlapping_v1"]
pub fn copy_one_nonoverlapping<T: Copy, IndexSpace>(
    source: &[T],
    source_index: usize,
    destination: &mut DisjointSlice<T, IndexSpace>,
    destination_index: &DisjointIndex<IndexSpace>,
) {
    assert_supported_element::<T>();
    assert_element(source.len(), source_index);
    let destination_index = destination_index.get();
    assert_element(destination.len, destination_index);

    // SAFETY: Both checked indices select initialized, aligned elements in
    // their respective allocations.
    let source_pointer = unsafe { source.as_ptr().add(source_index) };
    let destination_pointer = unsafe { destination.ptr.add(destination_index) };
    assert_nonoverlapping(
        source_pointer,
        destination_pointer,
        core::mem::size_of::<T>(),
    );

    // SAFETY: Both elements were checked and their positive-byte address
    // ranges were proven disjoint. The witness proves destination ownership.
    unsafe { core::ptr::copy_nonoverlapping(source_pointer, destination_pointer, 1) }
}

/// Copies an arbitrary element range under an expert-provided safety proof.
///
/// Bounds, byte-count overflow, and positive-byte overlap are checked before
/// the provider performs the copy. These checks cannot prove GPU ownership or
/// synchronization across concurrently executing invocations.
///
/// # Panics
///
/// The caller must prove that every destination element in
/// `destination_index..destination_index + count` belongs to this invocation,
/// or that no other invocation can concurrently access those elements without
/// compatible synchronization. This proof must preserve the `DisjointSlice`
/// mapping contract and all aliases for the duration of the operation.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_memory_copy_nonoverlapping_v1"]
pub unsafe fn copy_nonoverlapping_unchecked<T: Copy, IndexSpace>(
    source: &[T],
    source_index: usize,
    destination: &mut DisjointSlice<T, IndexSpace>,
    destination_index: usize,
    count: usize,
) {
    assert_supported_element::<T>();
    assert_range(source.len(), source_index, count);
    assert_range(destination.len, destination_index, count);

    let byte_count = count
        .checked_mul(core::mem::size_of::<T>())
        .expect("memory operation byte count overflowed usize");

    // SAFETY: Both starts are in bounds or one-past-the-end, and the slice
    // constructors establish alignment and pointer validity.
    let source_pointer = unsafe { source.as_ptr().add(source_index) };
    let destination_pointer = unsafe { destination.ptr.add(destination_index) };
    assert_nonoverlapping(source_pointer, destination_pointer, byte_count);

    // SAFETY: The checked ranges are valid and non-overlapping. The caller
    // establishes the cross-invocation destination ownership obligation.
    unsafe { core::ptr::copy_nonoverlapping(source_pointer, destination_pointer, count) }
}

fn assert_supported_element<T>() {
    assert!(
        core::mem::size_of::<T>() != 0,
        "memory operations do not support zero-sized elements"
    );
}

fn assert_position(len: usize, index: usize) {
    assert!(
        index <= len,
        "memory operation position is outside the allocation"
    );
}

fn assert_element(len: usize, index: usize) {
    assert!(
        index < len,
        "memory operation index is outside the allocation"
    );
}

fn assert_range(len: usize, start: usize, count: usize) {
    let end = start
        .checked_add(count)
        .expect("memory operation element range overflowed usize");
    assert!(
        start <= len && end <= len,
        "memory operation range is outside the allocation"
    );
}

fn assert_nonoverlapping<T>(source: *const T, destination: *mut T, byte_count: usize) {
    if byte_count == 0 {
        return;
    }

    let source_start = source.addr();
    let destination_start = destination.addr();
    let source_end = source_start
        .checked_add(byte_count)
        .expect("source address range overflowed usize");
    let destination_end = destination_start
        .checked_add(byte_count)
        .expect("destination address range overflowed usize");
    assert!(
        source_end <= destination_start || destination_end <= source_start,
        "copy_nonoverlapping source and destination ranges overlap"
    );
}

#[cfg(test)]
mod tests {
    use super::{
        copy_nonoverlapping_unchecked, copy_one_nonoverlapping, offset_from, volatile_load,
        volatile_store,
    };
    use crate::{DisjointIndex, DisjointSlice, Index1D};

    #[test]
    fn host_semantics_preserve_signed_distance_and_volatile_access() {
        let source = [10_u32, 20, 30, 40];
        assert_eq!(offset_from(&source, 1, 3), -2);
        assert_eq!(volatile_load(&source, 2), 30);

        let mut destination = [0_u32; 4];
        // SAFETY: This host test has exclusive ownership of `destination`.
        let mut device = unsafe {
            DisjointSlice::<u32>::from_raw_parts(destination.as_mut_ptr(), destination.len())
        };
        let index = DisjointIndex::<Index1D>::from_model_index(1);
        volatile_store(&mut device, &index, 77);
        assert_eq!(destination, [0, 77, 0, 0]);
    }

    #[test]
    fn host_copy_uses_source_offset_and_witnessed_destination() {
        let source = [1_u32, 2, 3, 4, 5];
        let mut destination = [9_u32; 6];
        // SAFETY: This host test has exclusive ownership of `destination`.
        let mut device =
            unsafe { DisjointSlice::from_raw_parts(destination.as_mut_ptr(), destination.len()) };
        let index = DisjointIndex::<Index1D>::from_model_index(2);
        copy_one_nonoverlapping(&source, 1, &mut device, &index);
        assert_eq!(destination, [9, 9, 2, 9, 9, 9]);
    }

    #[test]
    fn host_expert_copy_retains_arbitrary_element_count() {
        let source = [1_u32, 2, 3, 4, 5];
        let mut destination = [9_u32; 6];
        // SAFETY: This host test exclusively owns the destination allocation,
        // and the source and destination arrays do not overlap.
        let mut device = unsafe {
            DisjointSlice::<u32>::from_raw_parts(destination.as_mut_ptr(), destination.len())
        };
        // SAFETY: No concurrent invocation or alias can access `destination`.
        unsafe { copy_nonoverlapping_unchecked(&source, 1, &mut device, 2, 3) };
        assert_eq!(destination, [9, 9, 2, 3, 4, 9]);
    }

    #[test]
    #[should_panic(expected = "memory operation index is outside the allocation")]
    fn safe_volatile_load_rejects_out_of_bounds_access() {
        let source = [1_u32];
        let _ = volatile_load(&source, 1);
    }

    #[test]
    #[should_panic(expected = "memory operation index is outside the allocation")]
    fn safe_copy_rejects_an_out_of_bounds_witness() {
        let source = [1_u32, 2];
        let mut destination = [0_u32; 2];
        // SAFETY: This host test has exclusive ownership of `destination`.
        let mut device =
            unsafe { DisjointSlice::from_raw_parts(destination.as_mut_ptr(), destination.len()) };
        let index = DisjointIndex::<Index1D>::from_model_index(2);

        copy_one_nonoverlapping(&source, 1, &mut device, &index);
    }
}
