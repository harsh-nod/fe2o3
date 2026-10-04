use fe2o3_host::{AuthenticatedServiceQueueSessionV1, AuthenticatedWorkerV3ProgramSetV1};
use fe2o3_service_host::{ServiceAllocationSessionV1, ServiceFixedBatchV1};

fn create_from_raw_batch(
    programs: AuthenticatedWorkerV3ProgramSetV1,
    allocations: ServiceAllocationSessionV1,
    raw_batch: ServiceFixedBatchV1<'_, 1>,
) {
    let _ = AuthenticatedServiceQueueSessionV1::<1>::create(
        programs,
        allocations,
        4096,
        raw_batch,
    );
}

fn main() {}
