fn numerical_contract() -> SemanticExecutionCapabilityContractV1 {
    execution_contract(
        SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
            context: SemanticTypeIdV1(2),
            capability: SemanticTypeIdV1(3),
            policy: SemanticTypeIdentityV1(identity(14)),
        },
    )
    .unwrap()
}

fn numerical_abi() -> SemanticFunctionAbiV1 {
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1(identity(40)),
        SemanticLayoutIdentityV1(identity(41)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticTypeIdV1(2)],
        SemanticTypeIdV1(3),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SemanticTypeIdV1(2),
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(SemanticTypeIdV1(3), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap()
}

fn numerical_request() -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let unit = SemanticTypeIdV1(0);
    request.types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1(identity(1)),
        SemanticLayoutIdentityV1(identity(2)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    request.functions[0].abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1(identity(3)),
        SemanticLayoutIdentityV1(identity(4)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    request.functions[0].locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(5)),
        unit,
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )]
    .into_boxed_slice();
    let zst = |tag| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(tag)),
            SemanticLayoutIdentityV1(identity(tag)),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        )
    };
    let mut types = request.types.to_vec();
    types.push(zst(11));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(12)),
            SemanticLayoutIdentityV1(identity(12)),
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
                    SemanticTypeIdV1(1),
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
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_rustc_layout_is_noundef(true)
                .with_scalar_pointee_info(
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
        ),
    );
    types.push(zst(13));
    request.types = types.into_boxed_slice();
    request.functions[0] =
        request.functions[0]
            .clone()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"numerical_entry".to_vec()).unwrap(),
                numerical_contract().provenance().kernel_binding(),
                SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
            ));
    let mut callables = request.callables.to_vec();
    let source = SemanticSourceProvenanceV1::unavailable();
    let context = SemanticTypeIdV1(1);
    let reference = SemanticTypeIdV1(2);
    let capability = SemanticTypeIdV1(3);
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1(identity(111)),
            SemanticItemDefinitionIdentityV1(identity(111)),
            SemanticMonomorphizationIdentityV1(identity(111)),
            SemanticGenericTypeArgumentsIdentityV1(identity(111)),
            SemanticConstGenericArgumentsIdentityV1(identity(111)),
            source,
            SemanticFunctionAbiV1::from_rustc_with_source_signature(
                SemanticAbiIdentityV1(identity(112)),
                SemanticLayoutIdentityV1(identity(41)),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                0,
                vec![],
                context,
                vec![],
                SemanticAbiValueV1::new(context, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context },
        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(111)),
    });
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            numerical_contract().source_identity(),
            SemanticItemDefinitionIdentityV1(identity(110)),
            SemanticMonomorphizationIdentityV1(identity(110)),
            SemanticGenericTypeArgumentsIdentityV1(identity(110)),
            SemanticConstGenericArgumentsIdentityV1(identity(110)),
            SemanticSourceProvenanceV1::unavailable(),
            numerical_abi(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
            contract: numerical_contract(),
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(110)),
    });
    // Keep the defined prefix index-aligned and sort the intrinsic suffix by source identity.
    callables[request.functions.len()..].sort_by_key(|callable| match callable {
        SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } => binding.identity(),
        _ => unreachable!("fixture suffix contains only compiler intrinsics"),
    });
    request.callables = callables.into_boxed_slice();
    request.functions[0].locals = [unit, context, reference, capability]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(120 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    let place = |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], ty).unwrap();
    let call = |callee, arguments, local, ty, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(local, ty),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(tag)),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    request.functions[0].blocks = vec![
        block(130, vec![], call(2, vec![], 1, context, 1)),
        block(
            131,
            vec![SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(2, reference),
                    SemanticRvalueV1::new(
                        reference,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(1, context),
                        },
                    ),
                )),
            )],
            call(
                1,
                vec![SemanticOperandV1::Copy(place(2, reference))],
                3,
                capability,
                2,
            ),
        ),
        block(132, vec![], SemanticTerminatorKindV1::Return),
    ]
    .into_boxed_slice();
    request
}

