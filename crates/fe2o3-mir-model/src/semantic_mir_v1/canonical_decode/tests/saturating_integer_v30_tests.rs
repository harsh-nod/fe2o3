//! Inert schema, ABI, version isolation, and bounded codec tests, not source authority.

use super::*;

use SemanticCompilerIntrinsicOperationV1 as Intrinsic;
use SemanticSaturatingIntegerOpV1 as Op;

const TY: SemanticTypeIdV1 = SemanticTypeIdV1(0);
const OLD: [SemanticMirWireVersionV1; 16] = [
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
];

fn mode() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap(),
    )
}

fn abi(inputs: &[SemanticTypeIdV1], output: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(3)),
        SemanticLayoutIdentityV1(identity(4)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs
            .iter()
            .map(|ty| SemanticAbiValueV1::new(*ty, mode()))
            .collect(),
        SemanticAbiValueV1::new(output, mode()),
    )
    .unwrap()
}

fn integer(request: &mut InertSemanticMirRequestV1, signed: bool, bits: u16) {
    let size = u64::from(bits / 8);
    request.types[0].shape =
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits });
    request.types[0].layout = SemanticTypeLayoutV1::new_with_backend_repr(
        Some(size),
        size,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(signed, bits, size),
            SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
        )),
        false,
    )
    .unwrap();
}

fn binding(abi: SemanticFunctionAbiV1) -> SemanticNonBodyCallableBindingV1 {
    SemanticNonBodyCallableBindingV1::new(
        SemanticFunctionIdentityV1(identity(134)),
        SemanticItemDefinitionIdentityV1(identity(135)),
        SemanticMonomorphizationIdentityV1(identity(136)),
        SemanticGenericTypeArgumentsIdentityV1(identity(137)),
        SemanticConstGenericArgumentsIdentityV1(identity(138)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
    )
}

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], TY).unwrap()
}

fn attach_call(request: &mut InertSemanticMirRequestV1, op: Op, rhs_local: u32) {
    let mut callables = request.callables.to_vec();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: binding(abi(&[TY, TY], TY)),
        operation: Intrinsic::SaturatingInteger(op),
        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(139)),
    });
    request.callables = callables.into_boxed_slice();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(1),
        [1, rhs_local]
            .map(|local| SemanticOperandV1::Copy(place(local)))
            .to_vec(),
        Some(SemanticCallDestinationV1::new(
            place(0),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    request.functions[0].blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(7)),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(15)),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap(),
    ]
    .into_boxed_slice();
}

fn request(signed: bool, bits: u16, op: Op) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    integer(&mut request, signed, bits);
    request.functions[0].abi = abi(&[TY, TY], TY);
    let mut locals = request.functions[0].locals.to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(14)),
        TY,
        SemanticLocalRoleV1::Argument(1),
        SemanticSourceProvenanceV1::unavailable(),
    ));
    request.functions[0].locals = locals.into_boxed_slice();
    attach_call(&mut request, op, 2);
    request
}

fn intrinsic_abi(request: &mut InertSemanticMirRequestV1) -> &mut SemanticFunctionAbiV1 {
    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut request.callables[1]
    else {
        unreachable!()
    };
    &mut binding.abi
}

