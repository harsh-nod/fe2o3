use super::*;

// Exact neutral-scan type and intrinsic recipes from projection_01_tests.
// Keep this fixture local: no synthetic functional receipts or skipped checks.
fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn cfg_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}
fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(tag)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}
fn ranked_root_input_1d(name: &str, binding: u8, size: u32) -> ProductionRankedRootInputV1 {
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(fe2o3_artifacts::Dimensions::new(size, 1, 1).unwrap()),
        fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    ProductionRankedRootInputV1::new(name, bytes(binding), &launch)
}

const NEUTRAL_UNIT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const NEUTRAL_LDS_SCOPE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const NEUTRAL_LDS_SCOPE_REFERENCE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const NEUTRAL_ELEMENT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const NEUTRAL_ELEMENT_POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const NEUTRAL_U64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const NEUTRAL_DYNAMIC_LDS_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const NEUTRAL_CONTEXT_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const NEUTRAL_CONTEXT_REFERENCE_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);

fn neutral_scalar_backend_v1(
    primitive: SemanticBackendPrimitiveV1,
    maximum: u128,
) -> SemanticBackendReprV1 {
    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        primitive,
        SemanticScalarValidityRangeV1::new(0, maximum),
    ))
}

fn neutral_pointer_type_v1(
    tag: u8,
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    validity_start: u128,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(validity_start, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn neutral_reference_type_v1(
    tag: u8,
    pointee: SemanticTypeIdV1,
    mutability: SemanticMutabilityV1,
    pointee_kind: SemanticAbiPointeeKindV1,
) -> SemanticTypeDeclV1 {
    neutral_pointer_type_v1(
        tag,
        pointee,
        SemanticPointerKindV1::Reference,
        mutability,
        1,
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(pointee_kind, 0, 1).unwrap()),
            None,
        ),
    )
}

fn neutral_semantic_types_v1() -> Vec<SemanticTypeDeclV1> {
    let unit = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(220)),
        SemanticLayoutIdentityV1::from_sha256(bytes(220)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(Vec::new(), Vec::new()).unwrap(),
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
    let scope = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(221)),
        SemanticLayoutIdentityV1::from_sha256(bytes(221)),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![0], Vec::new()).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![NEUTRAL_UNIT_TYPE]).unwrap(),
        ),
    );
    let scope_reference = neutral_reference_type_v1(
        222,
        NEUTRAL_LDS_SCOPE_TYPE,
        SemanticMutabilityV1::Mutable,
        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
    );
    let element = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(223)),
        SemanticLayoutIdentityV1::from_sha256(bytes(223)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            neutral_scalar_backend_v1(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                u32::MAX.into(),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    let element_pointer = neutral_pointer_type_v1(
        224,
        NEUTRAL_ELEMENT_TYPE,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Mutable,
        0,
    );
    let u64_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(225)),
        SemanticLayoutIdentityV1::from_sha256(bytes(225)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            neutral_scalar_backend_v1(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                u64::MAX.into(),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    let dynamic_lds = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(226)),
        SemanticLayoutIdentityV1::from_sha256(bytes(226)),
        SemanticTypeLayoutV1::aggregate(
            Some(24),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16, 24, 24, 24], Vec::new()).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![
                NEUTRAL_ELEMENT_POINTER_TYPE,
                NEUTRAL_U64_TYPE,
                NEUTRAL_U64_TYPE,
                NEUTRAL_UNIT_TYPE,
                NEUTRAL_UNIT_TYPE,
                NEUTRAL_UNIT_TYPE,
            ])
            .unwrap(),
        ),
    );
    let context = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(227)),
        SemanticLayoutIdentityV1::from_sha256(bytes(227)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let context_reference = neutral_reference_type_v1(
        228,
        NEUTRAL_CONTEXT_TYPE,
        SemanticMutabilityV1::Immutable,
        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
    );
    vec![
        unit,
        scope,
        scope_reference,
        element,
        element_pointer,
        u64_type,
        dynamic_lds,
        context,
        context_reference,
    ]
}

fn neutral_plain_direct_abi_value_v1(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn neutral_reference_abi_value_v1(ty: SemanticTypeIdV1, shared: bool) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
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
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn neutral_dynamic_lds_abi_value_v1() -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        NEUTRAL_DYNAMIC_LDS_TYPE,
        SemanticAbiPassModeV1::Indirect {
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
                24,
                Some(8),
            )
            .unwrap(),
            metadata_attributes: None,
            on_stack: false,
        },
    )
}

fn neutral_compiler_intrinsic_callable_v1(
    tag: u8,
    inputs: Vec<SemanticAbiValueV1>,
    output: SemanticAbiValueV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    let arguments = inputs
        .into_iter()
        .map(SemanticAbiArgumentV1::source)
        .collect::<Vec<_>>();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        u32::try_from(arguments.len()).unwrap(),
        arguments,
        output,
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(tag)),
    }
}

fn neutral_test_place_v1(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), Vec::new(), ty).unwrap()
}

