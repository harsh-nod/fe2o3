use super::*;
use crate::{
    MatrixElement, TensorCoordinateExprV1, TensorElementPackingV1, TensorFragmentLayoutV1,
    TensorInstructionProfileV1, TensorLayoutFindingV1, TensorLdsSwizzleV1, TensorMultiplicityV1,
    TensorOperandRoleV1, TensorSymbolicMapV1, TensorTailMaskV1, verify_tensor_layout_contract_v1,
};

// Literal V8 fragments, independent of the production encoders and Rust enum order.
const BF16_A: &[u8] = &[
    1, 16, 0, 16, 0, 1, 4, 1, 16, 0, 16, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 4, 0, 1, 0, 1,
    1, 1, 1,
];
const BF16_B: &[u8] = &[
    2, 16, 0, 16, 0, 1, 4, 1, 16, 0, 16, 0, 0, 0, 0, 0, 4, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1,
    1, 1, 1,
];
const ACCUMULATOR: &[u8] = &[
    3, 16, 0, 16, 0, 2, 4, 1, 16, 0, 16, 0, 0, 0, 0, 0, 4, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1,
    1, 2, 1,
];
const FP8_A: &[u8] = &[1, 16, 0, 128, 0, 3, 32, 3, 1, 4, 1];
const FP8_B: &[u8] = &[2, 128, 0, 16, 0, 3, 32, 3, 1, 4, 1];
const FP4_A: &[u8] = &[
    1, 16, 0, 128, 0, 4, 32, 1, 16, 0, 16, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 32, 0, 1, 0,
    1, 1, 5, 1,
];
const FP4_B: &[u8] = &[
    2, 128, 0, 16, 0, 4, 32, 1, 16, 0, 16, 0, 0, 0, 0, 0, 32, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0,
    1, 1, 5, 1,
];

fn canonical() -> [(TensorLayoutContractV1, Vec<u8>); 4] {
    [
        (
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            [&[1, 64, 0][..], BF16_A, BF16_B, ACCUMULATOR, &[1]].concat(),
        ),
        (
            TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64(),
            [&[4, 64, 0][..], FP8_A, FP8_B, ACCUMULATOR, &[2]].concat(),
        ),
        (
            TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
            [&[5, 64, 0][..], FP4_A, FP4_B, ACCUMULATOR, &[2]].concat(),
        ),
        (
            TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(),
            [&[6, 64, 0][..], FP4_A, FP8_B, ACCUMULATOR, &[2]].concat(),
        ),
    ]
}

fn encoded(value: TensorLayoutContractV1) -> Vec<u8> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    let mut buffer = [0xad; MAX_TENSOR_LAYOUT_LEAF_BYTES_V1];
    let length = crate::encode_tensor_layout_leaf_v1(value, &mut buffer, &mut budget).unwrap();
    assert_eq!(budget.work(), length + 2);
    assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
    assert_eq!(budget.failed_storage(), None);
    assert!(buffer[length..].iter().all(|byte| *byte == 0xad));
    buffer[..length].to_vec()
}

fn decoded(bytes: &[u8]) -> Result<TensorLayoutContractV1, KernelIrDecodeError> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    let result = crate::decode_tensor_layout_leaf_v1(bytes, &mut budget);
    if result.is_ok() {
        assert_eq!(budget.work(), bytes.len() + 2);
    }
    assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
    assert_eq!(budget.failed_storage(), None);
    result
}

fn round_trip(value: TensorLayoutContractV1) {
    let bytes = encoded(value);
    assert_eq!(decoded(&bytes).unwrap(), value);
    let mut writer = Writer::new(KERNEL_IR_VERSION_V8, None);
    encode_tensor_layout_contract_v1(&mut writer, value).unwrap();
    assert_eq!(writer.bytes, bytes);
}

#[test]
fn literal_canonical_bytes_and_independent_operand_swizzles() {
    for ((value, expected), length) in canonical().into_iter().zip([103, 59, 103, 81]) {
        assert_eq!(expected.len(), length);
        assert_eq!(encoded(value), expected);
        assert_eq!(decoded(&expected).unwrap(), value);
        round_trip(value);
        let a_end = match value.profile {
            TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64 => 13,
            _ => 35,
        };
        let b_end = length - 35;
        for (a, b) in [(true, false), (false, true), (true, true)] {
            let mut swizzled = value;
            let mut expected = expected.clone();
            if a {
                swizzled = swizzled.with_a_lds_xor4();
                expected[a_end] = 2;
            }
            if b {
                swizzled = swizzled.with_b_lds_xor4();
                expected[b_end] = 2;
            }
            assert_eq!(encoded(swizzled), expected);
            assert_eq!(decoded(&expected).unwrap(), swizzled);
        }
    }
}

