//! Inert shape and frozen-codec tests, not authenticated source admission.

use super::*;

const KINDS: [SemanticGfx942InlineInstructionV30; 6] = [
    SemanticGfx942InlineInstructionV30::VMovB32,
    SemanticGfx942InlineInstructionV30::VAddU32,
    SemanticGfx942InlineInstructionV30::VSubU32,
    SemanticGfx942InlineInstructionV30::VAndB32,
    SemanticGfx942InlineInstructionV30::VOrB32,
    SemanticGfx942InlineInstructionV30::VXorB32,
];

fn source(function: SemanticFunctionIdentityV1) -> SemanticInlineAssemblySourceV30 {
    SemanticInlineAssemblySourceV30::new([0xa1; 32], function, [0xa3; 32], [0xa4; 32]).unwrap()
}

fn request(kind: SemanticGfx942InlineInstructionV30) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let abi = &request.functions[0].abi;
    let intrinsic_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(20)),
        SemanticLayoutIdentityV1(identity(21)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![abi.arguments()[0].value().clone(); kind.input_count()],
        abi.return_value().clone(),
    )
    .unwrap();
    let mut callables = request.callables.into_vec();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1(identity(22)),
            SemanticItemDefinitionIdentityV1(identity(23)),
            SemanticMonomorphizationIdentityV1(identity(24)),
            SemanticGenericTypeArgumentsIdentityV1(identity(25)),
            SemanticConstGenericArgumentsIdentityV1(identity(26)),
            SemanticSourceProvenanceV1::unavailable(),
            intrinsic_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
            SemanticGfx942InlineU32V30::new(kind, 1).unwrap(),
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(0xa7)),
    });
    request.callables = callables.into_boxed_slice();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(1),
        vec![
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1(1), vec![], SemanticTypeIdV1(0),).unwrap()
            );
            kind.input_count()
        ],
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1(0), vec![], SemanticTypeIdV1(0)).unwrap(),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
    .with_inline_assembly_source_v30(source(request.functions[0].identity));
    request.functions[0].blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(30)),
            SemanticSourceProvenanceV1::unavailable(),
            vec![],
            SemanticTerminatorV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticTerminatorKindV1::Call(call),
            ),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(31)),
            SemanticSourceProvenanceV1::unavailable(),
            vec![],
            SemanticTerminatorV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticTerminatorKindV1::Return,
            ),
        )
        .unwrap(),
    ]
    .into_boxed_slice();
    request
}

fn call_mut(request: &mut InertSemanticMirRequestV1) -> &mut SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = &mut request.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    call
}

fn unique_offset(bytes: &[u8], needle: &[u8]) -> usize {
    let offsets: Vec<_> = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, value)| (value == needle).then_some(index))
        .collect();
    assert_eq!(offsets.len(), 1);
    offsets[0]
}

#[test]
fn all_six_v34_documents_round_trip_without_changing_instruction_or_occurrence() {
    for (tag, kind) in KINDS.into_iter().enumerate() {
        let request = request(kind);
        let original = request.clone();
        let admitted = request
            .admit_exact_v34(SemanticMirLimitsV1::default())
            .unwrap();
        assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V34);
        assert_eq!(
            minimum_wire_version(&original),
            SemanticMirWireVersionV1::V34
        );
        let bytes = admitted.canonical_encoding();
        let operation_offset = unique_offset(bytes, &[0xa7; 32]) - 4;
        assert_eq!(
            &bytes[operation_offset..operation_offset + 4],
            &[91, tag as u8, 1, 0]
        );
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
            bytes,
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.request, original);
        assert_eq!(decoded.canonical_encoding(), bytes);
        assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
        assert_eq!(
            original
                .clone()
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap()
                .canonical_encoding(),
            bytes
        );
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                bytes,
                SemanticMirLimitsV1::default()
            )
            .unwrap()
            .canonical_encoding(),
            bytes
        );
        assert_eq!(
            original
                .admit(SemanticMirLimitsV1::default())
                .unwrap()
                .canonical_encoding(),
            bytes
        );
        let SemanticTerminatorKindV1::Call(call) =
            decoded.functions()[0].blocks()[0].terminator().kind()
        else {
            unreachable!()
        };
        assert_eq!(
            call.inline_assembly_source_v30(),
            Some(source(decoded.functions()[0].identity()))
        );
    }
}

