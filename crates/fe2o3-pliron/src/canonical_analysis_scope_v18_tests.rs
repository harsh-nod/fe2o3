use super::*;
use fe2o3_kernel_analysis::{CanonicalKirMemorySsaNodeV1, CanonicalKirSparseValueV1};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant,
    Function, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1,
    StorageOperationV1, Terminator, Type, ValueDef, ValueId,
};
use std::mem::size_of;

const LIMIT: usize = 1_000_000;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 4,
    edges: 8,
    containment_depth: 4,
    object_bytes: 64,
};
type ScopeResult<T> = Result<T, CanonicalAnalysisScopeErrorV1>;

fn module() -> Module {
    let mut module = Module::new("actual-v18-analysis-scope");
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    let object = Type::StorageObject(StorageLayoutIdV1(0));
    let mut block = BasicBlock::new(BlockId(900));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(10),
                Type::pointer(object.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            ),
            OperationKind::Alloca {
                element: object,
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(10),
                value: ValueId(20),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(30), Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(30)],
    });
    module.functions.push(Function::internal_helper(
        "storage",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
        vec![],
        vec![block],
    ));
    module
}

fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV18, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}

#[test]
fn v18_scope_borrows_the_original_table_and_reuses_both_real_caches() {
    let (owner, retained) = admit(&module());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained + 17).unwrap();
    let floor = budget.storage();
    let cleanup = CanonicalAnalysisCleanupV1::new();
    with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| -> ScopeResult<()> {
        let inventory = std::ptr::from_ref(scope.inventory());
        assert!(scope.inventory().belongs_to(&owner));
        assert_eq!(scope.inventory().owner().module().storage_layouts.as_ptr(), owner.module().storage_layouts.as_ptr());
        let sparse = scope.with_sparse_v1(|report, _| {
            assert_eq!(std::ptr::from_ref(report.inventory()), inventory);
            assert_eq!(report.value(2), Some(CanonicalKirSparseValueV1::Dynamic));
            Ok::<_, CanonicalAnalysisScopeErrorV1>(std::ptr::from_ref(report))
        })?;
        let memory = scope.with_memory_ssa_v1(|report, budget| {
            assert_eq!(std::ptr::from_ref(report.inventory()), inventory);
            let write = report.operation(report.inventory().operations()[2].coordinate, budget).unwrap().unwrap();
            let read = report.operation(report.inventory().operations()[3].coordinate, budget).unwrap().unwrap();
            assert!(matches!(report.node(write, budget).unwrap(), CanonicalKirMemorySsaNodeV1::Def { .. }));
            assert!(matches!(report.node(read, budget).unwrap(), CanonicalKirMemorySsaNodeV1::Use { incoming, .. } if *incoming == write));
            Ok::<_, CanonicalAnalysisScopeErrorV1>(std::ptr::from_ref(report))
        })?;
        scope.with_sparse_v1(|report, _| {
            assert_eq!(std::ptr::from_ref(report), sparse);
            Ok::<_, CanonicalAnalysisScopeErrorV1>(())
        })?;
        scope.with_memory_ssa_v1(|report, _| {
            assert_eq!(std::ptr::from_ref(report), memory);
            Ok::<_, CanonicalAnalysisScopeErrorV1>(())
        })
    }).unwrap();
    assert!(!cleanup.refund_denied());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn v18_empty_scope_exact_and_short_limits_preserve_legacy_accounting() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let live = floor
        + size_of::<CanonicalKirInventoryV18<'_>>()
        + size_of::<CanonicalKirSparseV18<'_, '_>>();
    // Same engine: four scratch vector headers, flags, and five cursors.
    let peak = live + 4 * size_of::<Vec<usize>>() + size_of::<Vec<u8>>() + 5 * size_of::<usize>();
    for (allowance, storage, succeeds) in
        [(28, peak, true), (27, peak, false), (28, peak - 1, false)]
    {
        let mut work = Work::new(allowance);
        let mut budget = Budget::new(&mut work, storage);
        budget.reserve_storage(floor).unwrap();
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let result = with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| {
            scope.with_sparse_v1(|_, budget| -> ScopeResult<()> {
                assert_eq!(budget.storage(), live);
                Ok(())
            })?;
            scope.with_sparse_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))
        });
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(budget.storage(), floor);
        assert!(!cleanup.refund_denied());
        match result {
            Ok(()) => {
                assert_eq!(budget.work(), 28);
                assert_eq!(budget.peak_storage(), peak);
            }
            Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error))) => {
                assert_eq!(error.actual(), 28);
                assert_eq!(error.limit(), 27);
            }
            Err(CanonicalAnalysisScopeErrorV1::Sparse(CanonicalKirSparseErrorV1::Resource(
                Resource::Storage(error),
            ))) => {
                assert_eq!(error.actual(), peak);
                assert_eq!(error.limit(), peak - 1);
            }
            other => panic!("wrong resource phase: {other:?}"),
        }
    }
}

