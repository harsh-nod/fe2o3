//! Inert typed-call/codec fixtures, not authenticated compiler capture or execution.

use super::*;
use SemanticCompilerIntrinsicOperationV1 as Intrinsic;

const ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1(0);
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1(1);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1(2);
const LANE: SemanticTypeIdV1 = SemanticTypeIdV1(3);
const OP: Intrinsic = Intrinsic::Gfx942Wave64ShuffleIndex {
    context: CONTEXT,
    element: ELEMENT,
};

fn mode(reference: bool) -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                reference,
                reference.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                reference,
                reference,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap(),
    )
}

fn abi(can_unwind: bool) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(3)),
        SemanticLayoutIdentityV1(identity(4)),
        SemanticCanonAbiV1::Rust,
        can_unwind,
        false,
        vec![
            SemanticAbiValueV1::new(REFERENCE, mode(true)),
            SemanticAbiValueV1::new(ELEMENT, mode(false)),
            SemanticAbiValueV1::new(LANE, mode(false)),
        ],
        SemanticAbiValueV1::new(ELEMENT, mode(false)),
    )
    .unwrap()
}

fn scalar_type(tag: u8, scalar: SemanticScalarTypeV1) -> SemanticTypeDeclV1 {
    let primitive = match scalar {
        SemanticScalarTypeV1::Integer { signed, bits: 32 } => {
            SemanticBackendPrimitiveV1::integer(signed, 32, 4)
        }
        SemanticScalarTypeV1::Float { bits: 32 } => SemanticBackendPrimitiveV1::float(32, 4),
        _ => panic!("closed fixture scalar"),
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(tag)),
        SemanticLayoutIdentityV1(identity(tag + 1)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(scalar),
    )
}

fn request(scalar: SemanticScalarTypeV1, can_unwind: bool) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let context = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(30)),
        SemanticLayoutIdentityV1(identity(31)),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let reference = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(32)),
        SemanticLayoutIdentityV1(identity(33)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                CONTEXT,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    0,
                    1,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    request.types = vec![
        scalar_type(1, scalar),
        context,
        reference,
        scalar_type(
            34,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
    ]
    .into_boxed_slice();
    let abi = abi(can_unwind);
    let source = SemanticSourceProvenanceV1::unavailable();
    request.functions[0].abi = abi.clone();
    request.functions[0].locals = [ELEMENT, REFERENCE, ELEMENT, LANE]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(5 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Argument(index as u32 - 1)
                },
                source,
            )
        })
        .collect();
    let place = |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], ty).unwrap();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(1),
        [REFERENCE, ELEMENT, LANE]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| SemanticOperandV1::Copy(place(index as u32 + 1, ty)))
            .collect(),
        Some(SemanticCallDestinationV1::new(
            place(0, ELEMENT),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    request.functions[0].blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(20)),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(21)),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap(),
    ]
    .into_boxed_slice();
    request.callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1(0)),
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1(identity(134)),
                SemanticItemDefinitionIdentityV1(identity(135)),
                SemanticMonomorphizationIdentityV1(identity(136)),
                SemanticGenericTypeArgumentsIdentityV1(identity(137)),
                SemanticConstGenericArgumentsIdentityV1(identity(138)),
                source,
                abi,
            ),
            operation: OP,
            operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(139)),
        },
    ]
    .into_boxed_slice();
    request
}

fn scalars() -> [SemanticScalarTypeV1; 3] {
    [
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        },
        SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        },
        SemanticScalarTypeV1::Float { bits: 32 },
    ]
}

#[test]
fn wave_abi_codec_preserves_unwind_while_full_requests_require_nounwind() {
    let limits = SemanticMirLimitsV1::default();
    for scalar in scalars() {
        for unwind in [false, true] {
            let request = request(scalar, unwind);
            component_round_trip(
                abi(unwind),
                |writer, abi| encode_abi(writer, abi, SemanticMirWireVersionV1::V33),
                |decoder| {
                    decoder.wire_version = SemanticMirWireVersionV1::V33;
                    decoder.abi()
                },
            );
            assert!(compiler_intrinsic_signature_matches(
                &request,
                OP,
                &abi(unwind)
            ));
            if unwind {
                assert_eq!(
                    request.clone().admit_exact_v33(limits).unwrap_err(),
                    SemanticMirErrorV1::InvalidFunctionAbi
                );
                assert_eq!(
                    request.admit_current_production(limits).unwrap_err(),
                    SemanticMirErrorV1::InvalidFunctionAbi
                );
                continue;
            }
            let exact = request.clone().admit_exact_v33(limits).unwrap();
            let current = request.admit_current_production(limits).unwrap();
            assert_eq!(exact.wire_version(), SemanticMirWireVersionV1::V33);
            assert_eq!(exact.canonical_encoding(), current.canonical_encoding());
            for decoded in [
                AdmittedInertSemanticMirV1::decode_exact_v33_canonical(
                    exact.canonical_encoding(),
                    limits,
                ),
                AdmittedInertSemanticMirV1::decode_current_production_canonical(
                    exact.canonical_encoding(),
                    limits,
                ),
            ] {
                let decoded = decoded.unwrap();
                assert_eq!(decoded.callables(), exact.callables());
                assert_eq!(decoded.functions(), exact.functions());
                assert_eq!(decoded.functions()[0].abi().can_unwind(), unwind);
                assert_eq!(decoded.canonical_encoding(), exact.canonical_encoding());
            }
        }
    }
}

