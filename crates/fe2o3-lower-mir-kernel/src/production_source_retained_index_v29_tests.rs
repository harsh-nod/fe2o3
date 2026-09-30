#[derive(Clone, Copy, Debug)]
enum RetainedIndexCaseV29 {
    Constant,
    Mask,
    Guard,
    GuardLoop,
    StaleGuardLoop,
    ChangedAfterGuard,
    LifetimeRestart,
}

fn retained_index_owner_v29(case: RetainedIndexCaseV29) -> ProductionSemanticSsaOwnerV1 {
    let guarded = matches!(
        case,
        RetainedIndexCaseV29::Guard
            | RetainedIndexCaseV29::GuardLoop
            | RetainedIndexCaseV29::StaleGuardLoop
            | RetainedIndexCaseV29::ChangedAfterGuard
    );
    let base = runtime_selector_module_owner(guarded);
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    for index in [2, 3] {
        let old = &functions[index];
        let mut blocks = old.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        let SemanticStatementKindV1::Assign(first) = statements[0].kind() else {
            panic!("original index assignment");
        };
        let destination = first.destination().clone();
        let SemanticStatementKindV1::Assign(array) = statements[1].kind() else {
            panic!("original array construction");
        };
        let element = SemanticPlaceV1::new(
            array.destination().local(),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 4,
                        from_end: false,
                    },
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap();
        if matches!(case, RetainedIndexCaseV29::Constant) {
            statements[0] = assign(destination.clone(), SemanticRvalueKindV1::Use(literal(0)));
        }
        // An actual source Store makes this original local retained. It is not
        // an SSA annotation or a synthetic definition manufactured by a test.
        statements.insert(
            1,
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    destination.clone(),
                    SemanticOperandV1::Copy(destination.clone()),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            ),
        );
        // This explicit source memory write requires the ordinary array's
        // original backing even in the helper that only reads after its guard.
        statements.insert(
            3,
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    element,
                    literal(11),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            ),
        );
        if matches!(case, RetainedIndexCaseV29::LifetimeRestart) {
            statements.insert(
                2,
                SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageDead(destination.local()),
                ),
            );
            statements.insert(
                3,
                SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageLive(destination.local()),
                ),
            );
        }
        blocks[0] = block(
            if index == 2 { 90 } else { 115 },
            statements,
            blocks[0].terminator().kind().clone(),
        );
        if guarded && index == 3 && matches!(case, RetainedIndexCaseV29::ChangedAfterGuard) {
            let mut after = blocks[1].statements().to_vec();
            after.insert(
                0,
                SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        destination.clone(),
                        literal(9),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
            );
            blocks[1] = block(221, after, blocks[1].terminator().kind().clone());
        }
        if index == 3
            && matches!(
                case,
                RetainedIndexCaseV29::GuardLoop | RetainedIndexCaseV29::StaleGuardLoop
            )
        {
            let mut initial = blocks[0].statements().to_vec();
            let comparison = initial.pop().expect("original final comparison");
            let mut assertion = blocks[0].terminator().kind().clone();
            let SemanticTerminatorKindV1::Assert { target, .. } = &mut assertion else {
                panic!("original guard");
            };
            *target = SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::AssertSuccess,
                SemanticBlockIdV1::from_index(2),
            );
            let edge = |role, target| {
                SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
            };
            let choose = SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(2, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 4),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                )
                .unwrap(),
            };
            let latch = SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    destination.clone(),
                    SemanticOperandV1::Copy(place(2, U32)),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            );
            blocks = vec![
                block(
                    115,
                    initial,
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(222, vec![comparison], assertion),
                block(223, blocks[1].statements().to_vec(), choose),
                block(
                    224,
                    vec![latch],
                    SemanticTerminatorKindV1::Goto(edge(
                        SemanticEdgeRoleV1::Goto,
                        if matches!(case, RetainedIndexCaseV29::GuardLoop) {
                            1
                        } else {
                            2
                        },
                    )),
                ),
                block(225, vec![], SemanticTerminatorKindV1::Return),
            ];
        }
        functions[index] = function(
            if index == 2 { 100 } else { 110 },
            old.role(),
            old.abi().clone(),
            old.locals().to_vec(),
            blocks,
        );
    }
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

fn retained_index_prepared_v29(
    case: RetainedIndexCaseV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionPreparedSourceV18> {
    let projection = retained_index_owner_v29(case);
    let owner = retained_index_owner_v29(case);
    let (_, launch) = with_module_fixture_view(&owner, ModuleFixture::Array, budget, |_, _| ())?;
    with_module_fixture_view(
        &projection,
        ModuleFixture::Array,
        budget,
        |source, budget| {
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner,
                launch,
                source.input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
        },
    )?
    .0
}

fn retained_index_run_v29(
    case: RetainedIndexCaseV29,
    memory: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let completed = std::cell::Cell::new(false);
    let result = (|| {
        let prepared = retained_index_prepared_v29(case, &mut budget)?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_sparse_and_memory_ssa_v1(|_, versions, budget| {
                let inventory = versions.inventory();
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    // Root zero is deliberately ordinary; all lifecycle array
                    // work belongs to the separately declared second root.
                    scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 1,
                        memory.then_some(versions), budget, |physical, budget| -> SourceOwnedResultV18<()> {
                            let root = source.root_row(1)?;
                            let mut indices = 0;
                            let body = inventory.functions()[root.function_ordinal].function.body.as_ref().unwrap();
                            assert!(body.blocks.iter().flat_map(|block| &block.operations)
                                .any(|operation| matches!(operation.kind, OperationKind::Execution(_))),
                                "final memory analysis must consume the actual lifecycle graph");
                            for (block, body) in body.blocks.iter().enumerate() {
                              for (ordinal, actual) in body.operations.iter().enumerate() {
                                let OperationKind::Load { pointer, .. } = actual.kind else { continue; };
                                let operation = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(root.function_ordinal as u32),
                                        block: block as u32,
                                    }, operation: ordinal as u32,
                                };
                                let Some(original) = relation.retained_memory_access(1, operation, pointer, budget)? else { continue; };
                                let sidecar = source.sidecar(1, original.instance, budget)?;
                                let row = sidecar.scoped_memory_anchors.as_ref().unwrap().rows[original.row];
                                let ScopedMemoryAnchorKindV29::Access {
                                    payload: Some(ScopedMemoryPayloadV29::IndexLoad { result, read }), ..
                                } = row.kind else { continue; };
                                assert_eq!(actual.results[0].id, result);
                                let (function, _) = source.instance(1, original.instance, budget)?;
                                let semantic = source.source_ssa(budget)?;
                                let source_occurrences = semantic.occurrences_v1().unwrap();
                                let occurrences = source_occurrences.function(function).unwrap();
                                assert_eq!(occurrences.events()[read.event].role(), ExecutionEventV29::ProjectionIndexUse(read.projection));
                                assert!(!occurrences.events()[read.event].is_promoted());
                                assert!(occurrences.events()[read.event].resolved().is_none());
                                let access = physical.access(original.instance, original.row, operation, pointer, budget)?.unwrap();
                                assert_eq!(access.operation_pointer(budget)?, (operation, pointer));
                                let mut alternatives = 0;
                                access.visit_alternatives(budget, |instance, local, slot, _, _| {
                                    let object = &root.source_slots.slots[slot];
                                    assert_eq!(object.instance.index(), instance);
                                    assert_eq!(object.legacy_local().unwrap(), local.index());
                                    alternatives += 1;
                                    Ok(())
                                })?;
                                assert!(alternatives > 0);
                                indices += 1;
                              }
                            }
                            assert!(indices >= 2, "both original helper instances must execute retained loads");
                            if matches!(case, RetainedIndexCaseV29::Guard | RetainedIndexCaseV29::GuardLoop) {
                                assert!(root.sidecars.rows.iter().any(|row| row.scoped_memory_anchors.as_ref()
                                    .is_some_and(|rows| rows.rows.iter().any(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::FailureRead { .. })))));
                            }
                            completed.set(true);
                            Ok(())
                        })
                })
            }))
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (
        result,
        budget.work(),
        budget.peak_storage(),
        completed.get(),
    )
}

