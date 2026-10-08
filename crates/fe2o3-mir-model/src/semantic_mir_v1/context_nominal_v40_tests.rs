//! Provisional private V40 inert-model controls, not authenticated Rust capture.
//! No importer, executable owner, proof or launch authority is constructed here.
use super::*;

const V40: SemanticMirWireVersionV1 = SemanticMirWireVersionV1::V40;
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1(5);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1(1);

fn nominal_type(signed: bool) -> SemanticTypeDeclV1 {
    let tag = if signed { 129 } else { 128 };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(tag)),
        SemanticLayoutIdentityV1(identity(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits: 64 }),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
    )
    .with_rust_type_kind(if signed {
        SemanticRustTypeKindV1::Isize
    } else {
        SemanticRustTypeKindV1::Usize
    })
}

fn place(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1(
            request.functions[0]
                .locals
                .iter()
                .position(|local| local.ty == ty)
                .unwrap() as u32,
        ),
        vec![],
        ty,
    )
    .unwrap()
}

fn request(context: bool, nominal: bool) -> InertSemanticMirRequestV1 {
    let mut request = if context {
        capability_v29_tests::request(2, 2)
    } else {
        minimal_request()
    };
    if nominal {
        let mut types = request.types.to_vec();
        let mut locals = request.functions[0].locals.to_vec();
        let original = &request.functions[0].abi;
        let mut arguments = vec![original.arguments[0].value.clone()];
        for (index, signed) in [false, true].into_iter().enumerate() {
            let ty = SemanticTypeIdV1(types.len() as u32);
            types.push(nominal_type(signed));
            arguments.push(SemanticAbiValueV1::new(
                ty,
                original.return_value.mode.clone(),
            ));
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(200 + index as u8)),
                ty,
                SemanticLocalRoleV1::Argument(1 + index as u32),
                SemanticSourceProvenanceV1::unavailable(),
            ));
        }
        request.functions[0].abi = SemanticFunctionAbiV1::new(
            original.identity,
            original.layout_identity,
            SemanticCanonAbiV1::Rust,
            false,
            false,
            arguments,
            original.return_value.clone(),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 3])
        .unwrap();
        request.types = types.into_boxed_slice();
        request.functions[0].locals = locals.into_boxed_slice();
    }
    if context {
        let source = SemanticSourceProvenanceV1::unavailable();
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1(identity(60)),
            SemanticLayoutIdentityV1(identity(61)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![],
            SemanticAbiValueV1::new(CONTEXT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        request.callables = vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1(0)),
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1(identity(62)),
                    SemanticItemDefinitionIdentityV1(identity(63)),
                    SemanticMonomorphizationIdentityV1(identity(64)),
                    SemanticGenericTypeArgumentsIdentityV1(identity(65)),
                    SemanticConstGenericArgumentsIdentityV1(identity(66)),
                    source,
                    abi,
                ),
                operation: SemanticCompilerIntrinsicOperationV1::Execution(
                    SemanticExecutionOperationV29::ContextIssue { context: CONTEXT },
                ),
                operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(67)),
            },
        ]
        .into_boxed_slice();
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1(1),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(&request, CONTEXT),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let mut exit = request.functions[0].blocks[0].clone();
        exit.identity = SemanticBlockIdentityV1(identity(8));
        request.functions[0].blocks[0].terminator.kind = SemanticTerminatorKindV1::Call(call);
        request.functions[0].blocks =
            vec![request.functions[0].blocks[0].clone(), exit].into_boxed_slice();
    }
    request
}

fn roundtrip(request: InertSemanticMirRequestV1) -> AdmittedInertSemanticMirV1 {
    let limits = SemanticMirLimitsV1::default();
    let admitted = request.clone().admit_exact_v40(limits).unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v40_canonical(
        admitted.canonical_encoding(),
        limits,
    )
    .unwrap();
    assert_eq!(decoded.wire_version(), V40);
    assert_eq!(decoded.types(), request.types.as_ref());
    assert_eq!(decoded.functions(), request.functions.as_ref());
    assert_eq!(decoded.callables(), request.callables.as_ref());
    assert_eq!(decoded.roots(), request.roots.as_ref());
    assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
    admitted
}

