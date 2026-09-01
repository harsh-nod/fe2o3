use fe2o3_host::{
    AuthenticatedServiceQueueHostDataUpdateV1, AuthenticatedServiceQueuePartitionedDataUpdateV1,
};
use fe2o3_service_host::DeviceAllocationRoleMarkerV1;

fn escape_host(update: AuthenticatedServiceQueueHostDataUpdateV1) {
    let _raw = update.into_raw();
}

fn escape_partition<R: DeviceAllocationRoleMarkerV1, const N: usize>(
    update: AuthenticatedServiceQueuePartitionedDataUpdateV1<R, N>,
) {
    let _raw = update.into_raw();
}

fn main() {}