#[test]
fn legacy_scalar_siblings_keep_tag_87_while_standalone_uses_tag_91() {
    for (subtag, kind) in KINDS.into_iter().enumerate() {
        for (version, tag) in [
            (SemanticMirWireVersionV1::V31, 87),
            (SemanticMirWireVersionV1::V32, 87),
            (SemanticMirWireVersionV1::V34, 91),
        ] {
            let original = request(kind);
            let owner = original
                .clone()
                .admit_for_wire_version(version, SemanticMirLimitsV1::default())
                .unwrap();
            let bytes = owner.canonical_encoding();
            let offset = unique_offset(bytes, &[0xa7; 32]) - 4;
            assert_eq!(&bytes[offset..offset + 4], &[tag, subtag as u8, 1, 0]);
            let decoded = AdmittedInertSemanticMirV1::decode_with_policy(
                bytes,
                SemanticMirLimitsV1::default(),
                CanonicalDecodePolicyV1::Exact(version),
            )
            .unwrap();
            assert_eq!(decoded.request, original);
            assert_eq!(decoded.canonical_encoding(), bytes);
            let occurrence = unique_offset(bytes, &[0xa1; 32]);
            assert_eq!(bytes[occurrence - 1], 1);
            if version != SemanticMirWireVersionV1::V34 {
                // Scalar source identities retain the additional absent sibling option.
                assert_eq!(bytes[occurrence + 128], 0);
            }
        }
    }
}

#[test]
fn former_standalone_v30_bytes_are_not_reinterpreted_as_published_saturation() {
    for kind in KINDS {
        let owner = request(kind)
            .admit_exact_v34(SemanticMirLimitsV1::default())
            .unwrap();
        let mut former = owner.canonical_encoding().to_vec();
        let offset = unique_offset(&former, &[0xa7; 32]) - 4;
        former[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&30_u16.to_le_bytes());
        former[offset] = 87;
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v30_canonical(
                &former,
                SemanticMirLimitsV1::default()
            )
            .is_err()
        );
        assert!(
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                &former,
                SemanticMirLimitsV1::default()
            )
            .is_err()
        );
    }
}

#[test]
fn scalar_authoring_rejects_hidden_saturation_and_wave64_without_ordinal_promotion() {
    for (operation, required) in [
        (
            SemanticCompilerIntrinsicOperationV1::SaturatingInteger(
                SemanticSaturatingIntegerOpV1::Add,
            ),
            SemanticMirWireVersionV1::V30,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::SaturatingInteger(
                SemanticSaturatingIntegerOpV1::Subtract,
            ),
            SemanticMirWireVersionV1::V30,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex {
                context: SemanticTypeIdV1(0),
                element: SemanticTypeIdV1(0),
            },
            SemanticMirWireVersionV1::V33,
        ),
    ] {
        let mut fixture = request(KINDS[1]);
        let mut callables = fixture.callables.to_vec();
        let mut hidden = callables[1].clone();
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: hidden_operation,
            ..
        } = &mut hidden
        else {
            unreachable!()
        };
        *hidden_operation = operation;
        callables.push(hidden);
        fixture.callables = callables.into_boxed_slice();
        for version in [
            SemanticMirWireVersionV1::V31,
            SemanticMirWireVersionV1::V32,
            SemanticMirWireVersionV1::V34,
        ] {
            assert_eq!(
                fixture
                    .clone()
                    .admit_for_wire_version(version, SemanticMirLimitsV1::default())
                    .unwrap_err(),
                SemanticMirErrorV1::WireVersionCannotRepresent {
                    requested: version,
                    required
                }
            );
            let mut writer = CanonicalWriterV1::new(0);
            assert_eq!(
                encode_compiler_intrinsic_operation(&mut writer, operation, version),
                Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                    requested: version,
                    required
                })
            );
            assert!(writer.finish().is_empty());
        }
        assert_eq!(
            fixture
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V34,
                required
            }
        );
    }
}

