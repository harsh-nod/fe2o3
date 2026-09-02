use fe2o3_host::WorkerV3CompilerExecutionVerificationV1;

fn move_bound_lane_while_view_is_live(owner: WorkerV3CompilerExecutionVerificationV1) {
    let view = owner.current_record_evidence_view().unwrap();
    drop(owner);
    let _ = view.verification_identity();
}

fn main() {}