#[test]
fn wave_body_and_callable_unwind_abis_are_independently_refused() {
    let limits = SemanticMirLimitsV1::default();
    for scalar in scalars() {
        for (body_unwind, callable_unwind) in [(true, false), (false, true)] {
            let mut request = request(scalar, false);
            request.functions[0].abi.can_unwind = body_unwind;
            let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } =
                &mut request.callables[1]
            else {
                unreachable!()
            };
            binding.abi.can_unwind = callable_unwind;
            assert_eq!(request.functions[0].abi().can_unwind(), body_unwind);
            assert!(compiler_intrinsic_signature_matches(
                &request,
                OP,
                &abi(callable_unwind)
            ));
            assert_eq!(
                request.clone().admit_exact_v33(limits).unwrap_err(),
                SemanticMirErrorV1::InvalidFunctionAbi
            );
            assert_eq!(
                request.admit_current_production(limits).unwrap_err(),
                SemanticMirErrorV1::InvalidFunctionAbi
            );
        }
    }
}

#[test]
fn wave_signature_rejects_arity_scalar_lane_reference_and_rust_abi_substitutions() {
    for scalar in scalars() {
        let original = request(scalar, true);
        assert!(compiler_intrinsic_signature_matches(
            &original,
            OP,
            &abi(true)
        ));
        for mutation in 0..12 {
            let mut request = original.clone();
            let mut abi = abi(true);
            match mutation {
                0 => {
                    abi.source_signature.inputs = vec![REFERENCE, ELEMENT].into_boxed_slice();
                }
                1 => {
                    abi.source_signature.inputs[1] = REFERENCE;
                }
                2 => {
                    abi.source_signature.inputs[2] = REFERENCE;
                }
                3 => {
                    abi.source_signature.output = LANE;
                }
                4 => {
                    let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[2].shape else {
                        unreachable!()
                    };
                    pointer.mutability = SemanticMutabilityV1::Mutable;
                }
                5 => {
                    request.types[0].shape =
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 64 });
                }
                6 => {
                    request.types[3].shape =
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: true,
                            bits: 32,
                        });
                }
                7 => {
                    request.types[3].shape =
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits: 64,
                        });
                }
                8 => {
                    abi.canon_abi = SemanticCanonAbiV1::C;
                }
                9 => {
                    abi.source_signature.c_variadic = true;
                }
                10 => {
                    abi.source_signature.extern_abi = SemanticExternAbiV1::C { unwind: false };
                }
                11 => {
                    let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[2].shape else {
                        unreachable!()
                    };
                    pointer.kind = SemanticPointerKindV1::Raw;
                }
                _ => unreachable!(),
            }
            assert!(
                !compiler_intrinsic_signature_matches(&request, OP, &abi),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn v33_does_not_acquire_execution_types_and_legacy_requests_keep_their_bytes() {
    let limits = SemanticMirLimitsV1::default();
    let capability = capability_v29_tests::request(2, 2);
    let ty = capability
        .types
        .iter()
        .find(|ty| matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_)))
        .expect("genuine existing V29 capability type");
    let mut old_writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    encode_type(&mut old_writer, ty, SemanticMirWireVersionV1::V29).unwrap();
    let old_bytes = old_writer.finish();
    let mut decoder = CanonicalDecoderV1::new(&old_bytes, limits);
    decoder.wire_version = SemanticMirWireVersionV1::V33;
    assert!(decoder.ty().is_err());
    let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
    assert_eq!(
        encode_type(&mut writer, ty, SemanticMirWireVersionV1::V33),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V33,
            required: SemanticMirWireVersionV1::V29,
        })
    );
    assert!(writer.finish().is_empty());
    let plain = minimal_request().admit_current_production(limits).unwrap();
    assert_eq!(plain.wire_version(), SemanticMirWireVersionV1::V5);
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            plain.canonical_encoding(),
            limits
        )
        .unwrap()
        .canonical_encoding(),
        plain.canonical_encoding()
    );
}

