use fe2o3_host::WorkerV3CompilerCurrentRecordAuditV1;

fn move_owner_while_view_is_live(owner: WorkerV3CompilerCurrentRecordAuditV1) {
    let view = owner.canonical_evidence_view();
    drop(owner);
    let _ = view.attestation_identity();
}

fn main() {}
