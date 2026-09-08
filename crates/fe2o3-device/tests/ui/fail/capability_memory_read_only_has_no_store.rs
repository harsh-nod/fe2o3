use fe2o3_device::DisjointIndex;
use fe2o3_device::capability_memory::{Global, ReadOnly};

enum KernelBrand {}
enum Mapping {}

fn illegal_write(
    input: &mut Global<'_, u32, ReadOnly, KernelBrand>,
    index: DisjointIndex<Mapping>,
) {
    let _ = input.store(index, 7);
}

fn main() {}
