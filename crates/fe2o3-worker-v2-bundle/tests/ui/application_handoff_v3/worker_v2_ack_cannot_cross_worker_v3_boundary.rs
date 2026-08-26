use fe2o3_runtime_protocol::WorkerV3ApplicationHandoffAckV1;
use fe2o3_worker_v2_bundle::WorkerV2ApplicationHandoffAckV1;

fn require_worker_v3(_: WorkerV3ApplicationHandoffAckV1) {}

fn cross_generation(legacy: WorkerV2ApplicationHandoffAckV1) {
    require_worker_v3(legacy);
}

fn main() {}
