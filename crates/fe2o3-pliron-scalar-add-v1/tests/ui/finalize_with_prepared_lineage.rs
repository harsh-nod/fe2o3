use fe2o3_hsaco_finalize::InertCompilerHandoffExecutionV2;
use fe2o3_pliron_scalar_add_v1::{
    RepositoryScalarAddProfileV1, finalize_repository_scalar_add_v1,
};
use fe2o3_pliron_worker_v2::PreparedScalarAddWorkerV2;

fn reject_unsealed_lineage(
    prepared: PreparedScalarAddWorkerV2,
    execution: InertCompilerHandoffExecutionV2,
    policy: RepositoryScalarAddProfileV1,
) {
    let _ = finalize_repository_scalar_add_v1(prepared, execution, policy);
}

fn main() {}