#[test]
fn extension_intrinsic_allocations_have_exact_version_membership() {
    for version in [
        SemanticMirWireVersionV1::V30,
        SemanticMirWireVersionV1::V31,
        SemanticMirWireVersionV1::V32,
        SemanticMirWireVersionV1::V33,
        SemanticMirWireVersionV1::V34,
    ] {
        for tag in 69..=255 {
            let admitted = match (version, tag) {
                (SemanticMirWireVersionV1::V30, 87)
                | (SemanticMirWireVersionV1::V31, 87 | 88)
                | (SemanticMirWireVersionV1::V32, 87 | 89)
                | (SemanticMirWireVersionV1::V33, 87 | 90)
                | (SemanticMirWireVersionV1::V34, 91) => true,
                _ => false,
            };
            if admitted {
                continue;
            }
            let bytes = [tag];
            let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
            decoder.wire_version = version;
            assert!(
                matches!(decoder.compiler_intrinsic(),
                Err(SemanticMirDecodeErrorV1::InvalidTag { context: "compiler intrinsic", value, .. }) if value == tag),
                "version={version:?}, tag={tag}"
            );
        }
    }
    for version in [SemanticMirWireVersionV1::V30, SemanticMirWireVersionV1::V33] {
        for (subtag, expected) in [
            (0, SemanticSaturatingIntegerOpV1::Add),
            (1, SemanticSaturatingIntegerOpV1::Subtract),
        ] {
            let bytes = [87, subtag];
            let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
            decoder.wire_version = version;
            assert_eq!(
                decoder.compiler_intrinsic().unwrap(),
                SemanticCompilerIntrinsicOperationV1::SaturatingInteger(expected)
            );
            decoder.finish().unwrap();
        }
    }
}

#[test]
fn call_tails_cannot_be_added_removed_or_reinterpreted_across_sibling_schemas() {
    let ordinary = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1(7),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let base = vec![2, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
    for (version, tails) in [
        (SemanticMirWireVersionV1::V30, 0),
        (SemanticMirWireVersionV1::V31, 2),
        (SemanticMirWireVersionV1::V32, 2),
        (SemanticMirWireVersionV1::V33, 0),
        (SemanticMirWireVersionV1::V34, 1),
    ] {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_terminator(&mut writer, &ordinary, version).unwrap();
        let mut expected = base.clone();
        expected.extend(std::iter::repeat_n(0, tails));
        assert_eq!(writer.finish(), expected);
        for count in 0..=3 {
            let mut bytes = base.clone();
            bytes.extend(std::iter::repeat_n(0, count));
            let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
            decoder.wire_version = version;
            let result = decoder.terminator().and_then(|_| decoder.finish());
            assert_eq!(
                result.is_ok(),
                count == tails,
                "version={version:?}, tails={count}"
            );
        }
    }
}

#[test]
fn v34_rejects_unknown_instruction_options_and_every_execution_opcode() {
    for options in [0, 2, 3, 4, 5, 9, 17, 0xffff] {
        assert_eq!(
            SemanticGfx942InlineU32V30::new(KINDS[0], options),
            Err(SemanticMirErrorV1::InvalidInlineAssemblyV30)
        );
    }
    let admitted = request(KINDS[1])
        .admit_exact_v34(SemanticMirLimitsV1::default())
        .unwrap();
    let bytes = admitted.canonical_encoding();
    let offset = unique_offset(bytes, &[0xa7; 32]) - 4;
    for kind in [6, 87, 255] {
        let mut altered = bytes.to_vec();
        altered[offset + 1] = kind;
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
                &altered,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "gfx942 inline instruction",
                ..
            })
        ));
    }
    for tag in 69..=90 {
        let mut altered = bytes.to_vec();
        altered[offset] = tag;
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
                &altered,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "compiler intrinsic",
                ..
            })
        ));
    }
    for options in [0_u16, 2, 5, 0xffff] {
        let mut altered = bytes.to_vec();
        altered[offset + 2..offset + 4].copy_from_slice(&options.to_le_bytes());
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
                &altered,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidInlineAssemblyV30
            ))
        ));
    }
}

