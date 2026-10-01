use super::*;
use crate::{
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    ProductionSourceLaunchRosterV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const LENGTH: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const CARRIER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const BORROW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
const ENTRY: SemanticBlockIdV1 = SemanticBlockIdV1::from_index(0);

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}

fn scalar(bits: u16) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, bits, u64::from(bits / 8)),
        SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
    )
}

fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}

fn abi(tag: u8, kernel: bool) -> SemanticFunctionAbiV1 {
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let borrowed = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        16,
        Some(8),
    )
    .unwrap();
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
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
            if kernel { CARRIER } else { BORROW },
            if kernel {
                SemanticAbiPassModeV1::Pair {
                    first: attributes,
                    second: attributes,
                }
            } else {
                SemanticAbiPassModeV1::Direct(borrowed)
            },
        ))],
        SemanticAbiValueV1::new(
            if kernel { UNIT } else { LENGTH },
            if kernel {
                SemanticAbiPassModeV1::Ignore
            } else {
                SemanticAbiPassModeV1::Direct(attributes)
            },
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![if kernel {
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner
    } else {
        SemanticSourceArgumentOwnershipV1::SharedBorrow
    }])
    .unwrap()
}

// An admitted semantic component fixture, not a rustc/imported provenance token.
// The unchanged actual-backend source gate remains the genuine source test.
pub(super) fn owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with_live_carrier(false)
}

fn owner_with_live_carrier(live: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with_carrier_observation(live, false)
}

fn owner_with_carrier_observation(live: bool, field_read: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with_carrier_access(live, u8::from(field_read))
}

