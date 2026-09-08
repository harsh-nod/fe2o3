// expected-boundary: FE2O3-CAP-GEMM beta epilogue needs disjoint read-modify-write Global
use fe2o3_device::{ExclusiveReadWrite, Global, KernelContext, kernel};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn typed_global_epilogue(
    _context: KernelContext<'_>,
    _output: Global<'_, f32, ExclusiveReadWrite>,
) {
}
