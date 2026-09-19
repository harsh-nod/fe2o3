use fe2o3_device::{DisjointSlice, kernel, thread};

// λ: UTF-8 outside identifiers must not be confused with character offsets.
#[cfg(feature = "source-bitselect-feasibility")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn choose_bits(mut output: DisjointSlice<u32>, a: u32, b: u32, mask: u32) {
    let selected = b ^ ((a ^ b) & mask);
    let index = thread::index_1d();
    if let Some(slot) = output.get_mut(index) {
        *slot = selected;
    }
}

#[cfg(feature = "source-bitselect-ambiguous")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ambiguous_bits(mut output: DisjointSlice<u32>, a: u32, b: u32, mask: u32) {
    let first = b ^ ((a ^ b) & mask);
    let second = b ^ ((a ^ b) & mask);
    let index = thread::index_1d();
    if let Some(slot) = output.get_mut(index) {
        *slot = first ^ second;
    }
}

#[cfg(feature = "source-bitselect-local-alias")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn alias_bits(mut output: DisjointSlice<u32>, a: u32, b: u32, mask: u32) {
    let alias = a;
    let selected = b ^ ((alias ^ b) & mask);
    let index = thread::index_1d();
    if let Some(slot) = output.get_mut(index) {
        *slot = selected;
    }
}
