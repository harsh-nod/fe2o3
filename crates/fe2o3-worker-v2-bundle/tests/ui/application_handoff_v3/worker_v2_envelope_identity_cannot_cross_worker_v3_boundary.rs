use fe2o3_runtime_protocol::WorkerV3LoadEnvelopeIdentityV1;
use fe2o3_worker_v2_bundle::WorkerV2LoadEnvelopeIdentityV2;

fn require_worker_v3(_: WorkerV3LoadEnvelopeIdentityV1) {}

fn cross_generation(legacy: WorkerV2LoadEnvelopeIdentityV2) {
    require_worker_v3(legacy);
}

fn main() {}