#[test]
fn source_record_is_required_only_for_isa_and_binds_the_enclosing_caller() {
    let mut missing = request(KINDS[0]);
    call_mut(&mut missing).inline_assembly_source_v30 = None;
    assert_eq!(
        missing
            .admit_exact_v34(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidInlineAssemblyV30
    );
    let mut substituted = request(KINDS[0]);
    call_mut(&mut substituted).inline_assembly_source_v30 =
        Some(source(SemanticFunctionIdentityV1(identity(99))));
    assert_eq!(
        substituted
            .admit_exact_v34(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidInlineAssemblyV30
    );
    let mut ordinary = request(KINDS[0]);
    call_mut(&mut ordinary).callee = SemanticCallableIdV1(0);
    ordinary.callables =
        vec![SemanticCallableDeclV1::defined(SemanticFunctionIdV1(0))].into_boxed_slice();
    assert_eq!(
        ordinary
            .admit_exact_v34(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidInlineAssemblyV30
    );

    let admitted = request(KINDS[0])
        .admit_exact_v34(SemanticMirLimitsV1::default())
        .unwrap();
    let bytes = admitted.canonical_encoding();
    let start = unique_offset(bytes, &[0xa1; 32]);
    for option_tag in [2, 255] {
        let mut altered = bytes.to_vec();
        altered[start - 1] = option_tag;
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
                &altered,
                SemanticMirLimitsV1::default(),
            ),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "inline assembly source",
                ..
            })
        ));
    }
    for field in 0..4 {
        let mut altered = bytes.to_vec();
        altered[start + field * 32..start + (field + 1) * 32].fill(0);
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
                &altered,
                SemanticMirLimitsV1::default()
            ),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidInlineAssemblyV30
            ))
        ));
    }
    let mut wrong_caller = bytes.to_vec();
    wrong_caller[start + 32..start + 64].fill(99);
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
            &wrong_caller,
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
    // Inert schema admission intentionally does not authenticate these references.
    let mut different_reference = bytes.to_vec();
    different_reference[start..start + 32].fill(0xb1);
    let changed = AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
        &different_reference,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_ne!(changed.semantic_sha256(), admitted.semantic_sha256());
}

#[test]
fn frozen_schemas_never_reinterpret_v34_opcodes_or_source_fields() {
    for version in [
        SemanticMirWireVersionV1::V2,
        SemanticMirWireVersionV1::V3,
        SemanticMirWireVersionV1::V4,
        SemanticMirWireVersionV1::V5,
        SemanticMirWireVersionV1::V6,
        SemanticMirWireVersionV1::V7,
        SemanticMirWireVersionV1::V8,
        SemanticMirWireVersionV1::V9,
        SemanticMirWireVersionV1::V10,
        SemanticMirWireVersionV1::V11,
        SemanticMirWireVersionV1::V12,
        SemanticMirWireVersionV1::V13,
        SemanticMirWireVersionV1::V14,
        SemanticMirWireVersionV1::V15,
        SemanticMirWireVersionV1::V28,
        SemanticMirWireVersionV1::V29,
        SemanticMirWireVersionV1::V30,
        SemanticMirWireVersionV1::V33,
    ] {
        assert!(matches!(
            request(KINDS[0]).admit_for_wire_version(version, SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V34,
                ..
            })
        ));
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
                    SemanticGfx942InlineU32V30::new(KINDS[0], 1).unwrap()
                ),
                version
            )
            .is_err()
        );
        let mut fixture = request(KINDS[0]);
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(
            encode_terminator(
                &mut writer,
                &SemanticTerminatorKindV1::Call(call_mut(&mut fixture).clone()),
                version
            )
            .is_err()
        );
        let mut decoder = CanonicalDecoderV1::new(&[91, 0, 1, 0], SemanticMirLimitsV1::default());
        decoder.wire_version = version;
        assert!(decoder.compiler_intrinsic().is_err());
        // A legacy call parser must leave the new source field unconsumed,
        // never accept it as a legacy extension to the same component.
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_terminator(
            &mut writer,
            &SemanticTerminatorKindV1::Call(call_mut(&mut fixture).clone()),
            SemanticMirWireVersionV1::V34,
        )
        .unwrap();
        let encoded = writer.finish();
        let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
        decoder.wire_version = version;
        let SemanticTerminatorKindV1::Call(decoded) = decoder.terminator().unwrap() else {
            unreachable!()
        };
        assert_eq!(decoded.inline_assembly_source_v30(), None);
        assert!(decoder.finish().is_err());
    }

    // The unchanged legacy call encoding is frozen independently of the V34 encoder.
    let ordinary = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1(7),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let expected = vec![2, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
    for version in [
        SemanticMirWireVersionV1::V15,
        SemanticMirWireVersionV1::V28,
        SemanticMirWireVersionV1::V29,
        SemanticMirWireVersionV1::V30,
        SemanticMirWireVersionV1::V33,
    ] {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_terminator(&mut writer, &ordinary, version).unwrap();
        assert_eq!(writer.finish(), expected);
    }
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_terminator(&mut writer, &ordinary, SemanticMirWireVersionV1::V34).unwrap();
    let mut expected_v34 = expected;
    expected_v34.push(0);
    assert_eq!(writer.finish(), expected_v34);
}

