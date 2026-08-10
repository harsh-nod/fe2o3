use fe2o3_worker_v2_bundle::WorkerV2ApplicationHandoffAckV1;

fn use_as_authority(ack: WorkerV2ApplicationHandoffAckV1) {
    ack.recover();
    ack.load();
    ack.launch();
}

fn main() {}
