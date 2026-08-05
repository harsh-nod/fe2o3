#![no_std]
#![feature(rustc_attrs)]
#![allow(internal_features)]

//! Device-side API for fe2o3 kernels.
//!
//! The internal diagnostic-item attributes are semantic identities consumed by
//! the backend pinned to this repository's nightly toolchain. They do not
//! authenticate this crate's package source or contents.

use core::marker::PhantomData;

pub mod sync;
pub mod thread;

pub use fe2o3_macros::kernel;
pub use thread::{Index1D, Index2D, ThreadIndex};

#[derive(Debug)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_slice"]
pub struct DisjointSlice<T, IndexSpace = Index1D> {
    ptr: *mut T,
    len: usize,
    _index_space: PhantomData<fn() -> IndexSpace>,
}

impl<T, IndexSpace> DisjointSlice<T, IndexSpace> {
    /// Constructs a device slice from its raw representation.
    ///
    /// # Safety
    ///
    /// `ptr` must be aligned and valid for reads and writes of `len` consecutive
    /// `T` values for every use of the returned slice. Those values must not be
    /// accessed through another alias while this slice is used.
    /// `IndexSpace` must describe the invocation-to-element mapping used by
    /// every safe access to the returned view.
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        Self {
            ptr,
            len,
            _index_space: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[rustc_diagnostic_item = "fe2o3_device_disjoint_slice_get_mut"]
    pub fn get_mut(&mut self, index: ThreadIndex<IndexSpace>) -> Option<&mut T> {
        // SAFETY: `ThreadIndex` can only be produced for the current device
        // invocation and its index-space type matches this view, so its index
        // preserves the view's per-invocation write partition.
        // `from_raw_parts` establishes pointer validity and aliasing.
        unsafe { self.get_mut_at(index.get()) }
    }

    /// Returns mutable access to an arbitrary integer index in this view.
    ///
    /// Returns `None` when `index` is outside the view's element extent.
    ///
    /// # Safety
    ///
    /// In addition to satisfying the validity and aliasing requirements of
    /// [`Self::from_raw_parts`], the caller must prove that no concurrently
    /// executing invocation can access the selected element through this view
    /// or any alias unless those accesses are synchronized and compatible.
    /// The index calculation must not undermine the exclusive-write partition
    /// represented by this `DisjointSlice`.
    #[rustc_diagnostic_item = "fe2o3_device_disjoint_slice_get_mut_at"]
    pub unsafe fn get_mut_at(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        Some(unsafe { &mut *self.ptr.add(index) })
    }
}

#[cfg(test)]
mod tests {
    use super::{DisjointSlice, Index1D, Index2D};
    use core::mem::{align_of, size_of};

    #[test]
    fn index_space_markers_do_not_change_the_slice_abi() {
        let expected_size = size_of::<*mut u32>() + size_of::<usize>();
        let expected_align = align_of::<*mut u32>().max(align_of::<usize>());

        assert_eq!(size_of::<DisjointSlice<u32, Index1D>>(), expected_size);
        assert_eq!(align_of::<DisjointSlice<u32, Index1D>>(), expected_align);
        assert_eq!(size_of::<DisjointSlice<u32, Index2D<64>>>(), expected_size);
        assert_eq!(
            align_of::<DisjointSlice<u32, Index2D<64>>>(),
            expected_align
        );
    }
}