#[test]
fn all_admitted_integer_types_and_operations_round_trip_as_typed_calls() {
    let limits = SemanticMirLimitsV1::default();
    for signed in [false, true] {
        for bits in [8, 16, 32, 64] {
            for op in [Op::Add, Op::Subtract] {
                let request = request(signed, bits, op);
                assert_eq!(
                    minimum_wire_version(&request),
                    SemanticMirWireVersionV1::V30
                );
                let exact = request.clone().admit_exact_v30(limits).unwrap();
                let current = request.clone().admit_current_production(limits).unwrap();
                let minimal = request.admit(limits).unwrap();
                assert_eq!(exact.canonical_encoding(), current.canonical_encoding());
                assert_eq!(exact.canonical_encoding(), minimal.canonical_encoding());
                assert_eq!(exact.semantic_sha256(), current.semantic_sha256());
                for decoded in [
                    AdmittedInertSemanticMirV1::decode_exact_v30_canonical(
                        exact.canonical_encoding(),
                        limits,
                    ),
                    AdmittedInertSemanticMirV1::decode_current_production_canonical(
                        exact.canonical_encoding(),
                        limits,
                    ),
                    AdmittedInertSemanticMirV1::decode_canonical(
                        exact.canonical_encoding(),
                        limits,
                    ),
                ] {
                    let decoded = decoded.unwrap();
                    assert_eq!(decoded.functions(), exact.functions());
                    assert_eq!(decoded.callables(), exact.callables());
                    assert_eq!(decoded.canonical_encoding(), exact.canonical_encoding());
                    assert_eq!(decoded.functions()[0].locals().len(), 3);
                    assert_eq!(decoded.functions()[0].blocks().len(), 2);
                }
            }
        }
    }
}

#[test]
fn saturation_operation_tag_and_subtag_are_closed_and_old_versions_refuse_before_write() {
    let limits = SemanticMirLimitsV1::default();
    for (op, subtag) in [(Op::Add, 0), (Op::Subtract, 1)] {
        let operation = Intrinsic::SaturatingInteger(op);
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V30),
            [87, subtag]
        );
        for version in OLD {
            let mut writer = CanonicalWriterV1::new(0);
            assert_eq!(
                encode_compiler_intrinsic_operation(&mut writer, operation, version),
                Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                    requested: version,
                    required: SemanticMirWireVersionV1::V30,
                })
            );
            assert!(writer.finish().is_empty());
            let bytes = [87, subtag];
            let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
            decoder.wire_version = version;
            assert!(matches!(
                decoder.compiler_intrinsic(),
                Err(SemanticMirDecodeErrorV1::InvalidTag {
                    context: "compiler intrinsic",
                    offset: 0,
                    value: 87,
                })
            ));
        }
    }
    for subtag in 2..=255 {
        let bytes = [87, subtag];
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V30;
        assert!(matches!(
            decoder.compiler_intrinsic(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "saturating integer operation",
                offset: 1,
                value,
            }) if value == subtag
        ));
    }
    for bytes in [&[][..], &[87][..]] {
        let mut decoder = CanonicalDecoderV1::new(bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V30;
        assert!(decoder.compiler_intrinsic().is_err());
    }
}

#[test]
fn signature_refuses_wrong_arity_type_identity_width_sign_float_bool_and_unwind() {
    let original = request(false, 32, Op::Add);
    let operation = Intrinsic::SaturatingInteger(Op::Add);
    for inputs in [vec![], vec![TY], vec![TY, TY, TY]] {
        let mut changed = original.clone();
        *intrinsic_abi(&mut changed) = abi(&inputs, TY);
        assert_eq!(
            changed
                .admit_exact_v30(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi
        );
    }
    for shape in [
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 128,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 128,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 64 }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        SemanticTypeShapeV1::Unit,
    ] {
        let mut changed = original.clone();
        changed.types[0].shape = shape;
        let abi = intrinsic_abi(&mut changed).clone();
        // This tests the signature predicate itself, not a forged type layout's admission.
        assert!(!compiler_intrinsic_signature_matches(
            &changed, operation, &abi
        ));
    }
    for shape in [
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ] {
        let mut changed = original.clone();
        let mut alternate = changed.types[0].clone();
        alternate.identity = SemanticTypeIdentityV1(identity(20));
        alternate.layout_identity = SemanticLayoutIdentityV1(identity(21));
        alternate.shape = shape;
        let mut types = changed.types.to_vec();
        types.push(alternate);
        changed.types = types.into_boxed_slice();
        for bad_abi in [
            abi(&[TY, SemanticTypeIdV1(1)], TY),
            abi(&[TY, TY], SemanticTypeIdV1(1)),
        ] {
            assert!(!compiler_intrinsic_signature_matches(
                &changed, operation, &bad_abi
            ));
        }
    }
    for mutation in 0..4 {
        let mut changed = original.clone();
        let abi = intrinsic_abi(&mut changed);
        match mutation {
            0 => abi.can_unwind = true,
            1 => abi.source_signature.c_variadic = true,
            2 => abi.source_signature.extern_abi = SemanticExternAbiV1::C { unwind: false },
            3 => abi.canon_abi = SemanticCanonAbiV1::C,
            _ => unreachable!(),
        }
        let abi = abi.clone();
        assert!(!compiler_intrinsic_signature_matches(
            &changed, operation, &abi
        ));
    }
}

