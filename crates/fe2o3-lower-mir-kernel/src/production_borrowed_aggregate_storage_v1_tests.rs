#[test]
fn field_paths_use_the_shared_work_and_storage_ledger() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1024);
    let path = BorrowedAggregateFieldPathV1::new_in(&[0, 2], &mut budget).unwrap();
    assert_eq!(&*path.fields, &[0, 2]);
    assert!(budget.storage() >= 2 * std::mem::size_of::<u32>());
    assert!(BorrowedAggregateFieldPathV1::new_in(&[], &mut budget).is_err());
}

#[test]
fn exhausted_shared_work_rejects_before_path_allocation() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1024);
    assert!(BorrowedAggregateFieldPathV1::new_in(&[0], &mut budget).is_err());
    assert_eq!(budget.storage(), 0);
}

#[test]
fn failed_shape_build_restores_incoming_storage_floor() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1024);
    budget.reserve_storage(7).unwrap();
    let result: Result<(), _> = borrowed_aggregate_build_v1(&mut budget, |budget| {
        budget.reserve_storage(16)?;
        Err(borrowed_aggregate_error_v1("unsupported test shape"))
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), 7);
}

#[test]
fn borrowed_binding_cannot_be_flattened_into_by_value_state() {
    let binding = SemanticValueBindingV1::BorrowedAggregate { view: 0 };
    assert!(binding.value().is_err());
    assert!(binding.values().is_err());
}

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const UNIQUE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const SHARED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const MARKER_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn selection_types() -> Vec<SemanticTypeDeclV1> {
    let declaration = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            layout,
            shape,
        )
    };
    let aggregate = |size, alignment, offsets| {
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(size),
            alignment,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap()
    };
    let reference = |tag, pointee, mutability| {
        declaration(
            tag,
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
                    pointee,
                    SemanticPointerKindV1::Reference,
                    mutability,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
    };
    vec![
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
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
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
            aggregate(4, 4, vec![0]),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U32]).unwrap()),
        ),
        reference(4, ENV, SemanticMutabilityV1::Mutable),
        reference(5, ENV, SemanticMutabilityV1::Immutable),
        declaration(
            6,
            aggregate(0, 1, vec![]),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ),
        reference(7, MARKER, SemanticMutabilityV1::Immutable),
    ]
}

fn selection_place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn selection_dereference(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ENV).unwrap()],
        ENV,
    )
    .unwrap()
}

fn selection_assign(
    local: u32,
    ty: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            selection_place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}

fn selection_block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}

