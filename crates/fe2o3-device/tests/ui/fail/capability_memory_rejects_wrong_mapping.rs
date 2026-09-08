use fe2o3_device::DisjointIndex;
use fe2o3_device::capability_memory::{DisjointWrite, Global};

enum KernelBrand {}
enum ExpectedMapping {}
enum OtherMapping {}

fn wrong_mapping(
    output: &mut Global<'_, u32, DisjointWrite<ExpectedMapping>, KernelBrand>,
    index: DisjointIndex<OtherMapping>,
) {
    let _ = output.store(index, 7);
}

fn main() {}
