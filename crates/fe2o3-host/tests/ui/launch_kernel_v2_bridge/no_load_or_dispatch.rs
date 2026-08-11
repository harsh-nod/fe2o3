use fe2o3_host::CurrentRecoveredLaunchKernelMetadataV2;

fn misuse(binding: CurrentRecoveredLaunchKernelMetadataV2<'_>) {
    binding.load();
    binding.dispatch();
}

fn main() {}