#[test]
fn v33_tag_is_exact_and_does_not_admit_reserved_execution_or_assembly_holes() {
    let limits = SemanticMirLimitsV1::default();
    let expected = [90, 1, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        compiler_intrinsic_round_trip(OP, SemanticMirWireVersionV1::V33),
        expected
    );
    for version in [
        SemanticMirWireVersionV1::V15,
        SemanticMirWireVersionV1::V28,
        SemanticMirWireVersionV1::V29,
        SemanticMirWireVersionV1::V30,
    ] {
        let mut writer = CanonicalWriterV1::new(0);
        assert_eq!(
            encode_compiler_intrinsic_operation(&mut writer, OP, version),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: version,
                required: SemanticMirWireVersionV1::V33
            })
        );
        assert!(writer.finish().is_empty());
        let mut decoder = CanonicalDecoderV1::new(&expected, limits);
        decoder.wire_version = version;
        assert!(matches!(
            decoder.compiler_intrinsic(),
            Err(SemanticMirDecodeErrorV1::InvalidTag { value: 90, .. })
        ));
    }
    for tag in (69..=86).chain([88, 89]).chain(91..=255) {
        let bytes = [tag];
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V33;
        assert_eq!(
            decoder.compiler_intrinsic().unwrap_err(),
            SemanticMirDecodeErrorV1::InvalidTag {
                context: "compiler intrinsic",
                offset: 0,
                value: tag
            }
        );
    }
    for subtag in [
        SemanticSaturatingIntegerOpV1::Add,
        SemanticSaturatingIntegerOpV1::Subtract,
    ] {
        let operation = Intrinsic::SaturatingInteger(subtag);
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V30),
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V33)
        );
    }
    let execution =
        Intrinsic::Execution(SemanticExecutionOperationV29::ContextIssue { context: CONTEXT });
    let mut writer = CanonicalWriterV1::new(0);
    assert_eq!(
        encode_compiler_intrinsic_operation(&mut writer, execution, SemanticMirWireVersionV1::V33),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V33,
            required: SemanticMirWireVersionV1::V29
        })
    );
    assert!(writer.finish().is_empty());
    for (operation, expected) in [
        (Intrinsic::FabsF32, vec![6]),
        (Intrinsic::Trap, vec![64]),
        (
            Intrinsic::WorkgroupLdsScopeCurrent { scope: CONTEXT },
            vec![66, 1, 0, 0, 0],
        ),
        (
            Intrinsic::SaturatingInteger(SemanticSaturatingIntegerOpV1::Add),
            vec![87, 0],
        ),
        (
            Intrinsic::SaturatingInteger(SemanticSaturatingIntegerOpV1::Subtract),
            vec![87, 1],
        ),
    ] {
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V30),
            expected
        );
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V33),
            expected
        );
    }
}

#[test]
fn wave_wire_truncation_type_mutation_and_exact_byte_work_limits_fail_closed() {
    let request = request(scalars()[2], false);
    let defaults = SemanticMirLimitsV1::default();
    let admitted = request.clone().admit_exact_v33(defaults).unwrap();
    let bytes = admitted.canonical_encoding();
    for end in 0..bytes.len() {
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v33_canonical(&bytes[..end], defaults)
                .is_err()
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(AdmittedInertSemanticMirV1::decode_exact_v33_canonical(&trailing, defaults).is_err());
    let mut wrong = request.clone();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut wrong.callables[1]
    else {
        unreachable!()
    };
    *operation = Intrinsic::Gfx942Wave64ShuffleIndex {
        context: SemanticTypeIdV1(u32::MAX),
        element: ELEMENT,
    };
    assert_eq!(
        wrong.admit_exact_v33(defaults).unwrap_err(),
        SemanticMirErrorV1::InvalidFunctionAbi
    );
    let length = bytes.len() as u64;
    for (bound, success) in [(length, true), (length - 1, false)] {
        let limit = defaults
            .with_limit(SemanticMirResourceV1::CanonicalBytes, bound)
            .unwrap();
        assert_eq!(request.clone().admit_exact_v33(limit).is_ok(), success);
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_exact_v33_canonical(bytes, limit).is_ok(),
            success
        );
    }
    let mut low = 0;
    let mut high = defaults.limit(SemanticMirResourceV1::ValidationWork);
    while low < high {
        let middle = low + (high - low) / 2;
        let limit = defaults
            .with_limit(SemanticMirResourceV1::ValidationWork, middle)
            .unwrap();
        match request.clone().admit_exact_v33(limit) {
            Ok(_) => high = middle,
            Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                ..
            }) => low = middle + 1,
            other => panic!("unexpected work-bound result: {other:?}"),
        }
    }
    assert!(low > 0);
    for (bound, success) in [(low, true), (low - 1, false)] {
        let limit = defaults
            .with_limit(SemanticMirResourceV1::ValidationWork, bound)
            .unwrap();
        assert_eq!(request.clone().admit_exact_v33(limit).is_ok(), success);
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_exact_v33_canonical(bytes, limit).is_ok(),
            success
        );
    }
}