#[test]
fn numerical_policy_selects_v18_and_round_trips_exact_source_and_open_obligations() {
    let contract = numerical_contract();
    let operation = SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract };
    let encoded = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V18);
    assert_eq!(&encoded[..2], &[73, 23]);
    assert_eq!(
        contract.obligations().bits(),
        SemanticExecutionSafetyObligationsV1::TARGET_SUPPORT
            | SemanticExecutionSafetyObligationsV1::NUMERICAL_POLICY
    );
    assert_eq!(
        minimum_wire_version(&version_selection_request([operation])),
        SemanticMirWireVersionV1::V18
    );
    for version in [SemanticMirWireVersionV1::V16, SemanticMirWireVersionV1::V17] {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert_eq!(
            encode_compiler_intrinsic_operation(&mut writer, operation, version),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: version,
                required: SemanticMirWireVersionV1::V18
            })
        );
        assert!(writer.finish().is_empty());
        let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
        decoder.wire_version = version;
        assert!(decoder.compiler_intrinsic().is_err());
    }
    let request = numerical_request();
    assert!(compiler_intrinsic_signature_matches(
        &request,
        operation,
        &numerical_abi()
    ));
    let admitted = request.admit(SemanticMirLimitsV1::default()).unwrap();
    assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V18);
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v18_canonical(
        admitted.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v17_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
}