#[test]
fn v18_scope_custody_loss_preserves_selected_error_or_original_raw_panic() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let live = floor + size_of::<CanonicalKirInventoryV18<'_>>();
    for mode in 0..3 {
        let mut work = Work::new(12);
        let mut budget = Budget::new(&mut work, live);
        budget.reserve_storage(floor).unwrap();
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let payload = Box::new([41u8, 77, 5]);
        let payload_address = std::ptr::from_ref(&*payload);
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_canonical_analysis_scope_v18(
                &owner,
                &mut budget,
                &cleanup,
                |scope| -> ScopeResult<()> {
                    scope.with_inventory_v1(|_, budget| {
                        budget.release_storage(1).unwrap();
                        match mode {
                            0 => Ok(()),
                            1 => Err(CanonicalAnalysisScopeErrorV1::Resource(
                                Resource::Arithmetic,
                            )),
                            _ => std::panic::panic_any(payload),
                        }
                    })
                },
            )
        }));
        match (mode, result) {
            (0, Ok(Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting)))) => {}
            (1, Ok(Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic)))) => {}
            (2, Err(payload)) => {
                let actual = payload.downcast::<Box<[u8; 3]>>().unwrap();
                assert_eq!(std::ptr::from_ref(&**actual), payload_address);
                assert_eq!(**actual, [41, 77, 5]);
            }
            (_, other) => panic!("replaced selected result: {other:?}"),
        }
        assert!(cleanup.refund_denied());
        assert_eq!(budget.storage(), live - 1);
        assert_eq!(budget.work(), 12);
        let retry: ScopeResult<()> =
            with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |_| {
                panic!("denied enclosing attempt")
            });
        assert!(matches!(
            retry,
            Err(CanonicalAnalysisScopeErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!((budget.storage(), budget.work()), (live - 1, 12));
    }
}

#[test]
fn v18_scope_same_slot_foreign_ledger_never_refunds_either_owner() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let live = floor + size_of::<CanonicalKirInventoryV18<'_>>();
    let mut work = Work::new(12);
    let mut foreign_work = Work::new(0);
    let mut budget = Budget::new(&mut work, live);
    let mut foreign = Budget::new(&mut foreign_work, live);
    budget.reserve_storage(floor).unwrap();
    foreign.reserve_storage(live).unwrap();
    let original = budget.work_ledger_identity_v1();
    let donor = foreign.work_ledger_identity_v1();
    let cleanup = CanonicalAnalysisCleanupV1::new();
    let result: ScopeResult<()> =
        with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| {
            scope.with_inventory_v1(|_, budget| {
                std::mem::swap(budget, &mut foreign);
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Arithmetic,
                ))
            })
        });
    assert!(matches!(
        result,
        Err(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Arithmetic
        ))
    ));
    assert!(cleanup.refund_denied());
    assert!(budget.work_ledger_identity_v1() == donor);
    assert!(foreign.work_ledger_identity_v1() == original);
    assert_eq!((budget.storage(), foreign.storage()), (live, live));
    assert_eq!((budget.work(), foreign.work()), (0, 12));
    std::mem::swap(&mut budget, &mut foreign);
    budget.release_storage(live - floor).unwrap();
    foreign.release_storage(live).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn v18_explicitly_handled_resource_failure_does_not_poison_custody() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let live = floor + size_of::<CanonicalKirInventoryV18<'_>>();
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, live);
    budget.reserve_storage(floor).unwrap();
    let cleanup = CanonicalAnalysisCleanupV1::new();
    with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| -> ScopeResult<()> {
        for _ in 0..2 {
            let result: ScopeResult<()> = scope.with_sparse_v1(|_, _| panic!("unaffordable cache"));
            assert!(matches!(
                result,
                Err(CanonicalAnalysisScopeErrorV1::Sparse(
                    CanonicalKirSparseErrorV1::Resource(Resource::Storage(_))
                ))
            ));
            assert!(!scope.poisoned.get());
            assert!(!cleanup.refund_denied());
        }
        scope.with_inventory_v1(|_, budget| {
            assert_eq!(budget.storage(), live);
            Ok::<_, CanonicalAnalysisScopeErrorV1>(())
        })
    })
    .unwrap();
    assert!(!cleanup.refund_denied());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn linked_source_denial_vetoes_inner_scratch_refund_above_the_analysis_floor() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let live = floor + size_of::<CanonicalKirInventoryV18<'_>>();
    let parent = Cell::new(false);
    let cleanup = CanonicalAnalysisCleanupV1::linked(&parent);
    let mut work = Work::new(12);
    let mut budget = Budget::new(&mut work, live + 41);
    budget.reserve_storage(floor).unwrap();
    let result: ScopeResult<()> =
        with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| {
            let result: ScopeResult<()> = scope.with_inventory_v1(|_, budget| {
                budget.reserve_storage(41).unwrap();
                budget.release_storage(1).unwrap();
                parent.set(true);
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Arithmetic,
                ))
            });
            assert!(matches!(
                result,
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Arithmetic
                ))
            ));
            assert_eq!(scope.budget.storage(), live + 40);
            assert!(cleanup.refund_denied());
            parent.set(false);
            assert!(
                cleanup.refund_denied(),
                "the private sticky bit cannot be cleared by the linked cell"
            );
            result
        });
    assert!(matches!(
        result,
        Err(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Arithmetic
        ))
    ));
    assert_eq!(budget.storage(), live + 40);
    assert_eq!(budget.work(), 12);
    assert!(
        parent.get(),
        "outer denial must propagate back into the containing cell"
    );
}

