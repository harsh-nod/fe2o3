mod common;

use common::{binding, candidate, sentinel, source};
use fe2o3_qwen3_logits_compact_v1::*;

fn draft_decode() -> StructuralLogitsCandidateV1 {
    candidate(Qwen3LogitsRoleV1::Draft06B, B3LogitsBucketV1::DecodeS1C8192)
}

#[test]
fn exact_fp32_projection_tracks_independent_f64_oracle() {
    let candidate = draft_decode();
    let source = source(candidate);
    for token_id in [0, 1, 17, QWEN3_VOCABULARY_SIZE_V1 - 1] {
        let fp32 = qwen3_project_logit_v1(candidate, &source, 0, token_id).unwrap();
        let f64 = qwen3_project_logit_f64_oracle_v1(candidate, &source, 0, token_id).unwrap();
        assert!((f64::from(fp32) - f64).abs() <= 1.0e-5 * f64.abs().max(1.0));
    }
}

#[test]
fn source_extent_missing_nonfinite_and_overflow_mutations_reject() {
    let candidate = draft_decode();
    let exact = source(candidate);
    let mut mutated = exact;
    mutated.activation_elements -= 1;
    assert!(matches!(
        qwen3_project_logit_v1(candidate, &mutated, 0, 0),
        Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::Activation,
            ..
        })
    ));
    let mut mutated = exact;
    mutated.missing_weight = Some(0);
    assert_eq!(
        qwen3_project_logit_v1(candidate, &mutated, 0, 0),
        Err(LogitsReferenceErrorV1::MissingSourceElement {
            tensor: LogitsTensorV1::Weight,
            index: 0
        })
    );
    let mut mutated = exact;
    mutated.nonfinite_activation = Some(0);
    assert_eq!(
        qwen3_project_logit_v1(candidate, &mutated, 0, 0),
        Err(LogitsReferenceErrorV1::NonFiniteInput {
            tensor: LogitsTensorV1::Activation,
            index: 0
        })
    );
    let mut mutated = exact;
    mutated.nonfinite_weight = Some(0);
    assert_eq!(
        qwen3_project_logit_v1(candidate, &mutated, 0, 0),
        Err(LogitsReferenceErrorV1::NonFiniteInput {
            tensor: LogitsTensorV1::Weight,
            index: 0
        })
    );
    let mut mutated = exact;
    mutated.maximum_finite = true;
    assert!(matches!(
        qwen3_project_logit_v1(candidate, &mutated, 0, 0),
        Err(LogitsReferenceErrorV1::NonFiniteIntermediate {
            stage: LogitsArithmeticStageV1::Product,
            ..
        })
    ));
}

#[test]
fn coordinate_bounds_are_exact() {
    let candidate = draft_decode();
    let source = source(candidate);
    assert_eq!(
        qwen3_project_logit_v1(candidate, &source, 1, 0),
        Err(LogitsReferenceErrorV1::CoordinateOutOfRange)
    );
    assert_eq!(
        qwen3_project_logit_v1(candidate, &source, 0, QWEN3_VOCABULARY_SIZE_V1),
        Err(LogitsReferenceErrorV1::CoordinateOutOfRange)
    );
}

#[test]
fn exact_combined_entry_point_rejects_source_failure_transactionally() {
    let candidate = draft_decode();
    let (binding, expected) = binding(candidate);
    let mut source = source(candidate);
    source.nonfinite_activation = Some(0);
    let mut output = vec![sentinel(candidate)];
    let before = output.clone();
    assert!(matches!(
        qwen3_logits_argmax_compact_reference_v1(
            candidate,
            &binding,
            &expected,
            &source,
            &mut output
        ),
        Err(LogitsReferenceErrorV1::NonFiniteInput {
            tensor: LogitsTensorV1::Activation,
            index: 0
        })
    ));
    assert_eq!(output, before);
}
