use super::*;
use fe2o3_kernel_analysis::CanonicalKirMemorySsaNodeV1;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CanonicalKirOperationCoordinateV1, MemoryAccess, ScalarType,
};

fn memory_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(79));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), pointer),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(7),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), scalar.clone()),
            OperationKind::Load {
                pointer: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(9)],
    });
    let mut module = Module::new("scope_memory");
    module.functions.push(Function::internal_helper(
        "slot",
        Signature::new(vec![], vec![scalar]),
        vec![],
        vec![block],
    ));
    module
}

fn request(scope: &mut CanonicalAnalysisScopeV1<'_, '_, '_, '_>, memory: bool) -> ScopeResult<()> {
    if memory {
        scope.with_memory_ssa_v1(|_, _| Ok(()))
    } else {
        scope.with_sparse_v1(|_, _| Ok(()))
    }
}

#[test]
fn memory_ssa_is_lazy_and_both_cache_orders_reuse_the_exact_inventory() {
    let (owner, retained) = admit(&memory_module());
    for memory_first in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(retained + 17).unwrap();
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
            assert!(scope.sparse.is_none() && scope.memory_ssa.is_none());
            request(scope, memory_first)?;
            assert_eq!(scope.memory_ssa.is_some(), memory_first);
            assert_eq!(scope.sparse.is_some(), !memory_first);
            request(scope, !memory_first)?;
            let inventory = std::ptr::from_ref(scope.inventory());
            let sparse = std::ptr::from_ref(scope.sparse.as_ref().unwrap());
            let memory = std::ptr::from_ref(scope.memory_ssa.as_ref().unwrap());
            let floor = scope.budget.storage();
            let peak = scope.budget.peak_storage();
            let before = scope.budget.work();
            scope.with_sparse_v1(|report, _| -> ScopeResult<()> {
                assert_eq!(std::ptr::from_ref(report), sparse);
                assert_eq!(std::ptr::from_ref(report.inventory()), inventory);
                Ok(())
            })?;
            scope.with_memory_ssa_v1(|report, budget| -> ScopeResult<()> {
                assert_eq!(std::ptr::from_ref(report), memory);
                assert_eq!(std::ptr::from_ref(report.inventory()), inventory);
                assert!(report.belongs_to(report.inventory()));
                assert!(report.inventory().belongs_to(&owner));
                let block = CanonicalKirBlockCoordinateV1 { function: CanonicalKirFunctionCoordinateV1(0), block: 0 };
                let coordinate = |operation| CanonicalKirOperationCoordinateV1 { block, operation };
                assert!(report.operation(coordinate(0), budget).map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?.is_none());
                let store = report.operation(coordinate(2), budget).map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?.unwrap();
                let load = report.operation(coordinate(3), budget).map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?.unwrap();
                assert!(matches!(report.node(store, budget).map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?, CanonicalKirMemorySsaNodeV1::Def { operation, .. } if *operation == coordinate(2)));
                assert!(matches!(report.node(load, budget).map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?, CanonicalKirMemorySsaNodeV1::Use { operation, incoming } if *operation == coordinate(3) && *incoming == store));
                assert_eq!(budget.storage(), floor);
                Ok(())
            })?;
            assert_eq!(scope.budget.work() - before, 2 * 6 + 3 * 8 + 2 * 2);
            assert_eq!(scope.budget.peak_storage(), peak);
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), retained + 17);
    }
}