#[test]
fn context_nominal_v40_four_combinations_preserve_original_records_and_abi() {
    assert_eq!(SemanticMirWireVersionV1::from_u16(40), Some(V40));
    assert_eq!(V40.as_u16(), 40);
    assert_eq!(SemanticMirWireVersionV1::from_u16(42), None);
    for context in [false, true] {
        for nominal in [false, true] {
            let admitted = roundtrip(request(context, nominal));
            assert_eq!(
                admitted
                    .types()
                    .iter()
                    .any(|ty| matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_))),
                context
            );
            assert_eq!(
                admitted
                    .types()
                    .iter()
                    .filter(|ty| matches!(
                        ty.rust_type_kind(),
                        SemanticRustTypeKindV1::Usize | SemanticRustTypeKindV1::Isize
                    ))
                    .count(),
                if nominal { 2 } else { 0 }
            );
            assert_eq!(
                admitted.functions()[0].abi().source_input_types().len(),
                if nominal { 3 } else { 1 }
            );
            if nominal {
                for (argument, kind) in [
                    (1, SemanticRustTypeKindV1::Usize),
                    (2, SemanticRustTypeKindV1::Isize),
                ] {
                    let ty = admitted.functions()[0].abi().source_input_types()[argument];
                    assert_eq!(admitted.types()[ty.index() as usize].rust_type_kind(), kind);
                }
            }
        }
    }
}

#[test]
fn context_nominal_v40_is_explicit_only_and_preserves_legacy_bytes_and_refusals() {
    let limits = SemanticMirLimitsV1::default();
    for (context, nominal, old_version) in [
        (false, false, SemanticMirWireVersionV1::V15),
        (false, true, SemanticMirWireVersionV1::V35),
        (true, false, SemanticMirWireVersionV1::V29),
    ] {
        let original = request(context, nominal);
        let old = original
            .clone()
            .admit_for_wire_version(old_version, limits)
            .unwrap();
        let shared = roundtrip(original.clone());
        assert_eq!(
            &old.canonical_encoding()[MAGIC.len() + 2..],
            &shared.canonical_encoding()[MAGIC.len() + 2..]
        );
        assert_ne!(old.semantic_sha256(), shared.semantic_sha256());
        assert_ne!(original.clone().admit(limits).unwrap().wire_version(), V40);
        if !context {
            assert_ne!(
                original
                    .admit_current_production(limits)
                    .unwrap()
                    .wire_version(),
                V40
            );
        }
        assert!(
            AdmittedInertSemanticMirV1::decode_minimal_compatible_canonical(
                shared.canonical_encoding(),
                limits
            )
            .is_err()
        );
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                shared.canonical_encoding(),
                limits
            )
            .unwrap_err(),
            SemanticMirDecodeErrorV1::UnsupportedProductionWireVersion(V40)
        );
    }
    let combined = request(true, true);
    assert!(combined.clone().admit(limits).is_err());
    assert!(combined.clone().admit_current_production(limits).is_err());
    for version in (2..=15)
        .chain(28..=39)
        .filter_map(SemanticMirWireVersionV1::from_u16)
    {
        assert!(
            combined
                .clone()
                .admit_for_wire_version(version, limits)
                .is_err(),
            "{version:?}"
        );
    }
    let shared = roundtrip(combined);
    for expected in [
        SemanticMirWireVersionV1::V29,
        SemanticMirWireVersionV1::V35,
        SemanticMirWireVersionV1::V36,
        SemanticMirWireVersionV1::V37,
        SemanticMirWireVersionV1::V38,
        SemanticMirWireVersionV1::V39,
    ] {
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_with_policy(
                shared.canonical_encoding(),
                limits,
                CanonicalDecodePolicyV1::Exact(expected)
            )
            .unwrap_err(),
            SemanticMirDecodeErrorV1::WireVersionMismatch {
                expected,
                actual: V40
            }
        );
    }
}

#[test]
fn context_nominal_v40_nominal_substitution_changes_bytes_not_layout_or_source_identity() {
    let original = request(true, true);
    let admitted = roundtrip(original.clone());
    for signed in [false, true] {
        let mut changed = original.clone();
        let index = changed.types.len() - if signed { 1 } else { 2 };
        changed.types[index].rust_type_kind = SemanticRustTypeKindV1::Ordinary;
        let changed = roundtrip(changed);
        assert_eq!(
            changed.types()[index].identity(),
            admitted.types()[index].identity()
        );
        assert_eq!(
            changed.types()[index].layout_identity(),
            admitted.types()[index].layout_identity()
        );
        assert_eq!(
            changed.types()[index].layout(),
            admitted.types()[index].layout()
        );
        assert_ne!(changed.semantic_sha256(), admitted.semantic_sha256());
    }
}

