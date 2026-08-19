use fe2o3_hsaco_finalize::InertCompilerHandoffExecutionV2;
use fe2o3_pliron_scalar_add_v1::{
    FinalizedRepositoryScalarAddV1, execute_repository_scalar_add_v1_on_mi300x,
};

fn inject_second_execution(
    receipt: FinalizedRepositoryScalarAddV1,
    execution: InertCompilerHandoffExecutionV2,
) {
    let _ = execute_repository_scalar_add_v1_on_mi300x(receipt, execution);
}

fn main() {}
