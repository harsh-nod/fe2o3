use fe2o3_device_real::kernel;

mod fe2o3_device {
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub struct ThreadIndex(usize);

    impl ThreadIndex {
        pub fn get(self) -> usize {
            self.0
        }
    }

    pub mod thread {
        use super::ThreadIndex;

        #[inline(always)]
        pub fn index_1d() -> ThreadIndex {
            ThreadIndex(0)
        }
    }

    #[repr(C)]
    pub struct DisjointSlice<T> {
        ptr: *mut T,
        len: usize,
    }

    impl<T> DisjointSlice<T> {
        pub fn get_mut(&mut self, index: ThreadIndex) -> Option<&mut T> {
            if index.get() >= self.len {
                return None;
            }
            Some(unsafe { &mut *self.ptr.add(index.get()) })
        }
    }
}

#[kernel]
pub fn lookalike_type(mut output: fe2o3_device::DisjointSlice<f32>) {
    let index = fe2o3_device::thread::index_1d();
    if let Some(value) = output.get_mut(index) {
        *value = 1.0;
    }
}

fn main() {}
