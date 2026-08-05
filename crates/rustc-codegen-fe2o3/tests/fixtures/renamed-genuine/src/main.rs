use device_api::{DisjointSlice, thread};
use fe2o3_macros::kernel;

#[kernel]
pub fn renamed_genuine(mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    if let Some(value) = output.get_mut(index) {
        *value = 1.0;
    }
}

fn main() {}
