use fe2o3_qwen3_linear_reference_v1::linear::*;
use sha2::{Digest, Sha256};

fn selection(operator: Qwen3B3OperatorV1) -> Qwen3LinearSelectionV1 {
    Qwen3LinearSelectionV1 {
        role: Qwen3LinearRoleV1::Draft06B,
        mode: Qwen3LinearModeV1::Decode,
        bucket: Qwen3LinearBucketV1::DecodeS1C8192,
        operator,
        layer: 0,
    }
}

#[test]
fn host_identity_domains_and_shared_source_have_independent_goldens() {
    for (preimage, digest) in [
        (
            QWEN3_LINEAR_FAMILY_ID_PREIMAGE_V1,
            QWEN3_LINEAR_FAMILY_ID_V1,
        ),
        (
            QWEN3_LINEAR_CANDIDATE_SCHEMA_ID_PREIMAGE_V1,
            QWEN3_LINEAR_CANDIDATE_SCHEMA_ID_V1,
        ),
        (
            QWEN3_LINEAR_HOST_ROUTE_ID_PREIMAGE_V1,
            QWEN3_LINEAR_HOST_ROUTE_ID_V1,
        ),
    ] {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(preimage.as_bytes())),
            digest
        );
    }
    let source = include_bytes!("../../tiled_gemm_general_v1/src/reference.rs");
    assert_eq!(
        format!("{:x}", Sha256::digest(source)),
        QWEN3_LINEAR_REFERENCE_SOURCE_SHA256_V1
    );
    let candidate =
        exact_qwen3_linear_candidate_v1(selection(Qwen3B3OperatorV1::KeyProjection)).unwrap();
    // Independently encoded length-prefixed domains and little-endian B3 fields.
    assert_eq!(
        candidate.selection_identity,
        [
            0xea, 0xf7, 0x27, 0x0b, 0xf7, 0x98, 0x88, 0x98, 0x4a, 0xe1, 0x77, 0xcf, 0x41, 0xd6,
            0xe8, 0x71, 0xfb, 0xfb, 0x44, 0x5a, 0x36, 0x6b, 0x20, 0xc0, 0xcd, 0xf2, 0x60, 0xf0,
            0x8b, 0x0f, 0x17, 0xd4
        ]
    );
    assert_ne!(
        QWEN3_LINEAR_FAMILY_ID_PREIMAGE_V1,
        "fe2o3.qwen3.b3.linear.gemm_gemv.gfx942.v1"
    );
}

#[test]
fn pinned_b3_attention_shape_is_not_silently_reinterpreted() {
    for role in [Qwen3LinearRoleV1::Target8B, Qwen3LinearRoleV1::Draft06B] {
        let mut selected = selection(Qwen3B3OperatorV1::AttentionOutputResidual);
        selected.role = role;
        let candidate = exact_qwen3_linear_candidate_v1(selected).unwrap();
        assert_eq!(
            (candidate.dimensions.n, candidate.dimensions.k),
            (role.geometry().hidden, role.geometry().hidden)
        );
    }
}

#[test]
fn rectangular_query_weights_transpose_and_execute_exactly() {
    let selected = selection(Qwen3B3OperatorV1::QueryProjection);
    let candidate = exact_qwen3_linear_candidate_v1(selected).unwrap();
    assert_eq!(
        (
            candidate.dimensions.m,
            candidate.dimensions.n,
            candidate.dimensions.k
        ),
        (1, 2048, 1024)
    );
    let mut source = vec![0_u16; 2048 * 1024];
    for column in 0..2048 {
        let depth = column % 1024;
        source[column * 1024 + depth] = if column < 1024 { 0x3f80 } else { 0x4000 };
        source[column * 1024 + (depth + 1) % 1024] = 0x3f00;
    }
    let prepared = prepare_qwen_linear_weight_v1(&candidate, selected, &source).unwrap();
    for column in 0..2048 {
        for depth in 0..1024 {
            assert_eq!(
                prepared[depth * 2048 + column],
                source[column * 1024 + depth]
            );
        }
    }
    let values: Vec<f32> = (0..1024).map(|i| (i % 7) as f32 - 3.0).collect();
    let activation: Vec<_> = values.iter().map(|x| (x.to_bits() >> 16) as u16).collect();
    let result = execute_qwen3_linear_reference_v1(
        &candidate,
        selected,
        &activation,
        &prepared,
        &vec![91.0; 2048],
    )
    .unwrap();
    let expected: Vec<_> = (0..2048)
        .map(|column| {
            let depth = column % 1024;
            let coefficient = if column < 1024 { 1.0 } else { 2.0 };
            coefficient * values[depth] + 0.5 * values[(depth + 1) % 1024]
        })
        .collect();
    assert_eq!(result, expected);
}

