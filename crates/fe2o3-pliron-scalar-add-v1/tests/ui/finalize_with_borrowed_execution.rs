use fe2o3_hsaco_finalize::InertCompilerHandoffExecutionV2;
use fe2o3_pliron_scalar_add_v1::{
    RepositoryScalarAddProfileV1, finalize_repository_scalar_add_v1,
};
use fe2o3_pliron_worker_v2::InertScalarAddWorkerRequestV2;

fn reject_borrowed_execution(
    lineage: InertScalarAddWorkerRequestV2,
    execution: &InertCompilerHandoffExecutionV2,
    policy: RepositoryScalarAddProfileV1,
) {
    let _ = finalize_repository_scalar_add_v1(lineage, execution, policy);
}

fn main() {}