#[test]
fn v18_combined_query_reuses_each_individually_derived_cache_and_original_owner() {
    let (owner, retained) = admit(&module());
    for first in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(retained).unwrap();
        let cleanup = CanonicalAnalysisCleanupV1::new();
        with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| -> ScopeResult<()> {
            if first == 0 { scope.with_sparse_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))?; }
            if first == 1 { scope.with_memory_ssa_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))?; }
            let existing_sparse = scope.sparse.as_ref().map(std::ptr::from_ref);
            let existing_memory = scope.memory_ssa.as_ref().map(std::ptr::from_ref);
            let pair = scope.with_sparse_and_memory_ssa_v1(|sparse, memory, budget| -> ScopeResult<_> {
                assert!(std::ptr::eq(sparse.inventory(), memory.inventory()));
                assert!(sparse.inventory().belongs_to(&owner));
                assert!(memory.belongs_to(sparse.inventory()));
                if let Some(previous) = existing_sparse { assert_eq!(previous, std::ptr::from_ref(sparse)); }
                if let Some(previous) = existing_memory { assert_eq!(previous, std::ptr::from_ref(memory)); }
                let store = memory.operation(memory.inventory().operations()[2].coordinate, budget)
                    .map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?.unwrap();
                let read = memory.operation(memory.inventory().operations()[3].coordinate, budget)
                    .map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?.unwrap();
                assert!(matches!(memory.node(read, budget).map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?,
                    CanonicalKirMemorySsaNodeV1::Use { incoming, .. } if *incoming == store));
                Ok((std::ptr::from_ref(sparse), std::ptr::from_ref(memory)))
            })?;
            let prior = (scope.budget.work(), scope.budget.storage(), scope.budget.peak_storage());
            scope.with_sparse_and_memory_ssa_v1(|sparse, memory, budget| -> ScopeResult<()> {
                assert_eq!((std::ptr::from_ref(sparse), std::ptr::from_ref(memory)), pair);
                assert_eq!(budget.work(), prior.0 + 6);
                assert_eq!((budget.storage(), budget.peak_storage()), (prior.1, prior.2));
                Ok(())
            })
        }).unwrap();
        assert_eq!(budget.storage(), retained);
        assert!(!cleanup.refund_denied());
    }
}

