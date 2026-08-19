use fe2o3_hsaco_finalize::{
    InertCompilerHandoffExecutionV2, InertFirstBuildWorkerV2EvidenceV1,
};

fn take_twice(
    evidence: InertFirstBuildWorkerV2EvidenceV1,
) -> (
    InertCompilerHandoffExecutionV2,
    InertCompilerHandoffExecutionV2,
) {
    let first = evidence.into_authorized_execution();
    let second = evidence.into_authorized_execution();
    (first, second)
}

fn main() {}
