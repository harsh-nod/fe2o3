use std::collections::BTreeSet;

use fe2o3_qwen3_linear_reference_v1::linear::*;

const BUCKETS: [(Qwen3LinearModeV1, Qwen3LinearBucketV1); 11] = [
    (
        Qwen3LinearModeV1::Prefill,
        Qwen3LinearBucketV1::PrefillS1T128,
    ),
    (
        Qwen3LinearModeV1::Prefill,
        Qwen3LinearBucketV1::PrefillS8T128,
    ),
    (
        Qwen3LinearModeV1::Prefill,
        Qwen3LinearBucketV1::PrefillS1T512,
    ),
    (
        Qwen3LinearModeV1::Prefill,
        Qwen3LinearBucketV1::PrefillS1T2048,
    ),
    (
        Qwen3LinearModeV1::Decode,
        Qwen3LinearBucketV1::DecodeS1C8192,
    ),
    (
        Qwen3LinearModeV1::Decode,
        Qwen3LinearBucketV1::DecodeS8C8192,
    ),
    (
        Qwen3LinearModeV1::Decode,
        Qwen3LinearBucketV1::DecodeS32C8192,
    ),
    (
        Qwen3LinearModeV1::Speculative,
        Qwen3LinearBucketV1::SpeculativeS1K4C8192,
    ),
    (
        Qwen3LinearModeV1::Speculative,
        Qwen3LinearBucketV1::SpeculativeS8K4C8192,
    ),
    (
        Qwen3LinearModeV1::Speculative,
        Qwen3LinearBucketV1::SpeculativeS1K8C8192,
    ),
    (
        Qwen3LinearModeV1::Speculative,
        Qwen3LinearBucketV1::SpeculativeS1K16C8192,
    ),
];

const LINEAR_OPERATORS: [Qwen3B3OperatorV1; 8] = [
    Qwen3B3OperatorV1::QueryProjection,
    Qwen3B3OperatorV1::KeyProjection,
    Qwen3B3OperatorV1::ValueProjection,
    Qwen3B3OperatorV1::AttentionOutputResidual,
    Qwen3B3OperatorV1::GateProjection,
    Qwen3B3OperatorV1::UpProjection,
    Qwen3B3OperatorV1::DownResidual,
    Qwen3B3OperatorV1::LogitsProjection,
];

fn selection(
    role: Qwen3LinearRoleV1,
    mode: Qwen3LinearModeV1,
    bucket: Qwen3LinearBucketV1,
    operator: Qwen3B3OperatorV1,
    layer: u16,
) -> Qwen3LinearSelectionV1 {
    Qwen3LinearSelectionV1 {
        role,
        mode,
        bucket,
        operator,
        layer,
    }
}

fn bf16(value: f32) -> u16 {
    (value.to_bits() >> 16) as u16
}

fn exact_identity_source(n: usize, k: usize) -> Vec<u16> {
    let mut source = vec![bf16(0.0); n * k];
    for diagonal in 0..n.min(k) {
        source[diagonal * k + diagonal] = bf16(1.0);
    }
    source
}