#[test]
fn retained_source_index_constant_mask_and_success_guard_reach_actual_final_memory_census() {
    for case in [
        RetainedIndexCaseV29::Constant,
        RetainedIndexCaseV29::Mask,
        RetainedIndexCaseV29::Guard,
        RetainedIndexCaseV29::GuardLoop,
    ] {
        let (result, _, _, completed) =
            retained_index_run_v29(case, true, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        assert!(
            completed,
            "all actual final-census assertions must complete for {case:?}"
        );
    }
}

#[test]
fn retained_source_index_requires_shared_versions_and_refuses_post_guard_write_or_restart() {
    for (case, memory) in [
        (RetainedIndexCaseV29::Constant, false),
        (RetainedIndexCaseV29::ChangedAfterGuard, true),
        (RetainedIndexCaseV29::LifetimeRestart, true),
        (RetainedIndexCaseV29::StaleGuardLoop, true),
    ] {
        let (result, _, _, completed) =
            retained_index_run_v29(case, memory, MODULE_LIMIT, MODULE_LIMIT);
        assert!(
            result.is_err(),
            "{case:?} must not receive memory authority"
        );
        assert!(!completed);
    }
}

thread_local! {
    static RETAINED_INDEX_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static RETAINED_INDEX_MUTATED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn retained_index_observer_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for output in emitted.iter_mut().flatten() {
        let anchors = output.scoped_memory_anchors.as_mut().unwrap();
        let Some(ordinal) = anchors.rows.iter().position(|row| {
            matches!(
                row.kind,
                ScopedMemoryAnchorKindV29::Access {
                    payload: Some(ScopedMemoryPayloadV29::IndexLoad { .. }),
                    ..
                }
            )
        }) else {
            continue;
        };
        let row = &mut anchors.rows[ordinal];
        let ScopedMemoryAnchorKindV29::Access { payload, .. } = &mut row.kind else {
            unreachable!();
        };
        let Some(ScopedMemoryPayloadV29::IndexLoad { result, read }) = payload else {
            unreachable!();
        };
        match RETAINED_INDEX_FAULT_V29.get() {
            1 => read.event += 1,
            2 => read.local = SemanticLocalIdV1::from_index(read.local.index() + 1),
            3 => read.projection += 1,
            4 => {
                let other = output
                    .function
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .flat_map(|row| &row.results)
                    .find(|row| row.id != *result && row.ty == Type::Scalar(ScalarType::U32))
                    .expect("a distinct same-typed actual value")
                    .id;
                *result = other;
            }
            5 => *payload = None,
            6 => read.role = ExecutionOperandV29::AssertCondition,
            7 => {
                let effect = output
                    .private_arrays
                    .effects
                    .iter()
                    .find(|row| matches!(row.original_index, PrivateArrayIndexV1::Local { .. }))
                    .expect("actual dynamic array access");
                let location = effect.gep_location;
                let body = output.function.body.as_mut().unwrap();
                let replacement = body
                    .blocks
                    .iter()
                    .flat_map(|row| &row.operations)
                    .find_map(|row| match row.kind {
                        OperationKind::Constant(Constant::Index(4)) => {
                            row.results.first().map(|row| row.id)
                        }
                        _ => None,
                    })
                    .expect("original four-element array extent");
                let block = body
                    .blocks
                    .iter_mut()
                    .find(|row| row.id == location.block)
                    .unwrap();
                let OperationKind::GetElementPointer { offset, .. } =
                    &mut block.operations[location.operation].kind
                else {
                    panic!("actual array GEP");
                };
                assert_ne!(*offset, replacement);
                *offset = replacement;
            }
            _ => unreachable!(),
        }
        RETAINED_INDEX_MUTATED_V29.set(true);
        return Ok(());
    }
    Ok(())
}

#[test]
fn retained_source_index_rejects_missing_wrong_occurrence_local_projection_role_and_value_receipts()
{
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    for fault in 1..=7 {
        RETAINED_INDEX_FAULT_V29.set(fault);
        RETAINED_INDEX_MUTATED_V29.set(false);
        let restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(retained_index_observer_v29)));
        let (result, _, _, completed) = retained_index_run_v29(
            RetainedIndexCaseV29::Constant,
            true,
            MODULE_LIMIT,
            MODULE_LIMIT,
        );
        drop(restore);
        assert!(
            RETAINED_INDEX_MUTATED_V29.get(),
            "actual source mutation did not execute for {fault}: {result:?}"
        );
        assert!(!completed);
        assert!(
            result.is_err(),
            "mutated source index receipt {fault} escaped"
        );
    }
}

mod retained_index_equations_v29 {
    use super::*;
    include!("production_source_object_index_equations_v29_tests.rs");
    use fe2o3_kernel_analysis::{CanonicalKirInventoryV18, CanonicalKirMemorySsaV18};
    use fe2o3_kernel_ir::{
        CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
        CanonicalKirOperationCoordinateV1 as OperationCoordinate,
        VerifiedCanonicalKernelIrModuleV18,
    };

