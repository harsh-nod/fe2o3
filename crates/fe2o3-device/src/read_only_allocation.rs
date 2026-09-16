//! Consuming read access to an exclusively leased kernel allocation.
//!
//! A move in one invocation does not exclude writes by other invocations. The
//! compiler therefore admits this capability only after checking that the
//! entire kernel has no writes or escapes of the exact source allocation.
//! The original `DisjointSlice` argument and its host exclusive lease remain
//! unchanged; this local view grants no allocation or launch authority.

use crate::{DisjointSlice, Index1D};

mod sealed {
    pub trait Element {}
    impl Element for u16 {}
    impl Element for f32 {}
}

/// Scalar elements supported by the initial consuming read capability.
pub trait ReadOnlyAllocationElement: sealed::Element + Copy {}

impl ReadOnlyAllocationElement for u16 {}
impl ReadOnlyAllocationElement for f32 {}

/// Bounds-checked reads from a consumed, exclusively leased allocation.
///
/// This type has no public constructor, mutable access, `Copy`, or `Clone`.
/// Its compiler provenance is the exact consumed argument, not its layout.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_read_only_allocation_v1"]
pub struct ReadOnlyAllocation<T: ReadOnlyAllocationElement> {
    ptr: *const T,
    len: usize,
}

impl<T: ReadOnlyAllocationElement> DisjointSlice<T, Index1D> {
    /// Consumes this allocation for arbitrary-index reads by kernel invocations.
    ///
    /// Production compilation requires a whole-kernel proof that this exact
    /// allocation is never written or escaped, including before this call.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_disjoint_slice_into_read_only_v1"]
    pub fn into_read_only(self) -> ReadOnlyAllocation<T> {
        ReadOnlyAllocation {
            ptr: self.ptr.cast_const(),
            len: self.len,
        }
    }
}

impl<T: ReadOnlyAllocationElement> ReadOnlyAllocation<T> {
    /// Returns the actual extent retained from the consumed allocation.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_read_only_allocation_len_v1"]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the allocation has no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an element, or `fallback` without reading memory out of bounds.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_read_only_allocation_load_or_v1"]
    pub fn load_or(&self, index: usize, fallback: T) -> T {
        if index < self.len {
            // SAFETY: the consumed slice supplies validity/alignment/extent.
            // Compiler admission additionally excludes concurrent kernel writes.
            unsafe { self.ptr.add(index).read() }
        } else {
            fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consuming_read_retains_extent_and_total_bounds() {
        let mut values = [2_u16, 7, 11];
        // SAFETY: the array stays live and is accessed only through the view.
        let source = unsafe { DisjointSlice::<u16>::from_raw_parts(values.as_mut_ptr(), 3) };
        let view = source.into_read_only();
        assert_eq!(view.len(), 3);
        assert!(!view.is_empty());
        assert_eq!(view.load_or(0, 99), 2);
        assert_eq!(view.load_or(2, 99), 11);
        assert_eq!(view.load_or(3, 99), 99);
        assert_eq!(view.load_or(usize::MAX, 99), 99);
    }

    #[test]
    fn empty_consumed_allocation_never_reads() {
        let pointer = core::ptr::NonNull::<f32>::dangling().as_ptr();
        // SAFETY: a non-null aligned pointer is valid for an empty allocation.
        let source = unsafe { DisjointSlice::<f32>::from_raw_parts(pointer, 0) };
        let view = source.into_read_only();
        assert!(view.is_empty());
        assert_eq!(view.load_or(0, 4.5), 4.5);
        assert_eq!(view.load_or(usize::MAX, -1.0), -1.0);
    }
}
