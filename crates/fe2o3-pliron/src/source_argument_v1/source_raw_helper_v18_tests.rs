use super::*;

const RAW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const HELPER: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(1);

fn raw_source(
    space: u32,
    mutable: bool,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> AdmittedInertSemanticMirV1 {
    pointer_source(space, mutable, ownership, false)
}

fn pointer_source(
    space: u32,
    mutable: bool,
    ownership: SemanticSourceArgumentOwnershipV1,
    reference: bool,
) -> AdmittedInertSemanticMirV1 {
    let template = semantic(&[U32]);
    // Match the admitted gfx942 target layout, including its narrow local pointers.
    let (pointer_bytes, pointer_bits, pointer_max) = match space {
        0 | 1 | 4 => (8, 64, u128::from(u64::MAX)),
        3 | 5 => (4, 32, u128::from(u32::MAX)),
        _ => unreachable!(),
    };
    let mut types = template.types().to_vec();
    types.push(
        declaration(
            91,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(pointer_bytes),
                pointer_bytes,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(space, pointer_bytes, pointer_bytes),
                    SemanticScalarValidityRangeV1::new(u128::from(reference), pointer_max),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    U32,
                    if reference {
                        SemanticPointerKindV1::Reference
                    } else {
                        SemanticPointerKindV1::Raw
                    },
                    if mutable {
                        SemanticMutabilityV1::Mutable
                    } else {
                        SemanticMutabilityV1::Immutable
                    },
                    space,
                    pointer_bits,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        if !reference {
                            SemanticAbiPointeeKindV1::Raw
                        } else if mutable {
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                        } else {
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        },
                        if reference { 4 } else { 0 },
                        if reference { 4 } else { 1 },
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            reference,
            (reference && !mutable).then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            reference,
            reference && !mutable,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        if reference { 4 } else { 0 },
        reference.then_some(4),
    )
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    let make = |tag: u8, kernel: bool, blocks| {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            if kernel {
                SemanticFunctionRoleV1::KernelRoot
            } else {
                SemanticFunctionRoleV1::InternalHelper
            },
            SemanticItemDefinitionIdentityV1::from_sha256([tag + 1; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag + 2; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag + 3; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag + 4; 32]),
            source,
            SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([tag + 5; 32]),
                SemanticLayoutIdentityV1::from_sha256([250; 32]),
                if kernel {
                    SemanticCanonAbiV1::GpuKernel
                } else {
                    SemanticCanonAbiV1::Rust
                },
                if kernel {
                    SemanticExternAbiV1::GpuKernel
                } else {
                    SemanticExternAbiV1::Rust
                },
                false,
                false,
                1,
                vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    RAW,
                    SemanticAbiPassModeV1::Direct(plain),
                ))],
                SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap()
            .with_source_argument_ownership(vec![ownership])
            .unwrap(),
            vec![
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 6; 32]),
                    UNIT,
                    SemanticLocalRoleV1::Return,
                    source,
                ),
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 7; 32]),
                    RAW,
                    SemanticLocalRoleV1::Argument(0),
                    source,
                ),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let returned = |tag| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap()
    };
    let call = SemanticDirectCallV1::new(
        HELPER,
        vec![if reference && mutable {
            SemanticOperandV1::Move(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], RAW).unwrap(),
            )
        } else {
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], RAW).unwrap(),
            )
        }],
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], UNIT).unwrap(),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let root = make(
        101,
        true,
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([109; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
            returned(110),
        ],
    )
    .with_kernel_entry(template.functions()[0].kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new(
        template.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, make(121, false, vec![returned(129)])],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap()
}

fn expected_pointer(space: u32, mutable: bool) -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        match space {
            0 => AddressSpace::Generic,
            1 => AddressSpace::Global,
            3 => AddressSpace::Workgroup,
            4 => AddressSpace::Constant,
            5 => AddressSpace::Private,
            _ => unreachable!(),
        },
        if mutable {
            AccessMode::ReadWrite
        } else {
            AccessMode::ReadOnly
        },
    )
}

