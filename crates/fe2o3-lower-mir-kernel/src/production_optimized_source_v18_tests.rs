use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as OptimizedDefinition,
    CanonicalKirUseCoordinateV1 as OptimizedUse,
};

const OPTIMIZED_SOURCE_WORK_LIMIT_V18: usize = 500_000_000;
const OPTIMIZED_SOURCE_PRECHARGE_BOUND_V18: usize = 454_463_680;

include!("production_optimized_source_admission_oracle_v18_tests.rs");

fn with_actual_optimized_source_v18(
    prepared: ProductionPreparedSourceV18,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    with_actual_optimized_transition_v18(prepared, budget, |source, checked, budget| {
        source.with_ranked_correspondence_v18(checked.input(), budget, |original, budget| {
            let floor = budget.storage();
            let expected = std::cell::Cell::new(0);
            let consume = |view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
                           budget: &mut ArgumentBudgetV1<'_>| {
                assert_eq!(
                    budget.storage() - floor,
                    expected.get(),
                    "independent owner, control, attachment and site coexistence oracle"
                );
                consume(view, budget)
            };
            expected.set(optimized_source_retained_oracle_v18(
                original, checked, &consume,
            ));
            original.with_optimized_correspondence_v18(checked, budget, consume)
        })
    })
}

fn with_actual_optimized_transition_v18(
    prepared: ProductionPreparedSourceV18,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'source, 'inventory, 'input, 'output, 'rows, 'work> FnOnce(
        &ProductionSourceOwnedViewV18<'source>,
        &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'inventory, 'input, 'output, 'rows>,
        &mut ArgumentBudgetV1<'work>,
    )
        -> SourceOwnedResultV18<
        (),
    >,
) -> SourceOwnedResultV18<()> {
    prepared.with_source_consumer_v18(budget, |source, budget| {
        let floor = budget.storage();
        let (prepaid, nodes) =
            optimized_source_fixture_precharge_v18(&source.owner.inner.pending.graph);
        let observation_start = budget.work();
        let observed = fe2o3_pliron::optimize_neutral_kernel_ir_v18(
            &source.owner.inner.pending.graph,
            ProductionSemanticKirLimitsV1::default().storage_layout_limits,
            budget,
        )
        .expect("actual Stage A observation");
        let execution = observed.execution().canonical_bytes();
        // The fixed V18 frame stores its registered-node census after the
        // control word, both module identities and the storage-table identity.
        assert_eq!(
            u64::from_le_bytes(execution[128..136].try_into().unwrap()),
            nodes as u64
        );
        let prepaid = prepaid
            + optimized_source_fixture_bridge_work_v18(
                source.owner.inner.pending.graph.canonical_bytes().len(),
                observed.owner().module(),
                true,
            );
        assert!(prepaid <= OPTIMIZED_SOURCE_PRECHARGE_BOUND_V18);
        assert!(budget.work() >= prepaid);
        assert!(
            budget.work() - prepaid
                <= OPTIMIZED_SOURCE_WORK_LIMIT_V18 - OPTIMIZED_SOURCE_PRECHARGE_BOUND_V18,
            "work envelope: before={observation_start}, after={}, prepaid={prepaid}, bytes={}",
            budget.work(),
            source.owner.inner.pending.graph.canonical_bytes().len()
        );
        budget.reserve_storage(observed.storage().retained_storage())?;
        let result = observed.try_check_and_finish_with_v18(budget, |checked, budget| {
            consume(source, checked, budget).map(|()| ((), 0))
        });
        match result {
            Ok((owner, (), receipt)) => {
                assert_eq!(
                    receipt.retained_storage(),
                    std::mem::size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>()
                );
                drop(owner);
                assert_eq!(budget.storage(), floor);
                Ok(())
            }
            Err(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(error)) => Err(error),
            Err(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error)) => {
                Err(error.into())
            }
            Err(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::OriginAccounting) => {
                Err(ArgumentResourceV1::Accounting.into())
            }
            Err(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Panicked) => Err(
                ProductionSourceOwnedViewErrorV18::Binding("actual optimizer consumer panicked"),
            ),
            Err(error) => panic!("actual Stage A adoption: {error:?}"),
        }
    })
}

