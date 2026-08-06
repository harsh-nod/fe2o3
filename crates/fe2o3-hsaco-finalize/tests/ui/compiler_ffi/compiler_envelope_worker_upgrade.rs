use fe2o3_hsaco_finalize::{
    StagedCompilerFfiEnvelopeV1, StagedFfiLinkPlanV1, WorkerOutputV1,
};

fn cannot_reduce_or_upgrade(
    staged: &StagedCompilerFfiEnvelopeV1,
    output: &WorkerOutputV1,
) {
    let _ = staged.envelope();
    let _ = staged.contracts();
    let _ = staged.worker_request_v1();
    let _ = staged.bind_worker_output_v1(output);
    let _: StagedFfiLinkPlanV1 = staged.clone().into();
}

fn main() {}
