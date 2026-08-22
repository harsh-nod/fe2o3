use fe2o3_artifact_transaction::{
    DurablePublishedHsacoClaimV3, VerifiedWorkerV3LoadEnvelopeAuthorityV1,
    WorkerV3LoadEnvelopeBindingV1,
};

fn fabricate_authority(
    binding: WorkerV3LoadEnvelopeBindingV1,
    claim: &DurablePublishedHsacoClaimV3,
) {
    let _ = VerifiedWorkerV3LoadEnvelopeAuthorityV1::from_complete_compact_replay_preimages_unchecked(
        binding,
        claim,
    );
}

fn main() {}
