use fe2o3_host::{LoadedWave64CollectivesV1, ReviewedWave64CollectivesRuntimeAdapterV1};

fn extract<A: ReviewedWave64CollectivesRuntimeAdapterV1>(
    value: &LoadedWave64CollectivesV1<'_, '_, '_, '_, A>,
) {
    let _ = value.native_executable_handle();
    let _ = value.native_kernel_handle();
}

fn main() {}
