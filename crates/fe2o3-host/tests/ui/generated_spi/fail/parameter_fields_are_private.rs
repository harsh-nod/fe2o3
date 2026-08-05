use fe2o3_host::{GeneratedKernelParams, KernelParams};
use std::marker::PhantomData;

struct Kernel;

fn forge(params: &mut KernelParams) -> GeneratedKernelParams<'_, Kernel, ()> {
    GeneratedKernelParams {
        params,
        resources: (),
        marker: PhantomData,
    }
}

fn main() {}
