// expected-boundary: FE2O3-CAP-GEMM matrix numerical policy needs a typed source owner
use fe2o3_device::MatrixCapability;

enum StrictIeee {}

fn missing_policy_owner<Brand>(matrix: &MatrixCapability<Brand>) {
    let _ = matrix.with_numerical_policy::<StrictIeee>();
}