#[test]
fn mixed_saturation_and_genuine_capability_roles_cannot_select_around_production_fence() {
    let limits = SemanticMirLimitsV1::default();
    let legacy = capability_v29_tests::request(2, 2);
    let legacy_admitted = legacy.clone().admit_exact_v29(limits).unwrap();
    let mut mixed = legacy;
    attach_call(&mut mixed, Op::Subtract, 1);
    // Both feature families are structurally valid. Their wire dialects are intentionally disjoint.
    validate_request(&mixed, limits).unwrap();
    assert_eq!(minimum_wire_version(&mixed), SemanticMirWireVersionV1::V30);
    assert_eq!(
        mixed.clone().admit_current_production(limits).unwrap_err(),
        SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V28,
            required: SemanticMirWireVersionV1::V29,
        }
    );
    assert_eq!(
        mixed.clone().admit_exact_v30(limits).unwrap_err(),
        SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V30,
            required: SemanticMirWireVersionV1::V29,
        }
    );
    assert_eq!(
        mixed.admit_exact_v29(limits).unwrap_err(),
        SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V29,
            required: SemanticMirWireVersionV1::V30,
        }
    );
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            legacy_admitted.canonical_encoding(),
            limits
        ),
        Err(SemanticMirDecodeErrorV1::UnsupportedProductionWireVersion(
            SemanticMirWireVersionV1::V29
        ))
    ));
    let mut promoted = legacy_admitted.canonical_encoding().to_vec();
    promoted[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&30u16.to_le_bytes());
    assert!(AdmittedInertSemanticMirV1::decode_exact_v30_canonical(&promoted, limits).is_err());
    assert!(
        AdmittedInertSemanticMirV1::decode_current_production_canonical(&promoted, limits).is_err()
    );
}

#[test]
fn capability_callables_are_fenced_even_without_capability_types_or_reachability() {
    let limits = SemanticMirLimitsV1::default();
    for saturated in [false, true] {
        let mut request = if saturated {
            request(false, 64, Op::Add)
        } else {
            minimal_request()
        };
        let mut callables = request.callables.to_vec();
        callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
            binding: binding(abi(&[], TY)),
            operation: Intrinsic::Execution(SemanticExecutionOperationV29::ContextIssue {
                context: TY,
            }),
            operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(150)),
        });
        request.callables = callables.into_boxed_slice();
        // Intentionally invalid/unreachable capability content still cannot become production.
        assert_eq!(
            request.admit_current_production(limits).unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V28,
                required: SemanticMirWireVersionV1::V29,
            }
        );
    }
}

#[test]
fn v30_rejects_all_capability_and_historical_holes_before_payload_decode() {
    let limits = SemanticMirLimitsV1::default();
    for tag in (69..=86).chain(88..=255) {
        let bytes = [tag];
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V30;
        assert_eq!(
            decoder.compiler_intrinsic().unwrap_err(),
            SemanticMirDecodeErrorV1::InvalidTag {
                context: "compiler intrinsic",
                offset: 0,
                value: tag
            }
        );
    }
    for operation in [
        SemanticExecutionOperationV29::ContextIssue { context: TY },
        SemanticExecutionOperationV29::WorkgroupDerive {
            context: TY,
            workgroup: TY,
        },
        SemanticExecutionOperationV29::MaskedTileLoadU32 {
            workgroup: TY,
            tile: TY,
        },
        SemanticExecutionOperationV29::MaskedTileIntoFragmentU32 {
            tile: TY,
            fragment: TY,
        },
        SemanticExecutionOperationV29::LaneFragmentIntoPartsU32 {
            fragment: TY,
            parts: TY,
        },
    ] {
        let mut writer = CanonicalWriterV1::new(0);
        assert_eq!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                Intrinsic::Execution(operation),
                SemanticMirWireVersionV1::V30
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V30,
                required: SemanticMirWireVersionV1::V29
            })
        );
        assert!(writer.finish().is_empty());
    }
}