fn maximal_contract() -> TensorLayoutContractV1 {
    fn fragment(role: TensorOperandRoleV1, value: u16) -> TensorFragmentLayoutV1 {
        TensorFragmentLayoutV1 {
            role,
            shape: [value, u16::MAX],
            element: MatrixElement::Fp4E2M1,
            fragment_elements: 255,
            mapping: TensorSymbolicMapV1::LaneComponentAffine {
                lane_modulus: 0,
                lane_divisor: u16::MAX,
                axes: [
                    TensorCoordinateExprV1 {
                        constant: value,
                        lane_mod_scale: 0x1234,
                        lane_div_scale: 0x5678,
                        component_scale: 0x9abc,
                        tile_origin: false,
                    },
                    TensorCoordinateExprV1 {
                        constant: u16::MAX,
                        lane_mod_scale: 0,
                        lane_div_scale: 1,
                        component_scale: value,
                        tile_origin: true,
                    },
                ],
            },
            multiplicity: TensorMultiplicityV1::Broadcast { factor: 0 },
            packing: TensorElementPackingV1::Unsupported(255),
            lds_swizzle: TensorLdsSwizzleV1::Unsupported(0),
        }
    }
    TensorLayoutContractV1 {
        profile: TensorInstructionProfileV1::Opaque(0x1234_abcd),
        subgroup_width: u16::MAX,
        a: fragment(TensorOperandRoleV1::A, 0x0123),
        b: fragment(TensorOperandRoleV1::B, 0x4567),
        accumulator: fragment(TensorOperandRoleV1::Accumulator, 0x89ab),
        tail_mask: TensorTailMaskV1::Unsupported(255),
    }
}

#[test]
fn full_schema_bound_is_reached_without_normalizing_any_fields() {
    let value = maximal_contract();
    let expected: [u8; 117] = [
        3, 0xcd, 0xab, 0x34, 0x12, 255, 255, 1, 0x23, 0x01, 255, 255, 4, 255, 1, 0, 0, 255, 255,
        0x23, 0x01, 0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0, 255, 255, 0, 0, 1, 0, 0x23, 0x01, 1, 2,
        0, 3, 255, 3, 0, 2, 0x67, 0x45, 255, 255, 4, 255, 1, 0, 0, 255, 255, 0x67, 0x45, 0x34,
        0x12, 0x78, 0x56, 0xbc, 0x9a, 0, 255, 255, 0, 0, 1, 0, 0x67, 0x45, 1, 2, 0, 3, 255, 3, 0,
        3, 0xab, 0x89, 255, 255, 4, 255, 1, 0, 0, 255, 255, 0xab, 0x89, 0x34, 0x12, 0x78, 0x56,
        0xbc, 0x9a, 0, 255, 255, 0, 0, 1, 0, 0xab, 0x89, 1, 2, 0, 3, 255, 3, 0, 5, 255,
    ];
    assert_eq!(encoded(value), expected);
    assert_eq!(decoded(&expected).unwrap(), value);
    round_trip(value);
}