#[test]
fn all_330_b3_selections_have_exact_storage_and_unique_host_identities() {
    let mut selection_ids = BTreeSet::new();
    let mut checked = 0;
    for role in [Qwen3LinearRoleV1::Target8B, Qwen3LinearRoleV1::Draft06B] {
        let geometry = role.geometry();
        for (mode, bucket) in BUCKETS {
            for operator in LINEAR_OPERATORS {
                let layers: &[u16] = if operator == Qwen3B3OperatorV1::LogitsProjection {
                    &[QWEN3_NO_LAYER_V1]
                } else {
                    &[0, geometry.layers - 1]
                };
                for layer in layers {
                    let selection = selection(role, mode, bucket, operator, *layer);
                    let candidate = exact_qwen3_linear_candidate_v1(selection).unwrap();
                    validate_qwen3_linear_candidate_v1(&candidate, selection).unwrap();
                    let dimensions = candidate.dimensions;
                    assert_eq!(
                        candidate.layout.strides,
                        [dimensions.k, dimensions.n, dimensions.n]
                    );
                    let expected = [
                        dimensions.m as usize * dimensions.k as usize,
                        dimensions.k as usize * dimensions.n as usize,
                        dimensions.m as usize * dimensions.n as usize,
                    ];
                    assert_eq!(candidate.resources.storage_elements, expected);
                    assert_eq!(
                        candidate.resources.storage_bytes,
                        [
                            expected[0] as u64 * 2,
                            expected[1] as u64 * 2,
                            expected[2] as u64 * 4
                        ]
                    );
                    assert!(selection_ids.insert(candidate.selection_identity));
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 330);
    assert_eq!(selection_ids.len(), 330);
}

#[test]
fn exact_target_and_draft_projection_dimensions_match_b3() {
    for (role, expected) in [
        (
            Qwen3LinearRoleV1::Target8B,
            [
                (4_096, 4_096),
                (1_024, 4_096),
                (12_288, 4_096),
                (4_096, 12_288),
            ],
        ),
        (
            Qwen3LinearRoleV1::Draft06B,
            [
                (2_048, 1_024),
                (1_024, 1_024),
                (3_072, 1_024),
                (1_024, 3_072),
            ],
        ),
    ] {
        let dimensions: Vec<_> = [
            Qwen3B3OperatorV1::QueryProjection,
            Qwen3B3OperatorV1::KeyProjection,
            Qwen3B3OperatorV1::GateProjection,
            Qwen3B3OperatorV1::DownResidual,
        ]
        .map(|operator| {
            exact_qwen3_linear_candidate_v1(selection(
                role,
                Qwen3LinearModeV1::Decode,
                Qwen3LinearBucketV1::DecodeS1C8192,
                operator,
                0,
            ))
            .unwrap()
            .dimensions
        })
        .into_iter()
        .map(|dimensions| (dimensions.n, dimensions.k))
        .collect();
        assert_eq!(dimensions, expected);

        let logits = exact_qwen3_linear_candidate_v1(selection(
            role,
            Qwen3LinearModeV1::Decode,
            Qwen3LinearBucketV1::DecodeS1C8192,
            Qwen3B3OperatorV1::LogitsProjection,
            QWEN3_NO_LAYER_V1,
        ))
        .unwrap();
        assert_eq!(logits.dimensions.n, QWEN3_VOCABULARY_SIZE_V1);
        assert_eq!(logits.dimensions.k, role.geometry().hidden);
    }
}

#[test]
fn gemv_is_selected_only_for_the_single_flattened_row() {
    for role in [Qwen3LinearRoleV1::Target8B, Qwen3LinearRoleV1::Draft06B] {
        for (mode, bucket) in BUCKETS {
            let selection = selection(role, mode, bucket, Qwen3B3OperatorV1::KeyProjection, 0);
            let candidate = exact_qwen3_linear_candidate_v1(selection).unwrap();
            assert_eq!(
                candidate.implementation,
                if candidate.dimensions.m == 1 {
                    Qwen3LinearImplementationV1::Gemv
                } else {
                    Qwen3LinearImplementationV1::Gemm
                }
            );
            assert_eq!(
                candidate.implementation == Qwen3LinearImplementationV1::Gemv,
                bucket == Qwen3LinearBucketV1::DecodeS1C8192
            );
        }
    }
}

#[test]
fn non_authority_boundary_is_exact() {
    let selection = selection(
        Qwen3LinearRoleV1::Draft06B,
        Qwen3LinearModeV1::Decode,
        Qwen3LinearBucketV1::DecodeS1C8192,
        Qwen3B3OperatorV1::KeyProjection,
        0,
    );
    let candidate = exact_qwen3_linear_candidate_v1(selection).unwrap();
    assert_eq!(
        candidate.authority,
        Qwen3LinearAuthorityBoundaryV1 {
            attributed_source_authority: false,
            kernel_ir_authority: false,
            artifact_authority: false,
            load_authority: false,
            launch_authority: false,
            hardware_authority: false,
            performance_authority: false,
            machine_refinement: false,
        }
    );
}

#[test]
fn exact_gemv_and_gemm_match_identity_weight_oracle() {
    for (bucket, expected_rows) in [
        (Qwen3LinearBucketV1::DecodeS1C8192, 1),
        (Qwen3LinearBucketV1::SpeculativeS1K4C8192, 4),
    ] {
        let mode = if expected_rows == 1 {
            Qwen3LinearModeV1::Decode
        } else {
            Qwen3LinearModeV1::Speculative
        };
        let selection = selection(
            Qwen3LinearRoleV1::Draft06B,
            mode,
            bucket,
            Qwen3B3OperatorV1::KeyProjection,
            0,
        );
        let candidate = exact_qwen3_linear_candidate_v1(selection).unwrap();
        assert_eq!(candidate.dimensions.m, expected_rows);
        assert_eq!(
            (candidate.dimensions.n, candidate.dimensions.k),
            (1_024, 1_024)
        );
        let source = exact_identity_source(1_024, 1_024);
        let prepared = prepare_qwen_linear_weight_v1(&candidate, selection, &source).unwrap();
        let activation: Vec<_> = (0..expected_rows as usize * 1_024)
            .map(|index| bf16((index % 7) as f32 - 3.0))
            .collect();
        let initial = vec![91.0; expected_rows as usize * 1_024];
        let result = execute_qwen3_linear_reference_v1(
            &candidate,
            selection,
            &activation,
            &prepared,
            &initial,
        )
        .unwrap();
        let expected: Vec<_> = activation
            .iter()
            .map(|bits| f32::from_bits(u32::from(*bits) << 16))
            .collect();
        assert_eq!(result, expected);
    }
}

#[test]
fn residual_epilogue_is_exactly_alpha_ab_plus_c() {
    let selection = selection(
        Qwen3LinearRoleV1::Draft06B,
        Qwen3LinearModeV1::Decode,
        Qwen3LinearBucketV1::DecodeS1C8192,
        Qwen3B3OperatorV1::AttentionOutputResidual,
        0,
    );
    let candidate = exact_qwen3_linear_candidate_v1(selection).unwrap();
    assert_eq!(candidate.numerical.beta_bits, 1.0_f32.to_bits());
    assert!(candidate.numerical.residual_epilogue);
    let source = exact_identity_source(1_024, 1_024);
    let prepared = prepare_qwen_linear_weight_v1(&candidate, selection, &source).unwrap();
    let activation = vec![bf16(1.0); 1_024];
    let initial = vec![2.0; 1_024];
    let result =
        execute_qwen3_linear_reference_v1(&candidate, selection, &activation, &prepared, &initial)
            .unwrap();
    assert_eq!(result, vec![3.0; 1_024]);
}