#[test]
fn numerical_policy_reference_abi_rejects_missing_or_inconsistent_rustc_facts() {
    let valid = numerical_request();
    valid.clone().admit(SemanticMirLimitsV1::default()).unwrap();

    let mut missing = valid.clone();
    missing.types[2].abi_properties = SemanticTypeAbiPropertiesV1::new(false, false);
    assert!(matches!(
        missing.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let capture = Some(SemanticAbiPointerCaptureV1::CapturesReadOnly);
    for regular in [
        SemanticAbiRegularAttributesV1::new(false, capture, true, true, false, true),
        SemanticAbiRegularAttributesV1::new(true, None, true, true, false, true),
        SemanticAbiRegularAttributesV1::new(true, capture, false, true, false, true),
        SemanticAbiRegularAttributesV1::new(true, capture, true, false, false, true),
        SemanticAbiRegularAttributesV1::new(true, capture, true, true, true, true),
        SemanticAbiRegularAttributesV1::new(true, capture, true, true, false, false),
    ] {
        let mut bad = valid.clone();
        let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut bad.callables[1]
        else {
            unreachable!()
        };
        binding.abi.arguments[0].value.mode = SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(regular, SemanticAbiExtensionV1::None, 0, None)
                .unwrap(),
        );
        assert!(matches!(
            bad.admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

#[test]
fn numerical_policy_decoder_rejects_forged_policy_source_arity_and_obligations() {
    let encoded = compiler_intrinsic_round_trip(
        SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
            contract: numerical_contract(),
        },
        SemanticMirWireVersionV1::V18,
    );
    let rejects = |bytes: &[u8]| {
        let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V18;
        assert!(decoder.compiler_intrinsic().is_err());
    };
    for length in 0..encoded.len() {
        rejects(&encoded[..length]);
    }
    let mut bad = encoded.clone();
    bad[10..42].fill(0);
    rejects(&bad);
    let mut bad = encoded.clone();
    let length = bad.len();
    bad[length - 32..].fill(0);
    rejects(&bad);
    for count in [0, 2, 5] {
        let mut bad = encoded.clone();
        bad[42] = count;
        rejects(&bad);
    }
    for bits in [0u32, 1, 1 << 16, (1 << 16) | 1 | (1 << 17)] {
        let mut bad = encoded.clone();
        let offset = bad.len() - 36;
        bad[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
        rejects(&bad);
    }
    let mut bad = encoded;
    bad[6..10].copy_from_slice(&2u32.to_le_bytes());
    rejects(&bad);
}

#[test]
fn numerical_policy_admission_rejects_wrong_reference_layout_abi_and_source() {
    let operation = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: numerical_contract(),
    };
    for mutate in [
        (|request: &mut InertSemanticMirRequestV1| {
            let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[2].shape else {
                unreachable!()
            };
            pointer.kind = SemanticPointerKindV1::Raw;
        }) as fn(&mut InertSemanticMirRequestV1),
        |request| {
            let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[2].shape else {
                unreachable!()
            };
            pointer.mutability = SemanticMutabilityV1::Mutable;
        },
        |request| request.types[3].shape = SemanticTypeShapeV1::Unit,
        |request| request.types[3].layout.uninhabited = true,
        |request| request.types[3].layout.alignment_bytes = 2,
    ] {
        let mut bad = numerical_request();
        mutate(&mut bad);
        assert!(!compiler_intrinsic_signature_matches(
            &bad,
            operation,
            &numerical_abi()
        ));
        assert!(bad.admit(SemanticMirLimitsV1::default()).is_err());
    }
    let mut bad = numerical_request();
    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut bad.callables[1] else {
        unreachable!()
    };
    binding.identity = SemanticFunctionIdentityV1(identity(99));
    assert!(bad.admit(SemanticMirLimitsV1::default()).is_err());
    let bad_abi = numerical_abi()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap();
    assert!(!compiler_intrinsic_signature_matches(
        &numerical_request(),
        operation,
        &bad_abi
    ));
}

#[test]
fn numerical_policy_claims_reject_same_type_with_another_policy_or_custody() {
    let contract = numerical_contract();
    let mut claims = IntrinsicCapabilityClaimsV1::default();
    assert!(claims.record_execution_contract(contract));
    assert!(claims.record_execution_contract(contract));
    let mut forged = contract;
    let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { policy, .. } =
        &mut forged.operation
    else {
        unreachable!()
    };
    *policy = SemanticTypeIdentityV1(identity(99));
    assert!(!claims.record_execution_contract(forged));
    let mut forged = contract;
    forged.provenance.issuance = SemanticKernelCapabilityIssuanceIdentityV1(identity(99));
    assert!(!claims.record_execution_contract(forged));
}

#[test]
fn numerical_policy_cannot_alias_any_source_authority_type_identity() {
    for tag in [11, 12, 13] {
        let mut contract = numerical_contract();
        let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { policy, .. } =
            &mut contract.operation
        else {
            unreachable!()
        };
        *policy = SemanticTypeIdentityV1(identity(tag));
        assert!(!compiler_intrinsic_signature_matches(
            &numerical_request(),
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            &numerical_abi(),
        ));
    }
}

#[test]
fn numerical_policy_mixed_v18_records_preserve_legacy_bytes_and_exact_cursors() {
    let legacy = [
        SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
            context: SemanticTypeIdV1(1),
            workgroup: SemanticTypeIdV1(2),
        },
        SemanticExecutionCapabilityOperationV1::SubgroupDerive {
            workgroup: SemanticTypeIdV1(2),
            subgroup: SemanticTypeIdV1(3),
            width: 64,
        },
    ]
    .map(
        |operation| SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
            contract: execution_contract(operation).unwrap(),
        },
    );
    let numerical = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: numerical_contract(),
    };
    let operations = [legacy[0], numerical, legacy[1], numerical, legacy[0]];
    let records = operations
        .map(|operation| compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V18));
    // Fixed record extents also catch coordinated encoder/decoder width drift.
    assert_eq!(
        records.each_ref().map(|record| record.len()),
        [316, 286, 320, 286, 316]
    );
    for index in [0, 2, 4] {
        assert_eq!(
            records[index],
            compiler_intrinsic_round_trip(operations[index], SemanticMirWireVersionV1::V17)
        );
        let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } =
            operations[index]
        else {
            unreachable!()
        };
        let source_offset = records[index].len() - 32;
        assert_eq!(
            &records[index][source_offset - 2..source_offset],
            &u16::try_from(contract.obligations().bits())
                .unwrap()
                .to_le_bytes()
        );
        assert_eq!(
            &records[index][source_offset..],
            contract.source_identity().as_bytes()
        );
    }
    for index in [1, 3] {
        let source_offset = records[index].len() - 32;
        assert_eq!(
            &records[index][source_offset - 4..source_offset],
            &numerical_contract().obligations().bits().to_le_bytes()
        );
    }
    let encoded = records.concat();
    let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V18;
    let mut offset = 0;
    for (operation, record) in operations.iter().zip(&records) {
        assert_eq!(decoder.compiler_intrinsic().unwrap(), *operation);
        offset += record.len();
        assert_eq!(decoder.offset, offset);
    }
    decoder.finish().unwrap();

    let exact_decode = |bytes: &[u8]| {
        let mut decoder = CanonicalDecoderV1::new(bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V18;
        operations
            .iter()
            .all(|expected| decoder.compiler_intrinsic().ok() == Some(*expected))
            && decoder.finish().is_ok()
    };
    for index in 0..records.len() {
        let mut malformed = records.clone();
        let source_offset = malformed[index].len() - 32;
        if index == 1 || index == 3 {
            // A V17-width numerical obligation loses the policy bit and moves the source cursor.
            malformed[index].drain(source_offset - 2..source_offset);
        } else {
            // Widening a preexisting record moves its source identity into the next record.
            malformed[index].splice(source_offset..source_offset, [0, 0]);
        }
        assert!(!exact_decode(&malformed.concat()), "record {index}");
    }
    let mut old_reader = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
    old_reader.wire_version = SemanticMirWireVersionV1::V17;
    assert_eq!(old_reader.compiler_intrinsic().unwrap(), legacy[0]);
    assert_eq!(old_reader.offset, records[0].len());
    assert!(old_reader.compiler_intrinsic().is_err());
}