#[test]
fn every_choice_and_payload_boundary_remains_distinct_syntax() {
    let base = canonical()[0].0;
    for profile in [
        TensorInstructionProfileV1::IncompatibleWave32,
        TensorInstructionProfileV1::Opaque(0),
        TensorInstructionProfileV1::Opaque(u32::MAX),
    ] {
        round_trip(TensorLayoutContractV1 { profile, ..base });
    }
    for tail_mask in [
        TensorTailMaskV1::ExactPhysicalTile,
        TensorTailMaskV1::ZeroFilledPredicateInputs,
        TensorTailMaskV1::PredicateMask,
        TensorTailMaskV1::Missing,
        TensorTailMaskV1::Unsupported(0),
        TensorTailMaskV1::Unsupported(255),
    ] {
        round_trip(TensorLayoutContractV1 { tail_mask, ..base });
    }
    for subgroup_width in [0, u16::MAX] {
        round_trip(TensorLayoutContractV1 {
            subgroup_width,
            ..base
        });
    }
    let mut variants = Vec::new();
    for role in [
        TensorOperandRoleV1::A,
        TensorOperandRoleV1::B,
        TensorOperandRoleV1::Accumulator,
    ] {
        variants.push(TensorFragmentLayoutV1 { role, ..base.a });
    }
    for element in [
        MatrixElement::Bf16,
        MatrixElement::F32,
        MatrixElement::Fp8E4M3,
        MatrixElement::Fp4E2M1,
    ] {
        variants.push(TensorFragmentLayoutV1 { element, ..base.a });
    }
    for mapping in [
        TensorSymbolicMapV1::Opaque(0),
        TensorSymbolicMapV1::Opaque(u32::MAX),
        TensorSymbolicMapV1::Gfx950Fp8M16N16K128SplitK,
        TensorSymbolicMapV1::LaneComponentAffine {
            lane_modulus: 0,
            lane_divisor: 0,
            axes: [TensorCoordinateExprV1::new(0, 0, 0); 2],
        },
        maximal_contract().a.mapping,
    ] {
        variants.push(TensorFragmentLayoutV1 { mapping, ..base.a });
    }
    for factor in [0, 1, 255] {
        variants.push(TensorFragmentLayoutV1 {
            multiplicity: TensorMultiplicityV1::Broadcast { factor },
            ..base.a
        });
    }
    for packing in [
        TensorElementPackingV1::Bf16PairInI32,
        TensorElementPackingV1::F32Scalar,
        TensorElementPackingV1::Fp8FourInI32,
        TensorElementPackingV1::Fp4EightInI32,
        TensorElementPackingV1::Unsupported(0),
        TensorElementPackingV1::Unsupported(255),
    ] {
        variants.push(TensorFragmentLayoutV1 { packing, ..base.a });
    }
    for lds_swizzle in [
        TensorLdsSwizzleV1::None,
        TensorLdsSwizzleV1::Xor4,
        TensorLdsSwizzleV1::Unsupported(0),
        TensorLdsSwizzleV1::Unsupported(255),
    ] {
        variants.push(TensorFragmentLayoutV1 {
            lds_swizzle,
            ..base.a
        });
    }
    for shape in [[0, u16::MAX], [u16::MAX, 0]] {
        variants.push(TensorFragmentLayoutV1 { shape, ..base.a });
    }
    for fragment_elements in [0, 255] {
        variants.push(TensorFragmentLayoutV1 {
            fragment_elements,
            ..base.a
        });
    }
    for fragment in variants {
        round_trip(TensorLayoutContractV1 {
            a: fragment,
            ..base
        });
        round_trip(TensorLayoutContractV1 {
            b: fragment,
            ..base
        });
        round_trip(TensorLayoutContractV1 {
            accumulator: fragment,
            ..base
        });
    }
}

#[test]
fn every_truncated_prefix_and_trailing_input_is_rejected() {
    let mut cases = canonical()
        .into_iter()
        .map(|(_, bytes)| bytes)
        .collect::<Vec<_>>();
    cases.push(encoded(maximal_contract()));
    for bytes in cases {
        for length in 0..bytes.len() {
            assert_eq!(
                decoded(&bytes[..length]),
                Err(KernelIrDecodeError::Truncated)
            );
        }
        for suffix in [vec![0], bytes.clone()] {
            let mut trailing = bytes.clone();
            trailing.extend_from_slice(&suffix);
            let expected = if trailing.len() > MAX_TENSOR_LAYOUT_LEAF_BYTES_V1 {
                KernelIrDecodeError::LimitExceeded {
                    field: "tensor layout leaf bytes",
                    actual: trailing.len(),
                    max: MAX_TENSOR_LAYOUT_LEAF_BYTES_V1,
                }
            } else {
                KernelIrDecodeError::TrailingBytes
            };
            assert_eq!(decoded(&trailing), Err(expected));
        }
    }
}

