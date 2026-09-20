use super::*;
use fe2o3_kernel_ir::LocalFrameAccessKindV1;

#[test]
fn retained_array_length_joins_source_extent_to_actual_typed_u64_constant() {
    let owner = unit_owner(UnitCase::ArrayLength, &[1]);
    let rows = &owner.helper_memory.unit_source;
    assert_eq!(rows.associations.len(), 1);
    let association = rows.associations[0];
    assert_eq!(association.key.root, SemanticFunctionIdV1::from_index(0));
    assert_eq!(
        association.key.function,
        SemanticFunctionIdV1::from_index(1)
    );
    let source = &owner.semantic_ssa().source_semantic().functions()[1];
    assert_eq!(association.source_identity, source.identity());
    assert_eq!(
        association.plan_identity,
        owner
            .semantic_ssa()
            .plan_for_function(association.key.function)
            .unwrap()
            .plan()
            .identity()
    );
    let SemanticStatementKindV1::Assign(assignment) = source.blocks()[0].statements()[3].kind()
    else {
        panic!("actual source Length assignment")
    };
    assert_eq!(assignment.value().result_type(), LOCAL_U64);
    assert!(
        matches!(assignment.value().kind(), SemanticRvalueKindV1::Length(place)
        if place.local().index() == 1 && place.projections().is_empty() && place.ty() == ARRAY_TYPE)
    );
    let lengths = rows.values[association.values.0..association.values.1]
        .iter()
        .filter(|row| matches!(row.recipe, UnitLocalValueRecipeV1::ArrayLength { .. }));
    let mut lengths = lengths;
    let length = lengths.next().unwrap();
    assert!(lengths.next().is_none());
    assert_eq!(length.key, association.key);
    assert_eq!(length.source_type, LOCAL_U64);
    assert_eq!(length.known_bits, Some(8));
    let UnitLocalValueRecipeV1::ArrayLength { local, allocation } = length.recipe else {
        unreachable!()
    };
    assert_eq!(local.index(), 1);
    assert!(
        matches!(rows.memory[allocation], UnitLocalMemoryRowV1::Allocation {
        key, local, source_type: ARRAY_TYPE, count: 8, array_slot: Some(_), ..
    } if key == association.key && local.index() == 1)
    );
    let UnitLocalNativeValueV1::Scalar {
        value,
        scalar,
        definition: Some(definition),
    } = length.native
    else {
        panic!("actual typed native Length definition")
    };
    assert_eq!(scalar, ScalarType::U64);
    assert_eq!(
        definition.block.function.0 as usize,
        association.key.physical
    );
    let native = &owner.executable().module().functions[association.key.physical]
        .body
        .as_ref()
        .unwrap()
        .blocks[definition.block.block as usize]
        .operations[definition.operation as usize];
    assert_eq!(native.kind, OperationKind::Constant(Constant::U64(8)));
    assert_eq!(
        native.results,
        vec![ValueDef::new(value, Type::Scalar(ScalarType::U64))]
    );
}

fn assert_latest_store_rows(owner: &ProductionPreRankedKirOwnerV1) -> (usize, usize) {
    let rows = &owner.helper_memory.unit_source;
    let mut scalar_reads = 0;
    let mut array_reads = 0;
    for (ordinal, row) in rows.memory.iter().enumerate() {
        let UnitLocalMemoryRowV1::Access {
            source_key,
            key,
            local,
            allocation,
            cell,
            physical_access,
            value,
            latest_source_store,
        } = row
        else {
            continue;
        };
        assert_eq!(
            (source_key[0], source_key[1]),
            (key.root.index() as usize, key.function.index() as usize)
        );
        let physical = owner.helper_memory.accesses[*physical_access];
        assert_eq!(physical.cell(), *cell);
        let UnitLocalMemoryRowV1::Allocation {
            key: allocation_key,
            local: allocation_local,
            physical_allocation,
            array_slot,
            ..
        } = rows.memory[*allocation]
        else {
            panic!("same-owner source allocation row")
        };
        assert_eq!(allocation_key, *key);
        assert_eq!(allocation_local, *local);
        assert_eq!(
            owner.helper_memory.allocations[physical_allocation]
                .location()
                .function_ordinal(),
            key.physical
        );
        if physical.kind() != LocalFrameAccessKindV1::Read {
            continue;
        }
        if array_slot.is_some() {
            array_reads += 1;
        } else {
            scalar_reads += 1;
        }
        let latest = latest_source_store.expect("every actual Load needs its latest source Store");
        assert!(latest < ordinal);
        let independently_latest = rows.memory[..ordinal].iter().rposition(|candidate| {
            matches!(candidate, UnitLocalMemoryRowV1::Access {
                key: previous_key, allocation: previous_allocation, cell: previous_cell,
                physical_access: previous_access, ..
            } if *previous_key == *key && *previous_allocation == *allocation
                && *previous_cell == *cell
                && owner.helper_memory.accesses[*previous_access].kind() == LocalFrameAccessKindV1::Write)
        });
        assert_eq!(independently_latest, Some(latest));
        let UnitLocalMemoryRowV1::Access {
            physical_access: previous,
            ..
        } = rows.memory[latest]
        else {
            panic!("latest source Store is an Access row")
        };
        assert_eq!(
            physical.initializing_store(),
            Some(owner.helper_memory.accesses[previous].location())
        );
        assert!(
            matches!(rows.values[*value].recipe, UnitLocalValueRecipeV1::Load { access } if access == ordinal)
        );
        assert!(matches!(rows.values[*value].native,
            UnitLocalNativeValueV1::Scalar { value, .. } if value == physical.value()));
        if array_slot.is_some() {
            assert_eq!(rows.values[*value].known_bits, Some(99));
        }
    }
    (scalar_reads, array_reads)
}

