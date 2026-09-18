// Producer tests deliberately stop short of independent caller/value replay.
fn entry_owner_source(
    prefix: Vec<SemanticStatementV1>,
    dead: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let root = source_function(
        100,
        None,
        vec![
            local(101, UNIT, SemanticLocalRoleV1::Return),
            local(102, ENV, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            source_block(
                103,
                vec![assign(
                    place(1, ENV),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Aggregate,
                            vec![scalar(10), scalar(20)],
                        )
                        .unwrap(),
                    ),
                )],
                source_call(1, 1, ENV, 1),
            ),
            source_block(104, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let mut statements = prefix;
    statements.push(assign(
        place(2, MUT),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: place(1, ENV),
        },
    ));
    statements.push(assign(
        place(4, MUT),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ENV).unwrap(),
                ],
                ENV,
            )
            .unwrap(),
        },
    ));
    let mut suffix = vec![];
    if dead {
        suffix.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
        ));
    }
    // Reborrow the existing view, not the owner: this must also check liveness.
    suffix.push(assign(
        place(3, SHARED),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ENV).unwrap(),
                ],
                ENV,
            )
            .unwrap(),
        },
    ));
    let owned = source_function(
        110,
        Some(ENV),
        vec![
            local(111, UNIT, SemanticLocalRoleV1::Return),
            local(112, ENV, SemanticLocalRoleV1::Argument(0)),
            local(113, MUT, SemanticLocalRoleV1::Temporary),
            local(114, SHARED, SemanticLocalRoleV1::Temporary),
            local(118, MUT, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            source_block(115, statements, source_call(2, 4, MUT, 1)),
            source_block(116, suffix, source_call(3, 3, SHARED, 2)),
            source_block(117, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let mut functions = vec![root, owned];
    for (tag, ty) in [(120, MUT), (130, SHARED)] {
        let mut statements = vec![assign(
            place(2, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1, true, 0))),
        )];
        if ty == MUT {
            statements.push(assign(
                field(1, true, 1),
                SemanticRvalueKindV1::Use(scalar(7)),
            ));
        }
        functions.push(source_function(
            tag,
            Some(ty),
            vec![
                local(tag + 1, UNIT, SemanticLocalRoleV1::Return),
                local(tag + 2, ty, SemanticLocalRoleV1::Argument(0)),
                local(tag + 3, U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![source_block(
                tag + 4,
                statements,
                SemanticTerminatorKindV1::Return,
            )],
        ));
    }
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        source_types(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn lower_entry_owner(
    source: &ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(Module, SemanticKirCorrespondenceV1), ProductionSemanticKirErrorV1>,
    usize,
    usize,
) {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let mut origin_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut origin_budget = ArgumentBudgetV1::new(&mut origin_work, usize::MAX);
    let mut origins = AssertOriginEmissionV1::new(&mut origin_budget);
    let mut admission = HelperLoweringAdmissionV1::PendingSource {
        requires_source: false,
        requires_borrowed: false,
    };
    budget.reserve_storage(23).unwrap();
    let result = lower_module_with_call_budget_for_helper_admission_v1(
        source,
        ProductionSemanticKirLimitsV1::default(),
        None,
        Some(&mut origins),
        &mut budget,
        &mut admission,
    );
    if result.is_err() {
        assert_eq!(budget.storage(), 23);
    } else {
        assert!(matches!(
            admission,
            HelperLoweringAdmissionV1::PendingSource {
                requires_borrowed: true,
                ..
            }
        ));
    }
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn placed_owned_entry_keeps_source_anchors_and_relocates_borrowed_operations() {
    let source = entry_owner_source(vec![], false);
    let semantic = source.source_semantic();
    let root = semantic.roots()[0];
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let mut closure = ReachableClosureBudgetV1::new(1_000_000);
    let plans = (1..4)
        .map(|index| {
            let id = SemanticFunctionIdV1::from_index(index);
            direct_scalar_helper_plan_v1(
                semantic, root, id,
                helper_function_id_v1(id, &semantic.functions()[index as usize]),
                1_000_000, &mut closure, &mut budget,
            ).unwrap()
        })
        .collect::<Vec<_>>();
    let ids = plans.iter().map(|plan| (plan.semantic_function, plan.kernel_ir_function.clone())).collect();
    let signatures = plans.iter().map(|plan| {
        let abi = semantic.functions()[plan.semantic_function.index() as usize].abi();
        (plan.semantic_function, LoweredFunctionSignatureV1 {
            parameter_semantic_types: abi.source_input_types().to_vec(),
            call_arguments: plan.call_arguments.clone(),
            parameter_types: plan.parameter_types.clone(),
            result_types: plan.result_types.clone(),
            result_semantic_type: abi.source_output_type(),
        })
    }).collect();
    let mut private = PrivateArrayLazyBudgetV1::new(1, 10_000);
    let placed = lower_one_semantic_function_v1(
        semantic, &plans[0], source.plan_for_function(plans[0].semantic_function).unwrap(),
        &ids, &signatures, Some([64, 1, 1]), BTreeSet::new(), 1, false, 10_000,
        None, &mut private, None, &mut budget,
        SemanticEmissionPlacementV1 { first_block: 17, first_value: 100 },
    ).unwrap();
    let function = &placed.function;
    let body = function.body.as_ref().unwrap();
    let operation = |locator: &BorrowedAggregateOperationLocatorV1| {
        assert_eq!(locator.function, function.id);
        &body.blocks.iter().find(|block| block.id == locator.location.block).unwrap()
            .operations[locator.location.operation_index]
    };
    assert_eq!(placed.borrowed_aggregate_fields.len(), 2);
    for (index, field) in placed.borrowed_aggregate_fields.iter().enumerate() {
        assert_eq!(field.owner.function, plans[0].semantic_function);
        assert_eq!(field.owner.local, SemanticLocalIdV1::from_index(1));
        assert_eq!(field.source_definition, BorrowedAggregateSourceAnchorV1::Entry { local: field.owner.local });
        assert_eq!(field.owner.lifetime_start, field.source_definition);
        let BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer, allocation, initialization } = &field.carrier else {
            panic!("entry parameter must have a scalar cell");
        };
        assert_eq!(pointer.function, function.id);
        assert_eq!(allocation.location.block, BlockId(17));
        assert_eq!(initialization.location.block, BlockId(17));
        let alloca = operation(allocation);
        assert!(matches!(alloca.kind, OperationKind::Alloca { .. }));
        assert_eq!(alloca.results[0].id, pointer.value);
        assert!(matches!(operation(initialization).kind, OperationKind::Store { pointer: actual, value, .. }
            if actual == pointer.value && value == body.parameters[index]));
    }
    assert_eq!(placed.borrowed_aggregate_calls.len(), 4);
    for call in &placed.borrowed_aggregate_calls {
        assert_eq!(call.root, root);
        assert_eq!(call.caller, plans[0].semantic_function);
        assert!(call.block.index() < 2);
        assert_eq!(call.call.location.block, BlockId(17 + call.block.index()));
        let OperationKind::Call { arguments, .. } = &operation(&call.call).kind else {
            panic!("borrowed call locator must point to the emitted call");
        };
        assert_eq!(arguments[call.physical_argument as usize], call.actual.value);
    }
}

#[test]
fn owned_entry_storage_uses_exact_parameters_once_and_keeps_replay_closed() {
    let source = entry_owner_source(vec![], false);
    let (module, rows) = lower_entry_owner(&source, usize::MAX, usize::MAX)
        .0
        .unwrap();
    verify_module(&module).unwrap();
    assert_eq!(rows.borrowed_aggregate_fields.len(), 2);
    let BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer, .. } =
        &rows.borrowed_aggregate_fields[0].carrier
    else {
        panic!("scalar cell required")
    };
    let function = module.function(&pointer.function).unwrap();
    let body = function.body.as_ref().unwrap();
    assert_eq!(
        body.blocks
            .iter()
            .flat_map(|b| &b.operations)
            .filter(|op| matches!(op.kind, OperationKind::Alloca { .. }))
            .count(),
        2
    );
    let mut cells = BTreeSet::new();
    for (index, row) in rows.borrowed_aggregate_fields.iter().enumerate() {
        assert_eq!(row.owner.function, SemanticFunctionIdV1::from_index(1));
        assert_eq!(row.owner.local, SemanticLocalIdV1::from_index(1));
        assert_eq!(
            row.source_definition,
            BorrowedAggregateSourceAnchorV1::Entry {
                local: row.owner.local
            }
        );
        assert_eq!(row.owner.lifetime_start, row.source_definition);
        let BorrowedAggregateCarrierCandidateV1::ScalarCell {
            pointer,
            initialization,
            allocation,
        } = &row.carrier
        else {
            panic!("scalar cell required")
        };
        assert!(
            cells.insert(pointer.value),
            "fields must not share a scalar cell"
        );
        assert_eq!(allocation.location.block, BlockId(0));
        let store = &body.blocks[0].operations[initialization.location.operation_index];
        assert!(
            matches!(store.kind, OperationKind::Store { pointer: actual, value, .. }
            if actual == pointer.value && value == body.parameters[index])
        );
    }
    assert_eq!(rows.borrowed_aggregate_calls.len(), 4);
    for call in &rows.borrowed_aggregate_calls {
        let field = rows
            .borrowed_aggregate_fields
            .iter()
            .find(|field| field.owner == call.actual_owner && field.path == call.actual_path)
            .unwrap();
        let BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer, .. } = &field.carrier else {
            unreachable!()
        };
        if call.callee == SemanticFunctionIdV1::from_index(2) {
            assert_eq!(call.actual, *pointer);
        } else {
            assert_eq!(call.callee, SemanticFunctionIdV1::from_index(3));
            assert_eq!(call.actual.function, pointer.function);
            assert!(body.blocks.iter().flat_map(|block| &block.operations).any(|operation|
                operation.results.first().is_some_and(|value| value.id == call.actual.value)
                    && matches!(operation.kind, OperationKind::Cast { kind: CastKind::RestrictPointerAccess, value, .. }
                        if value == pointer.value)));
        }
    }
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        source.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "borrowed_replay_root",
            [90; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let error = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        source,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .err()
    .expect("entry storage alone must not admit caller/value replay");
    assert!(
        matches!(
            error,
            ProductionPreRankedKirErrorV1::Lowering(ProductionSemanticKirErrorV1::Unsupported {
                detail: "borrowed aggregate is outside the checked straight-line source subset",
                ..
            })
        ),
        "{error:?}"
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn owned_entry_view_cannot_outlive_the_parameter() {
    let source = entry_owner_source(vec![], true);
    let error = lower_entry_owner(&source, usize::MAX, usize::MAX)
        .0
        .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "borrowed aggregate view outlives its source owner",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn owned_entry_storage_refuses_prior_assignment_and_move() {
    for prefix in [
        assign(
            place(1, ENV),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Aggregate,
                    vec![scalar(30), scalar(40)],
                )
                .unwrap(),
            ),
        ),
        assign(
            place(1, ENV),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, ENV))),
        ),
    ] {
        let source = entry_owner_source(vec![prefix], false);
        let error = lower_entry_owner(&source, usize::MAX, usize::MAX)
            .0
            .unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "borrowed entry owner must first be borrowed in the helper entry block",
                    ..
                }
            ),
            "{error:?}"
        );
    }
}

#[test]
fn owned_entry_storage_respects_exact_work_and_failed_storage_cleanup() {
    let source = entry_owner_source(vec![], false);
    let (output, work, storage) = lower_entry_owner(&source, usize::MAX, usize::MAX);
    output.unwrap();
    lower_entry_owner(&source, work, storage).0.unwrap();
    assert!(matches!(
        lower_entry_owner(&source, work - 1, usize::MAX).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        lower_entry_owner(&source, work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}