#[test]
fn unknown_tags_and_non_boolean_flags_never_become_opaque_or_true() {
    let bytes = canonical()[0].1.clone();
    let mut fields = vec![
        (0, "tensor instruction profile"),
        (102, "tensor tail-mask contract"),
    ];
    for base in [3, 36, 69] {
        fields.extend([
            (base, "tensor operand role"),
            (base + 5, "matrix element"),
            (base + 7, "tensor symbolic map"),
            (base + 30, "tensor multiplicity"),
            (base + 31, "tensor element packing"),
            (base + 32, "tensor LDS storage transform"),
        ]);
    }
    for (offset, kind) in fields {
        for tag in [0, 255] {
            let mut invalid = bytes.clone();
            invalid[offset] = tag;
            assert_eq!(
                decoded(&invalid),
                Err(KernelIrDecodeError::UnknownTag { kind, tag })
            );
        }
    }
    for base in [3, 36, 69] {
        for offset in [base + 20, base + 29] {
            for tag in [2, 255] {
                let mut invalid = bytes.clone();
                invalid[offset] = tag;
                assert_eq!(
                    decoded(&invalid),
                    Err(KernelIrDecodeError::UnknownTag {
                        kind: "tensor tile-origin flag",
                        tag,
                    })
                );
            }
        }
    }
}

#[test]
fn every_work_quota_preserves_exact_accepted_prefixes_and_storage_history() {
    let mut cases = canonical()
        .into_iter()
        .map(|(value, _)| value)
        .collect::<Vec<_>>();
    cases.push(maximal_contract());
    for value in cases {
        let bytes = encoded(value);
        for quota in 0..=bytes.len() + 2 {
            let mut encoded_work = CanonicalKernelIrWorkBudgetV1::new(5 + quota);
            let mut decoded_work = CanonicalKernelIrWorkBudgetV1::new(5 + quota);
            for work in [&mut encoded_work, &mut decoded_work] {
                work.charge_work(5).unwrap();
                assert!(work.charge_work(quota + 99).is_err());
            }
            let mut encode_budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut encoded_work, 7);
            let mut decode_budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut decoded_work, 7);
            for budget in [&mut encode_budget, &mut decode_budget] {
                budget.reserve_storage(7).unwrap();
                assert!(budget.reserve_storage(1).is_err());
            }
            let identity = encode_budget.work_ledger_identity_v1();
            let mut buffer = [0xad; MAX_TENSOR_LAYOUT_LEAF_BYTES_V1];
            let encode = encode_tensor_layout_leaf_v1(value, &mut buffer, &mut encode_budget);
            let decode = decode_tensor_layout_leaf_v1(&bytes, &mut decode_budget);
            assert!(identity == encode_budget.work_ledger_identity_v1());
            assert_eq!(encode_budget.work(), decode_budget.work());
            let success = quota == bytes.len() + 2;
            if success {
                assert_eq!(encode, Ok(bytes.len()));
                assert_eq!(decode, Ok(value));
            } else {
                let Err(KernelIrEncodeError::WorkLimit(encode_error)) = encode else {
                    panic!("expected encoder work refusal at quota {quota}");
                };
                let Err(KernelIrDecodeError::WorkLimit(decode_error)) = decode else {
                    panic!("expected decoder work refusal at quota {quota}");
                };
                assert_eq!(encode_error, decode_error);
                assert!(encode_error.actual() > 5 + quota);
            }
            let written = (encode_budget.work() - 5)
                .saturating_sub(1)
                .min(bytes.len());
            assert_eq!(&buffer[..written], &bytes[..written]);
            assert!(buffer[written..].iter().all(|byte| *byte == 0xad));
            for budget in [encode_budget, decode_budget] {
                assert_eq!((budget.storage(), budget.peak_storage()), (7, 7));
                assert_eq!(budget.failed_storage(), Some(8));
            }
            assert_eq!(encoded_work.failed_work(), Some(5 + quota + 99));
            assert_eq!(decoded_work.failed_work(), Some(5 + quota + 99));
        }
    }
}

#[test]
fn oversized_input_and_entry_overflow_are_bounded_before_payload_traversal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert_eq!(
        decode_tensor_layout_leaf_v1(&[0; 118], &mut budget),
        Err(KernelIrDecodeError::LimitExceeded {
            field: "tensor layout leaf bytes",
            actual: 118,
            max: 117,
        })
    );
    assert_eq!(budget.work(), 1);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    work.charge_work(usize::MAX).unwrap();
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 3);
    budget.reserve_storage(3).unwrap();
    let mut buffer = [0xad; MAX_TENSOR_LAYOUT_LEAF_BYTES_V1];
    assert!(matches!(
        encode_tensor_layout_leaf_v1(maximal_contract(), &mut buffer, &mut budget),
        Err(KernelIrEncodeError::WorkLimit(_))
    ));
    assert_eq!(buffer, [0xad; MAX_TENSOR_LAYOUT_LEAF_BYTES_V1]);
    assert!(matches!(
        decode_tensor_layout_leaf_v1(&buffer, &mut budget),
        Err(KernelIrDecodeError::WorkLimit(_))
    ));
    assert_eq!((budget.work(), budget.storage()), (usize::MAX, 3));
}

