use fe2o3_device::{DisjointSlice, memory};

fn forged_integer(source: &[u32], mut output: DisjointSlice<u32>) {
    memory::volatile_store(&mut output, 0, 7);
    memory::copy_one_nonoverlapping(source, 0, &mut output, 0);
}

fn main() {}
