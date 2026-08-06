use fe2o3_host::{GeneratedAdmittedLaunch, KernelParams};

struct Kernel;

fn extract(
    paired: GeneratedAdmittedLaunch<'_, '_, Kernel, ()>,
) -> KernelParams {
    paired.params
}

fn main() {}
