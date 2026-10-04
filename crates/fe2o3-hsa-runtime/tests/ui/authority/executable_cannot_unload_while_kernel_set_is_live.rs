use fe2o3_host::ReviewedHsaExecutableLifecycleAdapterV1;
use fe2o3_hsa_runtime::{
    ReviewedHsaExecutableV1, ReviewedHsaRuntimeAdapterV1,
};

unsafe fn unload_with_live_kernels(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: ReviewedHsaExecutableV1,
) {
    let (kernels, _) = unsafe {
        adapter
            .resolve_kernel_set(&executable, ["first", "second"])
            .unwrap()
    };
    let _ = unsafe {
        ReviewedHsaExecutableLifecycleAdapterV1::unload_executable(adapter, executable)
    };
    let _ = kernels.get(0);
}

fn main() {}
