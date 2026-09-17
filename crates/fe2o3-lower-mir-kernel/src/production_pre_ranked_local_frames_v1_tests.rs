use super::assert_origins_v1_tests::{Fixture, materialize};
use super::*;
use fe2o3_kernel_analysis::{CanonicalKirCallEffectDecisionV1, CanonicalKirCallEffectsV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, CastKind, LocalFrameAccessKindV1, LocalFrameErrorV1,
    LocalFrameRefusalReasonV1, MemoryAccess, VerifiedCanonicalKernelIrModuleV12,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

const FLOOR: usize = 19;

fn accounting(error: ProductionSemanticKirErrorV1) -> bool {
    matches!(
        error,
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            ArgumentResourceV1::Accounting
        )
    )
}

fn with_source(
    shared: bool,
    next: impl FnOnce(
        &ProductionPreRankedKirOwnerV1,
        &CanonicalKirInventoryV1<'_>,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize(Fixture::Literal(true), shared, &mut budget);
    assert_eq!(budget.storage(), FLOOR);
    let total = owner.retained_analysis_storage_v1();
    assert_eq!(
        total,
        owner.executable_storage().retained_storage()
            + owner.assert_origin_storage().payload_storage()
            + owner.helper_memory_storage_v1().retained_storage()
    );
    budget.reserve_storage(total).unwrap();
    let (inventory, receipt) =
        CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    next(&owner, &inventory, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(total).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn genuine_shared_helpers_keep_every_source_association_and_true_empty_membership() {
    with_source(true, |owner, inventory, budget| {
        let helper_rows = owner
            .correspondence
            .lowered_functions
            .iter()
            .enumerate()
            .filter(|(_, row)| row.role == SemanticKirFunctionRoleV1::InternalHelper)
            .collect::<Vec<_>>();
        assert_eq!(helper_rows.len(), 3);
        assert_eq!(
            owner
                .helper_memory
                .functions
                .iter()
                .filter(|state| **state == RetainedHelperKindV1::RawEmpty)
                .count(),
            2
        );
        assert!(owner.helper_memory.allocations.is_empty());
        assert!(owner.helper_memory.accesses.is_empty());
        assert!(owner.helper_memory.control.is_empty());
        assert!(owner.helper_memory.edge_bindings.is_empty());
        let empty = owner.empty_effect_helpers().iter().collect::<Vec<_>>();
        assert_eq!(empty.len(), helper_rows.len());
        for ((_, expected), actual) in helper_rows.iter().zip(empty) {
            assert!(std::ptr::eq(*expected, actual));
        }
        let rows = &owner.helper_memory;
        let expected_bytes = std::mem::size_of::<SealedHelperMemoryV1>()
            + rows.functions.capacity() * std::mem::size_of::<RetainedHelperKindV1>()
            + rows.associations.capacity() * std::mem::size_of::<RetainedHelperAssociationV1>()
            + rows.allocations.capacity() * std::mem::size_of::<RetainedLocalAllocationV1>()
            + rows.accesses.capacity() * std::mem::size_of::<RetainedLocalAccessV1>()
            + rows.control.capacity() * std::mem::size_of::<RetainedLocalControlV1>()
            + rows.edge_bindings.capacity() * std::mem::size_of::<RetainedLocalEdgeBindingV1>();
        assert_eq!(rows.storage.retained_storage(), expected_bytes);
        assert!(std::ptr::eq(
            owner.executable().verified_module_ref_v1().module(),
            owner.executable().module()
        ));
        owner
            .with_checked_helper_memory_v1(inventory, budget, |memory, budget| {
                assert_eq!(
                    memory.association_count(),
                    owner.correspondence.lowered_functions.len()
                );
                for (index, _) in &helper_rows {
                    assert!(memory.local_frame(*index, budget)?.is_none());
                }
                let root = owner
                    .correspondence
                    .lowered_functions
                    .iter()
                    .position(|row| row.role != SemanticKirFunctionRoleV1::InternalHelper)
                    .unwrap();
                assert!(matches!(
                    memory.local_frame(root, budget),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
                Ok(())
            })
            .unwrap();
        owner
            .with_checked_canonical_calls_v1(inventory, budget, |calls, budget| {
                assert_eq!(calls.call_count(), 3);
                budget.charge_work(calls.call_count())?;
                for (root, call) in calls.sites() {
                    calls.with_call_local_frame_v1(root, call, budget, |view, local| {
                        assert!(local.is_none());
                        assert_eq!(view.caller().correspondence_owner(), root);
                        assert!(std::ptr::eq(
                            view.operation(),
                            inventory.calls()[call].operation
                        ));
                        assert_eq!(view.result_count(), 0);
                        assert_eq!(view.result_source_type(), SemanticTypeIdV1::from_index(0));
                        Ok(())
                    })?;
                }
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn no_helper_census_has_source_derived_work_header_and_cached_sum_boundaries() {
    with_source(false, |owner, _, _| {
        let j = owner.correspondence.lowered_functions.len();
        let f = owner.executable.module().functions.len();
        let exact = 4 + 2 * j + 2 * f + 2;
        let header = std::mem::size_of::<SealedHelperMemoryV1>();
        for (work_limit, storage_limit, success) in [
            (exact, FLOOR + header, true),
            (exact - 1, FLOOR + header, false),
            (exact, FLOOR + header - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result =
                SealedHelperMemoryV1::derive(CanonicalCallSubjectV1::owner(owner), &mut budget);
            if success {
                let rows = result.unwrap();
                assert_eq!(rows.storage.retained_storage(), header);
                assert!(
                    rows.functions.is_empty()
                        && rows.associations.is_empty()
                        && rows.allocations.is_empty()
                        && rows.accesses.is_empty()
                        && rows.control.is_empty()
                        && rows.edge_bindings.is_empty()
                );
                assert_eq!(budget.work(), exact);
                assert_eq!(budget.peak_storage(), FLOOR + header);
                drop(rows);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionPreRankedKirErrorV1::Lowering(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_)
                    ))
                ));
            }
            assert_eq!(budget.storage(), FLOOR);
        }
        assert_eq!(helper_memory_live_storage_v1(3, 5, 7), Ok(15));
        assert!(matches!(
            helper_memory_live_storage_v1(usize::MAX, 1, 0),
            Err(ArgumentResourceV1::Arithmetic)
        ));
        assert!(matches!(
            helper_memory_live_storage_v1(usize::MAX - 1, 1, 1),
            Err(ArgumentResourceV1::Arithmetic)
        ));
    });
}

#[test]
fn both_helper_absence_censuses_are_required_before_the_fast_path() {
    with_source(true, |owner, _, _| {
        let mut rows = owner.correspondence.clone();
        rows.lowered_functions = rows
            .lowered_functions
            .iter()
            .filter(|row| row.role != SemanticKirFunctionRoleV1::InternalHelper)
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut setup_work = Work::new(usize::MAX);
        let mut setup = ArgumentBudgetV1::new(&mut setup_work, usize::MAX);
        let no_helpers = materialize(Fixture::Literal(true), false, &mut setup);
        // Hostile borrowed input components, not reconstructed source owners.
        for subject in [
            CanonicalCallSubjectV1 {
                semantic_ssa: &owner.semantic_ssa,
                executable: owner.executable(),
                correspondence: &rows,
            },
            CanonicalCallSubjectV1 {
                semantic_ssa: &owner.semantic_ssa,
                executable: no_helpers.executable(),
                correspondence: &owner.correspondence,
            },
        ] {
            let exact = 4
                + 2 * subject.correspondence.lowered_functions.len()
                + 2 * subject.executable.module().functions.len()
                + 2;
            let mut work = Work::new(exact);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let result = SealedHelperMemoryV1::derive(subject, &mut budget);
            assert!(matches!(
                result,
                Err(ProductionPreRankedKirErrorV1::Lowering(
                    ProductionSemanticKirErrorV1::CorrespondenceMismatch
                ))
            ));
            assert_eq!((budget.work(), budget.storage()), (exact, FLOOR));
        }
    });
}

#[test]
fn retained_builder_header_and_both_roster_reservations_restore_the_real_floor() {
    with_source(true, |owner, _, _| {
        let f = owner.executable.module().functions.len();
        let j = owner.correspondence.lowered_functions.len();
        let header = std::mem::size_of::<SealedHelperMemoryV1>();
        let function_bytes = f * std::mem::size_of::<RetainedHelperKindV1>();
        let mut capacity_probe = Vec::<RetainedHelperKindV1>::new();
        capacity_probe.try_reserve_exact(f).unwrap();
        let actual_function_bytes =
            capacity_probe.capacity() * std::mem::size_of::<RetainedHelperKindV1>();
        drop(capacity_probe);
        let association_bytes = j * std::mem::size_of::<RetainedHelperAssociationV1>();
        let census = 4 + 2 * j + 2 * f + 2;
        let floor = FLOOR + owner.retained_analysis_storage_v1();
        for (extra, work_expected, attempted) in [
            (header - 1, 4, floor + header),
            (
                header + function_bytes - 1,
                census + 3,
                floor + header + function_bytes,
            ),
            (
                header + actual_function_bytes + association_bytes - 1,
                census + 6,
                floor + header + actual_function_bytes + association_bytes,
            ),
        ] {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, floor + extra);
            budget.reserve_storage(floor).unwrap();
            let result =
                SealedHelperMemoryV1::derive(CanonicalCallSubjectV1::owner(owner), &mut budget);
            assert!(matches!(
                result,
                Err(ProductionPreRankedKirErrorV1::Lowering(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                ))
            ));
            assert_eq!(budget.work(), work_expected);
            assert_eq!(budget.failed_storage(), Some(attempted));
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn helper_scope_rejects_foreign_inventory_ledger_and_missing_live_receipt() {
    with_source(true, |owner, inventory, budget| {
        let mut foreign_work = Work::new(usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
        let foreign_owner = materialize(Fixture::Literal(true), true, &mut foreign);
        let (foreign_inventory, _) =
            CanonicalKirInventoryV1::derive(foreign_owner.executable(), &mut foreign).unwrap();
        let mut entered = false;
        let result = owner.with_checked_helper_memory_v1(&foreign_inventory, budget, |_, _| {
            entered = true;
            Ok(())
        });
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert!(!entered);
        let helper = owner
            .correspondence
            .lowered_functions
            .iter()
            .position(|row| row.role == SemanticKirFunctionRoleV1::InternalHelper)
            .unwrap();
        owner
            .with_checked_helper_memory_v1(inventory, budget, |memory, budget| {
                foreign.reserve_storage(budget.storage())?;
                let before = foreign.work();
                assert!(accounting(
                    memory.local_frame(helper, &mut foreign).err().unwrap()
                ));
                assert_eq!(foreign.work() - before, 5);
                assert!(memory.local_frame(helper, budget)?.is_none());
                Ok(())
            })
            .unwrap();
        let saved = budget.storage();
        let short = owner.retained_analysis_storage_v1() - 1;
        budget.release_storage(saved - short).unwrap();
        let result = owner.with_checked_helper_memory_v1(inventory, budget, |_, _| {
            entered = true;
            Ok(())
        });
        assert!(accounting(result.err().unwrap()));
        assert!(!entered);
        assert_eq!(budget.storage(), short);
        budget.reserve_storage(saved - short).unwrap();
    });
}

#[test]
fn helper_scope_preserves_body_error_panic_and_detects_storage_tampering() {
    with_source(true, |owner, inventory, budget| {
        let floor = budget.storage();
        let mut entered = false;
        let result: Result<(), _> =
            owner.with_checked_helper_memory_v1(inventory, budget, |_, _| {
                entered = true;
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            });
        assert!(
            entered
                && matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                )
        );
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(), _> =
                owner.with_checked_helper_memory_v1(inventory, budget, |_, _| {
                    panic!("retained helper original panic");
                });
        }))
        .unwrap_err();
        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some("retained helper original panic")
        );
        assert_eq!(budget.storage(), floor);
        for grow in [false, true] {
            for mode in 0..3 {
                entered = false;
                let result: Result<(), _> =
                    owner.with_checked_helper_memory_v1(inventory, budget, |_, budget| {
                        entered = true;
                        if grow {
                            budget.reserve_storage(1)?;
                        } else {
                            budget.release_storage(1)?;
                        }
                        match mode {
                            0 => Ok(()),
                            1 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                            _ => panic!("tampered retained helper panic"),
                        }
                    });
                assert!(entered && accounting(result.err().unwrap()));
                assert_eq!(budget.storage(), if grow { floor } else { floor - 1 });
                if !grow {
                    budget.reserve_storage(1).unwrap();
                }
                owner
                    .with_checked_helper_memory_v1(inventory, budget, |_, _| Ok(()))
                    .unwrap();
            }
        }
    });
}

#[test]
fn joined_call_cannot_reset_its_preparation_ledger_or_omit_index_storage() {
    with_source(true, |owner, inventory, budget| {
        let outer = budget.storage();
        owner
            .with_checked_canonical_calls_v1(inventory, budget, |calls, budget| {
                let (root, call) = calls.sites().next().unwrap();
                let full = budget.storage();
                assert!(full > outer);
                for numeric_floor in [owner.retained_analysis_storage_v1(), full] {
                    for work_limit in [4, 5] {
                        let mut foreign_work = Work::new(work_limit);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, numeric_floor);
                        foreign.reserve_storage(numeric_floor)?;
                        let mut entered = false;
                        let result =
                            calls.with_call_local_frame_v1(root, call, &mut foreign, |_, _| {
                                entered = true;
                                Ok(())
                            });
                        assert!(!entered);
                        if work_limit == 4 {
                            assert!(matches!(result,
                                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                    ArgumentResourceV1::Work(error)
                                )) if error.actual() == 5 && error.limit() == 4
                            ));
                            assert_eq!(foreign.work(), 0);
                        } else {
                            assert!(accounting(result.err().unwrap()));
                            assert_eq!(foreign.work(), 5);
                        }
                        assert_eq!(foreign.storage(), numeric_floor);
                    }
                }
                budget.release_storage(full - outer)?;
                let mut entered = false;
                let result = calls.with_call_local_frame_v1(root, call, budget, |_, _| {
                    entered = true;
                    Ok(())
                });
                assert!(!entered && accounting(result.err().unwrap()));
                assert_eq!(budget.storage(), outer);
                budget.reserve_storage(full - outer)?;
                calls.with_call_local_frame_v1(root, call, budget, |_, local| {
                    assert!(local.is_none());
                    Ok(())
                })?;
                assert_eq!(budget.storage(), full);
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn joined_call_preserves_errors_panics_and_nested_floor_accounting_precedence() {
    with_source(true, |owner, inventory, budget| {
        owner
            .with_checked_canonical_calls_v1(inventory, budget, |calls, budget| {
                let (root, call) = calls.sites().next().unwrap();
                let floor = budget.storage();
                let mut entered = false;
                let error: Result<(), _> =
                    calls.with_call_local_frame_v1(root, call, budget, |_, local| {
                        entered = true;
                        assert!(local.is_none());
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    });
                assert!(
                    entered
                        && matches!(
                            error,
                            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                        )
                );
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    let _: Result<(), _> =
                        calls.with_call_local_frame_v1(root, call, budget, |_, _| {
                            panic!("joined call original panic");
                        });
                }))
                .unwrap_err();
                assert_eq!(
                    panic.downcast_ref::<&str>().copied(),
                    Some("joined call original panic")
                );
                assert_eq!(budget.storage(), floor);
                // The Unit result has one shape node. This is the exact existing
                // prepay_typed_shape reservation before parameter scope entry.
                let parameter_floor = floor
                    + std::mem::size_of::<SemanticKirParameterProjectionV1>()
                    + std::mem::size_of::<ProductionArgumentProjectionV1>()
                    + 512
                    + 2048;
                assert!(parameter_floor > floor);
                // These private hostile callbacks can reach storage unavailable
                // through the public view. Distinguish both actual nested floors.
                for position in 0..6 {
                    for mode in 0..3 {
                        entered = false;
                        let result: Result<(), _> =
                            calls.with_call_local_frame_v1(root, call, budget, |view, _| {
                                entered = true;
                                let active = view.entry.budget.storage();
                                assert!(active > parameter_floor);
                                match position {
                                    0 => view.entry.budget.release_storage(active - floor)?,
                                    1 => view.entry.budget.release_storage(1)?,
                                    2 => view.entry.budget.reserve_storage(1)?,
                                    3 => view
                                        .entry
                                        .budget
                                        .release_storage(active - parameter_floor)?,
                                    4 => view
                                        .entry
                                        .budget
                                        .release_storage(active - (parameter_floor - 1))?,
                                    _ => view.entry.budget.release_storage(active - (floor - 1))?,
                                }
                                match mode {
                                    0 => Ok(()),
                                    1 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                                    _ => panic!("joined call tampered panic"),
                                }
                            });
                        assert!(entered && accounting(result.err().unwrap()));
                        assert_eq!(
                            budget.storage(),
                            if position == 5 { floor - 1 } else { floor }
                        );
                        if position == 5 {
                            // Only the test restores the stolen byte, after
                            // proving production did not silently recreate it.
                            budget.reserve_storage(1)?;
                        }
                        calls.with_call_local_frame_v1(root, call, budget, |_, local| {
                            assert!(local.is_none());
                            Ok(())
                        })?;
                    }
                }
                Ok(())
            })
            .unwrap();
    });
}

fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn pointer() -> Type {
    Type::pointer(scalar(), AddressSpace::Private, AccessMode::ReadWrite)
}
fn constant(id: u32, value: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}

fn local_component(cast_index: bool) -> Module {
    let mut block = BasicBlock::new(BlockId(7));
    block.operations = vec![
        constant(0, Constant::U32(1)),
        Operation::effect_free(
            ValueDef::new(ValueId(1), pointer()),
            OperationKind::Alloca {
                element: scalar(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar()),
            OperationKind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ];
    if cast_index {
        block.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(3), Type::INDEX),
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    value: ValueId(2),
                    to: Type::INDEX,
                },
            ),
            constant(4, Constant::Index(2)),
            Operation::effect_free(
                ValueDef::new(ValueId(5), pointer()),
                OperationKind::Alloca {
                    element: scalar(),
                    count: Some(ValueId(4)),
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(6), pointer()),
                OperationKind::GetElementPointer {
                    base: ValueId(5),
                    offset: ValueId(3),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(6),
                    value: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(7), scalar()),
                OperationKind::Load {
                    pointer: ValueId(6),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ]);
    }
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(if cast_index { 7 } else { 2 })],
    });
    let mut module = Module::new("retained_local_component");
    module.functions.push(Function::internal_helper(
        "local",
        Signature::new(vec![], vec![scalar()]),
        vec![],
        vec![block],
    ));
    module
}

fn with_physical_component(
    module: Module,
    next: impl FnOnce(
        &VerifiedCanonicalKernelIrModuleV12,
        &CanonicalKirInventoryV1<'_>,
        &CanonicalKirCallEffectsV1<'_, '_>,
    ),
) {
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let (executable, executable_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    drop(module);
    budget
        .reserve_storage(executable_storage.retained_storage())
        .unwrap();
    assert!(std::ptr::eq(
        executable.verified_module_ref_v1().module(),
        executable.module()
    ));
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&executable, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let (effects, effect_storage) =
        CanonicalKirCallEffectsV1::derive(&inventory, &mut budget).unwrap();
    budget
        .reserve_storage(effect_storage.retained_storage())
        .unwrap();
    next(&executable, &inventory, &effects);
    drop(effects);
    budget
        .release_storage(effect_storage.retained_storage())
        .unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(executable);
    budget
        .release_storage(executable_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn verified_physical_components_copy_full_local_obligations_without_purity() {
    for cast in [false, true] {
        let mut module = local_component(cast);
        let mut second = module.functions[0].clone();
        second.id = FunctionId::new("second");
        module.functions.push(second);
        with_physical_component(module, |executable, inventory, effects| {
            // This component meter does not reconstruct a source owner or
            // claim these helpers passed the existing admission gate.
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let mut states = vec![RetainedHelperKindV1::Pending; 2];
            for row in inventory.functions() {
                assert_eq!(
                    effects.decision(row.coordinate, &mut budget).unwrap(),
                    CanonicalKirCallEffectDecisionV1::CompleteNonempty
                );
            }
            let (allocations, accesses, control, edge_bindings) =
                derive_retained_helper_physical_rows_v1(
                    executable,
                    inventory,
                    effects,
                    &mut states,
                    &mut budget,
                )
                .unwrap();
            let a = if cast { 2 } else { 1 };
            let m = if cast { 4 } else { 2 };
            assert_eq!((allocations.len(), accesses.len()), (2 * a, 2 * m));
            assert_eq!((control.len(), edge_bindings.len()), (2, 0));
            let bytes = allocations.capacity() * std::mem::size_of::<RetainedLocalAllocationV1>()
                + accesses.capacity() * std::mem::size_of::<RetainedLocalAccessV1>()
                + control.capacity() * std::mem::size_of::<RetainedLocalControlV1>()
                + edge_bindings.capacity() * std::mem::size_of::<RetainedLocalEdgeBindingV1>();
            assert_eq!(budget.storage(), FLOOR + bytes);
            for ordinal in 0..2 {
                assert_eq!(
                    states[ordinal],
                    RetainedHelperKindV1::Local {
                        allocations: (ordinal * a, (ordinal + 1) * a),
                        accesses: (ordinal * m, (ordinal + 1) * m),
                        control: (ordinal, ordinal + 1),
                        edge_bindings: (0, 0),
                    }
                );
                for allocation in &allocations[ordinal * a..(ordinal + 1) * a] {
                    assert_eq!(allocation.location().function_ordinal(), ordinal);
                }
                for pair in accesses[ordinal * m..(ordinal + 1) * m].chunks_exact(2) {
                    assert_eq!(pair[0].kind(), LocalFrameAccessKindV1::Write);
                    assert_eq!(pair[1].kind(), LocalFrameAccessKindV1::Read);
                    assert_eq!(pair[1].initializing_store(), Some(pair[0].location()));
                    assert_eq!(pair[0].location().function_ordinal(), ordinal);
                }
                if cast {
                    assert_eq!(allocations[ordinal * a + 1].count(), 2);
                    assert_eq!(accesses[ordinal * m + 2].cell(), 1);
                    assert_eq!(accesses[ordinal * m + 2].allocation(), 1);
                }
                assert_eq!(control[ordinal].function_ordinal(), ordinal);
                assert_eq!(control[ordinal].block(), BlockId(7));
                assert_eq!(
                    control[ordinal].kind(),
                    fe2o3_kernel_ir::LocalFrameControlKindV1::Return
                );
            }
            drop(edge_bindings);
            drop(control);
            drop(accesses);
            drop(allocations);
            budget.release_storage(bytes).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        });
    }
}

#[test]
fn physical_census_cannot_skip_real_helpers_and_typed_refusal_keeps_location() {
    with_physical_component(local_component(false), |executable, inventory, effects| {
        let mut work = Work::new(4);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let result = derive_retained_helper_physical_rows_v1(
            executable,
            inventory,
            effects,
            &mut [RetainedHelperKindV1::NotHelper],
            &mut budget,
        );
        assert!(matches!(
            result,
            Err(ProductionPreRankedKirErrorV1::Lowering(
                ProductionSemanticKirErrorV1::CorrespondenceMismatch
            ))
        ));
        assert_eq!((budget.work(), budget.storage()), (4, FLOOR));
    });
    let mut module = local_component(false);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .swap(2, 3);
    with_physical_component(module, |executable, inventory, effects| {
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let result = derive_retained_helper_physical_rows_v1(
            executable,
            inventory,
            effects,
            &mut [RetainedHelperKindV1::Pending],
            &mut budget,
        );
        match result {
            Err(ProductionPreRankedKirErrorV1::LocalFrame(LocalFrameErrorV1::Unsupported {
                function_ordinal,
                operation,
                reason,
            })) => {
                assert_eq!(function_ordinal, 0);
                assert_eq!(operation, Some(2));
                assert_eq!(reason, LocalFrameRefusalReasonV1::UninitializedRead);
            }
            other => panic!("expected exact local initialization refusal: {other:?}"),
        }
        // This private copy routine relies on its enclosing constructor scope
        // for destination rollback. Its producer scope has already dropped.
        assert!(budget.storage() > FLOOR);
        budget.release_storage(budget.storage() - FLOOR).unwrap();
    });
}

#[test]
fn physical_copy_meter_has_literal_partial_copy_and_final_check_boundaries() {
    with_physical_component(local_component(false), |executable, inventory, effects| {
        // Core: wrapper/lookup/chain18 + signature1 + census9 + reserve14 +
        // publish15 + sort/window26 + visit/lookup9 + epochs23 + four ops123 +
        // dispatch4 + return6 + block census1 + cell sort/scan34 = 283.
        // Retention: role4 + decision1 + body6 + block3 + operations12 +
        // reserve12 + row1 + callback12 + queries6/7/6/5 + copies3/4/4/3 + final5.
        const CORE: usize = 18 + 1 + 9 + 14 + 15 + 26 + 9 + 23 + 123 + 4 + 6 + 1 + 34;
        const EXACT: usize =
            4 + 1 + 6 + 3 + 12 + 12 + 1 + CORE + 12 + 6 + 7 + 6 + 5 + 3 + 4 + 4 + 3 + 5;
        assert_eq!((CORE, EXACT), (283, 377));
        for limit in [368, EXACT - 1, EXACT] {
            let mut work = Work::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let result = derive_retained_helper_physical_rows_v1(
                executable,
                inventory,
                effects,
                &mut [RetainedHelperKindV1::Pending],
                &mut budget,
            );
            match (limit, result) {
                (
                    368,
                    Err(ProductionPreRankedKirErrorV1::LocalFrame(LocalFrameErrorV1::Resource(
                        ArgumentResourceV1::Work(error),
                    ))),
                ) => {
                    assert_eq!(
                        (error.actual(), error.limit(), budget.work()),
                        (369, 368, 365)
                    );
                }
                (
                    376,
                    Err(ProductionPreRankedKirErrorV1::Lowering(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(error),
                        ),
                    )),
                ) => {
                    assert_eq!(
                        (error.actual(), error.limit(), budget.work()),
                        (377, 376, 372)
                    );
                }
                (377, Ok((allocations, accesses, control, edge_bindings))) => {
                    assert_eq!(
                        (allocations.len(), accesses.len(), budget.work()),
                        (1, 2, EXACT)
                    );
                    assert_eq!((control.len(), edge_bindings.len()), (1, 0));
                    drop(edge_bindings);
                    drop(control);
                    drop(accesses);
                    drop(allocations);
                }
                (_, result) => panic!("wrong derived copy prefix: {result:?}"),
            }
            let retained = budget.storage() - FLOOR;
            assert!(retained > 0);
            assert!(budget.peak_storage() > budget.storage());
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    });
}

#[test]
fn physical_output_capacity_is_reserved_before_entering_the_local_classifier() {
    with_physical_component(local_component(false), |executable, inventory, effects| {
        let allocation_bytes = std::mem::size_of::<RetainedLocalAllocationV1>();
        let access_bytes = 2 * std::mem::size_of::<RetainedLocalAccessV1>();
        let mut probe = Vec::<RetainedLocalAllocationV1>::new();
        probe.try_reserve_exact(1).unwrap();
        let actual_allocation_bytes =
            probe.capacity() * std::mem::size_of::<RetainedLocalAllocationV1>();
        drop(probe);
        // Census26 precedes reservation3; the second reservation adds3.
        for (limit, work_expected, attempted, retained) in [
            (
                FLOOR + allocation_bytes - 1,
                29,
                FLOOR + allocation_bytes,
                0,
            ),
            (
                FLOOR + actual_allocation_bytes + access_bytes - 1,
                32,
                FLOOR + actual_allocation_bytes + access_bytes,
                actual_allocation_bytes,
            ),
        ] {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = derive_retained_helper_physical_rows_v1(
                executable,
                inventory,
                effects,
                &mut [RetainedHelperKindV1::Pending],
                &mut budget,
            );
            assert!(matches!(
                result,
                Err(ProductionPreRankedKirErrorV1::Lowering(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                ))
            ));
            assert_eq!(
                (budget.work(), budget.failed_storage()),
                (work_expected, Some(attempted))
            );
            assert_eq!(budget.storage(), FLOOR + retained);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    });
}

fn assert_retained_query_same_slot_refusal(joined: bool) {
    // Both borrowed work meters exist before the fixed-'w public scope.
    let mut original_work = Work::new(usize::MAX);
    let mut replacement_work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut original_work, usize::MAX);
    let mut other = ArgumentBudgetV1::new(&mut replacement_work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize(Fixture::Literal(true), true, &mut budget);
    let retained = owner.retained_analysis_storage_v1();
    budget.reserve_storage(retained).unwrap();
    let (inventory, receipt) =
        CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    if joined {
        owner
            .with_checked_canonical_calls_v1(&inventory, &mut budget, |calls, budget| {
                let (root, call) = calls.sites().next().unwrap();
                calls.with_call_local_frame_v1(root, call, budget, |_, local| {
                    assert!(local.is_none());
                    Ok(())
                })?;
                let full = budget.storage();
                let original_prefix = budget.work();
                let slot = budget as *const ArgumentBudgetV1<'_>;
                other.reserve_storage(full)?;
                std::mem::swap(budget, &mut other);
                assert_eq!(budget as *const ArgumentBudgetV1<'_>, slot);
                let mut entered = false;
                let result = calls.with_call_local_frame_v1(root, call, budget, |_, _| {
                    entered = true;
                    Ok(())
                });
                let replacement_floor = budget.storage();
                std::mem::swap(budget, &mut other);
                assert!(!entered, "foreign meter must not enter the call view");
                assert!(result.err().is_some_and(accounting));
                assert_eq!(other.work(), 5);
                assert_eq!(budget.work(), original_prefix);
                assert_eq!((budget.storage(), replacement_floor), (full, full));
                assert_eq!(other.storage(), full);
                other.release_storage(full)?;
                calls.with_call_local_frame_v1(root, call, budget, |_, local| {
                    assert!(local.is_none());
                    Ok(())
                })
            })
            .unwrap();
    } else {
        let helper = owner
            .empty_effect_helpers()
            .iter()
            .next()
            .map(|helper| {
                owner
                    .correspondence
                    .lowered_functions
                    .iter()
                    .position(|row| std::ptr::eq(row, helper))
                    .unwrap()
            })
            .unwrap();
        owner
            .with_checked_helper_memory_v1(&inventory, &mut budget, |memory, budget| {
                assert!(memory.local_frame(helper, budget)?.is_none());
                let full = budget.storage();
                let original_prefix = budget.work();
                let slot = budget as *const ArgumentBudgetV1<'_>;
                other.reserve_storage(full)?;
                std::mem::swap(budget, &mut other);
                assert_eq!(budget as *const ArgumentBudgetV1<'_>, slot);
                let result = memory.local_frame(helper, budget);
                let rejected = result.err().is_some_and(accounting);
                let replacement_floor = budget.storage();
                std::mem::swap(budget, &mut other);
                assert!(rejected, "same slot is not the original work meter");
                assert_eq!(other.work(), 5);
                assert_eq!(budget.work(), original_prefix);
                assert_eq!((budget.storage(), replacement_floor), (full, full));
                assert_eq!(other.storage(), full);
                other.release_storage(full)?;
                assert!(memory.local_frame(helper, budget)?.is_none());
                Ok(())
            })
            .unwrap();
    }
    assert_eq!((budget.storage(), other.storage()), (floor, 0));
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn retained_helper_query_rejects_same_slot_foreign_work() {
    assert_retained_query_same_slot_refusal(false);
}

#[test]
fn retained_joined_call_rejects_same_slot_foreign_work() {
    assert_retained_query_same_slot_refusal(true);
}

#[test]
fn retained_scope_exit_rejects_same_slot_swap_without_query_or_foreign_cleanup() {
    for joined in [false, true] {
        for mode in 0..3 {
            let mut original_work = Work::new(usize::MAX);
            let mut replacement_work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut original_work, usize::MAX);
            let mut other = ArgumentBudgetV1::new(&mut replacement_work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let owner = materialize(Fixture::Literal(true), true, &mut budget);
            let retained = owner.retained_analysis_storage_v1();
            budget.reserve_storage(retained).unwrap();
            let (inventory, receipt) =
                CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let floor = budget.storage();
            let mut entered = false;
            let mut callback_floor = 0;
            let mut original_prefix = 0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                if joined {
                    owner.with_checked_canonical_calls_v1(
                        &inventory,
                        &mut budget,
                        |_calls, budget| {
                            entered = true;
                            callback_floor = budget.storage();
                            original_prefix = budget.work();
                            other.reserve_storage(callback_floor)?;
                            std::mem::swap(budget, &mut other);
                            match mode {
                                0 => Ok(()),
                                1 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                                _ => std::panic::panic_any(0x367usize),
                            }
                        },
                    )
                } else {
                    owner.with_checked_helper_memory_v1(
                        &inventory,
                        &mut budget,
                        |_memory, budget| {
                            entered = true;
                            callback_floor = budget.storage();
                            original_prefix = budget.work();
                            other.reserve_storage(callback_floor)?;
                            std::mem::swap(budget, &mut other);
                            match mode {
                                0 => Ok(()),
                                1 => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                                _ => std::panic::panic_any(0x367usize),
                            }
                        },
                    )
                }
            }));
            assert!(entered);
            let returned = match result {
                Ok(result) => result,
                Err(_) => panic!("foreign-ledger postflight must suppress the body panic"),
            };
            assert!(returned.err().is_some_and(accounting));
            assert_eq!(
                budget.work(),
                0,
                "postflight was prepaid on the original meter"
            );
            assert_eq!(other.work(), original_prefix);
            assert_eq!(budget.peak_storage(), callback_floor);
            assert_eq!(budget.failed_storage(), None);
            assert_eq!(
                budget.storage(),
                callback_floor,
                "no foreign storage cleanup"
            );
            assert_eq!(
                other.storage(),
                callback_floor,
                "no access to the displaced ledger"
            );
            if joined {
                assert!(
                    callback_floor > floor,
                    "genuine call-index scratch was live"
                );
            } else {
                assert_eq!(callback_floor, floor);
            }
            std::mem::swap(&mut budget, &mut other);
            // Only the test restores ownership and releases the now-dead scratch.
            budget.release_storage(callback_floor - floor).unwrap();
            other.release_storage(callback_floor).unwrap();
            owner
                .with_checked_canonical_calls_v1(&inventory, &mut budget, |calls, budget| {
                    let (root, call) = calls.sites().next().unwrap();
                    calls.with_call_local_frame_v1(root, call, budget, |_, local| {
                        assert!(local.is_none());
                        Ok(())
                    })
                })
                .unwrap();
            assert_eq!((budget.storage(), other.storage()), (floor, 0));
            drop(inventory);
            budget.release_storage(receipt.retained_storage()).unwrap();
            drop(owner);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[path = "production_pre_ranked_local_frame_ledger_v1_tests.rs"]
mod ledger_identity_tests;

#[path = "production_pre_ranked_local_frame_chain_v1_tests.rs"]
mod chain_tests;

#[test]
fn raw_empty_construction_does_not_install_source_capture_or_local_policy() {
    for shared in [false, true] {
        with_source(shared, |owner, _, _| {
            assert!(matches!(
                owner.helper_source_policy_v1(),
                ProductionHelperSourcePolicyV1::RawEmpty
            ));
            assert!(matches!(
                owner.helper_memory.capture,
                HelperOccurrenceCaptureV1::Absent
            ));
            assert!(owner.semantic_ssa().occurrence_storage().is_none());
            assert!(owner.helper_memory.unit_source.is_empty());
        });
    }
}
