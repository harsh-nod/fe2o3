use fe2o3_device::{ExclusiveReadWrite, Global};

enum KernelA {}
enum KernelB {}

fn needs_a(_: Global<'_, f32, ExclusiveReadWrite, KernelA>) {}

fn reject(output: Global<'_, f32, ExclusiveReadWrite, KernelB>) {
    needs_a(output);
}

fn main() {}
