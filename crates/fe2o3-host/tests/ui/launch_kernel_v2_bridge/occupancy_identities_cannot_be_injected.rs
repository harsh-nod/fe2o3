use fe2o3_host::CurrentRecoveredLaunchKernelMetadataV2;
use fe2o3_kernel_ir::{OccupancyMetadataIdentityV2, OccupancyVerifierIdentityV2};

fn inject_caller_claims(binding: &CurrentRecoveredLaunchKernelMetadataV2<'_>) {
    let verifier = OccupancyVerifierIdentityV2::from_bytes([1; 32]);
    let metadata = OccupancyMetadataIdentityV2::from_bytes([2; 32]);
    binding.require_occupancy_dependent_admission(verifier, metadata);
}

fn main() {}
