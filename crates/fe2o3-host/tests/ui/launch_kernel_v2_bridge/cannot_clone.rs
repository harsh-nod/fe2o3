use fe2o3_host::CurrentRecoveredLaunchKernelMetadataV2;

fn clone_binding(binding: CurrentRecoveredLaunchKernelMetadataV2<'_>) {
    let _duplicate = binding.clone();
}

fn main() {}