fn owner_with_carrier_access(live: bool, observation: u8) -> ProductionSemanticSsaOwnerV1 {
    let raw_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let reference_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let raw_properties = SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
        Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        None,
    );
    let reference_properties = SemanticTypeAbiPropertiesV1::new(false, false)
        .with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    16,
                    8,
                )
                .unwrap(),
            ),
            None,
        );
    let mut types = vec![
        declaration(
            1,
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
        ),
        declaration(
            2,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(scalar(32)),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        declaration(
            3,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(scalar(64)),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
        declaration(
            4,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(raw_scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ELEMENT,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(raw_properties),
        declaration(
            5,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(raw_scalar, scalar(64)),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![POINTER, LENGTH, UNIT]).unwrap(),
            ),
        )
        .with_rustc_abi_properties(raw_properties),
        declaration(
            6,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(reference_scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    CARRIER,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(reference_properties),
    ];
    let observed_pointer = SemanticTypeIdV1::from_index(6);
    if matches!(observation, 2 | 4) {
        let raw = observation == 2;
        let pointee = if raw { CARRIER } else { LENGTH };
        types.push(
            declaration(
                7,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(if raw { raw_scalar } else { reference_scalar }),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        if raw {
                            SemanticPointerKindV1::Raw
                        } else {
                            SemanticPointerKindV1::Reference
                        },
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(if raw {
                raw_properties
            } else {
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            8,
                            8,
                        )
                        .unwrap(),
                    ),
                    None,
                )
            }),
        );
    }
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![SemanticOperandV1::Copy(place(2, BORROW))],
        Some(SemanticCallDestinationV1::new(
            place(3, LENGTH),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let mut continuation = Vec::new();
    if live {
        continuation.push(assign(
            2,
            BORROW,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1, CARRIER),
            },
        ));
    }
    let field = || {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), LENGTH).unwrap()],
            LENGTH,
        )
        .unwrap()
    };
    match observation {
        0 | 5 => {}
        1 | 6 => continuation.push(assign(
            3,
            LENGTH,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field())),
        )),
        2 => continuation.push(assign(
            4,
            observed_pointer,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(1, CARRIER),
            },
        )),
        3 => continuation.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1)),
        )),
        4 => continuation.push(assign(
            4,
            observed_pointer,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: field(),
            },
        )),
        _ => panic!("receiver observation mode"),
    }
    continuation.push(assign(
        0,
        UNIT,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))),
    ));
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([20; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([20; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([20; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([20; 32]),
        source(),
        abi(20, true),
        [UNIT, CARRIER, BORROW, LENGTH]
            .into_iter()
            .chain(matches!(observation, 2 | 4).then_some(observed_pointer))
            .chain(matches!(observation, 5 | 6).then_some(CARRIER))
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([40 + index as u8; 32]),
                    ty,
                    match index {
                        0 => SemanticLocalRoleV1::Return,
                        1 => SemanticLocalRoleV1::Argument(0),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                    source(),
                )
            })
            .collect(),
        ENTRY,
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([60; 32]),
                source(),
                {
                    let mut statements = Vec::new();
                    if matches!(observation, 5 | 6) {
                        statements.push(assign(
                            4,
                            CARRIER,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, CARRIER))),
                        ));
                        statements.push(assign(
                            1,
                            CARRIER,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(4, CARRIER))),
                        ));
                    }
                    statements.push(assign(
                        2,
                        BORROW,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(1, CARRIER),
                        },
                    ));
                    statements
                },
                SemanticTerminatorV1::new(source(), SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([61; 32]),
                source(),
                continuation,
                SemanticTerminatorV1::new(source(), SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let function = function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"allocation_receiver".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([22; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let callable = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([21; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([21; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([21; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([21; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([21; 32]),
            source(),
            abi(21, false),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
            disjoint_slice: CARRIER,
            element: ELEMENT,
            raw_index: LENGTH,
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([21; 32]),
    };
    let source = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(ROOT), callable],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let mut owner =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    owner
}

fn original_call(function: &SemanticFunctionDeclV1) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[0].terminator().kind() else {
        panic!("original call")
    };
    call
}

fn run(
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    super::source_reference_plan_v29_tests::run_owner_with_storage(owner(), consume)
}

fn binding(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SemanticSourceReferenceBindingV29 {
    assert_eq!(plan.loans.len(), 1);
    assert!(matches!(
        plan.loans[0].representation,
        SourceReferenceRepresentationV29::ExistingAllocationBinding(_)
    ));
    let ty = source_reference_payload_types_v29(plan, 0, budget)
        .unwrap()
        .pop()
        .unwrap();
    SemanticSourceReferenceBindingV29 {
        owner: plan as *const SourceReferencePlanV29<'_, '_> as usize,
        source: plan.source,
        ssa: plan.ssa,
        root: plan.root,
        origin: SourceReferenceBindingOriginV29::SingleLoan(0),
        source_type: BORROW,
        values: vec![ValueDef::new(ValueId(17), ty)],
    }
}

#[test]
fn checked_allocation_receiver_emits_original_borrow_and_len_without_synthetic_deref() {
    for live in [false, true] {
        let completed = std::cell::Cell::new(false);
        let result = super::source_reference_plan_v29_tests::run_owner_with_storage(
            owner_with_live_carrier(live),
            |plan, budget| {
                let references = SourceReferenceEmissionV29::new(plan, budget)?;
                let instances = plan.instances;
                let semantic = instances.owner().source_semantic();
                with_execution_instance_layouts_v29(plan, budget, |signatures, budget| {
                    with_execution_call_scope_v29(budget, |scope, budget| {
                        let mut closure = ReachableClosureBudgetV1::new(16_384);
                        let root_plan = kernel_entry_plan_v1(
                            semantic,
                            ROOT,
                            ROOT,
                            FunctionId::new("allocation.receiver"),
                            16_384,
                            &mut closure,
                        )?;
                        let mut private = PrivateArrayLazyBudgetV1::new(1, 16_384);
                        let mut sink = ExecutionDefinedCallSinkV29::new(scope, instances, budget)?;
                        let lowered = with_source_reference_availability_v29(
                            instances,
                            instances.root(),
                            Some(&references),
                            budget,
                            |cursor, budget| {
                                if live {
                                    // Taking the original carrier's address retains it;
                                    // only promoted references belong to the SSA cursor.
                                    assert!(
                                        !instances
                                            .instance(instances.root())
                                            .unwrap()
                                            .ssa()
                                            .plan()
                                            .promoted_variables()
                                            .iter()
                                            .any(|local| local.get() == 1)
                                    );
                                    assert!(!cursor.cfg.reference_locals[1]);
                                    assert!(
                                        !cursor.cfg.entries.iter().any(|entry| entry.local == 1)
                                    );
                                    for block in [ENTRY, SemanticBlockIdV1::from_index(1)] {
                                        let node = references.block_node(instances.root(), block,
                                    SemanticLocalIdV1::from_index(1), budget)?
                                    .expect("the retained original carrier reaches both borrow sites");
                                        assert!(matches!(
                                            plan.nodes[node].kind,
                                            SourceReferenceNodeKindV29::Plain(Some(
                                                SourceReferenceAnchorV29 {
                                                    argument: 0,
                                                    ty: CARRIER,
                                                }
                                            ))
                                        ));
                                        assert!(plan.nodes[node].descriptor.is_none());
                                    }
                                }
                                lower_one_source_function_with_calls_v29(
                                    semantic,
                                    &root_plan,
                                    instances.instance(instances.root()).unwrap().ssa(),
                                    &BTreeMap::new(),
                                    signatures,
                                    None,
                                    BTreeSet::new(),
                                    1,
                                    true,
                                    16_384,
                                    None,
                                    &mut private,
                                    None,
                                    budget,
                                    SemanticEmissionPlacementV1::default(),
                                    Some(cursor),
                                    Some(&mut sink),
                                    None,
                                )
                            },
                        )?;
                        sink.finish([&lowered].into_iter(), instances, budget)?;
                        assert_eq!(lowered.source_call_instance, Some(instances.root()));
                        let body = lowered.function.body.as_ref().unwrap();
                        let lengths: Vec<_> = body
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .filter_map(|operation| match operation.kind {
                                OperationKind::SliceLength { slice } => Some(slice),
                                _ => None,
                            })
                            .collect();
                        assert_eq!(lengths.len(), 1);
                        assert!(
                            plan.accesses.is_empty(),
                            "receiver is not a fabricated dereference"
                        );
                        let archive = lowered.execution_observation.as_ref().unwrap();
                        let SemanticValueBindingV1::SourceReference(binding) =
                            archive.locals[2].as_ref().unwrap()
                        else {
                            panic!("actual original borrow remains a checked reference");
                        };
                        assert_eq!(binding.values.len(), 1);
                        assert_eq!(binding.values[0].id, lengths[0]);
                        assert!(matches!(binding.values[0].ty, Type::Slice(_)));
                        assert_eq!(plan.loans.len(), if live { 2 } else { 1 });
                        for (loan_index, loan) in plan.loans.iter().enumerate() {
                            assert!(matches!(
                                loan.representation,
                                SourceReferenceRepresentationV29::ExistingAllocationBinding(_)
                            ));
                            assert_eq!(
                                loan.site.block,
                                SemanticBlockIdV1::from_index(loan_index as u32)
                            );
                            let mut archived =
                                archive.bindings.values().filter_map(|value| match value {
                                    SemanticValueBindingV1::SourceReference(candidate)
                                        if candidate.origin
                                            == SourceReferenceBindingOriginV29::SingleLoan(
                                                loan_index,
                                            ) =>
                                    {
                                        Some(candidate)
                                    }
                                    _ => None,
                                });
                            let original = archived
                                .next()
                                .expect("each original borrow definition is archived");
                            assert_eq!(original.values.len(), 1);
                            assert_eq!(original.values[0], binding.values[0]);
                            assert!(
                                archived.next().is_none(),
                                "one definition per original borrow"
                            );
                        }
                        assert!(archive.locals[2].as_ref().unwrap().value().is_err());
                        let definition = *archive.bindings.iter().find(|(_, value)| {
                    matches!(value, SemanticValueBindingV1::SourceReference(candidate) if candidate == binding)
                }).expect("the actual original borrow definition is archived").0;
                        check_execution_archive_v29(
                            &archive.locals,
                            &archive.bindings,
                            &place(2, BORROW),
                            definition,
                            budget,
                        )?;
                        let mut forged = archive.locals.clone();
                        let SemanticValueBindingV1::SourceReference(changed) =
                            forged[2].as_mut().unwrap()
                        else {
                            unreachable!()
                        };
                        changed.values[0].id = ValueId(u32::MAX);
                        assert!(
                            check_execution_archive_v29(
                                &forged,
                                &archive.bindings,
                                &place(2, BORROW),
                                definition,
                                budget
                            )
                            .is_err(),
                            "same-typed receiver payload cannot replace the original current/archive value"
                        );
                        let error = references.finish(budget).expect_err(
                            "the low-level observer has not consumed expanded physical admission",
                        );
                        assert!(
                            matches!(&error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
                    if *detail == "original raw source requires consuming expanded physical admission")
                        );
                        completed.set(true);
                        Err(error)
                    })
                })
            },
        );
        assert!(
            matches!(&result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
        if *detail == "original raw source requires consuming expanded physical admission"),
            "{result:?}"
        );
        assert!(
            completed.get(),
            "all checks inside guarded scopes completed"
        );
        allocation_receiver_full_pending_v29(live, false);
    }
}

#[test]
fn allocation_receiver_profiled_pending_rejects_one_short_preexisting_occurrence_reservation() {
    for live in [false, true] {
        allocation_receiver_full_pending_v29(live, true);
    }
}

fn allocation_receiver_full_pending_v29(live: bool, one_short_occurrences: bool) {
    allocation_receiver_full_pending_observation_v29(live, one_short_occurrences, false);
}

fn allocation_receiver_full_pending_observation_v29(
    live: bool,
    one_short_occurrences: bool,
    field_read: bool,
) {
    allocation_receiver_full_pending_access_v29(live, one_short_occurrences, u8::from(field_read));
}

fn allocation_receiver_full_pending_access_v29(
    live: bool,
    one_short_occurrences: bool,
    observation: u8,
) {
    use fe2o3_kernel_descriptor::{
        DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName,
    };
    let owner = owner_with_carrier_access(live, observation);
    let occurrences = owner.occurrence_storage().unwrap().retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000_000);
    const FLOOR: usize = 37;
    budget.reserve_storage(FLOOR).unwrap();
    // Capture returns a transfer receipt, not a reservation on this ledger.
    budget.reserve_storage(occurrences).unwrap();
    let semantic = owner.source_semantic();
    let hash = *owner.source_semantic_sha256();
    let entry = semantic.functions()[0].kernel_entry().unwrap();
    let binding = *entry.kernel_binding_identity().as_bytes();
    assert_eq!(
        semantic.functions()[0].abi().source_argument_ownership(),
        &[SemanticSourceArgumentOwnershipV1::ExclusiveOwner]
    );
    let launch = ProductionSourceLaunchRosterV1::try_new(
        semantic,
        &[ProductionSourceLaunchRootInputV1::new(
            "allocation_receiver",
            binding,
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let arguments = [ProductionKernelArgumentAbiArgumentV18 {
        semantic_type_identity: semantic.types()[CARRIER.index() as usize].identity(),
        kind: ProductionKernelArgumentAbiKindV18::Descriptor {
            source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
            argument: LogicalArgumentV1::disjoint_slice(
                0,
                ValidName::new("owner").unwrap(),
                &source,
                &layout,
                fe2o3_kernel_descriptor::AccessMode::ReadWrite,
                0,
            )
            .unwrap(),
        },
    }];
    let roots = [ProductionKernelArgumentAbiRootV18 {
        kernel_binding: &binding,
        export: "allocation_receiver",
        arguments: &arguments,
        explicit_argument_bytes: 16,
        kernarg_alignment_bytes: 8,
    }];
    let classes = [ProductionScopeCallableCandidateV29::Ordinary; 2];
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &hash,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    if one_short_occurrences {
        assert!(occurrences > FLOOR);
        // The constructor compares its total entry storage with the receipt.
        budget.release_storage(FLOOR + 1).unwrap();
        assert_eq!(budget.storage(), occurrences - 1);
    }
    let before_work = budget.work();
    let result = ProductionPendingScopedSourceOwnerV29::try_materialize_with_kernel_abi_budget_v18(
        owner,
        launch,
        input,
        ProductionKernelArgumentAbiInputV18 { roots: &roots },
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    );
    if one_short_occurrences {
        assert!(
            matches!(
                result,
                Err(ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ))
            ),
            "one-short occurrence reservation, live={live}"
        );
        assert_eq!(budget.work(), before_work);
        assert_eq!(budget.storage(), occurrences - 1);
        budget.release_storage(occurrences - 1 - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        return;
    }
    if observation != 0 && observation != 5 {
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("wrapper layout observation needs object materialization"),
        };
        let expected = if observation == 3 {
            // This original StorageLive restarts the owner while local2 still
            // holds its shared loan. Source lifetime checking must refuse
            // before a backing choice or physical entry initialization.
            "source reference referent storage dies with a live loan"
        } else {
            "typed entry allocation requires source-bound object materialization"
        };
        assert!(
            matches!(error, ProductionPendingScopedSourceErrorV29::Source(
            ProductionSemanticKirErrorV1::Unsupported { detail, .. })
            if detail == expected),
            "observation {observation}: {error:?}"
        );
        assert_eq!(budget.storage(), FLOOR + occurrences);
        budget.release_storage(occurrences).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        return;
    }
    let pending = result
        .unwrap_or_else(|error| panic!("full carrier physical admission, live={live}: {error:?}"));
    let retained = pending.adopted_storage();
    assert_eq!(budget.storage(), FLOOR + occurrences + retained);
    let mut lengths = 0;
    for function in &pending.pending_module().functions {
        let Some(body) = &function.body else { continue };
        for block in &body.blocks {
            for operation in &block.operations {
                if matches!(operation.kind, OperationKind::SliceLength { .. }) {
                    lengths += 1;
                }
                assert!(
                    !matches!(
                        operation.kind,
                        OperationKind::Load { .. }
                            | OperationKind::Store { .. }
                            | OperationKind::Alloca { .. }
                            | OperationKind::Storage(_)
                    ),
                    "a borrow receiver must not become a synthetic dereference"
                );
            }
        }
    }
    assert_eq!(
        lengths, 1,
        "the full pending path must retain the genuine receiver call"
    );
    drop(pending);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR + occurrences);
    budget.release_storage(occurrences).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn allocation_receiver_existing_representation_does_not_erase_wrapper_field_observation() {
    allocation_receiver_full_pending_observation_v29(true, false, false);
    allocation_receiver_full_pending_observation_v29(true, false, true);
}

#[test]
fn allocation_receiver_existing_entry_keeps_original_restarts_raw_and_field_borrows_addressable() {
    allocation_receiver_full_pending_access_v29(true, false, 0);
    for observation in [2, 3, 4] {
        allocation_receiver_full_pending_access_v29(true, false, observation);
    }
}

#[test]
fn allocation_receiver_whole_owner_moves_preserve_the_native_slice_without_wrapper_storage() {
    for live in [false, true] {
        allocation_receiver_full_pending_access_v29(live, false, 5);
        allocation_receiver_full_pending_access_v29(live, false, 6);
        allocation_receiver_full_pending_access_v29(live, true, 5);
    }
}

#[test]
fn allocation_receiver_whole_owner_write_rechecks_exact_original_destination_and_role() {
    let reached = std::cell::Cell::new(false);
    super::source_reference_plan_v29_tests::run_owner_with_storage(
        owner_with_carrier_access(false, 5),
        |plan, budget| {
            let original = plan
                .accesses
                .iter()
                .find(|access| {
                    access.key.access == SourceReferenceAccessV29::Write
                        && access.local.index() == 1
                })
                .expect("original owner replacement");
            let before = budget.work();
            assert!(source_existing_receiver_write_v53(plan, original, budget)?);
            assert_eq!(budget.work() - before, 20);
            for fault in 0..5 {
                let mut changed = SourceReferenceAccessRecordV29 {
                    key: original.key,
                    source_local: original.source_local,
                    ty: original.ty,
                    instance: original.instance,
                    local: original.local,
                    generation: original.generation,
                    projections: original.projections.clone(),
                    loan: original.loan,
                    traversed: original.traversed.clone(),
                    shared_path: original.shared_path,
                };
                match fault {
                    0 => changed.key.source = 0,
                    1 => changed.key.access = SourceReferenceAccessV29::Read,
                    2 => changed.key.site.statement = None,
                    3 => changed.source_local = SemanticLocalIdV1::from_index(4),
                    4 => changed.ty = LENGTH,
                    _ => unreachable!(),
                }
                assert!(
                    !source_existing_receiver_write_v53(plan, &changed, budget)?,
                    "fault {fault}"
                );
            }
            reached.set(true);
            Ok(())
        },
    )
    .unwrap();
    assert!(reached.get());
}

#[test]
fn allocation_receiver_existing_entry_disposition_rechecks_original_local_and_activation_kind() {
    let mut completed = false;
    run(|plan, budget| {
        let rows = source_existing_receiver_rows_v29(plan, budget)?;
        let original = SourceReferenceStorageActivationV29 {
            instance: plan.instances.root(),
            local: SemanticLocalIdV1::from_index(1),
            generation: 0,
            origin: SourceReferenceActivationOriginV29::Entry,
        };
        // These activation rows are inert hypotheses over a real original plan.
        // The full pending tests exercise the authentic activation census.
        assert!(source_existing_receiver_entry_v29(
            plan, &rows, original, budget
        )?);
        for changed in [
            SourceReferenceStorageActivationV29 {
                generation: 1,
                ..original
            },
            SourceReferenceStorageActivationV29 {
                local: SemanticLocalIdV1::from_index(2),
                ..original
            },
            SourceReferenceStorageActivationV29 {
                origin: SourceReferenceActivationOriginV29::StorageLive(SourceReferenceSiteV29 {
                    instance: plan.instances.root(),
                    block: ENTRY,
                    statement: Some(0),
                }),
                ..original
            },
        ] {
            assert!(!source_existing_receiver_entry_v29(
                plan, &rows, changed, budget
            )?);
        }
        assert!(!source_existing_receiver_entry_v29(
            plan,
            &[],
            original,
            budget
        )?);
        completed = true;
        Ok(())
    })
    .unwrap();
    assert!(completed);
}

#[test]
fn allocation_receiver_existing_index_reuses_original_loans_and_exact_local_groups() {
    let mut completed = false;
    let result = run(|plan, budget| {
        let rows = source_existing_receiver_rows_v29(plan, budget)?;
        assert_eq!(rows, vec![((plan.instances.root().index(), 1), true)]);
        assert!(source_existing_receiver_v29(
            &rows,
            plan.instances.root(),
            SemanticLocalIdV1::from_index(1),
            budget
        )?);
        assert!(!source_existing_receiver_v29(
            &rows,
            plan.instances.root(),
            SemanticLocalIdV1::from_index(2),
            budget
        )?);
        completed = true;
        Ok(())
    });
    assert!(completed);
    result.unwrap();
    // Inert grouping controls do not stand in for original-source qualification.
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let mut rows = vec![
        ((7, 1), true),
        ((2, 1), true),
        ((7, 1), false),
        ((2, 3), false),
        ((2, 1), true),
        ((7, u32::MAX), true),
    ];
    source_existing_receiver_group_v29(&mut rows, &mut budget).unwrap();
    assert_eq!(
        rows,
        vec![
            ((2, 1), true),
            ((2, 3), false),
            ((7, 1), false),
            ((7, u32::MAX), true)
        ]
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn allocation_receiver_existing_group_has_independent_exact_and_short_work() {
    for count in [1usize, 16, 64, 1024] {
        let width = count * 2;
        let logarithm = (usize::BITS - width.leading_zeros()) as usize + 1;
        let expected = 4 * width * logarithm + 3 * count;
        for limit in [expected, expected - 1] {
            let mut rows: Vec<_> = (0..count)
                .rev()
                .map(|local| ((7, local as u32), true))
                .collect();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = source_existing_receiver_group_v29(&mut rows, &mut budget);
            if limit == expected {
                result.unwrap();
                assert_eq!(rows.len(), count);
                for (local, row) in rows.iter().enumerate() {
                    assert_eq!(*row, ((7, local as u32), true));
                }
                assert_eq!(budget.work(), expected);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.actual() == expected && error.limit() == limit)
                );
            }
            assert_eq!(budget.storage(), 0);
        }
    }
}

#[test]
fn allocation_receiver_rejects_equal_cloned_call_wrong_argument_and_block() {
    let owner = owner();
    let source = owner.source_semantic();
    let function = &source.functions()[0];
    let call = original_call(function);
    let cloned = call.clone();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    assert_eq!(
        allocation_receiver_contract_v29(function, source.callables(), ENTRY, call, 0, &mut budget)
            .unwrap(),
        (CARRIER, false)
    );
    for (block, candidate, receiver) in [
        (ENTRY, &cloned, 0),
        (ENTRY, call, 1),
        (SemanticBlockIdV1::from_index(1), call, 0),
    ] {
        assert!(matches!(
            allocation_receiver_contract_v29(
                function,
                source.callables(),
                block,
                candidate,
                receiver,
                &mut budget
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "allocation receiver differs from its original checked carrier",
                ..
            })
        ));
    }
}

#[test]
fn allocation_receiver_keeps_ordinary_slice_receiver_path_without_checked_references() {
    let owner = owner();
    let source = owner.source_semantic();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let mut closure = ReachableClosureBudgetV1::new(16_384);
    let plan = kernel_entry_plan_v1(
        source,
        ROOT,
        ROOT,
        FunctionId::new("allocation.legacy"),
        16_384,
        &mut closure,
    )
    .unwrap();
    let mut private = PrivateArrayLazyBudgetV1::new(1, 16_384);
    let lowered = lower_one_semantic_function_v1(
        source,
        &plan,
        owner.plan_for_function(ROOT).unwrap(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
        BTreeSet::new(),
        1,
        true,
        16_384,
        None,
        &mut private,
        None,
        &mut budget,
        SemanticEmissionPlacementV1::default(),
        None,
    )
    .unwrap();
    assert_eq!(
        lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::SliceLength { .. }))
            .count(),
        1
    );
}

#[test]
fn allocation_receiver_binding_cannot_change_owner_carrier_or_physical_type() {
    let completed = std::cell::Cell::new(0);
    let result = run(|plan, budget| {
        for hostile in 0..10 {
            let mut binding = binding(plan, budget);
            let mut source_type = BORROW;
            let mut referent = CARRIER;
            match hostile {
                0 => binding.owner ^= 1,
                1 => binding.source[0] ^= 1,
                2 => binding.origin = SourceReferenceBindingOriginV29::SingleLoan(1),
                3 => binding.source_type = POINTER,
                4 => source_type = POINTER,
                5 => referent = ELEMENT,
                6 => binding.values.clear(),
                7 => binding.values[0].ty = Type::Scalar(ScalarType::U64),
                8 => binding.values.push(binding.values[0].clone()),
                9 => {}
                _ => unreachable!(),
            }
            assert!(
                source_reference_allocation_value_v29(
                    plan,
                    plan.instances.owner().source_semantic().types(),
                    &mut binding,
                    source_type,
                    referent,
                    hostile == 9,
                    budget
                )
                .is_err()
            );
            completed.set(completed.get() + 1);
        }
        let mut binding = binding(plan, budget);
        let expected = binding.values[0].clone();
        assert_eq!(
            source_reference_allocation_value_v29(
                plan,
                plan.instances.owner().source_semantic().types(),
                &mut binding,
                BORROW,
                CARRIER,
                false,
                budget
            )?,
            Some((expected.id, expected.ty))
        );
        assert!(
            binding.values.is_empty(),
            "the paid payload moved, not cloned"
        );
        completed.set(completed.get() + 1);
        Ok(())
    });
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(completed.get(), 11);
}

#[test]
fn allocation_receiver_source_contract_has_independent_exact_work_bound() {
    let owner = owner();
    let source = owner.source_semantic();
    let function = &source.functions()[0];
    for limit in [8, 7] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = allocation_receiver_contract_v29(
            function,
            source.callables(),
            ENTRY,
            original_call(function),
            0,
            &mut budget,
        );
        if limit == 8 {
            assert_eq!(result.unwrap(), (CARRIER, false));
            assert_eq!(budget.work(), 8);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work { .. }
                    )
                )
            ));
        }
        assert_eq!(budget.storage(), 0);
    }
}

fn header<T>() -> usize {
    std::mem::size_of::<T>() + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
}

fn allocation_receiver_validation_header_oracle() -> usize {
    type Args<'a, 'p, 's> = (
        &'a SourceReferencePlanV29<'p, 's>,
        &'a SemanticSourceReferenceBindingV29,
        &'a mut dyn SemanticEmissionBudgetV1,
    );
    type Capture<'a, 'p, 's> = (
        &'a SourceReferencePlanV29<'p, 's>,
        &'a SemanticSourceReferenceBindingV29,
        &'a mut dyn SemanticEmissionBudgetV1,
        Option<&'a source_storage_v29::SourceStorageRootCustodyViewV29<'p, 's>>,
        &'a mut Option<source_storage_v29::SourceStorageRootGrowthV29<'a, 'p, 's>>,
    );
    4 * header::<Args<'_, '_, '_>>()
        + header::<Capture<'_, '_, '_>>()
        + std::mem::align_of::<Capture<'_, '_, '_>>()
        + header::<std::panic::AssertUnwindSafe<Capture<'_, '_, '_>>>()
        + header::<&SourceReferencePlanV29<'_, '_>>()
        + header::<&SemanticSourceReferenceBindingV29>()
        + header::<&mut dyn SemanticEmissionBudgetV1>()
        + header::<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>()
        + header::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + header::<&Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + header::<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()
        + header::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + header::<&mut Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + header::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 4 * header::<usize>()
        + header::<Option<usize>>()
        + header::<bool>()
        + header::<Result<(), ProductionSemanticKirErrorV1>>()
        + header::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()
        + header::<&std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()
        + header::<Box<dyn std::any::Any + Send>>()
        + header::<&ProductionSemanticKirErrorV1>()
        + header::<Option<ProductionSemanticKirErrorV1>>()
        + header::<(
            Option<&SourceReferencePlanV29<'_, '_>>,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
            usize,
            Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
            Option<usize>,
            &mut dyn SemanticEmissionBudgetV1,
        )>()
        + header::<Option<&SourceReferencePlanV29<'_, '_>>>()
        + header::<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()
        + header::<Option<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + header::<usize>()
        + header::<bool>()
}

#[test]
fn allocation_receiver_extraction_exact_and_one_short_work_storage_preserve_first_failure() {
    // Each original owner check costs five; only the existing descriptor scan
    // depends on source cardinality. The carrier has pointer, length and Unit fields.
    let extraction_work = 5
        + 5
        + 2 * 5 // Borrowed result and nested-result prepayments each check the owner.
        + (5 + 5 + 8)
        + (5 + 12 + 4 + 4) // Scratch owner check, unwind frame, root growth capture/refund.
        + 5 // Scratch-header reservation performs its own owner check.
        + (5 + 1) // Original-owner check and Object-versus-Scalar strategy lookup.
        + (5 + 3)
        + (5 + 1)
        + 5
        + (5 + 3)
        + (5 + 2 * 4 + 3 * 20 + 32)
        + 5
        + (5 + 5 + 3)
        + (5 + 1)
        + (5 + 8);
    let retained_storage = header::<Option<(ValueId, Type)>>()
        + header::<Result<Option<(ValueId, Type)>, ProductionSemanticKirErrorV1>>()
        + header::<Option<&ValueDef>>()
        + header::<Result<Option<&ValueDef>, ProductionSemanticKirErrorV1>>();
    let extraction_peak = retained_storage
        + allocation_receiver_validation_header_oracle()
        + 3 * header::<Vec<Type>>()
        + 2 * std::mem::size_of::<Type>();
    const LIMIT: usize = 20_000_000;
    for denial in [None, Some(false), Some(true)] {
        let completed = std::cell::Cell::new(false);
        let result = super::source_reference_plan_v29_tests::run_owner_with_storage_limits(
            owner(),
            LIMIT,
            LIMIT,
            |plan, budget| {
                let mut binding = binding(plan, budget);
                if let Some(storage) = denial {
                    if storage {
                        budget.reserve_storage(LIMIT - budget.storage() - extraction_peak + 1)?;
                    } else {
                        budget.charge_work(LIMIT - budget.work() - extraction_work + 1)?;
                    }
                } else {
                    budget.reserve_storage(LIMIT - budget.storage() - extraction_peak)?;
                }
                let before = (budget.work(), budget.storage());
                let result = source_reference_allocation_value_v29(
                    plan,
                    plan.instances.owner().source_semantic().types(),
                    &mut binding,
                    BORROW,
                    CARRIER,
                    false,
                    budget,
                );
                match denial {
                    None => {
                        assert_eq!(result.as_ref().unwrap().as_ref().unwrap().0, ValueId(17));
                        assert_eq!(budget.work() - before.0, extraction_work);
                        assert_eq!(budget.storage() - before.1, retained_storage);
                        assert_eq!(
                            budget.peak_storage(),
                            LIMIT,
                            "exact independent scratch peak"
                        );
                        assert!(binding.values.is_empty());
                        completed.set(true);
                        Ok(())
                    }
                    Some(storage) => {
                        let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            error,
                        )) = result
                        else {
                            panic!("one-short extraction must report a resource failure");
                        };
                        assert!(if storage {
                            matches!(error, ArgumentResourceV1::Storage(bound)
                            if bound.actual() == LIMIT + 1 && bound.limit() == LIMIT)
                        } else {
                            matches!(error, ArgumentResourceV1::Work(bound)
                            if bound.actual() == LIMIT + 1 && bound.limit() == LIMIT)
                        });
                        assert!(
                            matches!(plan.failure.first_error(), Some(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first)) if first == error)
                        );
                        assert_eq!(budget.storage() - before.1, retained_storage);
                        assert_eq!(binding.values.len(), 1);
                        let before_replay = (budget.work(), budget.storage());
                        let replay = source_reference_allocation_value_v29(
                            plan,
                            plan.instances.owner().source_semantic().types(),
                            &mut binding,
                            BORROW,
                            CARRIER,
                            false,
                            budget,
                        );
                        assert!(
                            matches!(replay, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first)) if first == error)
                        );
                        assert_eq!((budget.work(), budget.storage()), before_replay);
                        completed.set(true);
                        Err(error.into())
                    }
                }
            },
        );
        assert!(
            completed.get(),
            "resource assertions completed inside guarded scope"
        );
        assert_eq!(result.is_ok(), denial.is_none(), "{result:?}");
    }
}

