fn guarded_read_source_v360(pending_first: bool, mixed: bool) -> ProductionSemanticMirOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;

    const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
    const READ_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
    const READ_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
    const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
    let unavailable = SemanticSourceProvenanceV1::unavailable();
    let declaration = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            layout,
            shape,
        )
    };
    let pointer_scalar = |nonnull| {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(if nonnull { 1 } else { 0 }, u64::MAX.into()),
        )
    };
    let attributes = |reference: bool, size, align| {
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
            size,
            align,
        )
        .unwrap()
    };
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            unavailable,
            statements,
            SemanticTerminatorV1::new(unavailable, terminator),
        )
        .unwrap()
    };
    let integer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let mut catalog = vec![unit_type()];
    for (tag, bits, size) in [(101, 32, 4), (102, 64, 8)] {
        let scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, bits, size),
            SemanticScalarValidityRangeV1::new(
                0,
                if bits == 32 {
                    u32::MAX.into()
                } else {
                    u64::MAX.into()
                },
            ),
        );
        catalog.push(declaration(
            tag,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                SemanticBackendReprV1::scalar(scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        ));
    }
    catalog.push(declaration(
        103,
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    catalog.push(declaration(
        104,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(integer),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U64, MARKER]).unwrap()),
    ));
    catalog.push(declaration(
        150,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::Array {
                stride_bytes: 4,
                count: 0,
            },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element: U32 },
    ));
    catalog.push(
        declaration(
            151,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: pointer_scalar(true),
                    second: integer,
                },
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    READ_SLICE,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
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
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let direct = |ty| {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(attributes(false, 0, None)),
        )
    };
    let pair = || {
        SemanticAbiValueV1::new(
            READ_REF,
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::new(
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
                    Some(4),
                )
                .unwrap(),
                second: attributes(false, 0, None),
            },
        )
    };
    let call = |callee, arguments, destination, ty, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let raw_index = || {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64).unwrap()],
                U64,
            )
            .unwrap(),
        )
    };
    let index = |pending| {
        if pending {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                U64,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
            ))
        } else {
            raw_index()
        }
    };
    let blocks = vec![
        block(40, vec![], call(1, vec![], 2, WITNESS, 1)),
        block(
            41,
            vec![],
            call(
                2,
                vec![
                    SemanticOperandV1::Copy(place(1, READ_REF)),
                    index(mixed && pending_first),
                ],
                3,
                U32,
                2,
            ),
        ),
        block(
            42,
            vec![],
            call(
                2,
                vec![
                    SemanticOperandV1::Copy(place(1, READ_REF)),
                    index(mixed && !pending_first),
                ],
                4,
                U32,
                3,
            ),
        ),
        block(43, vec![], SemanticTerminatorKindV1::Return),
    ];
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([160; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(pair())],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    let locals = [
        (UNIT, SemanticLocalRoleV1::Return),
        (READ_REF, SemanticLocalRoleV1::Argument(0)),
        (WITNESS, SemanticLocalRoleV1::Temporary),
        (U32, SemanticLocalRoleV1::Temporary),
        (U32, SemanticLocalRoleV1::Temporary),
    ];
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([170; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([171; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([172; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([173; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([174; 32]),
        unavailable,
        abi,
        locals
            .into_iter()
            .enumerate()
            .map(|(ordinal, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([180 + ordinal as u8; 32]),
                    ty,
                    role,
                    unavailable,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"unsupported_index_correlation".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([190; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let intrinsic = |tag, inputs, output, operation| SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            unavailable,
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                inputs,
                output,
            )
            .unwrap(),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        catalog,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            intrinsic(
                220,
                vec![],
                direct(WITNESS),
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                    index_witness: WITNESS,
                    raw_index: U64,
                },
            ),
            intrinsic(
                221,
                vec![pair(), direct(U64)],
                direct(U32),
                SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad { element: U32 },
            ),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn live_guarded_formal_v4_preserves_exact_owner_and_policy_pairing() {
    use crate::{
        FormalMemoryAdmissionValidationPolicyV4, InertCanonicalFormalMemoryAdmissionEvidenceV4,
        ProductionFormalMemoryEvidenceErrorV4, ProductionFormalMemoryOwnerV1,
    };
    use fe2o3_kernel_ir::{FormalMemoryReceiptEncodingV3, InertFormalMemoryReceiptFormatV3};
    let semantic = ProductionSemanticKirOwnerV1::try_lower(
        guarded_read_source_v360(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    semantic.verify_equivalence().unwrap();
    assert!(!semantic.retains_mandatory_generic_checks());
    let identity = semantic.canonical_kernel_ir_identity();
    let formal = ProductionFormalMemoryOwnerV1::try_admit(semantic).unwrap();
    formal.verify_equivalence().unwrap();
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(&formal).unwrap();
    assert_eq!(
        evidence.validation_policy(),
        FormalMemoryAdmissionValidationPolicyV4::GuardedV2
    );
    assert_eq!(evidence.canonical_kernel_ir_identity(), identity);
    let nested = InertFormalMemoryReceiptFormatV3::decode_current(
        evidence.canonical_bytes()[120..].to_vec(),
    )
    .unwrap();
    assert_eq!(
        nested.metadata().encoding(),
        FormalMemoryReceiptEncodingV3::GuardedV3
    );
    assert!(!nested.grants_authority());
    let decoded =
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(evidence.canonical_bytes()).unwrap();
    assert_eq!(decoded, evidence);
    for policy in [1_u16, 3] {
        let mut bytes = evidence.canonical_bytes().to_vec();
        bytes[10..12].copy_from_slice(&policy.to_le_bytes());
        let result = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes);
        if policy == 1 {
            assert!(matches!(
                result,
                Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
            ));
        } else {
            assert!(result.is_err());
        }
    }
    for witness in [0_u64, 63, 65] {
        let mut bytes = evidence.canonical_bytes().to_vec();
        bytes[96..104].copy_from_slice(&witness.to_le_bytes());
        assert!(matches!(
            InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes),
            Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
        ));
    }
}

#[test]
fn live_fixed_formal_v4_retains_legacy_policy_and_exact_bytes() {
    use crate::{
        FormalMemoryAdmissionValidationPolicyV4, InertCanonicalFormalMemoryAdmissionEvidenceV4,
        ProductionFormalMemoryEvidenceErrorV4, ProductionFormalMemoryOwnerV1,
    };
    use fe2o3_kernel_ir::{FormalMemoryReceiptEncodingV3, InertFormalMemoryReceiptFormatV3};
    let semantic = ProductionSemanticKirOwnerV1::try_lower(
        noop_semantic_owner(&["guarded_legacy_fixed"]),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let formal = ProductionFormalMemoryOwnerV1::try_admit(semantic).unwrap();
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(&formal).unwrap();
    assert_eq!(
        evidence.validation_policy(),
        FormalMemoryAdmissionValidationPolicyV4::LegacyV1
    );
    let nested = InertFormalMemoryReceiptFormatV3::decode_current(
        evidence.canonical_bytes()[120..].to_vec(),
    )
    .unwrap();
    assert_eq!(
        nested.metadata().encoding(),
        FormalMemoryReceiptEncodingV3::LegacyV1
    );
    assert_eq!(&evidence.canonical_bytes()[8..12], &[4, 0, 1, 0]);
    assert_eq!(
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(evidence.canonical_bytes()).unwrap(),
        evidence
    );
    let mut bytes = evidence.canonical_bytes().to_vec();
    bytes[10..12].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes),
        Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
    ));
}

#[test]
fn guarded_v4_rejects_inert_bits32_even_after_exact_nested_identity_rebinding() {
    use crate::{
        InertCanonicalFormalMemoryAdmissionEvidenceV4,
        MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4, ProductionFormalMemoryEvidenceErrorV4,
        ProductionFormalMemoryOwnerV1,
    };
    use fe2o3_kernel_ir::{FormalIndexWidth, InertFormalMemoryReceiptFormatV3};
    let semantic = ProductionSemanticKirOwnerV1::try_lower(
        guarded_read_source_v360(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let formal = ProductionFormalMemoryOwnerV1::try_admit(semantic).unwrap();
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(&formal).unwrap();
    let mut nested = evidence.formal_obligation_receipt_bytes().to_vec();
    let mut cursor = 20;
    for _ in 0..2 {
        let length = u32::from_le_bytes(nested[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4 + length;
    }
    assert_eq!(nested[cursor], 2);
    nested[cursor] = 1;
    let receipt = InertFormalMemoryReceiptFormatV3::decode_current(nested).unwrap();
    assert_eq!(receipt.metadata().index_width(), FormalIndexWidth::Bits32);
    assert!(!receipt.grants_authority());
    let mut bytes = evidence.canonical_bytes()[..120].to_vec();
    bytes[64..96].copy_from_slice(receipt.identity_digest());
    bytes.extend_from_slice(receipt.canonical_bytes());
    assert!(matches!(
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes),
        Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
    ));
    assert!(matches!(
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&vec![
            0;
            MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4
                + 1
        ]),
        Err(ProductionFormalMemoryEvidenceErrorV4::TooLarge)
    ));
    let mut trailing = evidence.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&trailing).is_err());
}

fn mixed_guarded_owner_v360(pending_first: bool) -> ProductionSemanticKirOwnerV1 {
    let semantic = guarded_read_source_v360(pending_first, true);
    let lowering =
        ranked_correlation_input_for_accesses(&[AccessKindAttr::Read, AccessKindAttr::Read], 1);
    let receipt = ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        semantic,
        vec![ProductionRankedSemanticProjectionRootV1::new(
            SemanticFunctionIdV1::from_index(0), 1, lowering,
            "func @unsupported_index_correlation { read; read; }".to_owned(),
            vec![ProductionRankedAccessSourceV1::new(1, None, 0, 0, 3),
                ProductionRankedAccessSourceV1::new(2, None, 0, 0, 4)],
            vec![],
        )],
    ).expect("actual compiled ranked reads and admitted source roster");
    ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
    )
    .expect("fresh source lowering and independent MIR/ranked translation validation")
}

#[test]
fn mixed_guarded_reads_admit_the_same_live_owner_in_both_orders() {
    use fe2o3_kernel_ir::{
        ExplicitLaunchExtent, FormalBoundsKindV1, FormalMemoryObligationAnalysis,
        FormalMemoryReceiptEncodingV3, InertFormalMemoryReceiptFormatV3,
        derive_kernel_memory_obligations_for_launch,
    };
    for pending_first in [false, true] {
        let owner = mixed_guarded_owner_v360(pending_first);
        let translation = owner.mir_pliron_translation_validation().unwrap();
        assert_eq!(translation.memory_effects(), 2);
        assert!(!translation.claims_indexed_address_equivalence());
        assert!(!translation.claims_complete_operational_equivalence());
        owner.verify_equivalence().unwrap();
        assert!(owner.retains_mandatory_generic_checks());
        let kernel = &owner.module().kernels[0];
        let analysis = derive_kernel_memory_obligations_for_launch(
            owner.module(),
            &kernel.id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        let FormalMemoryObligationAnalysis::Incomplete { partial, reasons } = analysis else {
            panic!("exactly the constant-index read still needs ranked structural proof")
        };
        let [FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location }] =
            reasons.as_slice()
        else {
            panic!("unexpected earlier phase: {reasons:?}")
        };
        assert_eq!(partial.accesses().len(), 2);
        let pending = usize::from(!pending_first);
        assert_eq!(partial.accesses()[pending].location(), *location);
        assert_eq!(
            partial.accesses()[pending].domain(),
            FormalAccessDomainV1::LaunchEnvelope
        );
        assert_eq!(
            partial.accesses()[pending].byte_offset(),
            fe2o3_kernel_ir::ByteExpression::Unbounded
        );
        assert!(matches!(
            partial.accesses()[1 - pending].domain(),
            FormalAccessDomainV1::SliceBounded(_)
        ));
        let body = owner
            .module()
            .function(&kernel.entry)
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        assert_eq!(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|op| matches!(
                    op.kind,
                    OperationKind::GuardedLoad {
                        access: MemoryAccess { volatile: true, .. },
                        ..
                    }
                ))
                .count(),
            2
        );
        assert_eq!(
            body.blocks
                .iter()
                .filter(|block| matches!(
                    block.terminator,
                    Some(Terminator::ConditionalBranch { .. })
                ))
                .count(),
            2
        );
        assert!(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|op| op == &AmdGpuDiagnosticOperation::Trap.operation(None))
        );
        let admitted = crate::ProductionFormalMemoryOwnerV1::try_admit(owner)
            .expect("mixed fresh-report partition and ranked proof must compose");
        admitted.verify_equivalence().unwrap();
        let report = admitted.kernels()[0].obligations();
        assert_eq!(report, &partial);
        assert_eq!(report.bounds_requirements().len(), 1);
        assert_eq!(
            report
                .bounds_requirements()
                .iter()
                .filter(|bound| matches!(
                    bound.kind(),
                    FormalBoundsKindV1::SliceElementAtGuardedIndex(_)
                ))
                .count(),
            1
        );
        assert!(report.runtime_alias_requirements().is_empty());
        assert!(report.inter_invocation_conflicts().is_empty());
        let receipt = InertFormalMemoryReceiptFormatV3::from_current_obligations(report).unwrap();
        assert_eq!(
            receipt.metadata().encoding(),
            FormalMemoryReceiptEncodingV3::GuardedV3
        );
        assert!(!receipt.grants_authority());
    }
}

#[test]
fn mixed_guarded_report_refuses_a_distinct_live_owner() {
    let owner = mixed_guarded_owner_v360(false);
    let foreign = mixed_guarded_owner_v360(false);
    assert_eq!(
        owner.canonical_kernel_ir_identity(),
        foreign.canonical_kernel_ir_identity()
    );
    assert_eq!(
        crate::production_formal_memory_v1::check_fresh_guarded_report_owner_binding_for_test(
            &owner, &foreign
        ),
        Err(ProductionMemoryDischargeFailureV1::Stage(
            "guarded proof received a report from another semantic owner"
        ))
    );
}

#[test]
fn mixed_guarded_partition_closes_pending_rows_and_exact_work() {
    use fe2o3_kernel_ir::{
        ExplicitLaunchExtent, FormalAccessDomainV1, derive_kernel_memory_obligations_for_launch,
    };
    for pending_first in [false, true] {
        let owner = mixed_guarded_owner_v360(pending_first);
        let module = owner.module();
        let kernel = &module.kernels[0];
        let analysis = derive_kernel_memory_obligations_for_launch(
            module,
            &kernel.id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        let report = analysis.obligations();
        let pending = report
            .accesses()
            .iter()
            .find(|row| row.domain() == FormalAccessDomainV1::LaunchEnvelope)
            .unwrap()
            .location();
        let proved = report
            .accesses()
            .iter()
            .find(|row| matches!(row.domain(), FormalAccessDomainV1::SliceBounded(_)))
            .unwrap()
            .location();
        let function = module.function(&kernel.entry).unwrap();
        let body = function.body.as_ref().unwrap();
        let operations = body
            .blocks
            .iter()
            .map(|block| block.operations.len())
            .sum::<usize>();
        // One direct LessThan pending proof costs one existing recursive debit.
        let exact = 24
            + kernel.id.as_str().len()
            + kernel.entry.as_str().len()
            + 2 * operations
            + 2 * 2
            + 2 * 2
            + 24
            + 1;
        let mut budget = GuardedAddressProofBudgetV1 { remaining: exact };
        assert_eq!(
            guarded_accesses_have_structural_bounds_result(
                module,
                kernel,
                report,
                [64, 1, 1],
                &[pending],
                operations,
                &mut budget
            ),
            Ok(())
        );
        assert_eq!(budget.remaining, 0);
        let mut short = GuardedAddressProofBudgetV1 {
            remaining: exact - 1,
        };
        assert!(
            matches!(guarded_accesses_have_structural_bounds_result(module, kernel, report, [64, 1, 1],
            &[pending], operations, &mut short), Err(ProductionMemoryDischargeFailureV1::GuardedBound { location, .. }) if location == pending)
        );
        assert_eq!(short.remaining, 0);
        for supplied in [
            vec![],
            vec![proved],
            vec![pending, pending],
            vec![pending, proved],
        ] {
            let mut budget = GuardedAddressProofBudgetV1 { remaining: exact };
            assert_eq!(
                guarded_accesses_have_structural_bounds_result(
                    module,
                    kernel,
                    report,
                    [64, 1, 1],
                    &supplied,
                    operations,
                    &mut budget
                ),
                Err(ProductionMemoryDischargeFailureV1::Stage(
                    "formal guarded-load locations do not match retained Kernel IR"
                ))
            );
        }
        let mut budget = GuardedAddressProofBudgetV1 { remaining: exact };
        assert_eq!(
            guarded_accesses_have_structural_bounds_result(
                module,
                kernel,
                report,
                [63, 1, 1],
                &[pending],
                operations,
                &mut budget
            ),
            Err(ProductionMemoryDischargeFailureV1::Stage(
                "guarded proof report does not match its kernel and exact witness"
            ))
        );
        let block = body
            .blocks
            .iter()
            .find(|block| block.id == proved.block)
            .unwrap();
        let operation = &block.operations[proved.operation_index];
        let row = report
            .accesses()
            .iter()
            .find(|row| row.location() == proved);
        let mut budget = GuardedAddressProofBudgetV1 { remaining: 26 };
        assert_eq!(
            guarded_load_is_core_proved_v1(function, report, proved, operation, row, &mut budget),
            Ok(true)
        );
        assert_eq!(budget.remaining, 0);
        let mut short = GuardedAddressProofBudgetV1 { remaining: 25 };
        assert_eq!(
            guarded_load_is_core_proved_v1(function, report, proved, operation, row, &mut short),
            Err(ProductionMemoryDischargeFailureV1::Stage(
                "guarded proof exceeded its work budget"
            ))
        );
        assert_eq!(short.remaining, 23);
    }
}

#[test]
fn guarded_row_cursor_does_not_skip_future_rows_or_accept_duplicate_effects() {
    use fe2o3_kernel_ir::{ExplicitLaunchExtent, derive_kernel_memory_obligations_for_launch};
    let owner = mixed_guarded_owner_v360(false);
    let module = owner.module();
    let kernel = &module.kernels[0];
    let analysis = derive_kernel_memory_obligations_for_launch(
        module,
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    let rows = analysis.obligations().accesses();
    assert_eq!(rows.len(), 2);
    // A pending operation may have no formal row. Keep the future row intact.
    let mut cursor = GuardedAccessCursorV1 {
        rows: &rows[1..],
        next: 0,
    };
    let mut budget = GuardedAddressProofBudgetV1 { remaining: 7 };
    assert!(
        cursor
            .observe(rows[0].location(), &mut budget)
            .unwrap()
            .is_none()
    );
    assert_eq!(cursor.next, 0);
    assert_eq!(
        cursor.observe(rows[1].location(), &mut budget).unwrap(),
        Some(&rows[1])
    );
    cursor.finish(&mut budget).unwrap();
    assert_eq!(budget.remaining, 0);
    let mut cursor = GuardedAccessCursorV1 { rows, next: 0 };
    let mut short = GuardedAddressProofBudgetV1 { remaining: 3 };
    assert_eq!(
        cursor.observe(rows[0].location(), &mut short),
        Err(ProductionMemoryDischargeFailureV1::Stage(
            "guarded proof exceeded its work budget"
        ))
    );
    assert_eq!(cursor.next, 0);
    assert_eq!(short.remaining, 1);
    // Component mutations of the borrowed row roster, not forged owner reports.
    for hostile in [
        vec![rows[0].clone(), rows[0].clone(), rows[1].clone()],
        vec![rows[1].clone(), rows[0].clone()],
    ] {
        let mut cursor = GuardedAccessCursorV1 {
            rows: &hostile,
            next: 0,
        };
        let mut budget = GuardedAddressProofBudgetV1 { remaining: 100 };
        for row in rows {
            cursor.observe(row.location(), &mut budget).unwrap();
        }
        assert_eq!(
            cursor.finish(&mut budget),
            Err(ProductionMemoryDischargeFailureV1::Stage(
                "formal access rows do not match retained Kernel IR body order"
            ))
        );
    }
}

#[test]
fn guarded_partition_uses_body_order_with_interleaved_ordinary_and_guarded_stores() {
    use fe2o3_kernel_ir::{
        ExplicitLaunchExtent, FormalMemoryObligationAnalysis,
        derive_kernel_memory_obligations_for_launch,
    };
    // A verified KIR component derived from the genuine owner. Renaming is
    // not reattached to its source correspondence or claimed as a new owner.
    let owner = mixed_guarded_owner_v360(true);
    let mut module = owner.module().clone();
    let function = &mut module.functions[0];
    let body = function.body.as_mut().unwrap();
    let next = body
        .parameters
        .iter()
        .copied()
        .chain(body.blocks.iter().flat_map(|block| {
            block.parameters.iter().map(|value| value.id).chain(
                block
                    .operations
                    .iter()
                    .flat_map(|operation| operation.results.iter().map(|value| value.id)),
            )
        }))
        .map(|value| value.0)
        .max()
        .unwrap()
        + 1;
    let pointer = ValueId(next);
    let loaded = ValueId(next + 1);
    function.signature.parameters.push(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    body.parameters.push(pointer);
    let block = body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| matches!(operation.kind, OperationKind::GuardedLoad { .. }))
        })
        .unwrap();
    let index = block
        .operations
        .iter()
        .position(|operation| matches!(operation.kind, OperationKind::GuardedLoad { .. }))
        .unwrap();
    let OperationKind::GuardedLoad { predicate, .. } = block.operations[index].kind else {
        unreachable!()
    };
    block.operations.splice(
        index + 1..index + 1,
        [
            Operation::effect_free(
                ValueDef::new(loaded, Type::Scalar(ScalarType::U32)),
                OperationKind::Load {
                    pointer,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer,
                    value: loaded,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::GuardedStore {
                    pointer,
                    predicate,
                    value: loaded,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ],
    );
    let rename = |id: &mut BlockId| {
        id.0 = 1_000_000 - id.0;
    };
    for block in &mut body.blocks {
        rename(&mut block.id);
        match block.terminator.as_mut().unwrap() {
            Terminator::Branch { target, .. } => rename(target),
            Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            } => {
                rename(then_target);
                rename(else_target);
            }
            Terminator::Switch {
                cases,
                default_target,
                ..
            } => {
                for case in cases {
                    rename(&mut case.target);
                }
                rename(default_target);
            }
            Terminator::IntegerSwitch {
                cases,
                default_target,
                ..
            } => {
                for case in cases {
                    rename(&mut case.target);
                }
                rename(default_target);
            }
            Terminator::Return { .. } | Terminator::Unreachable => {}
        }
    }
    verify_module(&module).unwrap();
    let kernel = &module.kernels[0];
    let analysis = derive_kernel_memory_obligations_for_launch(
        &module,
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    let FormalMemoryObligationAnalysis::Incomplete { partial, reasons } = analysis else {
        panic!("pending guard remains")
    };
    let locations = reasons
        .iter()
        .filter_map(|reason| match reason {
            FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location } => {
                Some(*location)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(locations.len(), 1);
    assert_eq!(partial.accesses().len(), 5);
    assert!(
        partial
            .accesses()
            .windows(2)
            .any(|rows| rows[0].location().block > rows[1].location().block)
    );
    let mut budget = GuardedAddressProofBudgetV1::new(1000).unwrap();
    assert_eq!(
        guarded_accesses_have_structural_bounds_result(
            &module,
            kernel,
            &partial,
            [64, 1, 1],
            &locations,
            1000,
            &mut budget
        ),
        Ok(())
    );
}
