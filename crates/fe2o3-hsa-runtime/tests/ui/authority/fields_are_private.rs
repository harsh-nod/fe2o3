use fe2o3_hsa_runtime::{
    ReviewedHsaExecutableV1, ReviewedHsaKernelV1, ReviewedHsaRuntimeAdapterV1,
};

fn inspect_executable(executable: &ReviewedHsaExecutableV1) {
    let _ = &executable.state;
}

fn inspect_kernel(kernel: &ReviewedHsaKernelV1) {
    let _ = kernel.kernel_object;
    let _ = kernel.symbol;
}

fn inspect_adapter(adapter: &ReviewedHsaRuntimeAdapterV1) {
    let _ = &adapter.core;
    let _ = &adapter.pending_dispatch;
}

fn main() {}