#[test]
fn context_nominal_v40_mixed_roots_and_defined_helper_retain_one_document() {
    let mut request = request(true, true);
    let mut second = minimal_request().functions[0].clone();
    second.identity = SemanticFunctionIdentityV1(identity(9));
    second.abi.identity = SemanticAbiIdentityV1(identity(90));
    second.abi.layout_identity = SemanticLayoutIdentityV1(identity(91));
    let mut helper = request.functions[0].clone();
    helper.identity = SemanticFunctionIdentityV1(identity(10));
    helper.role = SemanticFunctionRoleV1::InternalHelper;
    let intrinsic = request.callables[1].clone();
    request.functions = vec![request.functions[0].clone(), second, helper].into_boxed_slice();
    request.roots = vec![SemanticFunctionIdV1(0), SemanticFunctionIdV1(1)].into_boxed_slice();
    request.callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1(1)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1(2)),
        intrinsic,
    ]
    .into_boxed_slice();
    for function in &mut request.functions {
        if let SemanticTerminatorKindV1::Call(call) = &mut function.blocks[0].terminator.kind {
            call.callee = SemanticCallableIdV1(3);
        }
    }
    let arguments = request.functions[0]
        .abi
        .source_input_types()
        .iter()
        .enumerate()
        .map(|(argument, ty)| {
            let local = request.functions[0]
                .locals
                .iter()
                .position(|local| local.role == SemanticLocalRoleV1::Argument(argument as u32))
                .unwrap();
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1(local as u32), vec![], *ty).unwrap(),
            )
        })
        .collect();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(2),
        arguments,
        Some(SemanticCallDestinationV1::new(
            place(&request, SemanticTypeIdV1(0)),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    request.functions[0].blocks[0].terminator.kind = SemanticTerminatorKindV1::Call(call);
    let admitted = roundtrip(request.clone());
    assert_eq!(admitted.roots().len(), 2);
    assert_eq!(admitted.functions()[1].abi().source_input_types().len(), 1);
    assert!(
        admitted.functions()[1]
            .locals()
            .iter()
            .all(|local| local.ty() == SemanticTypeIdV1(0))
    );
    assert_eq!(
        admitted.functions()[2].role(),
        SemanticFunctionRoleV1::InternalHelper
    );
    let SemanticTerminatorKindV1::Call(call) = &mut request.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    call.arguments.swap(1, 2);
    assert!(matches!(
        request.admit_exact_v40(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::TypeMismatch { .. })
    ));
}

fn rust_call_request() -> InertSemanticMirRequestV1 {
    let mut request = request(true, true);
    let unsigned = SemanticTypeIdV1(request.types.len() as u32 - 2);
    let signed = SemanticTypeIdV1(request.types.len() as u32 - 1);
    let tuple = SemanticTypeIdV1(request.types.len() as u32);
    let mut types = request.types.to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(150)),
        SemanticLayoutIdentityV1(identity(150)),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![unsigned, signed, CONTEXT]).unwrap(),
        ),
    ));
    request.types = types.into_boxed_slice();
    let original = &request.functions[0].abi;
    let arguments = vec![
        SemanticAbiArgumentV1::source(original.arguments[0].value.clone()),
        SemanticAbiArgumentV1::rust_call_tuple_field(0, original.arguments[1].value.clone()),
        SemanticAbiArgumentV1::rust_call_tuple_field(1, original.arguments[2].value.clone()),
        SemanticAbiArgumentV1::rust_call_tuple_field(
            2,
            SemanticAbiValueV1::new(CONTEXT, SemanticAbiPassModeV1::Ignore),
        ),
    ];
    request.functions[0].abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        original.identity,
        original.layout_identity,
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![SemanticTypeIdV1(0), tuple],
        SemanticTypeIdV1(0),
        arguments,
        original.return_value.clone(),
    )
    .unwrap();
    for local in &mut request.functions[0].locals {
        if let Some(field) = [unsigned, signed, CONTEXT]
            .iter()
            .position(|ty| *ty == local.ty)
        {
            local.role = SemanticLocalRoleV1::RustCallTupleField {
                argument: 1,
                field: field as u32,
            };
        }
    }
    request
}