#[test]
fn v18_combined_empty_query_has_independent_exact_and_one_short_limits() {
    let (owner, retained) = admit(&Module::new("combined-empty"));
    let floor = retained + 17;
    let inventory = size_of::<CanonicalKirInventoryV18<'_>>();
    let sparse = size_of::<CanonicalKirSparseV18<'_, '_>>();
    let memory = size_of::<CanonicalKirMemorySsaV18<'_, '_>>();
    let sparse_scratch = 4 * size_of::<Vec<usize>>() + size_of::<Vec<u8>>() + 5 * size_of::<usize>();
    let live = floor + inventory + sparse + memory;
    let peak = live.max(floor + inventory + sparse + sparse_scratch);
    // Outer4 + inventory2 + request6 + sparse8/transfer2 + memory40/transfer2 + hit6.
    for (allowance, bytes) in [(70, peak), (69, peak), (70, peak - 1)] {
        let mut work = Work::new(allowance);
        let mut budget = Budget::new(&mut work, bytes);
        budget.reserve_storage(floor).unwrap();
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let calls = Cell::new(0);
        let result = with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| {
            scope.with_sparse_and_memory_ssa_v1(|sparse, memory, budget| -> ScopeResult<()> {
                calls.set(calls.get() + 1);
                assert!(std::ptr::eq(sparse.inventory(), memory.inventory()));
                assert_eq!(memory.node_count(), 0);
                assert_eq!(budget.storage(), live);
                Ok(())
            })?;
            scope.with_sparse_and_memory_ssa_v1(|_, _, _| {
                calls.set(calls.get() + 1);
                Ok::<_, CanonicalAnalysisScopeErrorV1>(())
            })
        });
        assert_eq!(budget.storage(), floor);
        assert!(!cleanup.refund_denied());
        match (allowance, bytes, result) {
            (70, actual, Ok(())) if actual == peak => {
                assert_eq!(calls.get(), 2);
                assert_eq!((budget.work(), budget.peak_storage()), (70, peak));
            }
            (69, _, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error)))) => {
                assert_eq!(calls.get(), 1);
                assert_eq!((error.actual(), error.limit(), budget.work()), (70, 69, 64));
            }
            (70, _, Err(CanonicalAnalysisScopeErrorV1::Sparse(CanonicalKirSparseErrorV1::Resource(Resource::Storage(error)))))
            | (70, _, Err(CanonicalAnalysisScopeErrorV1::MemorySsa(CanonicalKirMemorySsaErrorV1::Resource(Resource::Storage(error))))) => {
                assert_eq!(calls.get(), 0);
                assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
                assert_eq!(budget.failed_storage(), Some(peak));
            }
            (_, _, other) => panic!("wrong combined boundary: {other:?}"),
        }
    }
}

