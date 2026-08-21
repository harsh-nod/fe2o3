use fe2o3_worker_v2_bundle::{WorkerV2LoadEnvelopeV1, WorkerV2LoadEnvelopeV2};

fn require_protected_v2(_: WorkerV2LoadEnvelopeV2) {}

fn cross_schema(legacy: WorkerV2LoadEnvelopeV1) {
    require_protected_v2(legacy);
}

fn main() {}
