use fe2o3_device_real::{DisjointSlice as RealDisjointSlice, ThreadIndex, kernel, thread};

mod fe2o3_device {
    use super::{RealDisjointSlice, ThreadIndex};

    pub struct DisjointSlice;

    impl DisjointSlice {
        pub fn get_mut(
            output: &mut RealDisjointSlice<f32>,
            index: ThreadIndex,
        ) -> Option<&mut f32> {
            output.get_mut(index)
        }
    }
}

#[kernel]
pub fn lookalike_helper(mut output: RealDisjointSlice<f32>) {
    let index = thread::index_1d();
    if let Some(value) = fe2o3_device::DisjointSlice::get_mut(&mut output, index) {
        *value = 1.0;
    }
}

fn main() {}