#[test]
fn context_nominal_v40_rust_call_retains_nominal_and_ignored_context_fields() {
    let request = rust_call_request();
    let admitted = roundtrip(request.clone());
    let map = admitted
        .logical_arguments_v1(SemanticFunctionIdV1(0))
        .unwrap();
    assert_eq!(map.source_arguments().count(), 2);
    let adjusted: Vec<_> = map.adjusted_arguments().collect();
    assert_eq!(adjusted.len(), 4);
    assert_eq!(adjusted[3].tuple_field(), Some(2));
    assert!(matches!(
        adjusted[3].abi().mode(),
        SemanticAbiPassModeV1::Ignore
    ));
    assert_eq!(
        admitted.types()[adjusted[3].abi().ty().index() as usize].rust_type_kind(),
        SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::KernelContext)
    );
    for role in [
        SemanticLocalRoleV1::Temporary,
        SemanticLocalRoleV1::RustCallTupleField {
            argument: 1,
            field: 0,
        },
    ] {
        let mut changed = request.clone();
        changed.functions[0]
            .locals
            .iter_mut()
            .find(|local| local.ty == CONTEXT)
            .unwrap()
            .role = role;
        assert!(matches!(
            changed.admit_exact_v40(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidLocalRoles { .. })
        ));
    }
}

#[test]
fn context_nominal_v40_bad_shapes_and_role_erasure_remain_refused() {
    let original = request(true, true);
    for fault in 0..7 {
        let mut changed = original.clone();
        let nominal = changed.types.len() - 2;
        match fault {
            0 => {
                changed.types[nominal].shape =
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: true,
                        bits: 64,
                    })
            }
            1 => {
                changed.types[nominal]
                    .abi_properties
                    .rustc_layout_is_noundef = false
            }
            2 => changed.types[nominal].layout.alignment_bytes = 4,
            3 => changed.types[CONTEXT.index() as usize].shape = SemanticTypeShapeV1::Unit,
            4 => {
                changed.types[CONTEXT.index() as usize].rust_type_kind =
                    SemanticRustTypeKindV1::Ordinary
            }
            5 => {
                changed.types[9].rust_type_kind =
                    SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::MaskedTileU32 {
                        lanes: 0,
                        elements: 2,
                    })
            }
            6 => {
                let SemanticTypeShapeV1::Aggregate(fields) =
                    &mut changed.types[CONTEXT.index() as usize].shape
                else {
                    unreachable!()
                };
                fields.fields[0] = SemanticTypeIdV1(0);
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                changed.admit_exact_v40(SemanticMirLimitsV1::default()),
                Err(SemanticMirErrorV1::InvalidTypeLayout | SemanticMirErrorV1::InvalidFunctionAbi)
            ),
            "fault {fault}"
        );
    }
}

#[test]
fn context_nominal_v40_constants_and_aggregate_spelling_cannot_issue_context() {
    let original = request(true, true);
    for kind in [
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            CONTEXT,
            SemanticConstantValueV1::ZeroSized,
        ))),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Aggregate,
            vec![
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    MARKER,
                    SemanticConstantValueV1::ZeroSized
                ));
                5
            ],
        )
        .unwrap(),
    ] {
        let mut changed = original.clone();
        changed.functions[0].blocks[0].statements = vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(&changed, CONTEXT),
                SemanticRvalueV1::new(CONTEXT, kind),
            )),
        )]
        .into_boxed_slice();
        assert!(matches!(
            changed.admit_exact_v40(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeOperation {
                operation: SemanticTypeOperationV1::Constant | SemanticTypeOperationV1::Aggregate,
                ..
            })
        ));
    }
    let mut nested = original;
    let wrapper = SemanticTypeIdV1(nested.types.len() as u32);
    let mut types = nested.types.to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(150)),
        SemanticLayoutIdentityV1(identity(150)),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![CONTEXT]).unwrap()),
    ));
    nested.types = types.into_boxed_slice();
    let mut locals = nested.functions[0].locals.to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(202)),
        wrapper,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    nested.functions[0].locals = locals.into_boxed_slice();
    roundtrip(nested.clone());
    nested.functions[0].blocks[0].statements = vec![SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(&nested, wrapper),
            SemanticRvalueV1::new(
                wrapper,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    wrapper,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ),
        )),
    )]
    .into_boxed_slice();
    assert!(matches!(
        nested.admit_exact_v40(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Constant,
            ..
        })
    ));
}