fn selection_function(
    argument: Option<SemanticTypeIdV1>,
    local_types: &[SemanticTypeIdV1],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if argument.is_some() {
            SemanticCanonAbiV1::Rust
        } else {
            SemanticCanonAbiV1::GpuKernel
        },
        if argument.is_some() {
            SemanticExternAbiV1::Rust
        } else {
            SemanticExternAbiV1::GpuKernel
        },
        false,
        false,
        u32::from(argument.is_some()),
        argument
            .into_iter()
            .map(|ty| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty,
                    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(
        argument
            .into_iter()
            .map(|ty| {
                if ty == UNIQUE {
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                }
            })
            .collect(),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([21; 32]),
        if argument.is_some() {
            SemanticFunctionRoleV1::InternalHelper
        } else {
            SemanticFunctionRoleV1::KernelRoot
        },
        SemanticItemDefinitionIdentityV1::from_sha256([22; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([23; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([24; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([25; 32]),
        source,
        abi,
        local_types
            .iter()
            .enumerate()
            .map(|(index, &ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([index as u8 + 40; 32]),
                    ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else if index == 1 && argument.is_some() {
                        SemanticLocalRoleV1::Argument(0)
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    if argument.is_some() {
        return function;
    }
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"selection_regression".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([90; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ))
}

fn selection_plan() -> LoweredFunctionPlanV1 {
    LoweredFunctionPlanV1 {
        borrowed_parameter_bindings: vec![],
        correspondence_owner: SemanticFunctionIdV1::from_index(0),
        semantic_function: SemanticFunctionIdV1::from_index(0),
        kernel_ir_function: FunctionId::new("selection_regression"),
        role: SemanticKirFunctionRoleV1::KernelEntry,
        parameter_declarations: vec![],
        parameter_types: vec![],
        parameter_values: vec![],
        call_arguments: vec![],
        parameter_local_bindings: vec![],
        parameter_component_bindings: vec![],
        ignored_parameter_bindings: vec![],
        result_types: vec![],
    }
}

fn selection_call() -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![SemanticOperandV1::Move(selection_place(4, SHARED))],
            Some(SemanticCallDestinationV1::new(
                selection_place(0, UNIT),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn selection_caller(suffix: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let mut blocks = vec![selection_block(
        60,
        vec![
            selection_assign(
                1,
                ENV,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Aggregate,
                        vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                            U32,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(7, 4).unwrap(),
                            ),
                        ))],
                    )
                    .unwrap(),
                ),
            ),
            selection_assign(
                2,
                UNIQUE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: selection_place(1, ENV),
                },
            ),
            selection_assign(
                3,
                UNIQUE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(selection_place(2, UNIQUE))),
            ),
            selection_assign(
                4,
                SHARED,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: selection_dereference(3),
                },
            ),
            selection_assign(
                5,
                MARKER,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, vec![])
                        .unwrap(),
                ),
            ),
            selection_assign(
                6,
                MARKER_REF,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: selection_place(5, MARKER),
                },
            ),
        ],
        selection_call(),
    )];
    blocks.extend(suffix);
    selection_function(
        None,
        &[
            UNIT, ENV, UNIQUE, UNIQUE, SHARED, MARKER, MARKER_REF, SHARED,
        ],
        blocks,
    )
}

fn selection_signature() -> BTreeMap<SemanticFunctionIdV1, LoweredFunctionSignatureV1> {
    BTreeMap::from([(
        SemanticFunctionIdV1::from_index(1),
        LoweredFunctionSignatureV1 {
            parameter_semantic_types: vec![SHARED],
            parameter_types: vec![Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadOnly,
            )],
            call_arguments: vec![HelperCallArgumentV1 {
                source_argument: 0,
                tuple_field: None,
                component: Some(0),
                borrowed: Some(BorrowedAggregateFormalV1 {
                    local: SemanticLocalIdV1::from_index(1),
                    path: BorrowedAggregateFieldPathV1 {
                        fields: Box::new([0]),
                    },
                    value: ValueId(1),
                }),
            }],
            result_types: vec![],
            result_semantic_type: UNIT,
        },
    )])
}

