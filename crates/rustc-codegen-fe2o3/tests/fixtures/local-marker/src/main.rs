#![feature(rustc_attrs)]
#![allow(internal_features)]

use fe2o3_macros::kernel;

#[cfg(feature = "duplicate-genuine")]
const _: usize = core::mem::size_of::<fe2o3_device_real::ThreadIndex>();

#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_slice"]
pub struct DisjointSlice<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> DisjointSlice<T> {
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        Some(unsafe { &mut *self.ptr.add(index) })
    }
}

#[kernel]
pub fn local_marker(mut output: DisjointSlice<f32>) {
    if let Some(value) = output.get_mut(0) {
        *value = 1.0;
    }
}

fn main() {}