#[test]
fn memory_ssa_empty_exact_work_header_and_transfer_boundaries_are_derived() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let payload =
        size_of::<CanonicalKirInventoryV1<'_>>() + size_of::<CanonicalKirMemorySsaV1<'_, '_>>();
    // Scope4 + inventory2 + request6 + derive40 + transfer2 + cache hit6.
    for (limit, bytes) in [
        (60, payload),
        (59, payload),
        (60, payload - 1),
        (53, payload),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, floor + bytes);
        budget.reserve_storage(floor).unwrap();
        let result =
            with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
                let first = scope.with_memory_ssa_v1(|report, budget| -> ScopeResult<()> {
                    assert_eq!(report.node_count(), 0);
                    assert_eq!(budget.storage(), floor + payload);
                    Ok(())
                });
                if first.is_err() {
                    assert!(scope.memory_ssa.is_none());
                }
                first?;
                scope.with_memory_ssa_v1(|_, _| Ok(()))
            });
        assert_eq!(budget.storage(), floor);
        match (limit, bytes == payload, result) {
            (60, true, Ok(())) => {
                assert_eq!(budget.work(), 60);
                assert_eq!(budget.peak_storage(), floor + payload);
            }
            (59, true, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error)))) => {
                assert_eq!((error.actual(), error.limit(), budget.work()), (60, 59, 54));
                assert_eq!(budget.peak_storage(), floor + payload);
            }
            (53, true, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error)))) => {
                assert_eq!((error.actual(), error.limit(), budget.work()), (54, 53, 52));
                assert_eq!(budget.peak_storage(), floor + payload);
            }
            (
                60,
                false,
                Err(CanonicalAnalysisScopeErrorV1::MemorySsa(
                    CanonicalKirMemorySsaErrorV1::Resource(Resource::Storage(error)),
                )),
            ) => {
                assert_eq!(error.actual(), floor + payload);
                assert_eq!(error.limit(), floor + payload - 1);
                // Header fails after derive's initial6 + counts3 + header1.
                assert_eq!(budget.work(), 4 + 2 + 6 + 10);
                assert_eq!(
                    budget.peak_storage(),
                    floor + size_of::<CanonicalKirInventoryV1<'_>>()
                );
                assert_eq!(budget.failed_storage(), Some(floor + payload));
            }
            (_, _, other) => panic!("wrong independent boundary: {other:?}"),
        }
    }
}

#[test]
fn scope_entry_one_short_does_not_begin_inventory_or_callback() {
    let (owner, retained) = admit(&Module::new("empty"));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = Budget::new(&mut work, retained + 17);
    budget.reserve_storage(retained + 17).unwrap();
    let result: ScopeResult<()> =
        with_canonical_analysis_scope_v1(&owner, &mut budget, |_| panic!("entry rejected"));
    assert!(
        matches!(result, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error))) if error.actual() == 4 && error.limit() == 3)
    );
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), retained + 17);
    assert_eq!(budget.peak_storage(), retained + 17);
}

#[test]
fn memory_ssa_header_failure_keeps_the_existing_sparse_cache_live() {
    let (owner, retained) = admit(&Module::new("empty"));
    let inventory = size_of::<CanonicalKirInventoryV1<'_>>();
    let sparse = size_of::<CanonicalKirSparseV1<'_, '_>>();
    let memory = size_of::<CanonicalKirMemorySsaV1<'_, '_>>();
    let scratch = sparse_engine_extra_header_bytes();
    let sparse_floor = retained + inventory + sparse;
    let peak = sparse_floor + scratch;
    let attempted = peak + memory;
    let limit = attempted - 1;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = Budget::new(&mut work, limit);
    budget.reserve_storage(retained).unwrap();
    with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
        request(scope, false)?;
        let first = std::ptr::from_ref(scope.sparse.as_ref().unwrap());
        assert_eq!(scope.budget.storage(), sparse_floor);
        assert_eq!(scope.budget.peak_storage(), peak);
        assert_eq!(scope.budget.work(), 4 + 2 + 6 + 8 + 2);
        assert_eq!(scope.budget.failed_storage(), None);
        // Private fixture pressure admits sparse preparation, then makes the
        // MemorySSA header the first denied reservation. There is no backing
        // allocation; only finish_request may refund this logical scratch.
        scope.budget.reserve_storage(scratch).unwrap();
        assert_eq!(scope.budget.storage(), peak);
        let failed: ScopeResult<()> = scope.with_memory_ssa_v1(|_, _| panic!("header failed"));
        assert!(matches!(failed, Err(CanonicalAnalysisScopeErrorV1::MemorySsa(CanonicalKirMemorySsaErrorV1::Resource(Resource::Storage(error)))) if error.actual() == attempted && error.limit() == limit));
        assert!(scope.memory_ssa.is_none());
        assert_eq!(scope.budget.storage(), sparse_floor);
        assert_eq!(scope.budget.peak_storage(), peak);
        // Prior22 + request6 + derive(initial6 + counts3 + header1).
        assert_eq!(scope.budget.work(), 22 + 6 + 6 + 3 + 1);
        assert_eq!(scope.budget.failed_storage(), Some(attempted));
        scope.with_sparse_v1(|report, budget| -> ScopeResult<()> {
            assert_eq!(std::ptr::from_ref(report), first);
            assert_eq!(budget.storage(), sparse_floor);
            assert_eq!(budget.work(), 38 + 6);
            Ok(())
        })
    }).unwrap();
    assert_eq!(budget.storage(), retained);
    assert_eq!(budget.peak_storage(), peak);
    assert_eq!(budget.work(), 44);
    assert_eq!(budget.failed_storage(), Some(attempted));
    drop(budget);
    assert_eq!(work.failed_work(), None);
}