#[test]
fn v18_combined_query_custody_loss_poison_is_shared_by_both_cache_entries() {
    let (owner, retained) = admit(&module());
    for foreign_slot in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut foreign_work = Work::new(0);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut foreign = Budget::new(&mut foreign_work, LIMIT);
        budget.reserve_storage(retained).unwrap();
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let called = Cell::new(false);
        let outside = Cell::new(false);
        let result: ScopeResult<()> = with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| {
            let failed: ScopeResult<()> = scope.with_sparse_and_memory_ssa_v1(|sparse, memory, budget| {
                assert!(std::ptr::eq(sparse.inventory(), memory.inventory()));
                called.set(true);
                if foreign_slot {
                    foreign.reserve_storage(budget.storage()).unwrap();
                    std::mem::swap(budget, &mut foreign);
                } else { budget.release_storage(1).unwrap(); }
                Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic))
            });
            assert!(matches!(failed, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic))));
            assert!(scope.sparse.is_none() && scope.memory_ssa.is_none());
            assert!(cleanup.refund_denied());
            let stopped = (scope.budget.work(), scope.budget.storage());
            let retried: ScopeResult<()> = scope.with_sparse_and_memory_ssa_v1(|_, _, _| panic!("poisoned combined query"));
            assert!(matches!(retried, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting))));
            let retried: ScopeResult<()> = scope.with_memory_ssa_v1(|_, _| panic!("poisoned memory cache"));
            assert!(matches!(retried, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting))));
            let retried: ScopeResult<()> = scope.with_sparse_v1(|_, _| panic!("poisoned sparse cache"));
            assert!(matches!(retried, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting))));
            assert_eq!((scope.budget.work(), scope.budget.storage()), stopped);
            outside.set(true);
            failed
        });
        assert!(called.get() && outside.get());
        assert!(matches!(result, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic))));
        if foreign_slot {
            assert_eq!(budget.work(), 0, "foreign meter never debited");
            assert_eq!(budget.storage(), foreign.storage(), "neither ledger refunded");
        } else { assert!(budget.storage() > retained, "lost full floor not refunded"); }
    }
}

struct RejectedAnalysisValue<'a> { drops: &'a Cell<usize>, panic_on_drop: bool }
impl Drop for RejectedAnalysisValue<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        if self.panic_on_drop { panic!("rejected analysis output destructor"); }
    }
}

#[test]
fn v18_rejected_destructor_preserves_accounting_and_original_callback_chronology() {
    let (owner, retained) = admit(&Module::new("rejected-output"));
    let headers = super::tests::disposal_headers::<RejectedAnalysisValue<'_>, CanonicalAnalysisScopeErrorV1>();
    assert_eq!(callback_disposal_headers_v1::<RejectedAnalysisValue<'_>, CanonicalAnalysisScopeErrorV1>().unwrap(), headers);
    for layer in 0..2 {
        for mode in 0..4 {
            let floor = retained + 17;
            let live = floor + size_of::<CanonicalKirInventoryV18<'_>>() + headers;
            let expected_work = if layer == 0 { 11 } else { 17 };
            let mut work = Work::new(expected_work);
            let mut budget = Budget::new(&mut work, live);
            budget.reserve_storage(floor).unwrap();
            let cleanup = CanonicalAnalysisCleanupV1::new();
            let calls = Cell::new(0usize);
            let drops = Cell::new(0usize);
            let inspected = Cell::new(false);
            let invoke = |budget: &mut Budget<'_>| -> ScopeResult<RejectedAnalysisValue<'_>> {
                calls.set(calls.get() + 1);
                if mode != 0 { budget.release_storage(1).unwrap(); }
                match mode {
                    0 | 1 => Ok(RejectedAnalysisValue { drops: &drops, panic_on_drop: mode == 1 }),
                    2 => Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic)),
                    _ => std::panic::panic_any(Box::new([7u8, 9])),
                }
            };
            let result = catch_unwind(AssertUnwindSafe(|| -> ScopeResult<()> {
                let consume = |result: ScopeResult<RejectedAnalysisValue<'_>>| -> ScopeResult<()> {
                    match (mode, result) {
                        (0, Ok(value)) => drop(value),
                        (1, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting))) => {}
                        (2, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic))) => {}
                        _ => panic!("selected callback/framework result changed"),
                    }
                    inspected.set(true);
                    Ok(())
                };
                if layer == 0 {
                    let result = with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| invoke(scope.budget));
                    consume(result)
                } else {
                    with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| -> ScopeResult<()> {
                        let result = scope.with_inventory_v1(|_, budget| invoke(budget));
                        consume(result)
                    })
                }
            }));
            assert_eq!(calls.get(), 1, "real callback must run");
            assert_eq!(drops.get(), usize::from(mode < 2));
            assert_eq!(inspected.get(), mode != 3, "destructor panic must not bypass assertions");
            if mode == 3 {
                let payload = result.unwrap_err().downcast::<Box<[u8; 2]>>().unwrap();
                assert_eq!(**payload, [7, 9]);
            } else if layer == 1 && mode != 0 {
                assert!(matches!(result.unwrap(), Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting))));
            } else { result.unwrap().unwrap(); }
            assert_eq!(budget.work(), expected_work);
            assert_eq!(budget.storage(), if mode == 0 { floor } else { live - 1 });
            assert_eq!(cleanup.refund_denied(), mode != 0);
        }
    }
}

