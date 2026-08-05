#![no_std]

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ThreadIndex(usize);

impl ThreadIndex {
    pub fn get(self) -> usize {
        self.0
    }

    pub fn offset(self, offset: usize) -> usize {
        self.0 + offset
    }

    pub fn offset_signed(self, offset: isize) -> usize {
        self.0.wrapping_add_signed(offset)
    }

    pub fn stride(self, stride: usize) -> usize {
        self.0.wrapping_mul(stride)
    }

    pub fn stride_offset(self, stride: usize, offset: isize) -> usize {
        self.0.wrapping_mul(stride).wrapping_add_signed(offset)
    }
}

#[repr(C)]
pub struct DisjointSlice<T> {
    ptr: *mut T,
    len: usize,
}

impl<T> DisjointSlice<T> {
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

pub mod thread {
    #[inline(never)]
    pub fn index_1d() -> trusted_device::ThreadIndex {
        trusted_device::thread::index_1d()
    }
}