    const CELL: ValueId = ValueId(10);
    const ZERO: ValueId = ValueId(11);
    const FOUR: ValueId = ValueId(12);
    const FIRST: ValueId = ValueId(20);
    const CONDITION: ValueId = ValueId(21);
    const INDEX: ValueId = ValueId(22);

    fn load(id: ValueId) -> Operation {
        Operation::effect_free(
            ValueDef::new(id, Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: CELL,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        )
    }

    fn write(value: ValueId) -> Operation {
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: CELL,
                value,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        )
    }

    fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
        let mut block = BasicBlock::new(BlockId(id));
        block.operations = operations;
        block.terminator = Some(terminator);
        block
    }

    fn branch(target: u32) -> Terminator {
        Terminator::Branch {
            target: BlockId(target),
            arguments: vec![],
        }
    }

    fn conditional(condition: ValueId, left: u32, right: u32) -> Terminator {
        Terminator::ConditionalBranch {
            condition,
            then_target: BlockId(left),
            then_arguments: vec![],
            else_target: BlockId(right),
            else_arguments: vec![],
        }
    }

    fn module(initialized: bool, loop_write: bool, recheck: bool, mixed_entry: bool) -> Module {
        let mut entry = vec![
            Operation::effect_free(
                ValueDef::new(
                    CELL,
                    Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    ),
                ),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U32),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ZERO, Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(0)),
            ),
            Operation::effect_free(
                ValueDef::new(FOUR, Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(4)),
            ),
        ];
        if initialized && !mixed_entry {
            entry.push(write(ZERO));
        }
        let mut blocks = vec![block(
            77,
            entry,
            if mixed_entry {
                conditional(ValueId(1), 80, 90)
            } else {
                branch(90)
            },
        )];
        if mixed_entry {
            blocks.push(block(80, vec![write(ZERO)], branch(90)));
        }
        blocks.push(block(
            90,
            vec![
                load(FIRST),
                Operation::effect_free(
                    ValueDef::new(CONDITION, Type::BOOL),
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: FIRST,
                        rhs: FOUR,
                    },
                ),
            ],
            conditional(CONDITION, 100, 120),
        ));
        blocks.push(block(
            100,
            vec![load(INDEX)],
            if loop_write {
                conditional(ValueId(1), 110, 120)
            } else {
                branch(120)
            },
        ));
        if loop_write {
            blocks.push(block(
                110,
                vec![write(ValueId(0))],
                branch(if recheck { 90 } else { 100 }),
            ));
        }
        blocks.push(block(120, vec![], Terminator::Return { values: vec![] }));
        let mut module = Module::new("retained-index-memory-equations");
        module.functions.push(Function::internal_helper(
            "probe",
            Signature::new(vec![Type::Scalar(ScalarType::U32), Type::BOOL], vec![]),
            vec![ValueId(0), ValueId(1)],
            blocks,
        ));
        module
    }

    // These slot rows label independently checked physical objects only. Unit
    // equations never stand in for the genuine original-source tests above.
    fn with_model(
        module: &Module,
        inspect: impl FnOnce(
            &mut scoped_index_memory_v29::SourceIndexMemoryV29<'_, '_, '_>,
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        ),
    ) {
        with_model_or_error(module, |query, inventory, budget| {
            inspect(query.unwrap(), inventory, budget)
        });
    }

    fn with_model_or_error(
        module: &Module,
        inspect: impl FnOnce(
            Result<
                &mut scoped_index_memory_v29::SourceIndexMemoryV29<'_, '_, '_>,
                ProductionSemanticKirErrorV1,
            >,
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        ),
    ) {
        with_model_or_error_versions(module, true, inspect)
    }

    fn with_model_or_error_versions(
        module: &Module,
        supply_versions: bool,
        inspect: impl FnOnce(
            Result<
                &mut scoped_index_memory_v29::SourceIndexMemoryV29<'_, '_, '_>,
                ProductionSemanticKirErrorV1,
            >,
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        ),
    ) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (owner, owner_receipt) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                module,
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                &mut budget,
            )
            .unwrap();
        budget
            .reserve_storage(owner_receipt.retained_storage())
            .unwrap();
        let (inventory, inventory_receipt) =
            CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
        budget
            .reserve_storage(inventory_receipt.retained_storage())
            .unwrap();
        let (memory, memory_receipt) =
            CanonicalKirMemorySsaV18::derive_v18(&inventory, Default::default(), &mut budget)
                .unwrap();
        budget
            .reserve_storage(memory_receipt.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let function = inventory.functions()[0].function;
        let mut slots = Vec::new();
        for (block_ordinal, block) in function.body.as_ref().unwrap().blocks.iter().enumerate() {
            for (operation, row) in block.operations.iter().enumerate() {
                let OperationKind::Alloca { element, .. } = &row.kind else {
                    continue;
                };
                let (identity, source, representation) = match element {
                    Type::Scalar(ScalarType::U32) => (
                        ScopedAllocationIdentityV29::LegacyLocal(slots.len() as u32),
                        ScopedAllocationSourceV29::Legacy,
                        ScopedSlotRepresentationV29::ScalarArray(ScopedScalarArraySlotV29 {
                            element_type: SemanticTypeIdV1::from_index(0),
                            element: PrivateRetainedSlotFactsV1 {
                                element: PrivateRetainedElementFactsV1::Scalar(ScalarType::U32),
                                size: 4,
                                alignment: 4,
                            },
                            length: 1,
                            bytes: 4,
                            count: None,
                        }),
                    ),
                    Type::StorageObject(schema) => (
                        ScopedAllocationIdentityV29::OriginalObject {
                            local: slots.len() as u32,
                            generation: 0,
                        },
                        ScopedAllocationSourceV29::OriginalObject {
                            cell: slots.len(),
                            schema: *schema,
                        },
                        ScopedSlotRepresentationV29::Object {
                            schema: *schema,
                            bytes: 4,
                            alignment: 4,
                        },
                    ),
                    other => panic!("closed scalar holder test profile: {other:?}"),
                };
                slots.push(ScopedSourceSlotV29 {
                    instance: ProductionCallInstanceIdV1(0),
                    origin: ScopedSlotOriginV29 {
                        identity,
                        source,
                        semantic_type: SemanticTypeIdV1::from_index(0),
                        pointer: row.results[0].id,
                    },
                    representation,
                    allocation: PrivateArrayPhysicalLocationV1 {
                        block_ordinal,
                        block: block.id,
                        operation,
                    },
                });
            }
        }
        let accesses: Vec<_> = function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .operations
                    .iter()
                    .enumerate()
                    .filter_map(|(operation, row)| {
                        let pointer = match row.kind {
                            OperationKind::Load { pointer, .. }
                            | OperationKind::Store { pointer, .. }
                            | OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                                address: pointer,
                                ..
                            })
                            | OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                                address: pointer,
                                ..
                            }) => pointer,
                            _ => return None,
                        };
                        let slot = slots.iter().position(|row| row.origin.pointer == pointer)?;
                        Some(SourceAddressAccessV29 {
                            footprint: 0,
                            block: block.id,
                            operation,
                            slot,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        match SourceAddressMemoryV29::prepare_inventory(
            &inventory,
            FunctionCoordinate(0),
            &slots,
            &accesses,
            &mut budget,
        )
        .and_then(|graph| graph.solve(&slots, &accesses, &[], &mut budget))
        {
            Ok(graph) => {
                let mut query =
                    scoped_index_memory_v29::SourceIndexMemoryV29::with_optional_versions(
                        &inventory,
                        supply_versions.then_some(&memory),
                        &graph,
                        FunctionCoordinate(0),
                        &slots,
                        &accesses,
                        &mut budget,
                    )
                    .unwrap();
                inspect(Ok(&mut query), &inventory, &mut budget);
                drop(query);
                drop(graph);
            }
            Err(error) => inspect(Err(error), &inventory, &mut budget),
        }
        let scratch = budget.storage() - floor;
        budget.release_storage(scratch).unwrap();
        drop(memory);
        budget
            .release_storage(memory_receipt.retained_storage())
            .unwrap();
        drop(inventory);
        budget
            .release_storage(inventory_receipt.retained_storage())
            .unwrap();
        drop(owner);
        budget
            .release_storage(owner_receipt.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }

    fn coordinate(
        inventory: &CanonicalKirInventoryV18<'_>,
        id: u32,
        operation: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> OperationCoordinate {
        let block = inventory
            .block_for_id(FunctionCoordinate(0), BlockId(id), budget)
            .unwrap()
            .unwrap()
            .coordinate;
        OperationCoordinate { block, operation }
    }

    #[test]
    fn retained_memory_bound_requires_every_reaching_write_and_reuses_identical_queries() {
        for (initialized, mixed, accepted) in [
            (true, false, true),
            (false, false, false),
            (true, true, false),
        ] {
            with_model(
                &module(initialized, false, false, mixed),
                |query, inventory, budget| {
                    let load = coordinate(inventory, 90, 0, budget);
                    assert_eq!(
                        query.check_bound(0, load, FIRST, 4, budget).unwrap(),
                        accepted
                    );
                    let storage = budget.storage();
                    let work = budget.work();
                    assert_eq!(
                        query.check_bound(0, load, FIRST, 4, budget).unwrap(),
                        accepted
                    );
                    assert_eq!(
                        budget.storage(),
                        storage,
                        "identical query must not retain another equation/cache row"
                    );
                    assert!(budget.work() > work);
                    budget.reserve_storage(257).unwrap();
                    assert_eq!(
                        query.check_bound(0, load, FIRST, 3, budget).unwrap(),
                        accepted
                    );
                    budget.release_storage(257).unwrap();
                    assert_eq!(
                        query.check_bound(0, load, FIRST, 3, budget).unwrap(),
                        accepted,
                        "query-owned growth must not adopt unrelated temporary credit"
                    );
                    assert!(
                        query.check_bound(0, load, INDEX, 4, budget).is_err(),
                        "another same-typed result cannot stand in for this Load"
                    );
                    let comparison = coordinate(inventory, 90, 1, budget);
                    assert!(
                        query.check_bound(0, comparison, FIRST, 4, budget).is_err(),
                        "a source coordinate is not an actual MemorySSA Load"
                    );
                },
            );
        }
    }

    #[test]
    fn retained_memory_copy_cycle_needs_a_real_initialized_entry_write() {
        for initialized in [false, true] {
            let mut original = module(initialized, true, true, false);
            let latch = original.functions[0]
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .find(|row| row.id == BlockId(110))
                .unwrap();
            latch.operations[0] = write(FIRST);
            with_model(&original, |query, inventory, budget| {
                let load = coordinate(inventory, 90, 0, budget);
                assert_eq!(
                    query.check_bound(0, load, FIRST, 4, budget).unwrap(),
                    initialized,
                    "a saved read and its copied Store cannot seed their own bound"
                );
            });
        }
    }

    #[test]
    fn retained_memory_typed_cross_slot_copy_cycle_cannot_invent_a_bound() {
        const OTHER: ValueId = ValueId(13);
        const COPIED: ValueId = ValueId(40);
        for initialized in [false, true] {
            let mut original = module(initialized, true, true, false);
            let body = original.functions[0].body.as_mut().unwrap();
            let mut allocation = body.blocks[0].operations[0].clone();
            allocation.results[0].id = OTHER;
            body.blocks[0].operations.insert(1, allocation);
            let latch = body
                .blocks
                .iter_mut()
                .find(|row| row.id == BlockId(110))
                .unwrap();
            let mut store_other = write(FIRST);
            let OperationKind::Store { pointer, .. } = &mut store_other.kind else {
                unreachable!();
            };
            *pointer = OTHER;
            let mut read_other = load(COPIED);
            let OperationKind::Load { pointer, .. } = &mut read_other.kind else {
                unreachable!();
            };
            *pointer = OTHER;
            latch.operations = vec![store_other, read_other, write(COPIED)];
            with_model(&original, |query, inventory, budget| {
                let load = coordinate(inventory, 90, 0, budget);
                assert_eq!(
                    query.check_bound(0, load, FIRST, 4, budget).unwrap(),
                    initialized
                );
            });
        }
    }

    #[test]
    fn retained_guard_reexecuted_def_requires_fresh_check_and_lifetime_currentness() {
        for recheck in [false, true] {
            with_model(
                &module(true, true, recheck, false),
                |query, inventory, budget| {
                    let guard = SourceIndexGuardLocationV29 {
                        source: PendingSourceIndexGuardV29 {
                            instance: ProductionCallInstanceIdV1(0),
                            assertion: SemanticBlockIdV1::from_index(0),
                            condition_event: 0,
                            comparison_event: 0,
                            load_anchor: 0,
                            slot: 0,
                            value: FIRST,
                            scalar: ScalarType::U32,
                            length: 4,
                            condition: CONDITION,
                            block: BlockId(90),
                            success: BlockId(100),
                            failure: BlockId(120),
                        },
                        load: coordinate(inventory, 90, 0, budget),
                    };
                    let target = coordinate(inventory, 100, 0, budget);
                    assert_eq!(
                        query
                            .test_guard_bound_v29(&guard, target, &[], &[], budget)
                            .unwrap(),
                        recheck
                    );
                    let restart = [SourceAddressLifetimeV29 {
                        block: BlockId(100),
                        gap: 0,
                        sequence: 0,
                        slot: 0,
                        live: true,
                    }];
                    assert!(
                        !query
                            .test_guard_bound_v29(&guard, target, &[], &restart, budget)
                            .unwrap()
                    );
                },
            );
        }
    }

    #[test]
    fn retained_memory_unknown_private_or_generic_write_is_not_a_disjoint_store() {
        for space in [
            AddressSpace::Global,
            AddressSpace::Private,
            AddressSpace::Generic,
        ] {
            let mut original = module(true, false, false, false);
            let function = &mut original.functions[0];
            function.signature.parameters.push(Type::pointer(
                Type::Scalar(ScalarType::U32),
                space,
                AccessMode::ReadWrite,
            ));
            let body = function.body.as_mut().unwrap();
            body.parameters.push(ValueId(2));
            body.blocks[0].operations.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(2),
                    value: ValueId(0),
                    access: MemoryAccess::new(space, 4),
                },
            ));
            with_model_or_error(&original, |query, inventory, budget| {
                let load = coordinate(inventory, 90, 0, budget);
                let result = query.and_then(|query| query.check_bound(0, load, FIRST, 4, budget));
                if space == AddressSpace::Global {
                    assert!(
                        result.unwrap(),
                        "independently disjoint Global store preserves the bound"
                    );
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                        ),
                        "unknown {space:?} store cannot take the disjoint (None,None) route: {result:?}"
                    );
                }
            });
        }
    }

    #[test]
    fn retained_guard_or_bounded_write_never_substitutes_for_initialized_storage() {
        for initialized in [false, true] {
            with_model(
                &module(initialized, false, false, false),
                |query, inventory, budget| {
                    let guard = SourceIndexGuardLocationV29 {
                        source: PendingSourceIndexGuardV29 {
                            instance: ProductionCallInstanceIdV1(0),
                            assertion: SemanticBlockIdV1::from_index(0),
                            condition_event: 0,
                            comparison_event: 0,
                            load_anchor: 0,
                            slot: 0,
                            value: FIRST,
                            scalar: ScalarType::U32,
                            length: 4,
                            condition: CONDITION,
                            block: BlockId(90),
                            success: BlockId(100),
                            failure: BlockId(120),
                        },
                        load: coordinate(inventory, 90, 0, budget),
                    };
                    let target = coordinate(inventory, 100, 0, budget);
                    assert!(
                        query
                            .test_guard_bound_v29(&guard, target, &[], &[], budget)
                            .unwrap()
                    );
                    let initialization = query.test_initialized_v29(&[], budget);
                    if initialized {
                        initialization.unwrap();
                    } else {
                        assert!(
                            matches!(
                                initialization,
                                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                            ),
                            "the successful guard proves a range, not that its Load was initialized: {initialization:?}"
                        );
                    }
                    if initialized {
                        assert!(query.check_bound(0, target, INDEX, 4, budget).unwrap());
                        let kill = [SourceAddressKillV29 {
                            block: BlockId(100),
                            gap: 0,
                            slot: 0,
                        }];
                        assert!(
                            matches!(
                                query.test_initialized_v29(&kill, budget),
                                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                            ),
                            "a bounded reaching write does not survive a source storage kill"
                        );
                    }
                },
            );
        }
    }

    #[test]
    fn retained_memory_rejects_an_equivalent_report_from_another_inventory() {
        let mut module = Module::new("retained-index-foreign-report");
        module.functions.push(Function::internal_helper(
            "probe",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block(7, vec![], Terminator::Return { values: vec![] })],
        ));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (owner, paid) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                &mut budget,
            )
            .unwrap();
        budget.reserve_storage(paid.retained_storage()).unwrap();
        let (inventory, paid) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
        budget.reserve_storage(paid.retained_storage()).unwrap();
        let (other, paid) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
        budget.reserve_storage(paid.retained_storage()).unwrap();
        let (memory, paid) =
            CanonicalKirMemorySsaV18::derive_v18(&other, Default::default(), &mut budget).unwrap();
        budget.reserve_storage(paid.retained_storage()).unwrap();
        let graph = SourceAddressMemoryV29::new(
            inventory.functions()[0].function,
            &[],
            None,
            &[],
            &[],
            &mut budget,
        )
        .unwrap();
        assert!(matches!(
            scoped_index_memory_v29::SourceIndexMemoryV29::new(
                &inventory,
                &memory,
                &graph,
                FunctionCoordinate(0),
                &[],
                &[],
                &mut budget
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        drop(graph);
        drop(memory);
        drop(other);
        drop(inventory);
        drop(owner);
        let released = budget.storage().checked_sub(MODULE_FLOOR).unwrap();
        budget.release_storage(released).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }

    #[test]
    fn retained_guard_refuses_duplicate_polarities_failure_edge_reentry_and_bypass() {
        for fault in 0..3 {
            let mut original = module(true, false, false, false);
            let body = original.functions[0].body.as_mut().unwrap();
            match fault {
                0 => {
                    body.blocks
                        .iter_mut()
                        .find(|row| row.id == BlockId(90))
                        .unwrap()
                        .terminator = Some(conditional(CONDITION, 100, 100))
                }
                1 => {
                    body.blocks
                        .iter_mut()
                        .find(|row| row.id == BlockId(120))
                        .unwrap()
                        .terminator = Some(branch(100))
                }
                2 => body.blocks[0].terminator = Some(conditional(ValueId(1), 90, 100)),
                _ => unreachable!(),
            }
            with_model(&original, |query, inventory, budget| {
                let guard = SourceIndexGuardLocationV29 {
                    source: PendingSourceIndexGuardV29 {
                        instance: ProductionCallInstanceIdV1(0),
                        assertion: SemanticBlockIdV1::from_index(0),
                        condition_event: 0,
                        comparison_event: 0,
                        load_anchor: 0,
                        slot: 0,
                        value: FIRST,
                        scalar: ScalarType::U32,
                        length: 4,
                        condition: CONDITION,
                        block: BlockId(90),
                        success: BlockId(100),
                        failure: BlockId(if fault == 0 { 100 } else { 120 }),
                    },
                    load: coordinate(inventory, 90, 0, budget),
                };
                let target = coordinate(inventory, 100, 0, budget);
                let result = query.test_guard_bound_v29(&guard, target, &[], &[], budget);
                if fault == 0 {
                    assert!(matches!(
                        result,
                        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                    ));
                } else {
                    assert!(
                        !result.unwrap(),
                        "unguarded path {fault} cannot carry successful-edge evidence"
                    );
                }
            });
        }
    }
}

mod retained_array_initializer_payload_v29 {
    use super::*;

    thread_local! {
        static FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
        static OBSERVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static MUTATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    fn owner(duplicate: bool) -> ProductionSemanticSsaOwnerV1 {
        let original = retained_index_owner_v29(RetainedIndexCaseV29::Constant);
        if !duplicate {
            return original;
        }
        let semantic = original.source_semantic();
        let mut functions = semantic.functions().to_vec();
        for index in [2, 3] {
            let old = &functions[index];
            let mut blocks = old.blocks().to_vec();
            let mut statements = blocks[0].statements().to_vec();
            let SemanticStatementKindV1::Assign(assignment) = statements[2].kind() else {
                panic!("original array assignment");
            };
            let destination = assignment.destination().clone();
            statements[2] = assign(
                destination,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Array,
                        vec![literal(11), literal(11), literal(13), literal(14)],
                    )
                    .unwrap(),
                ),
            );
            blocks[0] = block(
                if index == 2 { 90 } else { 115 },
                statements,
                blocks[0].terminator().kind().clone(),
            );
            functions[index] = function(
                if index == 2 { 100 } else { 110 },
                old.role(),
                old.abi().clone(),
                old.locals().to_vec(),
                blocks,
            );
        }
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
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    fn observe(
        _: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        _: &OwnedScopedSourceSlotsV29,
        _: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut covered = BTreeMap::<usize, u8>::new();
        let mut target = None;
        for (instance, output) in emitted.iter().enumerate() {
            let Some(output) = output else {
                continue;
            };
            let original = instances
                .instance(instances.id_at(instance).unwrap())
                .unwrap()
                .declaration();
            let anchors = output.scoped_memory_anchors.as_ref().unwrap();
            let body = output.function.body.as_ref().unwrap();
            for effect in &output.private_arrays.effects {
                let PrivateArrayIndexV1::InitializerElement {
                    component,
                    value: PrivateArrayInitializerValueV1::LiteralScalar { value, definition },
                } = effect.original_index
                else {
                    continue;
                };
                let SemanticStatementKindV1::Assign(assignment) = original.blocks()
                    [effect.semantic_block as usize]
                    .statements()[effect.semantic_statement as usize]
                    .kind()
                else {
                    panic!("original assignment");
                };
                let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                    panic!("original array");
                };
                assert_eq!(aggregate.kind(), &SemanticAggregateKindV1::Array);
                assert_eq!(aggregate.operands().len(), 4);
                assert!(assignment.destination().projections().is_empty());
                assert_eq!(assignment.destination().local().index(), effect.local);
                let SemanticOperandV1::Constant(constant) =
                    &aggregate.operands()[component as usize]
                else {
                    panic!("literal component");
                };
                let SemanticConstantValueV1::Scalar(bits) = constant.value() else {
                    panic!("scalar literal");
                };
                assert_eq!(constant.ty(), U32);
                let site = execution_site_v29(
                    SemanticBlockIdV1::from_index(effect.semantic_block),
                    Some(effect.semantic_statement),
                );
                let mut matches = anchors.rows.iter().enumerate().filter(|(_, row)| {
                    row.block == effect.memory_location.block
                        && row.position == effect.memory_location.operation
                });
                let (anchor, row) = matches.next().expect("one initializer access");
                assert!(matches.next().is_none());
                assert_eq!(
                    row.source,
                    Some(ScopedMemoryFrameV29::operand(
                        site,
                        Some(ExecutionOperandV29::Destination)
                    ))
                );
                assert_eq!(
                    row.kind,
                    ScopedMemoryAnchorKindV29::Access {
                        pointer: effect.gep,
                        payload: Some(ScopedMemoryPayloadV29::Store {
                            value,
                            source: ScopedMemoryStoreSourceV29::Operand {
                                site,
                                role: ExecutionOperandV29::RvalueOperand(component),
                                ty: U32,
                                source: ScopedMemoryOperandSourceV29::Constant,
                            },
                        }),
                    }
                );
                let actual = &body
                    .blocks
                    .iter()
                    .find(|row| row.id == effect.memory_location.block)
                    .unwrap()
                    .operations[effect.memory_location.operation];
                assert!(
                    matches!(actual.kind, OperationKind::Store { pointer, value: actual, .. }
                    if pointer == effect.gep && actual == value)
                );
                let defined = &body
                    .blocks
                    .iter()
                    .find(|row| row.id == definition.block)
                    .unwrap()
                    .operations[definition.operation];
                assert_eq!(
                    defined.results.as_slice(),
                    &[ValueDef::new(value, Type::Scalar(ScalarType::U32))]
                );
                assert_eq!(
                    defined.kind,
                    OperationKind::Constant(
                        lower_constant(Type::Scalar(ScalarType::U32), *bits).unwrap()
                    )
                );
                let mask = covered.entry(instance).or_default();
                assert_eq!(
                    *mask & (1 << component),
                    0,
                    "no duplicate initializer component"
                );
                *mask |= 1 << component;
                if component == 0 && target.is_none() {
                    let second = output
                        .private_arrays
                        .effects
                        .iter()
                        .find_map(|row| match row.original_index {
                            PrivateArrayIndexV1::InitializerElement {
                                component: 1,
                                value: PrivateArrayInitializerValueV1::LiteralScalar { value, .. },
                            } if row.semantic_block == effect.semantic_block
                                && row.semantic_statement == effect.semantic_statement =>
                            {
                                Some(value)
                            }
                            _ => None,
                        })
                        .unwrap();
                    target = Some((instance, anchor, effect.memory_location, second));
                }
            }
        }
        if covered.is_empty() {
            return Ok(());
        }
        assert!(covered.len() >= 2, "both original helper instances");
        assert!(covered.values().all(|mask| *mask == 15));
        // This marker is after every observation assertion and outside mutation.
        OBSERVED.set(OBSERVED.get() + covered.len());
        if FAULT.get() == 0 {
            return Ok(());
        }
        let (instance, anchor, location, other) = target.unwrap();
        let output = emitted[instance].as_mut().unwrap();
        let row = &mut output.scoped_memory_anchors.as_mut().unwrap().rows[anchor];
        let ScopedMemoryAnchorKindV29::Access { payload, .. } = &mut row.kind else {
            unreachable!();
        };
        if FAULT.get() == 1 {
            *payload = None;
        } else {
            let Some(ScopedMemoryPayloadV29::Store {
                value,
                source: ScopedMemoryStoreSourceV29::Operand { role, .. },
            }) = payload
            else {
                unreachable!();
            };
            match FAULT.get() {
                2 => *role = ExecutionOperandV29::RvalueOperand(1),
                3 => *role = ExecutionOperandV29::RvalueOperand(4),
                4 => *value = other,
                5 => {
                    *value = other;
                    let actual = &mut output
                        .function
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .find(|row| row.id == location.block)
                        .unwrap()
                        .operations[location.operation];
                    let OperationKind::Store { value, .. } = &mut actual.kind else {
                        unreachable!();
                    };
                    *value = other;
                }
                _ => unreachable!(),
            }
        }
        MUTATED.set(true);
        Ok(())
    }

    fn run(duplicate: bool) -> (SourceOwnedResultV18<()>, bool) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let completed = std::cell::Cell::new(false);
        let result = (|| {
            let projection = owner(duplicate);
            let original = owner(duplicate);
            let (_, launch) =
                with_module_fixture_view(&original, ModuleFixture::Array, &mut budget, |_, _| ())?;
            let prepared = with_module_fixture_view(
                &projection,
                ModuleFixture::Array,
                &mut budget,
                |source, budget| {
                    ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                        original,
                        launch,
                        source.input,
                        ProductionSemanticKirLimitsV1::default(),
                        budget,
                    )
                },
            )?
            .0?;
            prepared.with_source_consumer_v18(
                &mut budget,
                |source, budget| -> SourceOwnedResultV18<()> {
                    source.with_analysis_v18(budget, |scope| {
                        scope.with_sparse_and_memory_ssa_v1(|_, memory, budget| {
                            source.with_ranked_correspondence_v18(
                                memory.inventory(),
                                budget,
                                |relation, budget| {
                                    scoped_raw_admission_v29::with_checked_source_memory_v29(
                                        relation,
                                        1,
                                        Some(memory),
                                        budget,
                                        |_, _| -> SourceOwnedResultV18<()> {
                                            completed.set(true);
                                            Ok(())
                                        },
                                    )
                                },
                            )
                        })
                    })
                },
            )
        })();
        assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
        (result, completed.get())
    }

    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }

    #[test]
    fn exact_array_initializer_operands_are_captured_for_every_actual_store() {
        for duplicate in [false, true] {
            FAULT.set(0);
            OBSERVED.set(0);
            MUTATED.set(false);
            let restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe)));
            let (result, completed) = run(duplicate);
            drop(restore);
            assert!(
                OBSERVED.get() >= 2,
                "all original source assertions completed: {result:?}"
            );
            result.unwrap();
            assert!(completed);
            assert!(!MUTATED.get());
        }
    }

    #[test]
    fn array_initializer_payload_cannot_change_component_even_for_equal_literals() {
        for duplicate in [false, true] {
            for fault in 1..=5 {
                FAULT.set(fault);
                OBSERVED.set(0);
                MUTATED.set(false);
                let restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe)));
                let (result, completed) = run(duplicate);
                drop(restore);
                assert!(
                    OBSERVED.get() >= 2,
                    "valid source receipts checked before sabotage: {result:?}"
                );
                assert!(
                    MUTATED.get(),
                    "mutation {fault} completed outside assertions"
                );
                assert!(
                    result.is_err(),
                    "component/value substitution {fault} escaped"
                );
                assert!(!completed);
            }
        }
    }

    #[test]
    fn array_initializer_source_lookup_has_exact_independent_work_and_no_storage() {
        let owner = owner(false);
        let source = owner.source_semantic();
        let function = &source.functions()[2];
        let site = execution_site_v29(SemanticBlockIdV1::from_index(0), Some(2));
        for limit in [16, 15] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = scoped_array_initializer_operand_v29(
                source.types(),
                function,
                site,
                0,
                &mut budget,
            );
            if limit == 16 {
                assert!(matches!(result.unwrap(), SemanticOperandV1::Constant(_)));
                assert_eq!(budget.work(), 16);
            } else {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
            }
            assert_eq!(budget.storage(), 0);
        }
    }
}

