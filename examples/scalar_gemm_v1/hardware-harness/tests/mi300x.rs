use fe2o3_host::WorkerV2PrerequisiteAuthenticatorV1;
use fe2o3_scalar_gemm_v1::kernel::scalar_gemm_v1_gpu;
use fe2o3_scalar_gemm_v1_hardware_harness::AuthenticatedScalarGemmHarness;

#[allow(dead_code)]
fn controller_requires_typed_authenticated_authority<Authenticator>(
    harness: AuthenticatedScalarGemmHarness<'_, '_, Authenticator>,
) where
    Authenticator: WorkerV2PrerequisiteAuthenticatorV1<scalar_gemm_v1_gpu::Marker>,
    Authenticator::Error: std::fmt::Debug,
{
    let _ = harness.run();
}

#[test]
#[ignore = "requires an inspected current Scalar GEMM V1 Worker V2 capability supplied by the protected publication controller"]
fn mi300x_authenticated_scalar_gemm_controller_is_externally_driven() {
    // The protected publication controller calls the typed function above.
    // There is intentionally no path or environment-variable fallback here.
}
