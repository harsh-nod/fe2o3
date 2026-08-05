use fe2o3_macros::kernel;
use trusted_device::DisjointSlice;

#[kernel]
pub fn external_spoof(mut output: DisjointSlice<f32>) {
    let index = fe2o3_device::thread::index_1d();
    if let Some(value) = output.get_mut(index) {
        *value = 1.0;
    }
}

fn main() {}