#[test]
fn anchored_owner_carrier_keeps_one_value_across_reference_cfg_transport() {
    let completed = std::cell::Cell::new(false);
    let result = run(|plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let node = source_reference_entry_node_v29(
            plan,
            plan.root,
            SemanticLocalIdV1::from_index(1),
            None,
            budget,
        )?
        .unwrap();
        assert!(matches!(
            plan.nodes[node].kind,
            SourceReferenceNodeKindV29::Plain(Some(SourceReferenceAnchorV29 {
                argument: 0,
                ty: CARRIER
            },))
        ));
        assert!(plan.nodes[node].descriptor.is_none());
        let ty = Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let held = SemanticValueBindingV1::Value {
            id: ValueId(17),
            ty: ty.clone(),
        };
        let mut leaves = [];
        source_reference_merge_node_v29(
            &references,
            node,
            &held,
            &held,
            &mut leaves.iter_mut(),
            &mut 0,
            budget,
        )?;
        assert_eq!(
            source_reference_node_types_v29(plan, node, budget)?,
            [ty.clone()]
        );
        assert_eq!(
            source_reference_cfg_node_types_v29(plan, node, budget)?,
            [ty.clone()]
        );
        assert!(source_reference_inactive_shape_matches_v29(
            plan, node, &held, &mut 0, budget
        )?);
        for canonical in [false, true] {
            let values = [ValueDef::new(ValueId(17), ty.clone())];
            let mut values = values.iter();
            let leaf_values = [];
            let mut leaves = leaf_values.iter();
            let rebuilt = source_reference_rebuild_node_v29(
                &references,
                node,
                canonical,
                &mut leaves,
                &mut values,
                &mut 0,
                budget,
            )?;
            assert!(
                matches!(rebuilt, SemanticValueBindingV1::Value { id: ValueId(17), ty: actual } if actual == ty)
            );
            assert!(values.next().is_none() && leaves.next().is_none());
        }
        let (leaves, exact) = source_reference_call_shape_v29(&references, node, &held, budget)?;
        assert!(leaves.is_empty());
        assert_eq!(exact, [None]);
        completed.set(true);
        Ok(())
    });
    assert!(
        completed.get(),
        "all carrier transport assertions completed"
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn anchored_owner_carrier_rejects_changed_value_type_shape_and_root_anchor() {
    for hostile in 0..8 {
        let completed = std::cell::Cell::new(false);
        // Every malformed input gets an independent owner and the owning
        // postflight must preserve this exact error, not a prior rejection.
        let result = run(|plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let node = source_reference_entry_node_v29(
                plan,
                plan.root,
                SemanticLocalIdV1::from_index(1),
                None,
                budget,
            )?
            .unwrap();
            let ty = Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            );
            let held = SemanticValueBindingV1::Value {
                id: ValueId(17),
                ty: ty.clone(),
            };
            let result = match hostile {
                0..=2 => {
                    let changed = match hostile {
                        0 => SemanticValueBindingV1::Value {
                            id: ValueId(18),
                            ty: ty.clone(),
                        },
                        1 => SemanticValueBindingV1::Value {
                            id: ValueId(17),
                            ty: Type::slice(
                                Type::Scalar(ScalarType::U32),
                                AddressSpace::Private,
                                AccessMode::ReadWrite,
                            ),
                        },
                        _ => SemanticValueBindingV1::Aggregate(vec![]),
                    };
                    source_reference_merge_node_v29(
                        &references,
                        node,
                        &changed,
                        &held,
                        &mut [].iter_mut(),
                        &mut 0,
                        budget,
                    )
                }
                3..=5 => {
                    let (anchor, requested) = match hostile {
                        3 => (
                            SourceReferenceAnchorV29 {
                                argument: 1,
                                ty: CARRIER,
                            },
                            CARRIER,
                        ),
                        4 => (
                            SourceReferenceAnchorV29 {
                                argument: 0,
                                ty: ELEMENT,
                            },
                            CARRIER,
                        ),
                        _ => (
                            SourceReferenceAnchorV29 {
                                argument: 0,
                                ty: CARRIER,
                            },
                            ELEMENT,
                        ),
                    };
                    source_reference_anchor_type_v29(plan, anchor, requested, budget).map(|_| ())
                }
                6 => {
                    // Ordinary aggregates still cannot bless a Slice value.
                    merge_execution_cfg_binding_v29(
                        plan.instances.owner().source_semantic().types(),
                        CARRIER,
                        &held,
                        &held,
                        &mut [].iter_mut(),
                        &mut 0,
                        budget,
                    )
                }
                7 => {
                    let values = [ValueDef::new(ValueId(17), Type::Scalar(ScalarType::U32))];
                    source_reference_rebuild_node_v29(
                        &references,
                        node,
                        true,
                        &mut [].iter(),
                        &mut values.iter(),
                        &mut 0,
                        budget,
                    )
                    .map(|_| ())
                }
                _ => unreachable!(),
            };
            let error = result.unwrap_err();
            assert!(matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution CFG transport differs from its captured SSA state",
                }
            ));
            assert_eq!(
                (budget.failed_work(), budget.failed_storage()),
                (None, None)
            );
            completed.set(true);
            Err(error)
        });
        // run_owner_with_storage checks source-scope cleanup against its floor.
        assert!(
            completed.get(),
            "hostile carrier {hostile} assertions completed"
        );
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution CFG transport differs from its captured SSA state",
                })
            ),
            "{hostile}: {result:?}"
        );
    }
}