#[test]
fn context_nominal_v40_intrinsic_tags_are_closed_and_reuse_execution_bytes() {
    let operations = [
        SemanticExecutionOperationV29::ContextIssue { context: CONTEXT },
        SemanticExecutionOperationV29::WorkgroupDerive {
            context: CONTEXT,
            workgroup: SemanticTypeIdV1(6),
        },
        SemanticExecutionOperationV29::MaskedTileLoadU32 {
            workgroup: SemanticTypeIdV1(6),
            tile: SemanticTypeIdV1(9),
        },
        SemanticExecutionOperationV29::MaskedTileIntoFragmentU32 {
            tile: SemanticTypeIdV1(9),
            fragment: SemanticTypeIdV1(10),
        },
        SemanticExecutionOperationV29::LaneFragmentIntoPartsU32 {
            fragment: SemanticTypeIdV1(10),
            parts: SemanticTypeIdV1(11),
        },
    ];
    for (operation, tag) in operations.into_iter().zip([81, 82, 84, 85, 86]) {
        let operation = SemanticCompilerIntrinsicOperationV1::Execution(operation);
        let mut old = CanonicalWriterV1::new(64);
        let mut shared = CanonicalWriterV1::new(64);
        encode_compiler_intrinsic_operation(&mut old, operation, SemanticMirWireVersionV1::V29)
            .unwrap();
        encode_compiler_intrinsic_operation(&mut shared, operation, V40).unwrap();
        let bytes = shared.finish();
        assert_eq!(bytes, old.finish());
        assert_eq!(bytes[0], tag);
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = V40;
        assert_eq!(decoder.compiler_intrinsic().unwrap(), operation);
        decoder.finish().unwrap();
    }
    for tag in (69..=80).chain([83]).chain(87..=255) {
        let bytes = [tag];
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = V40;
        assert!(
            matches!(decoder.compiler_intrinsic(), Err(SemanticMirDecodeErrorV1::InvalidTag { value, .. }) if value == tag)
        );
    }
}

#[test]
fn context_nominal_v40_excludes_every_specialized_sibling_family() {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    use SemanticMirWireVersionV1 as Version;
    let mut descriptors = [0; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32];
    descriptors[0] = 8;
    let operations = [
        (
            Op::SaturatingInteger(SemanticSaturatingIntegerOpV1::Add),
            Version::V30,
        ),
        (
            Op::Gfx942OrderedRegion(SemanticGfx942OrderedRegionProfileV31::XorAddU32E32),
            Version::V31,
        ),
        (
            Op::Gfx942OrderedProgram(
                SemanticGfx942U32ProgramV32::from_descriptors(1, descriptors).unwrap(),
            ),
            Version::V32,
        ),
        (
            Op::Gfx942Wave64ShuffleIndex {
                context: CONTEXT,
                element: SemanticTypeIdV1(0),
            },
            Version::V33,
        ),
        (
            Op::Gfx942InlineU32(
                SemanticGfx942InlineU32V30::new(SemanticGfx942InlineInstructionV30::VMovB32, 1)
                    .unwrap(),
            ),
            Version::V34,
        ),
        (
            Op::Gfx942CompleteBody(SemanticCompleteBodyPackingVNext {
                block_count: 1,
                instruction_count: 1,
                block_words: [0x41ff, 0, 0, 0],
                instruction_words: [8, 0, 0, 0],
            }),
            Version::V36,
        ),
        (Op::Gfx942PhysicalEntryBegin, Version::V37),
        (Op::Gfx942PhysicalGlobalCopyBegin, Version::V38),
        (
            Op::Gfx942PhysicalLdsExchangeBegin(
                SemanticPhysicalLdsExchangeFrameV39::new(0, 512, 4, 1).unwrap(),
            ),
            Version::V39,
        ),
    ];
    for (operation, required) in operations {
        let mut writer = CanonicalWriterV1::new(4096);
        writer.raw(&[71, 72]).unwrap();
        assert_eq!(
            encode_compiler_intrinsic_operation(&mut writer, operation, V40),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: V40,
                required
            })
        );
        assert_eq!(writer.finish(), [71, 72]);
        let mut mixed = request(true, true);
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: actual, ..
        } = &mut mixed.callables[1]
        else {
            unreachable!()
        };
        *actual = operation;
        // Membership must refuse before the intentionally incompatible signature.
        assert_eq!(
            mixed
                .admit_exact_v40(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: V40,
                required
            }
        );
    }
}

