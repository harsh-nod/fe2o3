use fe2o3_device::{DisjointSlice, kernel, thread};

// The leading UTF-8 BOM is intentional. Do not normalize this negative fixture.
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn normalized_bits(mut output: DisjointSlice<u32>, a: u32, b: u32, mask: u32) {
    let selected = b ^ ((a ^ b) & mask);
    let index = thread::index_1d();
    if let Some(slot) = output.get_mut(index) {
        *slot = selected;
    }
}