#[test]
fn anchored_owner_carrier_query_preserves_exact_resource_and_owner_checks() {
    // Six owner checks, sixteen fixed checks, four nominal-type visits, and
    // two callables plus three declared fields (including the unit marker).
    let exact_work = 6 * 5 + 16 + 4 + 2 * 4 + 3 * 20 + 32;
    let exact_storage = header::<Option<Type>>() + std::mem::size_of::<Type>();
    for denial in [None, Some(false), Some(true)] {
        let completed = std::cell::Cell::new(false);
        let result = run(|plan, budget| {
            match denial {
                None => budget.reserve_storage(usize::MAX - budget.storage() - exact_storage)?,
                Some(false) => budget.charge_work(usize::MAX - budget.work() - exact_work + 1)?,
                Some(true) => {
                    budget.reserve_storage(usize::MAX - budget.storage() - exact_storage + 1)?
                }
            }
            let before = (budget.work(), budget.storage());
            let result = source_reference_anchor_type_v29(
                plan,
                SourceReferenceAnchorV29 {
                    argument: 0,
                    ty: CARRIER,
                },
                CARRIER,
                budget,
            );
            match denial {
                None => {
                    assert!(matches!(result?, Some(Type::Slice(_))));
                    assert_eq!(budget.work() - before.0, exact_work);
                    assert_eq!(budget.storage() - before.1, exact_storage);
                    completed.set(true);
                    Ok(())
                }
                Some(storage) => {
                    let error = result.unwrap_err();
                    let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(resource) =
                        error
                    else {
                        panic!("one-short carrier query must preserve its resource failure");
                    };
                    assert!(if storage {
                        matches!(resource, ArgumentResourceV1::Storage(_))
                    } else {
                        matches!(resource, ArgumentResourceV1::Work(_))
                    });
                    assert!(matches!(plan.failure.first_error(), Some(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first),
                    ) if first == resource));
                    completed.set(true);
                    Err(resource.into())
                }
            }
        });
        assert!(completed.get(), "carrier resource checks completed");
        assert_eq!(result.is_ok(), denial.is_none(), "{result:?}");
    }
    let completed = std::cell::Cell::new(false);
    let result = run(|plan, _| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let error = source_reference_anchor_type_v29(
            plan,
            SourceReferenceAnchorV29 {
                argument: 0,
                ty: CARRIER,
            },
            CARRIER,
            &mut foreign,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting,
            )
        ));
        assert_eq!((foreign.work(), foreign.storage()), (0, 0));
        completed.set(true);
        Err(error)
    });
    assert!(completed.get(), "foreign owner rejection completed");
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting,
            )
        )
    ));
}
