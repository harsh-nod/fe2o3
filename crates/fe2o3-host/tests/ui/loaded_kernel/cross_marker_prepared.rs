use fe2o3_host::{LoadedKernel, PreparedLaunch};

struct KernelA;
struct KernelB;

fn cross(loaded: &LoadedKernel<KernelA>, prepared: PreparedLaunch<KernelB>) {
    let _ = loaded.bind(prepared);
}

fn main() {}