#[test]
fn actual_private_read_joins_the_latest_write_not_an_equal_initializer_component() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[1]);
    assert_eq!(owner.correspondence.private_arrays.effects.len(), 11);
    assert_eq!(owner.helper_memory.accesses.len(), 11);
    assert_eq!(assert_latest_store_rows(&owner), (0, 1));
    let rows = &owner.helper_memory.unit_source;
    let read = rows
        .memory
        .iter()
        .find_map(|row| match row {
            UnitLocalMemoryRowV1::Access {
                physical_access,
                latest_source_store,
                ..
            } if owner.helper_memory.accesses[*physical_access].kind()
                == LocalFrameAccessKindV1::Read =>
            {
                *latest_source_store
            }
            _ => None,
        })
        .unwrap();
    let UnitLocalMemoryRowV1::Access {
        source_key, cell, ..
    } = rows.memory[read]
    else {
        panic!("source Store")
    };
    assert_eq!((source_key[2], source_key[3], cell), (0, 2, 0));
}

#[test]
fn nonvolatile_scalar_store_forces_a_real_slot_and_load_without_pointer_escape() {
    let owner = unit_owner(UnitCase::ScalarSlot, &[1]);
    let plan = owner
        .semantic_ssa()
        .plan_for_function(SemanticFunctionIdV1::from_index(1))
        .unwrap();
    assert!(
        !plan
            .plan()
            .promoted_variables()
            .iter()
            .any(|variable| variable.get() == 2)
    );
    let (scalar_reads, array_reads) = assert_latest_store_rows(&owner);
    assert!(scalar_reads >= 1);
    assert_eq!(array_reads, 1);
    let rows = &owner.helper_memory.unit_source;
    assert_eq!(rows.memory.iter().filter(|row| matches!(row,
        UnitLocalMemoryRowV1::Allocation { local, array_slot: None, count: 1, element: ScalarType::U32, .. }
        if local.index() == 2)).count(), 1);
    assert_eq!(owner.helper_memory.allocations.len(), 2);
}

#[test]
fn source_only_kills_are_retained_and_reinitialization_does_not_reuse_old_store_identity() {
    for kill in [
        ScalarKill::Move,
        ScalarKill::Storage,
        ScalarKill::Deinitialize,
    ] {
        let owner = unit_owner(UnitCase::Reinitialize(kill), &[1]);
        let (scalar_reads, array_reads) = assert_latest_store_rows(&owner);
        assert!(scalar_reads > 0);
        assert_eq!(array_reads, 1);
        let rows = &owner.helper_memory.unit_source;
        let kills = rows
            .memory
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                matches!(row,
            UnitLocalMemoryRowV1::Invalidate { local, .. } if local.index() == 2)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kills.len(),
            if matches!(kill, ScalarKill::Storage) {
                2
            } else {
                1
            }
        );
        for (_, row) in &kills {
            let UnitLocalMemoryRowV1::Invalidate { reason, .. } = row else {
                unreachable!()
            };
            assert!(matches!(
                (kill, reason),
                (ScalarKill::Move, UnitLocalInvalidateV1::Move)
                    | (
                        ScalarKill::Storage,
                        UnitLocalInvalidateV1::StorageLive | UnitLocalInvalidateV1::StorageDead
                    )
                    | (
                        ScalarKill::Deinitialize,
                        UnitLocalInvalidateV1::Deinitialize
                    )
            ));
        }
        let last_kill = kills.last().unwrap().0;
        let mut later_reads = 0;
        for row in &rows.memory[last_kill + 1..] {
            if let UnitLocalMemoryRowV1::Access {
                local,
                physical_access,
                latest_source_store,
                ..
            } = row
                && local.index() == 2
                && owner.helper_memory.accesses[*physical_access].kind()
                    == LocalFrameAccessKindV1::Read
            {
                assert!(latest_source_store.unwrap() > last_kill);
                later_reads += 1;
            }
        }
        assert!(later_reads > 0);
    }
}