mod retained_array_payload_rpo_v29 {
    use super::*;

    thread_local! {
        static DUPLICATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static OBSERVED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static MUTATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    fn assert_split(original: &SemanticFunctionDeclV1, split: &SemanticFunctionDeclV1) {
        let count = original.blocks().len();
        let statements = original.blocks()[0].statements();
        assert_eq!(split.blocks().len(), count + 2);
        assert_eq!(split.identity(), original.identity());
        assert_eq!(split.role(), original.role());
        assert_eq!(split.abi(), original.abi());
        assert_eq!(split.locals(), original.locals());
        assert_eq!(&split.blocks()[1..count], &original.blocks()[1..]);
        assert_eq!(split.blocks()[0].statements(), &statements[..2]);
        assert_eq!(split.blocks()[count + 1].statements(), &statements[2..4]);
        assert_eq!(split.blocks()[count].statements(), &statements[4..]);
        for (from, to) in [(0, count + 1), (count + 1, count)] {
            assert_eq!(
                split.blocks()[from].terminator().kind(),
                &SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(u32::try_from(to).unwrap())
                ))
            );
        }
        assert_eq!(
            split.blocks()[count].terminator().kind(),
            original.blocks()[0].terminator().kind()
        );
    }

    fn owner() -> ProductionSemanticSsaOwnerV1 {
        let original = retained_index_owner_v29(RetainedIndexCaseV29::Constant);
        let semantic = original.source_semantic();
        let mut functions = semantic.functions().to_vec();
        for index in [2, 3] {
            let old = &functions[index];
            let count = old.blocks().len();
            assert_eq!(count, if index == 2 { 3 } else { 1 });
            let statements = old.blocks()[0].statements();
            assert!(
                matches!(statements[2].kind(), SemanticStatementKindV1::Assign(assignment)
                if matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(_)))
            );
            let jump = |to| {
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(to),
                ))
            };
            let key = if index == 2 { 90 } else { 115 };
            let suffix = u32::try_from(count).unwrap();
            let initializer = suffix.checked_add(1).unwrap();
            let mut blocks = old.blocks().to_vec();
            blocks[0] = block(key, statements[..2].to_vec(), jump(initializer));
            blocks.push(block(
                key + u8::try_from(count).unwrap(),
                statements[4..].to_vec(),
                old.blocks()[0].terminator().kind().clone(),
            ));
            blocks.push(block(
                key + u8::try_from(count + 1).unwrap(),
                statements[2..4].to_vec(),
                jump(suffix),
            ));
            let split = function(
                if index == 2 { 100 } else { 110 },
                old.role(),
                old.abi().clone(),
                old.locals().to_vec(),
                blocks,
            );
            assert_split(old, &split);
            functions[index] = split;
        }
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
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    fn observe(
        _: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        _: &OwnedScopedSourceSlotsV29,
        _: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut seen = 0;
        let mut target = None;
        let unsplit = retained_index_owner_v29(RetainedIndexCaseV29::Constant);
        for (instance, output) in emitted.iter().enumerate() {
            let Some(output) = output else {
                continue;
            };
            let effects = &output.private_arrays.effects;
            let Some(first) = effects.iter().find(|row| {
                matches!(
                    row.original_index,
                    PrivateArrayIndexV1::InitializerElement { component: 0, .. }
                )
            }) else {
                continue;
            };
            let instance_row = instances
                .instance(instances.id_at(instance).unwrap())
                .unwrap();
            let original = instance_row.declaration();
            let function = instance_row.function().index() as usize;
            assert!(matches!(function, 2 | 3));
            let prior = &unsplit.source_semantic().functions()[function];
            let count = prior.blocks().len();
            assert_eq!(count, if function == 2 { 3 } else { 1 });
            assert_split(prior, original);
            let suffix = u32::try_from(count).unwrap();
            let initializer = suffix.checked_add(1).unwrap();
            assert_eq!(first.semantic_block, initializer);
            assert_eq!(first.semantic_statement, 0);
            let later = effects
                .iter()
                .find(|row| row.semantic_block == suffix)
                .unwrap();
            assert!(
                later.semantic_block < first.semantic_block,
                "source keys place the suffix before the appended initializer"
            );
            assert!(
                first.memory_location.block_ordinal < later.memory_location.block_ordinal,
                "actual frame emission follows entry -> initializer -> suffix, not source ordinal"
            );
            assert!(
                effects
                    .windows(2)
                    .all(|pair| private_array_effect_key_v1(&pair[0])
                        < private_array_effect_key_v1(&pair[1]))
            );
            let second = effects
                .iter()
                .find(|row| {
                    matches!(
                        row.original_index,
                        PrivateArrayIndexV1::InitializerElement { component: 1, .. }
                    )
                })
                .unwrap();
            assert_eq!(
                &private_array_effect_key_v1(first)[..6],
                &private_array_effect_key_v1(second)[..6]
            );
            for limit in [29, 28] {
                let mut rows = [*second, *first];
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                let result = order_scoped_array_payload_recipes_v29(&mut rows, &mut budget);
                if limit == 29 {
                    result.unwrap();
                    assert_eq!(rows, [*first, *second]);
                    assert_eq!(budget.work(), 29);
                } else {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                }
                assert_eq!(budget.storage(), 0);
            }
            let mut duplicates = [*first, *first];
            let mut work = CanonicalKernelIrWorkBudgetV1::new(29);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert!(matches!(
                order_scoped_array_payload_recipes_v29(&mut duplicates, &mut budget),
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ));
            assert_eq!(budget.storage(), 0);
            if target.is_none() {
                target = Some((instance, *first));
            }
            seen += 1;
        }
        if seen == 0 {
            return Ok(());
        }
        assert!(seen >= 2);
        OBSERVED.set(true);
        if DUPLICATE.get() {
            let (instance, first) = target.unwrap();
            let rows = &mut emitted[instance].as_mut().unwrap().private_arrays.effects;
            let last = rows.last_mut().unwrap();
            assert_ne!(
                private_array_effect_key_v1(last),
                private_array_effect_key_v1(&first)
            );
            *last = first;
            MUTATED.set(true);
        }
        Ok(())
    }

    fn run() -> (SourceOwnedResultV18<()>, bool) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let completed = std::cell::Cell::new(false);
        let result = (|| {
            let projection = owner();
            let original = owner();
            let (_, launch) =
                with_module_fixture_view(&original, ModuleFixture::Array, &mut budget, |_, _| ())?;
            let prepared = with_module_fixture_view(
                &projection,
                ModuleFixture::Array,
                &mut budget,
                |source, budget| {
                    ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                        original,
                        launch,
                        source.input,
                        ProductionSemanticKirLimitsV1::default(),
                        budget,
                    )
                },
            )?
            .0?;
            prepared.with_source_consumer_v18(
                &mut budget,
                |source, budget| -> SourceOwnedResultV18<()> {
                    source.with_analysis_v18(budget, |scope| {
                        scope.with_sparse_and_memory_ssa_v1(|_, memory, budget| {
                            source.with_ranked_correspondence_v18(
                                memory.inventory(),
                                budget,
                                |relation, budget| {
                                    scoped_raw_admission_v29::with_checked_source_memory_v29(
                                        relation,
                                        1,
                                        Some(memory),
                                        budget,
                                        |_, _| -> SourceOwnedResultV18<()> {
                                            completed.set(true);
                                            Ok(())
                                        },
                                    )
                                },
                            )
                        })
                    })
                },
            )
        })();
        assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
        (result, completed.get())
    }

    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }

    #[test]
    fn initializer_payload_order_follows_source_keys_after_actual_reverse_postorder_emission() {
        DUPLICATE.set(false);
        OBSERVED.set(false);
        MUTATED.set(false);
        let restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe)));
        let (result, completed) = run();
        drop(restore);
        assert!(
            OBSERVED.get(),
            "RPO/order/resource assertions completed: {result:?}"
        );
        result.unwrap();
        assert!(completed);
        assert!(!MUTATED.get());
    }

    #[test]
    fn duplicate_initializer_recipe_cannot_replace_an_actual_source_component() {
        DUPLICATE.set(true);
        OBSERVED.set(false);
        MUTATED.set(false);
        let restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe)));
        let (result, completed) = run();
        drop(restore);
        assert!(
            OBSERVED.get(),
            "valid source and accounting assertions completed: {result:?}"
        );
        assert!(MUTATED.get());
        assert!(result.is_err());
        assert!(!completed);
    }
}

mod typed_object_allocation_tests_v29 {
    use super::*;
    include!("production_source_object_allocation_v29_tests.rs");
}
