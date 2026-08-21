use fe2o3_hsaco_finalize::{
    PreparedFinalizedProtectedWorkerV2HsacoPublicationV2,
    PreparedProtectedWorkerV2HsacoPublicationV2, SealedProtectedWorkerV2HsacoPublicationIntentV2,
};

fn inspect_raw_fields(prepared: &PreparedProtectedWorkerV2HsacoPublicationV2) {
    let _ = &prepared.inspected;
    let _ = prepared.plan;
    let _ = prepared.upstream;
}

fn inspect_finalized_fields(prepared: &PreparedFinalizedProtectedWorkerV2HsacoPublicationV2) {
    let _ = &prepared.finalized;
    let _ = prepared.plan;
    let _ = prepared.upstream;
}

fn inspect_intent_fields(intent: SealedProtectedWorkerV2HsacoPublicationIntentV2) {
    let _ = intent.route;
    let _ = intent.plan;
    let _ = intent.upstream;
    let _ = intent.raw_inspection;
    let _ = intent.canonical_finalization;
    let _ = intent.raw_snapshot;
    let _ = intent.retained_snapshot;
    let _ = intent.handoff_slot;
    let _ = intent.handoff_identity;
    let _ = intent.compiler_closure;
}

fn main() {}
