use super::*;

mod declaration_commitment_tests;

fn abi_value(request: &InertSemanticMirRequestV1, id: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let ty = &request.types[id.0 as usize];
    let scalar = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let pointer = || {
        let info = ty.abi_properties.first_pointee.unwrap();
        let shared = matches!(
            info.kind,
            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
        );
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                shared,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            info.guaranteed_size_bytes,
            (info.reliable_alignment_bytes > 1).then_some(info.reliable_alignment_bytes),
        )
        .unwrap()
    };
    let mode = if ty.layout.size_bytes == Some(0) {
        SemanticAbiPassModeV1::Ignore
    } else {
        match ty.layout.backend_repr {
            SemanticBackendReprV1::Memory { .. } => SemanticAbiPassModeV1::Indirect {
                attributes: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesNone),
                        true,
                        false,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    ty.layout.size_bytes.unwrap(),
                    Some(ty.layout.alignment_bytes),
                )
                .unwrap(),
                metadata_attributes: None,
                on_stack: false,
            },
            SemanticBackendReprV1::ScalarPair { .. } => SemanticAbiPassModeV1::Pair {
                first: if ty.abi_properties.first_pointee.is_some() {
                    pointer()
                } else {
                    scalar
                },
                second: scalar,
            },
            SemanticBackendReprV1::Scalar(_) => SemanticAbiPassModeV1::Direct(
                if matches!(ty.shape, SemanticTypeShapeV1::Pointer(_)) {
                    pointer()
                } else {
                    scalar
                },
            ),
            _ => unreachable!(),
        }
    };
    SemanticAbiValueV1::new(id, mode)
}

fn callable_request(ordinal: usize) -> InertSemanticMirRequestV1 {
    let mut request = request(2, 2);
    let (inputs, output) = match ordinal {
        0 => (vec![], CONTEXT),
        1 => (vec![CONTEXT_REF], WORKGROUP),
        2 => (vec![WORKGROUP_REF, SLICE_REF, INDEX], TILE),
        3 => (vec![TILE], FRAGMENT),
        4 => (vec![FRAGMENT], PARTS),
        _ => unreachable!(),
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1(identity(60)),
        SemanticLayoutIdentityV1(identity(61)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs.iter().map(|ty| abi_value(&request, *ty)).collect(),
        abi_value(&request, output),
    )
    .unwrap();
    let place = |ty| {
        let index = request.functions[0]
            .locals
            .iter()
            .position(|local| local.ty == ty)
            .unwrap();
        SemanticPlaceV1::new(SemanticLocalIdV1(index as u32), vec![], ty).unwrap()
    };
    let source = SemanticSourceProvenanceV1::unavailable();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1(1),
        inputs
            .into_iter()
            .map(|ty| SemanticOperandV1::Move(place(ty)))
            .collect(),
        Some(SemanticCallDestinationV1::new(
            place(output),
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1(1)),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    request.functions[0].blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(7)),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(8)),
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
                SemanticFunctionIdentityV1(identity(62)),
                SemanticItemDefinitionIdentityV1(identity(63)),
                SemanticMonomorphizationIdentityV1(identity(64)),
                SemanticGenericTypeArgumentsIdentityV1(identity(65)),
                SemanticConstGenericArgumentsIdentityV1(identity(66)),
                source,
                abi,
            ),
            operation: SemanticCompilerIntrinsicOperationV1::Execution(operations()[ordinal]),
            operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(67)),
        },
    ]
    .into_boxed_slice();
    request
}

#[test]
fn capability_full_callable_documents_round_trip_with_actual_abis() {
    let limits = SemanticMirLimitsV1::default();
    for ordinal in 0..5 {
        let request = callable_request(ordinal);
        let admitted = request
            .clone()
            .admit_exact_v29(limits)
            .unwrap_or_else(|error| panic!("operation {ordinal}: {error:?}"));
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
            admitted.canonical_encoding(),
            limits,
        )
        .unwrap();
        assert_eq!(decoded.callables(), admitted.callables());
        assert_eq!(decoded.functions(), admitted.functions());
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        assert!(request.admit_current_production(limits).is_err());
    }
}

#[test]
fn capability_full_callable_documents_reject_same_layout_role_substitution() {
    let limits = SemanticMirLimitsV1::default();
    for (ordinal, replaced) in [
        (0, CONTEXT),
        (1, WORKGROUP),
        (2, TILE),
        (3, FRAGMENT),
        (4, FRAGMENT),
    ] {
        let mut request = callable_request(ordinal);
        request.types[replaced.0 as usize].rust_type_kind = SemanticRustTypeKindV1::Ordinary;
        assert_eq!(
            request.admit_exact_v29(limits).unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi,
            "operation {ordinal}"
        );
    }
}