#[test]
fn selected_actual_follows_move_and_shared_reborrow_without_claiming_legacy_marker() {
    let types = selection_types();
    let function = selection_caller(vec![selection_block(
        61,
        vec![selection_assign(
            7,
            SHARED,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: selection_dereference(3),
            },
        )],
        SemanticTerminatorKindV1::Return,
    )]);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    let preparation = prepare_borrowed_aggregates_v1(
        &types,
        &function,
        &[SemanticCallableDeclV1::Defined {
            function: SemanticFunctionIdV1::from_index(1),
        }],
        &selection_plan(),
        &selection_signature(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(preparation.locals, BTreeSet::from([1, 2, 3, 4, 7]));
    assert_eq!(
        preparation.owners.keys().copied().collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(preparation.owners[&1].aggregate_type, ENV);
}

#[test]
fn selected_formal_reborrows_do_not_create_local_owner_storage() {
    let types = selection_types();
    let function = selection_function(
        Some(SHARED),
        &[UNIT, SHARED, SHARED, MARKER_REF],
        vec![selection_block(
            62,
            vec![selection_assign(
                2,
                SHARED,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: selection_dereference(1),
                },
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    let mut plan = selection_plan();
    let shape = borrowed_aggregate_shape_v1(&types, SHARED, &mut budget).unwrap();
    plan.parameter_local_bindings
        .push(PlannedParameterLocalBindingV1::BorrowedAggregate {
            local: 1,
            shape,
            values: vec![ValueDef::new(
                ValueId(1),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadOnly,
                ),
            )],
        });
    let preparation = prepare_borrowed_aggregates_v1(
        &types,
        &function,
        &[],
        &plan,
        &BTreeMap::new(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(preparation.locals, BTreeSet::from([1, 2]));
    assert!(preparation.owners.is_empty());
}

#[test]
fn selected_alias_after_branch_is_still_rejected() {
    let types = selection_types();
    let function = selection_caller(vec![
        selection_block(
            63,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    U32,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
                )),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(2),
                        ),
                    )],
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(3),
                    ),
                )
                .unwrap(),
            },
        ),
        selection_block(
            64,
            vec![selection_assign(
                7,
                SHARED,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: selection_dereference(3),
                },
            )],
            SemanticTerminatorKindV1::Return,
        ),
        selection_block(65, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    budget.reserve_storage(7).unwrap();
    let error = prepare_borrowed_aggregates_v1(
        &types,
        &function,
        &[SemanticCallableDeclV1::Defined {
            function: SemanticFunctionIdV1::from_index(1),
        }],
        &selection_plan(),
        &selection_signature(),
        &mut budget,
    )
    .err()
    .unwrap();
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::Unsupported {
            detail: "borrowed aggregate state is used beyond the straight-line affected region",
            ..
        }
    ));
    assert_eq!(budget.storage(), 7);
}

#[test]
fn ordinary_loop_with_unselected_aggregate_reference_uses_production_lowering() {
    let all_types = selection_types();
    let reference = &all_types[MARKER_REF.index() as usize];
    let types = vec![
        all_types[UNIT.index() as usize].clone(),
        all_types[MARKER.index() as usize].clone(),
        SemanticTypeDeclV1::new(
            reference.identity(),
            reference.layout_identity(),
            reference.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(1),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ];
    let goto = || {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(1),
        ))
    };
    let function = selection_function(
        None,
        &[UNIT, SemanticTypeIdV1::from_index(2)],
        vec![
            selection_block(70, vec![], goto()),
            selection_block(71, vec![], goto()),
        ],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::Defined {
            function: SemanticFunctionIdV1::from_index(0),
        }],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let owner =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let owner =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    owner.verify_replay().unwrap();
    let roster = crate::ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "selection_regression",
            [90; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let roots = materialization_launch_roots_v1(&owner, &roster).unwrap();
    let (module, correspondence) = lower_module(
        &owner,
        ProductionSemanticKirLimitsV1::default(),
        Some(&roots),
    )
    .unwrap();
    verify_module(&module).unwrap();
    assert!(correspondence.borrowed_parameter_bindings.is_empty());
    assert!(correspondence.borrowed_aggregate_fields.is_empty());
    let body = module
        .function(&FunctionId::new("selection_regression"))
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    assert_eq!(body.blocks.len(), 2);
    assert!(body.blocks.iter().all(|block| block.operations.is_empty()));
    assert!(
        matches!(&body.blocks[1].terminator, Some(Terminator::Branch { target: BlockId(1), arguments }) if arguments.is_empty())
    );
}

#[test]
fn selection_closure_exhaustion_restores_the_shared_storage_floor() {
    let function = selection_caller(vec![selection_block(
        72,
        vec![],
        SemanticTerminatorKindV1::Return,
    )]);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(120);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    budget.reserve_storage(11).unwrap();
    assert!(
        prepare_borrowed_aggregates_v1(
            &selection_types(),
            &function,
            &[SemanticCallableDeclV1::Defined {
                function: SemanticFunctionIdV1::from_index(1)
            }],
            &selection_plan(),
            &selection_signature(),
            &mut budget,
        )
        .is_err()
    );
    assert_eq!(budget.storage(), 11);
}
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;