#[test]
fn independent_equal_and_changed_graph_owners_never_share_memory_ssa() {
    let (first, a) = admit(&boolean_module(true));
    let (equal, b) = admit(&boolean_module(true));
    let (changed, c) = admit(&boolean_module(false));
    assert_eq!(first.canonical().identity(), equal.canonical().identity());
    assert_ne!(first.canonical().identity(), changed.canonical().identity());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(a + b + c).unwrap();
    for owner in [&first, &equal, &changed] {
        with_canonical_analysis_scope_v1(owner, &mut budget, |scope| -> ScopeResult<()> {
            assert!(scope.memory_ssa.is_none());
            scope.with_memory_ssa_v1(|report, _| -> ScopeResult<()> {
                assert!(report.inventory().belongs_to(owner));
                assert_eq!(
                    report.inventory().belongs_to(&first),
                    std::ptr::eq(owner, &first)
                );
                Ok(())
            })
        })
        .unwrap();
        assert_eq!(budget.storage(), a + b + c);
    }
}

fn callback_exit(mode: u8) -> ScopeResult<()> {
    match mode {
        0 => Ok(()),
        1 => Err(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Arithmetic,
        )),
        _ => std::panic::panic_any("analysis scope sentinel"),
    }
}

#[test]
fn clean_callback_errors_and_caught_panics_preserve_cache_and_release_scratch() {
    let (owner, retained) = admit(&Module::new("empty"));
    let inventory = size_of::<CanonicalKirInventoryV1<'_>>();
    let sparse = size_of::<CanonicalKirSparseV1<'_, '_>>();
    let memory_header = size_of::<CanonicalKirMemorySsaV1<'_, '_>>();
    let entry_floor = retained + 17;
    let cache_floor = entry_floor + inventory + sparse + memory_header;
    let peak = (entry_floor + inventory + sparse + sparse_engine_extra_header_bytes())
        .max(cache_floor + 19);
    for memory in [false, true] {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
            let mut budget = Budget::new(&mut work, 1_000_000);
            budget.reserve_storage(retained + 17).unwrap();
            with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
                request(scope, false)?;
                request(scope, true)?;
                let floor = scope.budget.storage();
                assert_eq!(floor, cache_floor);
                // Scope4 + inventory2 + sparse(request6 + derive8 + transfer2)
                // + MemorySSA(request6 + derive40 + transfer2).
                assert_eq!(scope.budget.work(), 4 + 2 + 6 + 8 + 2 + 6 + 40 + 2);
                let sparse = std::ptr::from_ref(scope.sparse.as_ref().unwrap());
                let memory_ptr = std::ptr::from_ref(scope.memory_ssa.as_ref().unwrap());
                let invoke = |budget: &mut Budget<'_>| -> ScopeResult<()> {
                    budget.reserve_storage(19).unwrap();
                    // Logical scratch has no surviving allocation on any exit.
                    callback_exit(mode)
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    if memory {
                        scope.with_memory_ssa_v1(|_, budget| invoke(budget))
                    } else {
                        scope.with_sparse_v1(|_, budget| invoke(budget))
                    }
                }));
                match (mode, result) {
                    (0, Ok(Ok(()))) => {}
                    (1, Ok(Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic)))) => {
                    }
                    (2, Err(payload)) => assert_eq!(
                        payload.downcast_ref::<&str>(),
                        Some(&"analysis scope sentinel")
                    ),
                    (_, other) => panic!("changed clean callback result: {other:?}"),
                }
                assert_eq!(scope.budget.storage(), floor);
                assert_eq!(scope.budget.peak_storage(), peak);
                assert_eq!(scope.budget.work(), 70 + 6);
                assert_eq!(scope.budget.failed_storage(), None);
                assert_eq!(std::ptr::from_ref(scope.sparse.as_ref().unwrap()), sparse);
                assert_eq!(
                    std::ptr::from_ref(scope.memory_ssa.as_ref().unwrap()),
                    memory_ptr
                );
                request(scope, memory)
            })
            .unwrap();
            assert_eq!(budget.storage(), retained + 17);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(budget.work(), 70 + 6 + 6);
            assert_eq!(budget.failed_storage(), None);
            drop(budget);
            assert_eq!(work.failed_work(), None);
        }
    }
}

