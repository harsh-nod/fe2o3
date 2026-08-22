use std::path::Path;

use fe2o3_worker_v2_bundle::WorkerV3LoadEnvelopeWireV1;

fn persist_inert_wire(value: &WorkerV3LoadEnvelopeWireV1, output: &Path) {
    let _ = value.persist_durable_replay_custody_v1(output);
}

fn main() {}
