#![no_std]
#![feature(rustc_attrs)]
#![allow(internal_features)]

//! Device-side API for fe2o3 kernels.
//!
//! The internal diagnostic-item attributes are semantic identities consumed by
//! the backend pinned to this repository's nightly toolchain. They do not
//! authenticate this crate's package source or contents.

pub mod sync;
pub mod thread;

pub use fe2o3_macros::kernel;
pub use thread::ThreadIndex;

#[derive(Debug)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_slice"]
pub struct DisjointSlice<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> DisjointSlice<T> {
    /// Constructs a device slice from its raw representation.
    ///
    /// # Safety
    ///
    /// `ptr` must be aligned and valid for reads and writes of `len` consecutive
    /// `T` values for every use of the returned slice. Those values must not be
    /// accessed through another alias while this slice is used.
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        Self { ptr, len }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[rustc_diagnostic_item = "fe2o3_device_disjoint_slice_get_mut"]
    pub fn get_mut(&mut self, index: ThreadIndex) -> Option<&mut T> {
        self.get_mut_at(index.get())
    }

    #[rustc_diagnostic_item = "fe2o3_device_disjoint_slice_get_mut_at"]
    pub fn get_mut_at(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        Some(unsafe { &mut *self.ptr.add(index) })
    }
}