#[test]
fn original_reference_helper_transport_preserves_exact_ownership_space_and_access() {
    for space in [0, 1, 4] {
        for mutable in [false, true] {
            let ownership = if mutable {
                SemanticSourceArgumentOwnershipV1::UniqueBorrow
            } else {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            };
            let source = pointer_source(space, mutable, ownership, true);
            let function = &source.functions()[1];
            let arguments = source.logical_arguments_v1(HELPER).unwrap();
            for policy in [
                ParameterLeafPolicyV1::SharedSliceLeaves,
                ParameterLeafPolicyV1::ExecutionAbiWords,
            ] {
                let mapped = arguments.adjusted_arguments().next().unwrap();
                let (shared_slice, rows) = source_helper_parameter_shape_with_policy_v18(
                    source.types(),
                    function,
                    HELPER,
                    mapped,
                    policy,
                )
                .unwrap();
                assert!(
                    !shared_slice,
                    "thin reference is not a shared-slice representation"
                );
                assert_eq!(rows, vec![(vec![], RAW, expected_pointer(space, mutable))]);
                let mapped = arguments.adjusted_arguments().next().unwrap();
                assert!(
                    helper_parameter_shape_with_policy_v1(
                        source.types(),
                        function,
                        HELPER,
                        mapped,
                        policy,
                    )
                    .is_err(),
                    "historical helper entry remains closed"
                );
            }
            let mapped = arguments.adjusted_arguments().next().unwrap();
            assert!(
                source_helper_parameter_shape_with_policy_v18(
                    source.types(),
                    function,
                    HELPER,
                    mapped,
                    ParameterLeafPolicyV1::PointerFree,
                )
                .is_err()
            );
        }
    }
}

#[test]
fn original_reference_helper_transport_rejects_kind_ownership_and_width_substitution() {
    for mutable in [false, true] {
        let exact = if mutable {
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        };
        for ownership in [
            SemanticSourceArgumentOwnershipV1::Unspecified,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::RawPointer,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        ] {
            if ownership == exact {
                continue;
            }
            let source = pointer_source(0, mutable, ownership, true);
            let arguments = source.logical_arguments_v1(HELPER).unwrap();
            let mapped = arguments.adjusted_arguments().next().unwrap();
            assert!(
                source_helper_parameter_shape_v18(
                    source.types(),
                    &source.functions()[1],
                    HELPER,
                    mapped,
                )
                .is_err(),
                "mutable={mutable} ownership={ownership:?}"
            );
        }
        for space in [3, 5] {
            let source = pointer_source(space, mutable, exact, true);
            let arguments = source.logical_arguments_v1(HELPER).unwrap();
            let mapped = arguments.adjusted_arguments().next().unwrap();
            assert!(
                source_helper_parameter_shape_v18(
                    source.types(),
                    &source.functions()[1],
                    HELPER,
                    mapped,
                )
                .is_err(),
                "32-bit source pointer must remain outside the 64-bit carrier"
            );
        }
    }
}

#[test]
fn original_raw_helper_values_preserve_address_space_access_and_legacy_refusal() {
    for space in [0, 1, 4] {
        for mutable in [false, true] {
            let source = raw_source(
                space,
                mutable,
                SemanticSourceArgumentOwnershipV1::RawPointer,
            );
            let function = &source.functions()[1];
            let arguments = source.logical_arguments_v1(HELPER).unwrap();
            for policy in [
                ParameterLeafPolicyV1::SharedSliceLeaves,
                ParameterLeafPolicyV1::ExecutionAbiWords,
            ] {
                let mapped = arguments.adjusted_arguments().next().unwrap();
                let (shared, rows) = source_helper_parameter_shape_with_policy_v18(
                    source.types(),
                    function,
                    HELPER,
                    mapped,
                    policy,
                )
                .unwrap();
                assert!(
                    !shared,
                    "raw value transport must not become a shared borrow"
                );
                assert_eq!(rows, vec![(vec![], RAW, expected_pointer(space, mutable))]);
                let mapped = arguments.adjusted_arguments().next().unwrap();
                assert!(matches!(
                    helper_parameter_shape_with_policy_v1(
                        source.types(),
                        function,
                        HELPER,
                        mapped,
                        policy
                    ),
                    Err(ProductionSourceArgumentErrorV1::Unsupported {
                        detail: "helper parameter is not an exact by-value scalar aggregate or shared slice",
                        ..
                    })
                ));
            }
            let mapped = arguments.adjusted_arguments().next().unwrap();
            assert!(
                source_helper_parameter_shape_with_policy_v18(
                    source.types(),
                    function,
                    HELPER,
                    mapped,
                    ParameterLeafPolicyV1::PointerFree
                )
                .is_err()
            );
        }
    }
}

