mod common;

use common::{FormulaProvider, binding, candidate, sentinel};
use fe2o3_qwen3_logits_compact_v1::*;

#[test]
fn target_decode_batch_records_bind_request_generation_epoch_plan_and_rows() {
    let candidate = candidate(Qwen3LogitsRoleV1::Target8B, B3LogitsBucketV1::DecodeS8C8192);
    let profile = candidate.profile().descriptor();
    let (binding, expected) = binding(candidate);
    let provider = FormulaProvider {
        rows: profile.rows,
        vocabulary: profile.vocabulary_size,
        first_winner: 7,
        second_winner: 29,
        nonfinite: None,
    };
    let mut output = vec![sentinel(candidate); profile.rows];
    let state = qwen3_argmax_compact_from_provider_reference_v1(
        candidate,
        &binding,
        &expected,
        &provider,
        &mut output,
    )
    .unwrap();
    assert_eq!(state.records, 8);
    assert_eq!(state.comparisons, 8 * (QWEN3_VOCABULARY_SIZE_V1 as u64 - 1));
    for (row, record) in output.iter().enumerate() {
        let logits: Vec<_> = (0..profile.vocabulary_size)
            .map(|token_id| provider.value(row, token_id))
            .collect();
        let oracle = independent_lowest_token_argmax_v1(&logits).unwrap();
        assert_eq!(record.schema_version, 1);
        assert_eq!(record.request, binding.requests[row]);
        assert_eq!(record.epoch, 73);
        assert_eq!(record.plan_identity, candidate.plan_identity());
        assert_eq!(record.candidate_identity, candidate.candidate_identity());
        assert_eq!(record.row, row as u32);
        assert_eq!(record.local_token, 0);
        assert_eq!(record.token_id, oracle);
        let encoded = encode_compact_completion_record_v1(*record);
        assert_eq!(encoded.len() as u64, COMPACT_COMPLETION_RECORD_BYTES_V1);
        assert_eq!(&encoded[4..8], &record.request.slot.to_le_bytes());
        assert_eq!(&encoded[8..12], &record.request.generation.to_le_bytes());
        assert_eq!(&encoded[12..20], &record.epoch.to_le_bytes());
        assert_eq!(&encoded[20..52], &record.plan_identity.0);
        assert_eq!(&encoded[92..96], &record.token_id.to_le_bytes());
    }
}

#[test]
fn speculative_local_rows_and_k_bound_are_exact() {
    let candidate = candidate(
        Qwen3LogitsRoleV1::Draft06B,
        B3LogitsBucketV1::SpeculativeS1K4C8192,
    );
    let profile = candidate.profile().descriptor();
    let (binding, expected) = binding(candidate);
    let provider = FormulaProvider {
        rows: profile.rows,
        vocabulary: profile.vocabulary_size,
        first_winner: 41,
        second_winner: 43,
        nonfinite: None,
    };
    let mut output = vec![sentinel(candidate); profile.rows];
    qwen3_argmax_compact_from_provider_reference_v1(
        candidate,
        &binding,
        &expected,
        &provider,
        &mut output,
    )
    .unwrap();
    assert_eq!(binding.speculative_k, 4);
    assert_eq!(output.len(), 4);
    for (local, record) in output.iter().enumerate() {
        assert_eq!(record.local_token, local as u32);
        assert_eq!(record.token_id, 41);
    }
}

#[test]
fn independent_two_pass_oracle_confirms_lowest_id_ties_and_signed_zero() {
    let mut logits = vec![-10.0_f32; QWEN3_VOCABULARY_SIZE_V1];
    logits[71] = 5.0;
    logits[9] = 5.0;
    assert_eq!(independent_lowest_token_argmax_v1(&logits), Ok(9));
    logits.fill(-1.0);
    logits[1] = -0.0;
    logits[0] = 0.0;
    assert_eq!(independent_lowest_token_argmax_v1(&logits), Ok(0));
}

#[test]
fn nonfinite_at_final_token_preserves_entire_output() {
    let candidate = candidate(Qwen3LogitsRoleV1::Draft06B, B3LogitsBucketV1::DecodeS1C8192);
    let profile = candidate.profile().descriptor();
    let (binding, expected) = binding(candidate);
    let provider = FormulaProvider {
        rows: 1,
        vocabulary: profile.vocabulary_size,
        first_winner: 1,
        second_winner: 2,
        nonfinite: Some((0, profile.vocabulary_size - 1)),
    };
    let mut output = vec![sentinel(candidate)];
    let before = output.clone();
    assert_eq!(
        qwen3_argmax_compact_from_provider_reference_v1(
            candidate,
            &binding,
            &expected,
            &provider,
            &mut output
        ),
        Err(LogitsReferenceErrorV1::NonFiniteLogit {
            row: 0,
            token_id: profile.vocabulary_size - 1
        })
    );
    assert_eq!(output, before);
}
