use fe2o3_device::DisjointSlice;

fn arbitrary_access(mut output: DisjointSlice<u32>, index: usize) {
    let _ = output.get_mut_at(index);
}

fn main() {}