#[test]
fn verified_native_store_value_and_initialized_cell_substitutions_are_not_source_proof() {
    for wrong_cell in [false, true] {
        with_pending_candidate(
            UnitCase::ReadThenWrite,
            |module| {
                let helper = module
                    .functions
                    .iter_mut()
                    .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
                    .unwrap();
                let body = helper.body.as_mut().unwrap();
                assert_eq!(body.blocks.len(), 1);
                let operations = &mut body.blocks[0].operations;
                if wrong_cell {
                    let second_cell = operations
                        .iter()
                        .filter_map(|operation| match operation.kind {
                            OperationKind::Store { pointer, .. } => Some(pointer),
                            _ => None,
                        })
                        .nth(1)
                        .unwrap();
                    let load = operations
                        .iter_mut()
                        .find(|operation| matches!(operation.kind, OperationKind::Load { .. }))
                        .unwrap();
                    let OperationKind::Load { pointer, .. } = &mut load.kind else {
                        unreachable!()
                    };
                    assert_ne!(*pointer, second_cell);
                    *pointer = second_cell;
                } else {
                    let constant = operations
                        .iter_mut()
                        .find(|operation| {
                            matches!(
                                operation.kind,
                                OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(99))
                            )
                        })
                        .unwrap();
                    constant.kind = OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(98));
                }
            },
            |subject, origins, budget| {
                let floor = budget.storage();
                assert!(matches!(
                    SealedHelperMemoryV1::derive_with_origins_v1(subject, Some(origins), budget),
                    Err(ProductionPreRankedKirErrorV1::Lowering(
                        ProductionSemanticKirErrorV1::CorrespondenceMismatch
                    ))
                ));
                assert_eq!(budget.storage(), floor);
            },
        );
    }
}

fn killed_source_error(error: &ProductionPreRankedKirErrorV1) -> bool {
    matches!(
        error,
        ProductionPreRankedKirErrorV1::Lowering(ProductionSemanticKirErrorV1::Unsupported {
            function: 1,
            block: None,
            statement: None,
            detail: "local helper source cell is not initialized",
        })
    )
}

#[test]
fn physical_initialization_cannot_resurrect_a_source_cell_after_any_source_only_kill() {
    for kill in [
        ScalarKill::Move,
        ScalarKill::Storage,
        ScalarKill::Deinitialize,
    ] {
        with_pending_candidate(
            UnitCase::Killed(kill),
            |_| {},
            |subject, origins, budget| {
                let floor = budget.storage();
                let physical = SealedHelperMemoryV1::derive(subject, budget).unwrap();
                let receipt = physical.storage.retained_storage();
                budget.reserve_storage(receipt).unwrap();
                assert!(physical.unit_source.is_empty());
                let mut reads = 0;
                for access in &physical.accesses {
                    if access.kind() == LocalFrameAccessKindV1::Read {
                        assert!(access.initializing_store().is_some());
                        reads += 1;
                    }
                }
                assert!(reads > 0);
                drop(physical);
                budget.release_storage(receipt).unwrap();
                assert_eq!(budget.storage(), floor);
                let error =
                    SealedHelperMemoryV1::derive_with_origins_v1(subject, Some(origins), budget)
                        .unwrap_err();
                assert!(killed_source_error(&error), "{error:?}");
                assert_eq!(budget.storage(), floor);
            },
        );
        let (ssa, launch) = unit_source(UnitCase::Killed(kill), &[1]);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let error = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                ProductionPreRankedKirErrorV1::Lowering(ProductionSemanticKirErrorV1::MissingLocalDefinition {
                    function: 1, block: 0, statement: Some(statement), local: 2,
                }) if statement == if matches!(kill, ScalarKill::Storage) { 3 } else { 2 }
            ),
            "{error:?}"
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}
