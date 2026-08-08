use fe2o3_host::RecoveredWorkerV2PinnedDescriptorV1;

fn extract(value: RecoveredWorkerV2PinnedDescriptorV1) {
    let RecoveredWorkerV2PinnedDescriptorV1 {
        admission,
        descriptor,
    } = value;
    let _ = (admission, descriptor);
}

fn main() {}
