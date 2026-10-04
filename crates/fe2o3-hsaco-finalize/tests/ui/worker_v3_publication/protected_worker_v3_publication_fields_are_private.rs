use fe2o3_hsaco_finalize::{
    PreparedProtectedWorkerV3CompactFinalizerReplayV2,
    PreparedProtectedWorkerV3HsacoPublicationV1,
};

fn inspect_replay(replay: &PreparedProtectedWorkerV3CompactFinalizerReplayV2) {
    let _ = &replay.transcript;
    let _ = &replay.outer_handoff;
    let _ = &replay.external_provider_payloads;
    let _ = &replay.finalized_hsaco;
}

fn inspect_publication(publication: &PreparedProtectedWorkerV3HsacoPublicationV1) {
    let _ = publication.producer_package;
    let _ = publication.intent;
    let _ = &publication.replay;
}

fn main() {}
