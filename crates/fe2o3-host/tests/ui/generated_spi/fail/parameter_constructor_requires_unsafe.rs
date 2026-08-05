use fe2o3_host::{GeneratedKernelParams, KernelParams};

struct Kernel;

fn pack(params: &mut KernelParams) {
    let _ = GeneratedKernelParams::<Kernel, _>::from_generated_unchecked(params, ());
}

fn main() {}
