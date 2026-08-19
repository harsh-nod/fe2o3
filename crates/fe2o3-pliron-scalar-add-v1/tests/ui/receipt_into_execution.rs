use fe2o3_hsaco_finalize::InertCompilerHandoffExecutionV2;
use fe2o3_pliron_scalar_add_v1::FinalizedRepositoryScalarAddV1;

fn expose_retained_execution(
    receipt: FinalizedRepositoryScalarAddV1,
) -> InertCompilerHandoffExecutionV2 {
    receipt.into_execution()
}

fn main() {}