fn run_optimized_source_v18(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
    with_actual_optimized_source_v18(prepared, &mut budget, consume).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn prepared_optimized_module_v18(
    kind: ModuleFixture,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ProductionPreparedSourceV18 {
    let projection = module_fixture_owner(kind);
    let owner = module_fixture_owner(kind);
    let (_, launch) = with_module_fixture_view(&owner, kind, budget, |_, _| ()).unwrap();
    with_module_fixture_view(&projection, kind, budget, |source, budget| {
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner,
            launch,
            source.input,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )
        .unwrap()
    })
    .unwrap()
    .0
}

fn folding_source_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    arithmetic_source_owner_v18(SemanticBinaryOpV1::Add, 7, 9)
}

fn division_source_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    arithmetic_source_owner_v18(SemanticBinaryOpV1::Divide, 32, 2)
}

fn arithmetic_source_owner_v18(
    operation: SemanticBinaryOpV1,
    left: u128,
    right: u128,
) -> ProductionSemanticSsaOwnerV1 {
    let base = scalar_payload_owner_v18();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut statements = helper.blocks()[0].statements().to_vec();
    statements[0] = assign(
        place(1, U32),
        SemanticRvalueKindV1::Binary {
            operation,
            left: literal(left),
            right: literal(right),
        },
    );
    functions[2] = function(
        210,
        helper.role(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        vec![block(214, statements, SemanticTerminatorKindV1::Return)],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn bitwise_cse_source_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = scalar_payload_owner_v18();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut locals = helper.locals().to_vec();
    locals.push(local(216, U32, SemanticLocalRoleV1::Temporary));
    let compute = || {
        assign(
            place(3, U32),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitAnd,
                left: SemanticOperandV1::Copy(place(1, U32)),
                right: literal(7),
            },
        )
    };
    let store = || {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(2, U32),
                SemanticOperandV1::Copy(place(3, U32)),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        )
    };
    functions[2] = function(
        210,
        helper.role(),
        helper.abi().clone(),
        locals,
        vec![block(
            214,
            vec![compute(), store(), compute(), store()],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn actual_bitwise_cse_has_explicit_rewrite_dispositions_and_exact_store_uses() {
    run_optimized_source_v18(bitwise_cse_source_owner_v18, |view, budget| {
        let mut rewritten = 0;
        let input = view.input_inventory(budget)?;
        for row in input.operations() {
            if matches!(
                row.operation.kind,
                OperationKind::Binary {
                    op: fe2o3_kernel_ir::BinaryOp::BitAnd,
                    ..
                }
            ) {
                match view.operation(row.coordinate, budget)? {
                    ProductionOptimizedSourceOperationV18::Rewritten { input: coordinate } => {
                        assert_eq!(coordinate, row.coordinate);
                        rewritten += 1;
                    }
                    ProductionOptimizedSourceOperationV18::Retained { .. } => {}
                    ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                        panic!("live CSE is not dead control")
                    }
                }
                assert!(
                    !view
                        .definition_descendants(
                            OptimizedDefinition::Result {
                                operation: row.coordinate,
                                result: 0,
                            },
                            budget
                        )?
                        .is_empty()
                );
            }
            for usage in &input.uses()[row.operands.clone()] {
                if matches!(row.operation.kind, OperationKind::Store { .. }) {
                    assert!(view.operand(usage.coordinate, budget)?.is_some());
                }
            }
        }
        assert!(rewritten > 0, "the actual fixed executor must perform CSE");
        Ok(())
    });
}

#[test]
fn actual_v18_successor_keeps_original_owner_and_distinct_inventories() {
    run_optimized_source_v18(folding_source_owner_v18, |view, budget| {
        let input = view.input_inventory(budget)?;
        let output = view.output_inventory(budget)?;
        let source = view.original_source(budget)?;
        assert!(input.belongs_to(&source.owner.inner.pending.graph));
        assert!(!output.belongs_to(&source.owner.inner.pending.graph));
        assert!(!std::ptr::eq(input, output));
        assert_ne!(
            input.owner().canonical_bytes(),
            output.owner().canonical_bytes()
        );
        assert_eq!(
            input.owner().module().storage_layouts,
            output.owner().module().storage_layouts
        );
        Ok(())
    });
}

#[test]
fn actual_checked_add_fold_preserves_result_ordinals_after_pure_operation_removal() {
    run_optimized_source_v18(folding_source_owner_v18, |view, budget| {
        let input = view.input_inventory(budget)?;
        let output = view.output_inventory(budget)?;
        let mut found = 0;
        for operation in input.operations() {
            if matches!(
                operation.operation.kind,
                OperationKind::Binary {
                    op: fe2o3_kernel_ir::BinaryOp::Checked(
                        fe2o3_kernel_ir::CheckedBinaryOperator::Add
                    ),
                    ..
                }
            ) {
                assert_eq!(operation.operation.results.len(), 2);
                assert!(matches!(view.operation(operation.coordinate, budget)?,
                    ProductionOptimizedSourceOperationV18::Rewritten { input } if input == operation.coordinate));
                let descendants = view.definition_descendants(
                    OptimizedDefinition::Result {
                        operation: operation.coordinate,
                        result: 0,
                    },
                    budget,
                )?;
                assert!(!descendants.is_empty(), "the stored result remains live");
                for descendant in descendants {
                    let OptimizedDefinition::Result {
                        operation: coordinate,
                        result: 0,
                    } = descendant.output
                    else {
                        panic!("checked value result must become a constant result")
                    };
                    let actual = output
                        .operations()
                        .iter()
                        .find(|row| row.coordinate == coordinate)
                        .unwrap();
                    assert!(matches!(
                        actual.operation.kind,
                        OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(16))
                    ));
                }
                for descendant in view.definition_descendants(
                    OptimizedDefinition::Result {
                        operation: operation.coordinate,
                        result: 1,
                    },
                    budget,
                )? {
                    let OptimizedDefinition::Result {
                        operation: coordinate,
                        result: 0,
                    } = descendant.output
                    else {
                        panic!("checked overflow result must become a boolean constant result")
                    };
                    let actual = output
                        .operations()
                        .iter()
                        .find(|row| row.coordinate == coordinate)
                        .unwrap();
                    assert!(matches!(
                        actual.operation.kind,
                        OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(false))
                    ));
                }
                found += 1;
            }
        }
        assert!(found > 0);
        Ok(())
    });
}

#[test]
fn actual_divide_fold_keeps_all_descendants_including_retained_operation() {
    run_optimized_source_v18(division_source_owner_v18, |view, budget| {
        let input = view.input_inventory(budget)?;
        let output = view.output_inventory(budget)?;
        let mut found = 0;
        for row in input.operations() {
            if !matches!(
                row.operation.kind,
                OperationKind::Binary {
                    op: fe2o3_kernel_ir::BinaryOp::Divide,
                    ..
                }
            ) {
                continue;
            }
            let descendants = view.definition_descendants(
                OptimizedDefinition::Result {
                    operation: row.coordinate,
                    result: 0,
                },
                budget,
            )?;
            let mut retained = 0;
            let mut constants = 0;
            for descendant in descendants {
                let OptimizedDefinition::Result {
                    operation,
                    result: 0,
                } = descendant.output
                else {
                    panic!("unexpected divide descendant")
                };
                let actual = output
                    .operations()
                    .iter()
                    .find(|row| row.coordinate == operation)
                    .unwrap();
                match actual.operation.kind {
                    OperationKind::Binary {
                        op: fe2o3_kernel_ir::BinaryOp::Divide,
                        ..
                    } => retained += 1,
                    OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(16)) => constants += 1,
                    _ => panic!("divide descendant changed value"),
                }
            }
            assert_eq!((retained, constants), (1, 1));
            found += 1;
        }
        assert!(found > 0);
        Ok(())
    });
}
