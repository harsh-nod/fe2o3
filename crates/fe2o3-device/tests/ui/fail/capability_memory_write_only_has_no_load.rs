use fe2o3_device::capability_memory::{DisjointWrite, Global};

enum KernelBrand {}
enum Mapping {}

fn illegal_read(output: &Global<'_, u32, DisjointWrite<Mapping>, KernelBrand>) {
    let _ = output.load(0);
}

fn main() {}
