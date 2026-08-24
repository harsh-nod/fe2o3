use fe2o3_device::{DisjointSlice, memory};

fn missing_expert_proof(source: &[u32], mut output: DisjointSlice<u32>, count: usize) {
    memory::copy_nonoverlapping_unchecked(source, 0, &mut output, 0, count);
}

fn main() {}
