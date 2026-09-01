use fe2o3_host::AuthenticatedServiceQueueUnboundSessionV1;
use fe2o3_service_host::ServiceFixedBatchV1;

fn bind_raw_batch(
    queue: AuthenticatedServiceQueueUnboundSessionV1,
    raw_batch: ServiceFixedBatchV1<'_, 1>,
) {
    let _ = queue.bind_retained(raw_batch);
}

fn main() {}
