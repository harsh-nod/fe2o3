#![no_std]

use fe2o3_device::{DisjointWrite, Global, Index1D, KernelContext, kernel};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
pub fn fill(context: KernelContext<'_>, mut out: Global<'_, f32, DisjointWrite<Index1D>>) {
    let index = context.invocation().index_1d();
    let _stored = out.store(index.into_disjoint(), 42.5);
}

#[cfg(not(target_arch = "amdgpu"))]
pub fn fill_cpu_reference(output: &mut [f32], launched: usize) {
    for value in output.iter_mut().take(launched) {
        *value = 42.5;
    }
}