#[test]
fn existing_intrinsic_and_capability_goldens_remain_byte_identical() {
    for (operation, bytes) in [
        (Intrinsic::FabsF32, vec![6]),
        (Intrinsic::Trap, vec![64]),
        (
            Intrinsic::WorkgroupLdsScopeCurrent {
                scope: SemanticTypeIdV1(7),
            },
            vec![66, 7, 0, 0, 0],
        ),
    ] {
        for version in [
            SemanticMirWireVersionV1::V15,
            SemanticMirWireVersionV1::V28,
            SemanticMirWireVersionV1::V29,
            SemanticMirWireVersionV1::V30,
        ] {
            assert_eq!(compiler_intrinsic_round_trip(operation, version), bytes);
        }
    }
    assert_eq!(
        compiler_intrinsic_round_trip(
            Intrinsic::Execution(SemanticExecutionOperationV29::ContextIssue {
                context: SemanticTypeIdV1(5)
            }),
            SemanticMirWireVersionV1::V29,
        ),
        [81, 5, 0, 0, 0]
    );
    let limits = SemanticMirLimitsV1::default();
    let original = minimal_request();
    for version in OLD {
        let admitted = original
            .clone()
            .admit_for_wire_version(version, limits)
            .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_with_policy(
            admitted.canonical_encoding(),
            limits,
            CanonicalDecodePolicyV1::Exact(version),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        assert_eq!(
            minimum_wire_version(&original),
            SemanticMirWireVersionV1::V2
        );
    }
    assert_eq!(
        original
            .admit_current_production(limits)
            .unwrap()
            .wire_version(),
        SemanticMirWireVersionV1::V5
    );
}

#[test]
fn v30_rejects_each_genuine_capability_type_without_inheriting_v29_role_tags() {
    let limits = SemanticMirLimitsV1::default();
    let request = capability_v29_tests::request(2, 2);
    let mut count = 0;
    for ty in &request.types {
        if !matches!(ty.rust_type_kind, SemanticRustTypeKindV1::Execution(_)) {
            continue;
        }
        count += 1;
        let mut legacy = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_type(&mut legacy, ty, SemanticMirWireVersionV1::V29).unwrap();
        let bytes = legacy.finish();
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V29;
        assert_eq!(&decoder.ty().unwrap(), ty);
        decoder.finish().unwrap();
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V30;
        assert!(matches!(
            decoder.ty(),
            Err(SemanticMirDecodeErrorV1::InvalidTag { value: 14..=17, .. })
        ));
        let mut writer = CanonicalWriterV1::new(0);
        assert_eq!(
            encode_type(&mut writer, ty, SemanticMirWireVersionV1::V30),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V30,
                required: SemanticMirWireVersionV1::V29,
            })
        );
        assert!(writer.finish().is_empty());
    }
    assert_eq!(count, 4);
}