#[test]
fn context_nominal_v40_cannot_silently_drop_specialized_source_tails() {
    let original = request(true, true);
    let SemanticTerminatorKindV1::Call(call) = &original.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    let scalar_source = SemanticInlineAssemblySourceV30::new(
        identity(1),
        SemanticFunctionIdentityV1(identity(2)),
        identity(3),
        identity(4),
    )
    .unwrap();
    let physical_source = SemanticPhysicalGlobalCopySourceV38::new(
        [
            identity(1),
            identity(2),
            identity(3),
            identity(4),
            identity(5),
        ],
        identity(6),
        identity(7),
        identity(8),
        identity(9),
        identity(10),
        (0, 0),
    )
    .unwrap();
    for (call, required) in [
        (
            call.clone().with_inline_assembly_source_v30(scalar_source),
            SemanticMirWireVersionV1::V34,
        ),
        (
            call.clone()
                .with_physical_global_copy_source_v38(physical_source),
            SemanticMirWireVersionV1::V38,
        ),
    ] {
        let mut writer = CanonicalWriterV1::new(4096);
        writer.raw(&[71, 72]).unwrap();
        assert_eq!(
            wire_schema_membership_v1::encode_direct_call(&mut writer, &call, V40),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: V40,
                required,
            })
        );
        assert_eq!(writer.finish(), [71, 72]);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].terminator.kind = SemanticTerminatorKindV1::Call(call);
        assert_eq!(
            changed
                .admit_exact_v40(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: V40,
                required,
            }
        );
    }
}

fn required_work(request: &InertSemanticMirRequestV1) -> u64 {
    let limits = SemanticMirLimitsV1::default();
    request.clone().admit_exact_v40(limits).unwrap();
    let (mut low, mut high) = (0, limits.limit(SemanticMirResourceV1::ValidationWork));
    while low < high {
        let middle = low + (high - low) / 2;
        match request.clone().admit_exact_v40(
            limits
                .with_limit(SemanticMirResourceV1::ValidationWork, middle)
                .unwrap(),
        ) {
            Ok(_) => high = middle,
            Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                actual,
                max,
            }) => {
                assert_eq!((actual, max), (middle + 1, middle));
                low = middle + 1;
            }
            Err(error) => panic!("unexpected work probe: {error:?}"),
        }
    }
    low
}

#[test]
fn context_nominal_v40_validation_work_keeps_exact_nominal_suffix_and_first_denial() {
    let original = request(true, true);
    let mut ordinary_integers = original.clone();
    for ty in &mut ordinary_integers.types {
        if matches!(
            ty.rust_type_kind,
            SemanticRustTypeKindV1::Usize | SemanticRustTypeKindV1::Isize
        ) {
            ty.rust_type_kind = SemanticRustTypeKindV1::Ordinary;
        }
    }
    let work = required_work(&original);
    assert_eq!(
        work,
        required_work(&ordinary_integers)
            + 2 * nominal_pointer_sized_v35::NOMINAL_TYPE_WORK_V35 as u64
    );
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, work)
        .unwrap();
    original.clone().admit_exact_v40(limits).unwrap();
    assert_eq!(
        original
            .clone()
            .admit_exact_v40(
                limits
                    .with_limit(SemanticMirResourceV1::ValidationWork, work - 1)
                    .unwrap()
            )
            .unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            actual: work,
            max: work - 1
        }
    );
    assert_eq!(
        original
            .admit_exact_v40(
                limits
                    .with_limit(SemanticMirResourceV1::ValidationWork, 0)
                    .unwrap()
            )
            .unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            actual: 1,
            max: 0
        }
    );
}

#[test]
fn context_nominal_v40_exact_bytes_types_and_truncation_are_bounded() {
    let original = request(true, true);
    let admitted = roundtrip(original.clone());
    let bytes = admitted.canonical_encoding();
    let count = bytes.len() as u64;
    let limits = SemanticMirLimitsV1::default();
    let exact = limits
        .with_limit(SemanticMirResourceV1::CanonicalBytes, count)
        .unwrap();
    assert_eq!(
        original
            .clone()
            .admit_exact_v40(exact)
            .unwrap()
            .canonical_encoding(),
        bytes
    );
    AdmittedInertSemanticMirV1::decode_exact_v40_canonical(bytes, exact).unwrap();
    let short = exact
        .with_limit(SemanticMirResourceV1::CanonicalBytes, count - 1)
        .unwrap();
    assert_eq!(
        original.clone().admit_exact_v40(short).unwrap_err(),
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            actual: count,
            max: count - 1
        }
    );
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_exact_v40_canonical(bytes, short).unwrap_err(),
        SemanticMirDecodeErrorV1::InputLimitExceeded {
            actual: count,
            max: count - 1
        }
    );
    let type_count = original.types.len() as u64;
    original
        .clone()
        .admit_exact_v40(
            limits
                .with_limit(SemanticMirResourceV1::Types, type_count)
                .unwrap(),
        )
        .unwrap();
    assert!(
        matches!(original.admit_exact_v40(limits.with_limit(SemanticMirResourceV1::Types, type_count - 1).unwrap()),
        Err(SemanticMirErrorV1::LimitExceeded { resource: SemanticMirResourceV1::Types, actual, max }) if actual == type_count && max == type_count - 1)
    );
    for length in 0..bytes.len() {
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v40_canonical(&bytes[..length], limits)
                .is_err()
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v40_canonical(&trailing, limits),
        Err(SemanticMirDecodeErrorV1::TrailingBytes { .. })
    ));
}

