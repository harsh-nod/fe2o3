use fe2o3_hsaco_finalize::{
    FlashAttentionV1FinalizationExpectationV1, InertFirstBuildWorkerV2EvidenceV1,
    finalize_flash_attention_v1_worker_v2_hsaco_v1,
};

fn replay(
    evidence: InertFirstBuildWorkerV2EvidenceV1,
    expectation: FlashAttentionV1FinalizationExpectationV1,
) {
    let _first = finalize_flash_attention_v1_worker_v2_hsaco_v1(
        evidence,
        expectation.clone(),
    );
    let _second = finalize_flash_attention_v1_worker_v2_hsaco_v1(evidence, expectation);
}

fn main() {}
