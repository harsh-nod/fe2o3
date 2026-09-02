use fe2o3_host::WorkerV3CompilerCurrentRecordAuditV1;

fn clone_view(owner: &WorkerV3CompilerCurrentRecordAuditV1) {
    let view = owner.canonical_evidence_view();
    let _ = view.clone();
}

fn main() {}
