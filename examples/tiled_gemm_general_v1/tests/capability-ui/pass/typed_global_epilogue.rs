// expected-supported: FE2O3-CAP-GEMM exclusive-read-write-indexed-rmw
#![no_std]

#[cfg(not(target_arch = "amdgpu"))]
compile_error!("GEMM device UI must compile the actual AMD target");

use fe2o3_device::{ExclusiveReadWrite, Global, KernelContext, kernel};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn typed_global_epilogue(
    context: KernelContext<'_>,
    mut output: Global<'_, f32, ExclusiveReadWrite>,
    product: f32,
    alpha: f32,
    beta: f32,
) {
    let index = context.invocation().index_1d().get();
    if let Some(previous) = output.load(index) {
        let value = alpha * product + beta * previous;
        if !output.store(index, value) {
            fe2o3_device::trap();
        }
    }
}
