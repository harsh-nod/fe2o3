mod common;

use common::{FormulaProvider, binding, candidate, sentinel};
use fe2o3_qwen3_logits_compact_v1::*;

fn fixture() -> (
    StructuralLogitsCandidateV1,
    CompactBatchBindingV1,
    CompactBatchExpectationV1,
) {
    let candidate = candidate(Qwen3LogitsRoleV1::Target8B, B3LogitsBucketV1::DecodeS8C8192);
    let (binding, expected) = binding(candidate);
    (candidate, binding, expected)
}

#[test]
fn stale_plan_epoch_request_generation_and_alias_reject() {
    let (candidate, exact, expected) = fixture();
    let mut mutated = exact.clone();
    mutated.plan_identity = LogitsPlanIdentityV1([0x72; 32]);
    assert_eq!(
        validate_compact_batch_binding_v1(
            candidate.profile(),
            candidate.plan_identity(),
            &mutated,
            &expected
        ),
        Err(CompactBindingErrorV1::StalePlanIdentity)
    );
    let mut mutated = exact.clone();
    mutated.epoch += 1;
    assert_eq!(
        validate_compact_batch_binding_v1(
            candidate.profile(),
            candidate.plan_identity(),
            &mutated,
            &expected
        ),
        Err(CompactBindingErrorV1::StaleEpoch)
    );
    let mut mutated = exact.clone();
    mutated.epoch = 0;
    assert_eq!(
        validate_compact_batch_binding_v1(
            candidate.profile(),
            candidate.plan_identity(),
            &mutated,
            &expected
        ),
        Err(CompactBindingErrorV1::MissingEpoch)
    );
    let mut mutated = exact.clone();
    mutated.requests[0].generation += 1;
    assert_eq!(
        validate_compact_batch_binding_v1(
            candidate.profile(),
            candidate.plan_identity(),
            &mutated,
            &expected
        ),
        Err(CompactBindingErrorV1::StaleRequest)
    );
    let mut mutated = exact.clone();
    mutated.requests[1].slot = mutated.requests[0].slot;
    assert_eq!(
        validate_compact_batch_binding_v1(
            candidate.profile(),
            candidate.plan_identity(),
            &mutated,
            &expected
        ),
        Err(CompactBindingErrorV1::DuplicateRequestSlot)
    );
    let mut mutated = exact.clone();
    mutated.requests[0].generation = 0;
    assert_eq!(
        validate_compact_batch_binding_v1(
            candidate.profile(),
            candidate.plan_identity(),
            &mutated,
            &expected
        ),
        Err(CompactBindingErrorV1::MissingRequestGeneration)
    );
    let mut mutated = exact;
    mutated.requests[0].slot = 32;
    assert_eq!(
        validate_compact_batch_binding_v1(
            candidate.profile(),
            candidate.plan_identity(),
            &mutated,
            &expected
        ),
        Err(CompactBindingErrorV1::RequestSlot)
    );
}

#[test]
fn k_provider_and_output_extent_mutations_fail_before_publication() {
    let (candidate, exact, expected) = fixture();
    let profile = candidate.profile().descriptor();
    let provider = FormulaProvider {
        rows: profile.rows,
        vocabulary: profile.vocabulary_size,
        first_winner: 1,
        second_winner: 2,
        nonfinite: None,
    };
    let mut output = vec![sentinel(candidate); profile.rows];
    let before = output.clone();
    let mut mutated = exact.clone();
    mutated.speculative_k = 17;
    assert_eq!(
        qwen3_argmax_compact_from_provider_reference_v1(
            candidate,
            &mutated,
            &expected,
            &provider,
            &mut output
        ),
        Err(LogitsReferenceErrorV1::Binding(
            CompactBindingErrorV1::SpeculativeK
        ))
    );
    assert_eq!(output, before);

    let wrong_rows = FormulaProvider {
        rows: profile.rows - 1,
        ..provider
    };
    assert!(matches!(
        qwen3_argmax_compact_from_provider_reference_v1(
            candidate,
            &exact,
            &expected,
            &wrong_rows,
            &mut output
        ),
        Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::Logits,
            ..
        })
    ));
    assert_eq!(output, before);

    assert!(matches!(
        qwen3_argmax_compact_from_provider_reference_v1(
            candidate,
            &exact,
            &expected,
            &provider,
            &mut output[..profile.rows - 1]
        ),
        Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::CompactOutput,
            ..
        })
    ));
}

#[test]
fn production_authorities_remain_closed() {
    assert!(!std::hint::black_box(
        LOGITS_COMPACT_SOURCE_TO_KIR_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        LOGITS_COMPACT_VERUS_PROOF_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        LOGITS_COMPACT_ARTIFACT_PUBLICATION_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        LOGITS_COMPACT_ARTIFACT_LOAD_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(LOGITS_COMPACT_LAUNCH_SUPPORTED_V1));
    assert!(!std::hint::black_box(
        LOGITS_COMPACT_MACHINE_REFINEMENT_PROVED_V1
    ));
    assert!(LOGITS_COMPACT_PRODUCTION_BLOCKER_V1.contains("#174"));
}