#[test]
fn original_raw_helper_refuses_admitted_32_bit_local_pointers() {
    for space in [3, 5] {
        for mutable in [false, true] {
            let source = raw_source(
                space,
                mutable,
                SemanticSourceArgumentOwnershipV1::RawPointer,
            );
            let function = &source.functions()[1];
            let arguments = source.logical_arguments_v1(HELPER).unwrap();
            for policy in [
                ParameterLeafPolicyV1::PointerFree,
                ParameterLeafPolicyV1::SharedSliceLeaves,
                ParameterLeafPolicyV1::ExecutionAbiWords,
            ] {
                let mapped = arguments.adjusted_arguments().next().unwrap();
                assert!(matches!(
                    source_helper_parameter_shape_with_policy_v18(
                        source.types(),
                        function,
                        HELPER,
                        mapped,
                        policy,
                    ),
                    Err(ProductionSourceArgumentErrorV1::Unsupported { .. })
                ));
            }
        }
    }
}

#[test]
fn original_raw_helper_requires_explicit_raw_pointer_ownership() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::Unspecified,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
    ] {
        let source = raw_source(0, false, ownership);
        let arguments = source.logical_arguments_v1(HELPER).unwrap();
        let mapped = arguments.adjusted_arguments().next().unwrap();
        assert!(matches!(
            source_helper_parameter_shape_v18(
                source.types(),
                &source.functions()[1],
                HELPER,
                mapped
            ),
            Err(ProductionSourceArgumentErrorV1::Unsupported {
                detail: "helper parameter is not an exact by-value scalar aggregate or shared slice",
                ..
            })
        ));
    }
}

#[test]
fn original_raw_helper_correspondence_is_atomic_and_rejects_substitution() {
    let source = raw_source(0, false, SemanticSourceArgumentOwnershipV1::RawPointer);
    check_pointer_correspondence(&source, false);
}

#[test]
fn original_reference_helper_correspondence_is_atomic_and_rejects_substitution() {
    for mutable in [false, true] {
        let ownership = if mutable {
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        };
        let source = pointer_source(0, mutable, ownership, true);
        check_pointer_correspondence(&source, mutable);
    }
}

fn check_pointer_correspondence(source: &AdmittedInertSemanticMirV1, mutable: bool) {
    for mutation in 0..4 {
        let mut ty = expected_pointer(0, mutable);
        if mutation == 1 {
            ty = expected_pointer(1, mutable);
        }
        if mutation == 2 {
            ty = expected_pointer(0, !mutable);
        }
        let target = Function::internal_helper(
            "raw_source_helper",
            Signature::new(vec![ty], vec![]),
            vec![ValueId(0)],
            vec![],
        );
        let rows = [SemanticKirParameterBindingV1 {
            correspondence_owner: ROOT,
            semantic_function: HELPER,
            semantic_local: SemanticLocalIdV1::from_index(if mutation == 3 { 0 } else { 1 }),
            kernel_ir_value: ValueId(0),
        }];
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        let visited = Cell::new(0);
        let result = with_parameter_correspondence_v18(
            source,
            ArgumentEntryV18 {
                correspondence_owner: ROOT,
                semantic_function: HELPER,
                kernel_ir_function: &target.id,
                role: SemanticKirFunctionRoleV1::InternalHelper,
            },
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            None,
            |view, budget| {
                view.visit_nodes_with_budget(budget, |node, _| {
                    assert_eq!(node.semantic_type(), RAW);
                    assert!(node.source_path().is_empty());
                    assert!(matches!(
                        node.coverage(),
                        ProductionArgumentCoverageV1::Parameter(_)
                    ));
                    visited.set(visited.get() + 1);
                    Ok(())
                })
            },
        );
        if mutation == 0 {
            result.unwrap();
            assert_eq!(visited.get(), 1);
        } else {
            assert!(matches!(
                result,
                Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(visited.get(), 0);
        }
        assert_eq!(budget.storage(), 17);
        assert!(!cleanup.refund_denied());
    }
}