#[test]
fn atomic_v41_composition_preserves_ordinary_rust_call_execution_and_nominal_records() {
    const VERSION: SemanticMirWireVersionV1 = SemanticMirWireVersionV1::V41;
    let limits = SemanticMirLimitsV1::default();
    let originals = [
        request(false, false),
        request(false, true),
        request(true, false),
        request(true, true),
        rust_call_request(),
    ];
    for original in originals {
        for add_atomic in [false, true] {
            let mut input = original.clone();
            if add_atomic {
                let mut types = input.types.into_vec();
                let mut previous = SemanticTypeIdV1(0);
                for index in 0..3u8 {
                    let current = SemanticTypeIdV1(types.len() as u32);
                    let mut declaration = SemanticTypeDeclV1::new(
                        SemanticTypeIdentityV1(identity(230 + index)),
                        SemanticLayoutIdentityV1(identity(240 + index)),
                        SemanticTypeLayoutV1::aggregate(
                            Some(4),
                            4,
                            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                        )
                        .unwrap(),
                        SemanticTypeShapeV1::Aggregate(
                            SemanticAggregateTypeV1::new(vec![previous]).unwrap(),
                        ),
                    );
                    if index == 2 {
                        declaration.rust_type_kind = SemanticRustTypeKindV1::AtomicU32;
                    }
                    types.push(declaration);
                    previous = current;
                }
                input.types = types.into_boxed_slice();
                let mut locals = input.functions[0].locals.to_vec();
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1(identity(250)),
                    previous,
                    SemanticLocalRoleV1::Temporary,
                    SemanticSourceProvenanceV1::unavailable(),
                ));
                input.functions[0].locals = locals.into_boxed_slice();
            }
            let admitted = input.clone().admit_exact_v41(limits).unwrap();
            let decoded = AdmittedInertSemanticMirV1::decode_exact_v41_canonical(
                admitted.canonical_encoding(),
                limits,
            )
            .unwrap();
            assert_eq!(decoded.wire_version(), VERSION);
            assert_eq!(decoded.types(), input.types.as_ref());
            assert_eq!(decoded.functions(), input.functions.as_ref());
            assert_eq!(decoded.callables(), input.callables.as_ref());
            assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
            assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
            if !add_atomic {
                let old = input.admit_exact_v40(limits).unwrap();
                assert_eq!(
                    &old.canonical_encoding()[MAGIC.len() + 2..],
                    &admitted.canonical_encoding()[MAGIC.len() + 2..]
                );
                assert_ne!(old.semantic_sha256(), admitted.semantic_sha256());
            }
        }
    }
}

#[test]
fn atomic_v41_composition_intrinsic_tags_are_closed_and_reuse_execution_bytes() {
    let operations = [
        SemanticExecutionOperationV29::ContextIssue { context: CONTEXT },
        SemanticExecutionOperationV29::WorkgroupDerive {
            context: CONTEXT,
            workgroup: SemanticTypeIdV1(6),
        },
        SemanticExecutionOperationV29::MaskedTileLoadU32 {
            workgroup: SemanticTypeIdV1(6),
            tile: SemanticTypeIdV1(9),
        },
        SemanticExecutionOperationV29::MaskedTileIntoFragmentU32 {
            tile: SemanticTypeIdV1(9),
            fragment: SemanticTypeIdV1(10),
        },
        SemanticExecutionOperationV29::LaneFragmentIntoPartsU32 {
            fragment: SemanticTypeIdV1(10),
            parts: SemanticTypeIdV1(11),
        },
    ];
    for (operation, tag) in operations.into_iter().zip([81, 82, 84, 85, 86]) {
        let operation = SemanticCompilerIntrinsicOperationV1::Execution(operation);
        let mut old = CanonicalWriterV1::new(64);
        let mut shared = CanonicalWriterV1::new(64);
        encode_compiler_intrinsic_operation(&mut old, operation, SemanticMirWireVersionV1::V29)
            .unwrap();
        encode_compiler_intrinsic_operation(&mut shared, operation, SemanticMirWireVersionV1::V41)
            .unwrap();
        let bytes = shared.finish();
        assert_eq!(bytes, old.finish());
        assert_eq!(bytes[0], tag);
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V41;
        assert_eq!(decoder.compiler_intrinsic().unwrap(), operation);
        decoder.finish().unwrap();
    }
    for tag in (69..=80).chain([83]).chain(87..=255) {
        let bytes = [tag];
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V41;
        assert!(
            matches!(decoder.compiler_intrinsic(), Err(SemanticMirDecodeErrorV1::InvalidTag { value, .. }) if value == tag)
        );
    }
}