#[test]
fn callback_floor_loss_poisons_both_caches_even_when_error_is_caught_and_floor_restored() {
    let (owner, retained) = admit(&Module::new("empty"));
    for memory in [false, true] {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
            let mut budget = Budget::new(&mut work, 1_000_000);
            budget.reserve_storage(retained + 17).unwrap();
            let result =
                with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
                    request(scope, false)?;
                    request(scope, true)?;
                    let floor = scope.budget.storage();
                    let invoke = |budget: &mut Budget<'_>| -> ScopeResult<()> {
                        budget.release_storage(1).unwrap();
                        callback_exit(mode)
                    };
                    let failed = if memory {
                        scope.with_memory_ssa_v1(|_, budget| invoke(budget))
                    } else {
                        scope.with_sparse_v1(|_, budget| invoke(budget))
                    };
                    assert!(matches!(
                        failed,
                        Err(CanonicalAnalysisScopeErrorV1::Resource(
                            Resource::Accounting
                        ))
                    ));
                    assert!(scope.sparse.is_none() && scope.memory_ssa.is_none());
                    assert_eq!(scope.budget.storage(), floor - 1);
                    // Private hostile recovery cannot revive either dropped cache.
                    scope.budget.reserve_storage(1).unwrap();
                    let before = scope.budget.work();
                    assert!(matches!(
                        request(scope, false),
                        Err(CanonicalAnalysisScopeErrorV1::Resource(
                            Resource::Accounting
                        ))
                    ));
                    assert!(matches!(
                        request(scope, true),
                        Err(CanonicalAnalysisScopeErrorV1::Resource(
                            Resource::Accounting
                        ))
                    ));
                    assert_eq!(scope.budget.work(), before);
                    Ok(())
                });
            assert!(matches!(
                result,
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert_eq!(budget.storage(), retained + 17);
        }
    }
}

#[test]
fn below_incoming_loss_is_not_recreated_and_fresh_scope_requires_explicit_restoration() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(floor).unwrap();
        let result = with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
            scope.with_memory_ssa_v1(|_, budget| {
                budget
                    .release_storage(budget.storage() - (floor - 1))
                    .unwrap();
                callback_exit(mode)
            })
        });
        assert!(matches!(
            result,
            Err(CanonicalAnalysisScopeErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!(budget.storage(), floor - 1);
        budget.reserve_storage(1).unwrap();
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| request(scope, true))
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

fn substituted_callback<'work>(
    owner: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'work>,
    foreign: &mut Budget<'work>,
    memory: bool,
    mode: u8,
) -> ScopeResult<()> {
    with_canonical_analysis_scope_v1(owner, budget, |scope| {
        let mut invoke = |budget: &mut Budget<'work>| {
            foreign.reserve_storage(budget.storage()).unwrap();
            std::mem::swap(budget, foreign);
            callback_exit(mode)
        };
        if memory {
            scope.with_memory_ssa_v1(|_, budget| invoke(budget))
        } else {
            scope.with_sparse_v1(|_, budget| invoke(budget))
        }
    })
}

#[test]
fn public_fixed_lifetime_same_slot_replacement_never_debits_or_cleans_foreign_work() {
    let (owner, retained) = admit(&Module::new("empty"));
    for memory in [false, true] {
        for mode in 0..3 {
            let mut original_work = CanonicalKernelIrWorkBudgetV1::new(10_000);
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = Budget::new(&mut original_work, 1_000_000);
            let mut foreign = Budget::new(&mut foreign_work, 1_000_000);
            budget.reserve_storage(retained + 17).unwrap();
            let result = substituted_callback(&owner, &mut budget, &mut foreign, memory, mode);
            assert!(matches!(
                result,
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.storage(), foreign.storage());
            assert_eq!(budget.peak_storage(), budget.storage());
            assert_eq!(budget.failed_storage(), None);
            assert!(budget.storage() > retained + 17);
            let foreign_floor = budget.storage();
            std::mem::swap(&mut budget, &mut foreign);
            // Scope dropped all owners, but cannot reach the displaced ledger.
            budget
                .release_storage(budget.storage() - retained - 17)
                .unwrap();
            foreign.release_storage(foreign_floor).unwrap();
            with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| request(scope, memory))
                .unwrap();
            assert_eq!(budget.storage(), retained + 17);
        }
    }
}