#[test]
fn syntax_transport_cannot_remove_independent_semantic_refusals() {
    let base = canonical()[0].0;
    let mut cases = vec![
        (
            TensorLayoutContractV1 {
                profile: TensorInstructionProfileV1::Opaque(7),
                ..base
            },
            TensorLayoutFindingV1::UnsupportedProfile,
        ),
        (
            TensorLayoutContractV1 {
                profile: TensorInstructionProfileV1::IncompatibleWave32,
                ..base
            },
            TensorLayoutFindingV1::ProfileMismatch {
                field: "wave32 target profile",
            },
        ),
        (
            TensorLayoutContractV1 {
                tail_mask: TensorTailMaskV1::Missing,
                ..base
            },
            TensorLayoutFindingV1::TailMaskMismatch,
        ),
    ];
    let mut opaque_map = base;
    opaque_map.a.mapping = TensorSymbolicMapV1::Opaque(7);
    cases.push((
        opaque_map,
        TensorLayoutFindingV1::UnsupportedSymbolicMap {
            role: TensorOperandRoleV1::A,
        },
    ));
    let mut unsupported_packing = base;
    unsupported_packing.b.packing = TensorElementPackingV1::Unsupported(255);
    cases.push((
        unsupported_packing,
        TensorLayoutFindingV1::PackingMismatch {
            role: TensorOperandRoleV1::B,
        },
    ));
    for (value, finding) in cases {
        let recovered = decoded(&encoded(value)).unwrap();
        assert_eq!(recovered, value);
        assert_eq!(
            verify_tensor_layout_contract_v1(&recovered),
            verify_tensor_layout_contract_v1(&value)
        );
        assert!(verify_tensor_layout_contract_v1(&recovered).contains(&finding));
    }
}

#[test]
fn v7_and_v8_gates_are_unchanged_for_every_low_precision_leaf_feature() {
    use super::super::KERNEL_IR_VERSION_V7;
    let (base, bytes) = canonical()[0].clone();
    let mut writer = Writer::new(KERNEL_IR_VERSION_V7, None);
    encode_tensor_layout_contract_v1(&mut writer, base).unwrap();
    assert_eq!(writer.bytes, bytes);
    let mut reader = Reader::new(&bytes, None);
    reader.version = KERNEL_IR_VERSION_V7;
    assert_eq!(decode_tensor_layout_contract_v1(&mut reader).unwrap(), base);
    assert!(reader.is_finished());

    let mut v8_only = Vec::new();
    for (profile, feature, tag) in [
        (
            TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64,
            "gfx950 FP8 tensor instruction profile",
            4,
        ),
        (
            TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64,
            "gfx950 FP4 tensor instruction profile",
            5,
        ),
        (
            TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64,
            "gfx950 mixed FP4-by-FP8 tensor instruction profile",
            6,
        ),
    ] {
        v8_only.push((
            TensorLayoutContractV1 { profile, ..base },
            feature,
            "tensor instruction profile",
            tag,
            0,
            1,
        ));
    }
    for (element, tag) in [(MatrixElement::Fp8E4M3, 3), (MatrixElement::Fp4E2M1, 4)] {
        let mut value = base;
        value.a.element = element;
        v8_only.push((
            value,
            "gfx950 low-precision matrix element",
            "matrix element",
            tag,
            8,
            9,
        ));
    }
    for (packing, feature, tag) in [
        (
            TensorElementPackingV1::Fp8FourInI32,
            "gfx950 FP8 tensor packing",
            4,
        ),
        (
            TensorElementPackingV1::Fp4EightInI32,
            "gfx950 FP4 tensor packing",
            5,
        ),
    ] {
        let mut value = base;
        value.b.packing = packing;
        v8_only.push((value, feature, "tensor element packing", tag, 67, 68));
    }
    let mut value = base;
    value.a.mapping = TensorSymbolicMapV1::Gfx950Fp8M16N16K128SplitK;
    v8_only.push((
        value,
        "gfx950 FP8 split-K tensor map",
        "tensor symbolic map",
        3,
        10,
        11,
    ));
    for (value, feature, kind, tag, encoded_prefix, decoded_prefix) in v8_only {
        let bytes = encoded(value);
        let mut writer = Writer::new(KERNEL_IR_VERSION_V7, None);
        assert_eq!(
            encode_tensor_layout_contract_v1(&mut writer, value),
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version: 7,
                feature
            })
        );
        assert_eq!(writer.bytes.len(), encoded_prefix);
        let mut reader = Reader::new(&bytes, None);
        reader.version = KERNEL_IR_VERSION_V7;
        assert_eq!(
            decode_tensor_layout_contract_v1(&mut reader),
            Err(KernelIrDecodeError::UnknownTag { kind, tag })
        );
        assert_eq!(reader.offset, decoded_prefix);
        round_trip(value);
    }
}

