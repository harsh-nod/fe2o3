use fe2o3_artifact_transaction::{
    VerifiedWorkerV3LoadEnvelopeAuthorityV1, WorkerV3LoadEnvelopeBindingV1,
};

fn fabricate_authority(binding: WorkerV3LoadEnvelopeBindingV1) {
    let _ = VerifiedWorkerV3LoadEnvelopeAuthorityV1::from_complete_compact_replay_preimages_unchecked(
        binding,
    );
}

fn main() {}