#[test]
fn atomic_v41_composition_excludes_every_specialized_sibling_family() {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    use SemanticMirWireVersionV1 as Version;
    let mut descriptors = [0; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32];
    descriptors[0] = 8;
    let operations = [
        (
            Op::SaturatingInteger(SemanticSaturatingIntegerOpV1::Add),
            Version::V30,
        ),
        (
            Op::Gfx942OrderedRegion(SemanticGfx942OrderedRegionProfileV31::XorAddU32E32),
            Version::V31,
        ),
        (
            Op::Gfx942OrderedProgram(
                SemanticGfx942U32ProgramV32::from_descriptors(1, descriptors).unwrap(),
            ),
            Version::V32,
        ),
        (
            Op::Gfx942Wave64ShuffleIndex {
                context: CONTEXT,
                element: SemanticTypeIdV1(0),
            },
            Version::V33,
        ),
        (
            Op::Gfx942InlineU32(
                SemanticGfx942InlineU32V30::new(SemanticGfx942InlineInstructionV30::VMovB32, 1)
                    .unwrap(),
            ),
            Version::V34,
        ),
        (
            Op::Gfx942CompleteBody(SemanticCompleteBodyPackingVNext {
                block_count: 1,
                instruction_count: 1,
                block_words: [0x41ff, 0, 0, 0],
                instruction_words: [8, 0, 0, 0],
            }),
            Version::V36,
        ),
        (Op::Gfx942PhysicalEntryBegin, Version::V37),
        (Op::Gfx942PhysicalGlobalCopyBegin, Version::V38),
        (
            Op::Gfx942PhysicalLdsExchangeBegin(
                SemanticPhysicalLdsExchangeFrameV39::new(0, 512, 4, 1).unwrap(),
            ),
            Version::V39,
        ),
    ];
    for (operation, required) in operations {
        let mut writer = CanonicalWriterV1::new(4096);
        writer.raw(&[71, 72]).unwrap();
        assert_eq!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                operation,
                SemanticMirWireVersionV1::V41
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V41,
                required
            })
        );
        assert_eq!(writer.finish(), [71, 72]);
        let mut mixed = request(true, true);
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: actual, ..
        } = &mut mixed.callables[1]
        else {
            unreachable!()
        };
        *actual = operation;
        // Membership must refuse before the intentionally incompatible signature.
        assert_eq!(
            mixed
                .admit_exact_v41(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V41,
                required
            }
        );
    }
}

#[test]
fn atomic_v41_composition_cannot_silently_drop_specialized_source_tails() {
    let original = request(true, true);
    let SemanticTerminatorKindV1::Call(call) = &original.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    let scalar_source = SemanticInlineAssemblySourceV30::new(
        identity(1),
        SemanticFunctionIdentityV1(identity(2)),
        identity(3),
        identity(4),
    )
    .unwrap();
    let physical_source = SemanticPhysicalGlobalCopySourceV38::new(
        [
            identity(1),
            identity(2),
            identity(3),
            identity(4),
            identity(5),
        ],
        identity(6),
        identity(7),
        identity(8),
        identity(9),
        identity(10),
        (0, 0),
    )
    .unwrap();
    for (call, required) in [
        (
            call.clone().with_inline_assembly_source_v30(scalar_source),
            SemanticMirWireVersionV1::V34,
        ),
        (
            call.clone()
                .with_physical_global_copy_source_v38(physical_source),
            SemanticMirWireVersionV1::V38,
        ),
    ] {
        let mut writer = CanonicalWriterV1::new(4096);
        writer.raw(&[71, 72]).unwrap();
        assert_eq!(
            wire_schema_membership_v1::encode_direct_call(
                &mut writer,
                &call,
                SemanticMirWireVersionV1::V41
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V41,
                required,
            })
        );
        assert_eq!(writer.finish(), [71, 72]);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].terminator.kind = SemanticTerminatorKindV1::Call(call);
        assert_eq!(
            changed
                .admit_exact_v41(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V41,
                required,
            }
        );
    }
}
