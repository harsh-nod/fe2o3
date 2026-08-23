use fe2o3_host::RecoveredWorkerV3PinnedDescriptorV1;

fn use_authority(value: RecoveredWorkerV3PinnedDescriptorV1) {
    value.load();
    value.launch();
}

fn main() {}