#[test]
fn literal_primitive_and_completion_quota_checkpoints() {
    let bf16 = canonical()[0].0;
    for (value, quota, accepted, attempted, written) in [
        (bf16, 3, 7, 9, 1),
        (maximal_contract(), 5, 7, 11, 1),
        (bf16, 104, 109, 110, 103),
    ] {
        let bytes = encoded(value);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + quota);
        work.charge_work(5).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let mut buffer = [0xad; MAX_TENSOR_LAYOUT_LEAF_BYTES_V1];
        let error = encode_tensor_layout_leaf_v1(value, &mut buffer, &mut budget).unwrap_err();
        assert!(matches!(error, KernelIrEncodeError::WorkLimit(error)
            if error.actual() == attempted && error.limit() == 5 + quota));
        assert_eq!(budget.work(), accepted);
        assert_eq!(&buffer[..written], &bytes[..written]);
        assert_eq!(
            &buffer[written..],
            &[0xad; MAX_TENSOR_LAYOUT_LEAF_BYTES_V1][written..]
        );

        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + quota);
        work.charge_work(5).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let error = decode_tensor_layout_leaf_v1(&bytes, &mut budget).unwrap_err();
        assert!(matches!(error, KernelIrDecodeError::WorkLimit(error)
            if error.actual() == attempted && error.limit() == 5 + quota));
        assert_eq!(budget.work(), accepted);
    }
}

#[test]
fn existing_count_materialize_and_compare_work_remain_exact() {
    use super::super::WriterModeV1;
    let mut cases = canonical()
        .into_iter()
        .map(|(value, _)| value)
        .collect::<Vec<_>>();
    cases.push(maximal_contract());
    // Per canonical affine fragment: five fixed, thirteen map, three choice fields.
    // Split-K replaces thirteen map fields with one; the maximal fragment has
    // three additional choice payloads and the profile/tail each add a payload.
    for (value, tokens) in cases.into_iter().zip([66, 42, 66, 54, 77]) {
        let bytes = encoded(value);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(tokens);
        let mut counter = Writer::counter(KERNEL_IR_VERSION_V8, &mut work);
        encode_tensor_layout_contract_v1(&mut counter, value).unwrap();
        assert_eq!(counter.length(), bytes.len());
        assert_eq!(
            (counter.bytes.capacity(), counter.peak_auxiliary_bytes),
            (0, 0)
        );
        drop(counter);
        assert_eq!(work.work(), tokens);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(bytes.len());
        let mut materializer = Writer::new(KERNEL_IR_VERSION_V8, Some(&mut work));
        encode_tensor_layout_contract_v1(&mut materializer, value).unwrap();
        assert_eq!(materializer.bytes, bytes);
        drop(materializer);
        assert_eq!(work.work(), bytes.len());

        for matches in [true, false] {
            let mut expected = bytes.clone();
            if !matches {
                expected[0] ^= 1;
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(bytes.len() + tokens);
            let mut comparing = Writer::comparing(KERNEL_IR_VERSION_V8, &expected, Some(&mut work));
            encode_tensor_layout_contract_v1(&mut comparing, value).unwrap();
            assert_eq!(comparing.length(), bytes.len());
            assert_eq!(
                (comparing.bytes.capacity(), comparing.peak_auxiliary_bytes),
                (0, 0)
            );
            assert!(
                matches!(comparing.mode, WriterModeV1::Compare { matches: actual, .. } if actual == matches)
            );
            drop(comparing);
            assert_eq!(work.work(), bytes.len() + tokens);
        }
    }
}
