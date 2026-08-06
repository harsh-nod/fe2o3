use fe2o3_hsaco_finalize::{
    PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerRequestV2,
};

fn cannot_execute(
    worker: &PinnedWorkerV1,
    request: &WorkerRequestV2,
    limits: WorkerExecutionLimitsV1,
) {
    let _ = worker.execute_v2(request, limits);
}

fn main() {}
