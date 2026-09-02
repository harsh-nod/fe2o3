use fe2o3_host::WorkerV3RosterLoadEnvelopeEvidenceViewV1;

fn duplicate(view: WorkerV3RosterLoadEnvelopeEvidenceViewV1<'_>) {
    let _second = view.clone();
}

fn main() {}
