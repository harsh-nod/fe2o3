use fe2o3_host::RecoveredWorkerV2PinnedDescriptorV1;

fn use_authority(value: RecoveredWorkerV2PinnedDescriptorV1) {
    value.load();
    value.launch();
}

fn main() {}
