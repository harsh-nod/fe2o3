use fe2o3_device::capability_memory::{Global, ReadOnly};

enum FirstKernel {}
enum SecondKernel {}

fn substitute<'kernel>(
    view: Global<'kernel, u32, ReadOnly, FirstKernel>,
) -> Global<'kernel, u32, ReadOnly, SecondKernel> {
    view
}

fn main() {}