#[test]
fn every_operand_requires_exact_extent_and_candidate_validation_precedes_storage() {
    let selected = selection(Qwen3B3OperatorV1::KeyProjection);
    let candidate = exact_qwen3_linear_candidate_v1(selected).unwrap();
    let [a, b, c] = candidate.resources.storage_elements;
    for index in 0..3 {
        for oversized in [false, true] {
            let mut lengths = [a, b, c];
            lengths[index] = if oversized {
                lengths[index] + 1
            } else {
                lengths[index] - 1
            };
            let lhs = vec![0; lengths[0]];
            let rhs = vec![0; lengths[1]];
            let initial = vec![0.0; lengths[2]];
            assert_eq!(
                execute_qwen3_linear_reference_v1(&candidate, selected, &lhs, &rhs, &initial),
                Err(Qwen3LinearErrorV1::InputExtent {
                    region: [
                        Qwen3LinearMemoryRegionV1::Activation,
                        Qwen3LinearMemoryRegionV1::PreparedWeight,
                        Qwen3LinearMemoryRegionV1::InitialOutput
                    ][index],
                    expected: [a, b, c][index],
                    actual: lengths[index],
                })
            );
        }
    }
    for actual in [b - 1, b + 1] {
        assert_eq!(
            prepare_qwen_linear_weight_v1(&candidate, selected, &vec![0; actual]),
            Err(Qwen3LinearErrorV1::WeightSourceExtent {
                expected: b,
                actual
            })
        );
    }
    let mut invalid = candidate;
    invalid.resources.storage_elements[0] = 0;
    assert_eq!(
        execute_qwen3_linear_reference_v1(&invalid, selected, &[], &[], &[]),
        Err(Qwen3LinearErrorV1::NonCanonical)
    );
    assert_eq!(
        prepare_qwen_linear_weight_v1(&invalid, selected, &[]),
        Err(Qwen3LinearErrorV1::NonCanonical)
    );
}

#[test]
fn sequential_order_and_complete_epilogue_preserve_documented_ieee_results() {
    for operator in [
        Qwen3B3OperatorV1::KeyProjection,
        Qwen3B3OperatorV1::AttentionOutputResidual,
    ] {
        let selected = selection(operator);
        let candidate = exact_qwen3_linear_candidate_v1(selected).unwrap();
        let mut activation = vec![0x3f80; 1024];
        activation[1023] = 0x4000;
        let mut weight = vec![0_u16; 1024 * 1024];
        for (depth, value) in [0x4b80, 0x3f80, 0xcb80, 0x3f80].into_iter().enumerate() {
            weight[depth * 1024] = value;
        }
        weight[1023 * 1024 + 4] = 0x7fc0;
        weight[1023 * 1024 + 5] = 0x7f80;
        weight[1023 * 1024 + 6] = 0xff80;
        weight[1023 * 1024 + 8] = 0x7f7f;
        let mut initial = vec![0.0; 1024];
        initial[1] = f32::NAN;
        initial[2] = f32::INFINITY;
        initial[3] = f32::NEG_INFINITY;
        initial[7] = -0.0;
        let result =
            execute_qwen3_linear_reference_v1(&candidate, selected, &activation, &weight, &initial)
                .unwrap();
        assert_eq!(result[0], 1.0);
        assert!(result[1].is_nan());
        if operator == Qwen3B3OperatorV1::KeyProjection {
            assert!(result[2].is_nan());
            assert!(result[3].is_nan());
        } else {
            assert_eq!(result[2], f32::INFINITY);
            assert_eq!(result[3], f32::NEG_INFINITY);
        }
        assert!(result[4].is_nan());
        assert_eq!(result[5], f32::INFINITY);
        assert_eq!(result[6], f32::NEG_INFINITY);
        assert_eq!(result[7].to_bits(), 0.0_f32.to_bits());
        assert_eq!(result[8], f32::INFINITY);
        assert!(result[9..].iter().all(|x| x.to_bits() == 0));
    }
}