#[test]
fn truncation_trailing_bytes_versions_and_opcode_mutations_do_not_gain_authority() {
    let limits = SemanticMirLimitsV1::default();
    let admitted = request(true, 64, Op::Add).admit_exact_v30(limits).unwrap();
    let bytes = admitted.canonical_encoding();
    for end in 0..bytes.len() {
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v30_canonical(&bytes[..end], limits).is_err()
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(AdmittedInertSemanticMirV1::decode_exact_v30_canonical(&trailing, limits).is_err());
    let mut changed = bytes.to_vec();
    changed[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&29u16.to_le_bytes());
    assert!(AdmittedInertSemanticMirV1::decode_exact_v29_canonical(&changed, limits).is_err());
    let subtract = request(true, 64, Op::Subtract)
        .admit_exact_v30(limits)
        .unwrap();
    assert_ne!(bytes, subtract.canonical_encoding());
    assert_ne!(admitted.semantic_sha256(), subtract.semantic_sha256());
}

#[test]
fn two_byte_component_and_full_canonical_byte_limits_are_exact() {
    let operation = Intrinsic::SaturatingInteger(Op::Add);
    for max in 0..=2 {
        let mut writer = CanonicalWriterV1::new(max);
        let result = encode_compiler_intrinsic_operation(
            &mut writer,
            operation,
            SemanticMirWireVersionV1::V30,
        );
        if max == 2 {
            result.unwrap();
            assert_eq!(writer.finish(), [87, 0]);
        } else {
            assert_eq!(
                result.unwrap_err(),
                SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::CanonicalBytes,
                    actual: max + 1,
                    max,
                }
            );
            assert_eq!(writer.finish().len() as u64, max);
        }
    }
    let request = request(false, 64, Op::Subtract);
    let admitted = request
        .clone()
        .admit_exact_v30(SemanticMirLimitsV1::default())
        .unwrap();
    let length = admitted.canonical_encoding().len() as u64;
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, length)
        .unwrap();
    let short = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, length - 1)
        .unwrap();
    assert_eq!(
        request
            .clone()
            .admit_exact_v30(exact)
            .unwrap()
            .canonical_encoding(),
        admitted.canonical_encoding()
    );
    assert!(matches!(
        request.admit_exact_v30(short),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v30_canonical(
            admitted.canonical_encoding(),
            exact
        )
        .is_ok()
    );
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_exact_v30_canonical(
            admitted.canonical_encoding(),
            short
        )
        .unwrap_err(),
        SemanticMirDecodeErrorV1::InputLimitExceeded {
            actual: length,
            max: length - 1
        }
    );
}

#[test]
fn inherited_validation_work_and_record_limits_remain_enforced() {
    let request = request(true, 32, Op::Add);
    let defaults = SemanticMirLimitsV1::default();
    let admitted = request.clone().admit_exact_v30(defaults).unwrap();
    // ValidationWork remains the existing semantic-validation domain, not decoder instructions.
    let mut low = 0;
    let mut high = defaults.limit(SemanticMirResourceV1::ValidationWork);
    while low < high {
        let mid = low + (high - low) / 2;
        let limits = defaults
            .with_limit(SemanticMirResourceV1::ValidationWork, mid)
            .unwrap();
        match request.clone().admit_exact_v30(limits) {
            Ok(_) => high = mid,
            Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                ..
            }) => low = mid + 1,
            other => panic!("unexpected validation result: {other:?}"),
        }
    }
    assert!(low > 0);
    let exact = defaults
        .with_limit(SemanticMirResourceV1::ValidationWork, low)
        .unwrap();
    let short = defaults
        .with_limit(SemanticMirResourceV1::ValidationWork, low - 1)
        .unwrap();
    request.clone().admit_exact_v30(exact).unwrap();
    assert!(matches!(
        request.clone().admit_exact_v30(short),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
    AdmittedInertSemanticMirV1::decode_exact_v30_canonical(admitted.canonical_encoding(), exact)
        .unwrap();
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v30_canonical(
            admitted.canonical_encoding(),
            short
        ),
        Err(SemanticMirDecodeErrorV1::Validation(
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                ..
            }
        ))
    ));
    for (resource, short) in [
        (SemanticMirResourceV1::Types, 0),
        (SemanticMirResourceV1::Functions, 0),
        (SemanticMirResourceV1::Callables, 1),
        (SemanticMirResourceV1::Locals, 2),
        (SemanticMirResourceV1::Blocks, 1),
        (SemanticMirResourceV1::Operands, 1),
    ] {
        let limits = defaults.with_limit(resource, short).unwrap();
        assert!(
            request.clone().admit_exact_v30(limits).is_err(),
            "{resource:?}"
        );
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v30_canonical(
                admitted.canonical_encoding(),
                limits
            )
            .is_err(),
            "{resource:?}"
        );
    }
}