#[test]
fn v34_is_not_a_superset_of_v29_even_for_hidden_or_unused_execution_content() {
    let expected = SemanticMirErrorV1::WireVersionCannotRepresent {
        requested: SemanticMirWireVersionV1::V34,
        required: SemanticMirWireVersionV1::V29,
    };
    for role in [
        SemanticExecutionRoleV29::KernelContext,
        SemanticExecutionRoleV29::Workgroup,
        SemanticExecutionRoleV29::MaskedTileU32 {
            lanes: 64,
            elements: 1,
        },
        SemanticExecutionRoleV29::LaneFragmentU32 {
            lanes: 64,
            elements: 1,
        },
    ] {
        let mut mixed = request(KINDS[0]);
        // The complete type table is checked, including roles behind otherwise
        // unused declarations; no root/ABI elision can hide the capability.
        let mut types = mixed.types.to_vec();
        let mut hidden = types[0].clone();
        hidden.rust_type_kind = SemanticRustTypeKindV1::Execution(role);
        types.push(hidden);
        mixed.types = types.into_boxed_slice();
        assert_eq!(
            mixed
                .clone()
                .admit_exact_v34(SemanticMirLimitsV1::default())
                .unwrap_err(),
            expected
        );
        assert_eq!(
            mixed
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V28,
                required: SemanticMirWireVersionV1::V29,
            }
        );
    }
    let mut mixed = request(KINDS[0]);
    let mut callables = mixed.callables.to_vec();
    let mut capability = callables[1].clone();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut capability else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::Execution(
        SemanticExecutionOperationV29::ContextIssue {
            context: SemanticTypeIdV1(0),
        },
    );
    callables.push(capability);
    mixed.callables = callables.into_boxed_slice();
    assert_eq!(
        mixed
            .admit_exact_v34(SemanticMirLimitsV1::default())
            .unwrap_err(),
        expected
    );
    let admitted = request(KINDS[0])
        .admit_exact_v34(SemanticMirLimitsV1::default())
        .unwrap();
    for tag in 14..=17 {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        let scalar = minimal_request().types[0].clone();
        encode_type(&mut writer, &scalar, SemanticMirWireVersionV1::V34).unwrap();
        let mut bytes = writer.finish();
        // Scalar shape tag precedes scalar kind, signedness and the u16 width.
        let index = bytes.len() - 5;
        assert_eq!(bytes[index], 2);
        let mut document = admitted.canonical_encoding().to_vec();
        let document_index = unique_offset(&document, &bytes) + index;
        document[document_index] = tag;
        for decoded in [
            AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
                &document,
                SemanticMirLimitsV1::default(),
            ),
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                &document,
                SemanticMirLimitsV1::default(),
            ),
        ] {
            assert!(matches!(
                decoded,
                Err(SemanticMirDecodeErrorV1::InvalidTag {
                    context: "type shape",
                    ..
                })
            ));
        }
        bytes[index] = tag;
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V34;
        assert!(matches!(
            decoder.ty(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "type shape",
                ..
            })
        ));
    }
}

#[test]
fn v34_signature_and_canonical_byte_limits_remain_closed() {
    for kind in KINDS {
        let fixture = request(kind);
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = &fixture.callables[1]
        else {
            unreachable!()
        };
        assert!(compiler_intrinsic_signature_matches(
            &fixture,
            *operation,
            binding.abi()
        ));
        let mut signed = fixture.clone();
        signed.types[0].shape = SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        });
        assert!(!compiler_intrinsic_signature_matches(
            &signed,
            *operation,
            binding.abi()
        ));
        let mut wrong_arity = fixture.clone();
        call_mut(&mut wrong_arity).arguments = vec![].into_boxed_slice();
        assert!(
            wrong_arity
                .admit_exact_v34(SemanticMirLimitsV1::default())
                .is_err()
        );
    }
    let fixture = request(KINDS[1]);
    let admitted = fixture
        .clone()
        .admit_exact_v34(SemanticMirLimitsV1::default())
        .unwrap();
    let length = admitted.canonical_encoding().len() as u64;
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, length)
        .unwrap();
    let short = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, length - 1)
        .unwrap();
    assert!(fixture.clone().admit_exact_v34(exact).is_ok());
    assert!(fixture.admit_exact_v34(short).is_err());
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
            admitted.canonical_encoding(),
            exact
        )
        .is_ok()
    );
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v34_canonical(
            admitted.canonical_encoding(),
            short
        ),
        Err(SemanticMirDecodeErrorV1::InputLimitExceeded { .. })
    ));
}