#[test]
fn capability_full_callable_documents_preserve_borrow_abi_constraints() {
    let limits = SemanticMirLimitsV1::default();
    for ordinal in [1, 2] {
        let mut request = callable_request(ordinal);
        let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut request.callables[1]
        else {
            unreachable!()
        };
        binding.abi.arguments[0].value.mode = SemanticAbiPassModeV1::Ignore;
        assert_eq!(
            request.admit_exact_v29(limits).unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi
        );
    }
    let mut request = callable_request(2);
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut request.callables[1]
    else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::Execution(Op::MaskedTileLoadU32 {
        workgroup: CONTEXT,
        tile: TILE,
    });
    assert_eq!(
        request.admit_exact_v29(limits).unwrap_err(),
        SemanticMirErrorV1::InvalidFunctionAbi
    );
}

#[test]
fn capability_numeric_scalar_pair_workgroup_abi_is_preserved() {
    let mut request = callable_request(1);
    let ty = &mut request.types[WORKGROUP.0 as usize];
    let SemanticTypeLayoutDetailsV1::Aggregate(fields) = ty.layout.details.clone() else {
        unreachable!()
    };
    let scalar = integer(64, u64::MAX.into());
    ty.layout = SemanticTypeLayoutV1::aggregate_with_backend_repr(
        Some(16),
        8,
        SemanticBackendReprV1::scalar_pair(scalar, scalar),
        false,
        fields,
    )
    .unwrap();
    let value = abi_value(&request, WORKGROUP);
    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut request.callables[1]
    else {
        unreachable!()
    };
    binding.abi.return_value = value;
    let limits = SemanticMirLimitsV1::default();
    let admitted = request.admit_exact_v29(limits).unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
        admitted.canonical_encoding(),
        limits,
    )
    .unwrap();
    assert_eq!(decoded.types(), admitted.types());
    assert_eq!(decoded.callables(), admitted.callables());
}

#[test]
fn capability_call_sites_require_exact_argument_and_result_types() {
    let limits = SemanticMirLimitsV1::default();
    let mut request = callable_request(2);
    let SemanticTerminatorKindV1::Call(call) = &mut request.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    call.arguments.swap(0, 1);
    let location = SemanticMirLocationV1::Terminator {
        function: SemanticFunctionIdV1(0),
        block: SemanticBlockIdV1(0),
    };
    assert_eq!(
        request.admit_exact_v29(limits).unwrap_err(),
        SemanticMirErrorV1::TypeMismatch {
            expected: WORKGROUP_REF,
            actual: SLICE_REF,
            location,
        }
    );
    let mut request = callable_request(3);
    let local = request.functions[0]
        .locals
        .iter()
        .position(|local| local.ty == TILE)
        .unwrap();
    let SemanticTerminatorKindV1::Call(call) = &mut request.functions[0].blocks[0].terminator.kind
    else {
        unreachable!()
    };
    call.destination.as_mut().unwrap().place =
        SemanticPlaceV1::new(SemanticLocalIdV1(local as u32), vec![], TILE).unwrap();
    assert_eq!(
        request.admit_exact_v29(limits).unwrap_err(),
        SemanticMirErrorV1::TypeMismatch {
            expected: FRAGMENT,
            actual: TILE,
            location,
        }
    );
}

#[test]
fn capability_equal_role_and_geometry_do_not_replace_exact_type_identity() {
    let mut request = callable_request(3);
    let mut duplicate = request.types[FRAGMENT.0 as usize].clone();
    duplicate.identity = SemanticTypeIdentityV1(identity(200));
    duplicate.layout_identity = SemanticLayoutIdentityV1(identity(200));
    let mut types = request.types.into_vec();
    let alternate = SemanticTypeIdV1(types.len() as u32);
    types.push(duplicate);
    request.types = types.into_boxed_slice();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut request.callables[1]
    else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::Execution(Op::MaskedTileIntoFragmentU32 {
        tile: TILE,
        fragment: alternate,
    });
    assert_eq!(
        request
            .admit_exact_v29(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidFunctionAbi
    );
}

#[test]
fn capability_parts_result_must_be_inhabited() {
    let mut request = callable_request(4);
    request.types[PARTS.0 as usize].layout.uninhabited = true;
    assert_eq!(
        request
            .admit_exact_v29(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidFunctionAbi
    );
}

#[test]
fn capability_borrows_reject_wrong_mutability_even_with_consistent_abi() {
    for (ordinal, reference, mutability, kind) in [
        (
            1,
            CONTEXT_REF,
            SemanticMutabilityV1::Immutable,
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
        ),
        (
            2,
            WORKGROUP_REF,
            SemanticMutabilityV1::Mutable,
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
        ),
    ] {
        let mut request = callable_request(ordinal);
        let ty = &mut request.types[reference.0 as usize];
        let SemanticTypeShapeV1::Pointer(pointer) = &mut ty.shape else {
            unreachable!()
        };
        pointer.mutability = mutability;
        ty.abi_properties.first_pointee.as_mut().unwrap().kind = kind;
        let value = abi_value(&request, reference);
        let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut request.callables[1]
        else {
            unreachable!()
        };
        binding.abi.arguments[0].value = value;
        assert_eq!(
            request
                .admit_exact_v29(SemanticMirLimitsV1::default())
                .unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi
        );
    }
}