fn neutral_test_call_v1(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination_local: u32,
    destination_ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                neutral_test_place_v1(destination_local, destination_ty),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn neutral_ranked_program_for_operation_v1(
    operation: SemanticCompilerIntrinsicOperationV1,
    elements: u32,
) -> ProductionRankedSemanticProgramV1 {
    let scope_borrow = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        neutral_test_place_v1(2, NEUTRAL_LDS_SCOPE_REFERENCE_TYPE),
        SemanticRvalueV1::new(
            NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: neutral_test_place_v1(1, NEUTRAL_LDS_SCOPE_TYPE),
            },
        ),
    )));
    let context_borrow = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        neutral_test_place_v1(5, NEUTRAL_CONTEXT_REFERENCE_TYPE),
        SemanticRvalueV1::new(
            NEUTRAL_CONTEXT_REFERENCE_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: neutral_test_place_v1(4, NEUTRAL_CONTEXT_TYPE),
            },
        ),
    )));
    let blocks = vec![
        block(
            230,
            vec![scope_borrow],
            neutral_test_call_v1(
                2,
                vec![SemanticOperandV1::Copy(neutral_test_place_v1(
                    2,
                    NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
                ))],
                3,
                NEUTRAL_DYNAMIC_LDS_TYPE,
                1,
            ),
        ),
        block(
            231,
            Vec::new(),
            neutral_test_call_v1(3, Vec::new(), 4, NEUTRAL_CONTEXT_TYPE, 2),
        ),
        block(
            232,
            vec![context_borrow],
            neutral_test_call_v1(
                4,
                vec![
                    SemanticOperandV1::Copy(neutral_test_place_v1(
                        5,
                        NEUTRAL_CONTEXT_REFERENCE_TYPE,
                    )),
                    // Match the post-borrow-check encoding produced by
                    // ordinary Rust for this non-Copy, no-drop value.
                    SemanticOperandV1::Copy(neutral_test_place_v1(3, NEUTRAL_DYNAMIC_LDS_TYPE)),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        NEUTRAL_ELEMENT_TYPE,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
                    )),
                ],
                6,
                NEUTRAL_ELEMENT_TYPE,
                3,
            ),
        ),
        block(233, Vec::new(), SemanticTerminatorKindV1::Return),
    ];
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(234)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        Vec::new(),
        SemanticAbiValueV1::new(NEUTRAL_UNIT_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [
        (NEUTRAL_UNIT_TYPE, SemanticLocalRoleV1::Return),
        (NEUTRAL_LDS_SCOPE_TYPE, SemanticLocalRoleV1::Temporary),
        (
            NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
            SemanticLocalRoleV1::Temporary,
        ),
        (NEUTRAL_DYNAMIC_LDS_TYPE, SemanticLocalRoleV1::Temporary),
        (NEUTRAL_CONTEXT_TYPE, SemanticLocalRoleV1::Temporary),
        (
            NEUTRAL_CONTEXT_REFERENCE_TYPE,
            SemanticLocalRoleV1::Temporary,
        ),
        (NEUTRAL_ELEMENT_TYPE, SemanticLocalRoleV1::Temporary),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (ty, role))| local(235 + index as u8, ty, role))
    .collect::<Vec<_>>();
    let dimensions = SemanticWorkgroupDimensionsV1::new([elements, 1, 1]).unwrap();
    let source_contract = SemanticKernelSourceContractV1::new(
        Some(SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap()),
        None,
        None,
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(242)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(243)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(244)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(245)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(246)),
        SemanticSourceProvenanceV1::unavailable(),
        root_abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"phase_alpha".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(0xa1)),
        source_contract,
    ));
    let second = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(251)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(71)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(72)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(73)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(74)),
        SemanticSourceProvenanceV1::unavailable(),
        function.abi().clone(),
        function
            .locals()
            .iter()
            .enumerate()
            .map(|(index, declaration)| {
                local(80 + index as u8, declaration.ty(), declaration.role())
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        function
            .blocks()
            .iter()
            .enumerate()
            .map(|(index, body)| {
                block(
                    90 + index as u8,
                    body.statements().to_vec(),
                    body.terminator().kind().clone(),
                )
            })
            .collect(),
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"phase_zeta".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(0x7a)),
        source_contract,
    ));
    // Functions are identity-sorted; defined callables occupy the matching prefix.
    // Kernel bindings intentionally retain the opposite canonical roster order.
    assert!(function.identity().as_bytes() < second.identity().as_bytes());
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        neutral_compiler_intrinsic_callable_v1(
            248,
            vec![neutral_reference_abi_value_v1(
                NEUTRAL_LDS_SCOPE_REFERENCE_TYPE,
                false,
            )],
            neutral_dynamic_lds_abi_value_v1(),
            SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
                scope: NEUTRAL_LDS_SCOPE_TYPE,
                dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
                element_storage: NEUTRAL_ELEMENT_TYPE,
                elements: u64::from(elements),
            },
        ),
        neutral_compiler_intrinsic_callable_v1(
            249,
            Vec::new(),
            SemanticAbiValueV1::new(NEUTRAL_CONTEXT_TYPE, SemanticAbiPassModeV1::Ignore),
            SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent {
                context: NEUTRAL_CONTEXT_TYPE,
            },
        ),
        neutral_compiler_intrinsic_callable_v1(
            250,
            vec![
                neutral_reference_abi_value_v1(NEUTRAL_CONTEXT_REFERENCE_TYPE, true),
                neutral_dynamic_lds_abi_value_v1(),
                neutral_plain_direct_abi_value_v1(NEUTRAL_ELEMENT_TYPE),
            ],
            neutral_plain_direct_abi_value_v1(NEUTRAL_ELEMENT_TYPE),
            operation,
        ),
    ];
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        neutral_semantic_types_v1(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![function, second],
        callables,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    project_and_verify_ranked_semantic_mir_v1(
        owner,
        &[
            ranked_root_input_1d("phase_alpha", 0xa1, elements),
            ranked_root_input_1d("phase_zeta", 0x7a, elements),
        ],
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap()
}

pub(super) fn source_fixture() -> (
    ProductionSemanticKirOwnerV1,
    AuthenticatedRankedVerificationRosterV1,
) {
    let program = neutral_ranked_program_for_operation_v1(
        SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum {
            context: NEUTRAL_CONTEXT_TYPE,
            dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
            element_storage: NEUTRAL_ELEMENT_TYPE,
            element: NEUTRAL_ELEMENT_TYPE,
            kind: SemanticWorkgroupScanKindV1::Inclusive,
        },
        3,
    );
    for root in program.roots() {
        let recipe = root
            .executable_effect_sources
            .first()
            .expect("neutral scan must retain its generated effect sources")
            .recipe_identity();
        assert_ne!(recipe, [0; 32]);
        let allocation = dialect_kernel::neutral_workgroup_allocation_contract_v1(recipe);
        // The collective projects allocation effects; indexed loads/stores
        // appear only after the independently checked KIR recipe expansion.
        let effects = root
            .executable_effect_sources
            .iter()
            .enumerate()
            .map(|(ordinal, source)| {
                assert_eq!(source.recipe_identity(), recipe);
                assert_eq!(source.semantic_block(), 2);
                assert_eq!(source.semantic_effect_ordinal(), ordinal as u32);
                assert_eq!(
                    source.origin(),
                    ProductionRankedExecutableEffectOriginV1::GeneratedFromSemanticTerminator
                );
                let operation = &root.lowering.kernel().blocks()[source.ranked_block() as usize]
                    .operations()[source.ranked_operation() as usize];
                match operation {
                    ProductionRankedOperationV1::AllocationEffect {
                        kind,
                        memory_space,
                        allocation_origin,
                        noalias_class,
                    } => {
                        assert_eq!(*memory_space, MemorySpaceAttr::Workgroup);
                        assert_eq!((*allocation_origin, *noalias_class), allocation);
                        Some(*kind)
                    }
                    ProductionRankedOperationV1::Barrier {
                        execution_scope,
                        memory_scope,
                        address_space,
                        order,
                    } => {
                        assert_eq!(*execution_scope, HierarchyAttr::Workgroup);
                        assert_eq!(*memory_scope, MemoryScopeAttr::Workgroup);
                        assert_eq!(*address_space, AddressSpaceAttr::Workgroup);
                        assert_eq!(*order, MemoryOrderAttr::AcquireRelease);
                        None
                    }
                    other => panic!("unexpected neutral scan effect: {other:?}"),
                }
            })
            .collect::<Vec<_>>();
        let read = Some(AccessKindAttr::Read);
        let write = Some(AccessKindAttr::Write);
        assert_eq!(
            effects,
            [
                write, None, read, read, None, write, None, read, read, None, write, None, read,
                None,
            ],
            "three-element inclusive scan must retain its complete memory/barrier sequence"
        );
        assert!(
            !root
                .lowering
                .has_retained_policy_checked_refinement_staging()
        );
    }
    let receipt = program.into_verified_roster_receipt().unwrap();
    let records = ranked_roster_identity_records_v1(&receipt.source_order_roots);
    let (identity, order) = derive_ranked_kernel_roster_identity_v1(&records).unwrap();
    let (module_receipt, roster) = receipt.into_module_verified_receipt().unwrap();
    assert_eq!(identity, roster.canonical_roster_identity());
    assert_eq!(order.as_ref(), roster.canonical_kernel_order());
    let owner = ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
        module_receipt,
        fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    for kernel in &owner.module().kernels {
        let function = owner
            .module()
            .functions
            .iter()
            .find(|function| function.id == kernel.entry)
            .unwrap();
        let blocks = &function.body.as_ref().unwrap().blocks;
        assert!(
            blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    &operation.kind, fe2o3_kernel_ir::OperationKind::Load { access, .. }
                        if access.address_space == fe2o3_kernel_ir::AddressSpace::Workgroup
                )),
            "neutral fixture must retain its workgroup loads in KIR"
        );
        assert!(
            blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    &operation.kind, fe2o3_kernel_ir::OperationKind::Store { access, .. }
                        if access.address_space == fe2o3_kernel_ir::AddressSpace::Workgroup
                )),
            "neutral fixture must retain its workgroup stores in KIR"
        );
    }
    (owner, roster)
}
