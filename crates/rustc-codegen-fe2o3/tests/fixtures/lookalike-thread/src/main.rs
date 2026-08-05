use fe2o3_device_real::{DisjointSlice, kernel};

mod fe2o3_device {
    pub mod thread {
        use fe2o3_device_real::ThreadIndex;

        #[inline(always)]
        pub fn index_1d() -> ThreadIndex {
            fe2o3_device_real::thread::index_1d()
        }
    }
}

#[kernel]
pub fn lookalike_thread(mut output: DisjointSlice<f32>) {
    let index = fe2o3_device::thread::index_1d();
    if let Some(value) = output.get_mut(index) {
        *value = 1.0;
    }
}

fn main() {}