#[test]
fn privately_restored_ledger_cannot_reenable_a_poisoned_scope_or_charge_foreign_query() {
    let (owner, retained) = admit(&Module::new("empty"));
    let mut work_a = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut work_b = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work_a, 1_000_000);
    let mut foreign = Budget::new(&mut work_b, 1_000_000);
    budget.reserve_storage(retained + 17).unwrap();
    let result =
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
            request(scope, false)?;
            request(scope, true)?;
            foreign.reserve_storage(scope.budget.storage()).unwrap();
            std::mem::swap(scope.budget, &mut foreign);
            assert!(matches!(
                request(scope, true),
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert_eq!(scope.budget.work(), 0);
            assert_eq!(scope.budget.storage(), foreign.storage());
            assert!(scope.sparse.is_none() && scope.memory_ssa.is_none());
            std::mem::swap(scope.budget, &mut foreign);
            assert!(matches!(
                request(scope, false),
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            Ok(())
        });
    assert!(matches!(
        result,
        Err(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(budget.storage(), retained + 17);
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.peak_storage(), foreign.storage());
}

#[test]
fn outer_unwind_drops_inventory_and_both_caches_and_preserves_payload() {
    let (owner, retained) = admit(&Module::new("empty"));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(retained + 17).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
            request(scope, false)?;
            request(scope, true)?;
            callback_exit(2)
        })
    }));
    assert_eq!(
        result.unwrap_err().downcast_ref::<&str>(),
        Some(&"analysis scope sentinel")
    );
    assert_eq!(budget.storage(), retained + 17);
    with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| request(scope, true)).unwrap();
    assert_eq!(budget.storage(), retained + 17);
}

#[test]
fn inventory_only_full_floor_loss_is_not_hidden_by_the_incoming_floor() {
    let (owner, retained) = admit(&Module::new("empty"));
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(retained + 17).unwrap();
        let result = with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
            // Private hostile access: the public inventory getter exposes no ledger.
            scope.budget.release_storage(1).unwrap();
            callback_exit(mode)
        });
        assert!(matches!(
            result,
            Err(CanonicalAnalysisScopeErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!(budget.storage(), retained + 17);
    }
}

#[test]
fn memory_ssa_header_pressure_cap_allows_clean_cache_installation() {
    let (owner, retained) = admit(&Module::new("empty"));
    let inventory = size_of::<CanonicalKirInventoryV1<'_>>();
    let sparse = size_of::<CanonicalKirSparseV1<'_, '_>>();
    let memory = size_of::<CanonicalKirMemorySsaV1<'_, '_>>();
    let sparse_floor = retained + inventory + sparse;
    let sparse_peak = sparse_floor + sparse_engine_extra_header_bytes();
    // Same global cap as the pressure case, but no scratch survives into
    // receipt transfer. A successful derive must preserve the exact live floor.
    let limit = sparse_peak + memory - 1;
    let cache_floor = sparse_floor + memory;
    let peak = sparse_peak.max(cache_floor);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = Budget::new(&mut work, limit);
    budget.reserve_storage(retained).unwrap();
    with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
        let inventory_ptr = std::ptr::from_ref(scope.inventory());
        request(scope, false)?;
        let sparse_ptr = std::ptr::from_ref(scope.sparse.as_ref().unwrap());
        assert_eq!(scope.budget.storage(), sparse_floor);
        assert_eq!(scope.budget.peak_storage(), sparse_peak);
        assert_eq!(scope.budget.work(), 4 + 2 + 6 + 8 + 2);
        scope.with_memory_ssa_v1(|report, budget| -> ScopeResult<()> {
            assert_eq!(std::ptr::from_ref(report.inventory()), inventory_ptr);
            assert!(report.inventory().belongs_to(&owner));
            assert_eq!(report.node_count(), 0);
            assert_eq!(budget.storage(), cache_floor);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(budget.work(), 22 + 6 + 40 + 2);
            assert_eq!(budget.failed_storage(), None);
            Ok(())
        })?;
        assert!(scope.memory_ssa.is_some());
        scope.with_sparse_v1(|report, budget| -> ScopeResult<()> {
            assert_eq!(std::ptr::from_ref(report), sparse_ptr);
            assert_eq!(std::ptr::from_ref(report.inventory()), inventory_ptr);
            assert_eq!(budget.storage(), cache_floor);
            assert_eq!(budget.work(), 22 + 6 + 40 + 2 + 6);
            Ok(())
        })
    })
    .unwrap();
    assert_eq!(budget.storage(), retained);
    assert_eq!(budget.peak_storage(), peak);
    assert_eq!(budget.work(), 76);
    assert_eq!(budget.failed_storage(), None);
    drop(budget);
    assert_eq!(work.failed_work(), None);
}