#[test]
fn v18_dropping_output_cleanup_is_prepaid_at_exact_and_one_short_boundaries() {
    assert_eq!(callback_disposal_headers_v1::<(), String>().unwrap(), 0);
    assert_eq!(callback_disposal_headers_v1::<&Module, String>().unwrap(), 0);
    assert_eq!(callback_disposal_work_v1::<()>(), 0);
    assert_eq!(callback_disposal_work_v1::<RejectedAnalysisValue<'_>>(), 5);
    let (owner, retained) = admit(&Module::new("dropping-output-limits"));
    let floor = retained + 17;
    let headers = super::tests::disposal_headers::<RejectedAnalysisValue<'_>, CanonicalAnalysisScopeErrorV1>();
    let live = floor + size_of::<CanonicalKirInventoryV18<'_>>() + headers;
    for layer in 0..2 {
        let exact = if layer == 0 { 11 } else { 17 };
        for (work_limit, storage_limit) in [(exact, live), (exact - 1, live), (exact, live - 1)] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let cleanup = CanonicalAnalysisCleanupV1::new();
            let called = Cell::new(false);
            let drops = Cell::new(0);
            let result: ScopeResult<()> = if layer == 0 {
                with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |_| {
                    called.set(true);
                    Ok::<_, CanonicalAnalysisScopeErrorV1>(RejectedAnalysisValue { drops: &drops, panic_on_drop: false })
                }).map(drop)
            } else {
                with_canonical_analysis_scope_v18(&owner, &mut budget, &cleanup, |scope| {
                    let value = scope.with_inventory_v1(|_, _| {
                        called.set(true);
                        Ok::<_, CanonicalAnalysisScopeErrorV1>(RejectedAnalysisValue { drops: &drops, panic_on_drop: false })
                    })?;
                    drop(value);
                    Ok::<_, CanonicalAnalysisScopeErrorV1>(())
                })
            };
            if work_limit == exact && storage_limit == live {
                assert!(called.get());
                result.unwrap();
                assert_eq!(drops.get(), 1);
                assert_eq!((budget.work(), budget.peak_storage()), (exact, live));
            } else {
                assert!(!called.get());
                match result {
                    Err(CanonicalAnalysisScopeErrorV1::Inventory(CanonicalKirInventoryErrorV1::Resource(Resource::Work(error)))) =>
                        assert_eq!((error.actual(), error.limit()), (exact, exact - 1)),
                    Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error))) =>
                        assert_eq!((error.actual(), error.limit(), budget.work()), (exact, exact - 1, 6)),
                    Err(CanonicalAnalysisScopeErrorV1::Inventory(CanonicalKirInventoryErrorV1::Resource(Resource::Storage(error)))) =>
                        assert_eq!((error.actual(), error.limit()), (live, live - 1)),
                    Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Storage(error))) =>
                        assert_eq!((error.actual(), error.limit()), (live, live - 1)),
                    _ => panic!("wrong dropping outer boundary"),
                }
                assert_eq!(drops.get(), 0);
            }
            assert_eq!(budget.storage(), floor);
            assert!(!cleanup.refund_denied());
        }
    }
}
