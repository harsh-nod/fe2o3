#![no_std]

pub mod sync;
pub mod thread;

pub use fe2o3_macros::kernel;
pub use thread::ThreadIndex;

#[derive(Debug)]
#[repr(C)]
pub struct DisjointSlice<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> DisjointSlice<T> {
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        Self { ptr, len }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get_mut(&mut self, index: ThreadIndex) -> Option<&mut T> {
        self.get_mut_at(index.get())
    }

    pub fn get_mut_at(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        Some(unsafe { &mut *self.ptr.add(index) })
    }
}
