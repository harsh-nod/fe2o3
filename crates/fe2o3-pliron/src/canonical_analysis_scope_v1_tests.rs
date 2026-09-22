use super::*;

#[path = "canonical_analysis_scope_memory_ssa_v1_tests.rs"]
mod memory_ssa_tests;
use fe2o3_kernel_analysis::{
    CanonicalKirBlockRefV1, CanonicalKirDefinitionRefV1, CanonicalKirFunctionRefV1,
    CanonicalKirOperationRefV1, CanonicalKirSparseExceptionV1, CanonicalKirSparseValueV1,
    CanonicalKirUseRefV1,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, CanonicalKirBlockCoordinateV1,
    CanonicalKirFunctionCoordinateV1, Constant, Function, Module, Operation, OperationKind,
    Signature, Terminator, Type, ValueDef, ValueId,
};
use std::mem::{align_of, size_of};

type ScopeResult<T> = Result<T, CanonicalAnalysisScopeErrorV1>;

fn sparse_engine_extra_header_bytes() -> usize {
    // Engine adds heads/next/queue/unresolved, queued, and five cursors to
    // the report. All fields share pointer alignment, so no padding is lost.
    assert_eq!(
        align_of::<CanonicalKirSparseV1<'_, '_>>(),
        align_of::<usize>()
    );
    assert_eq!(align_of::<Vec<usize>>(), align_of::<usize>());
    assert_eq!(align_of::<Vec<u8>>(), align_of::<usize>());
    4 * size_of::<Vec<usize>>() + size_of::<Vec<u8>>() + 5 * size_of::<usize>()
}

fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, storage.retained_storage())
}

fn boolean_module(value: bool) -> Module {
    let mut block = BasicBlock::new(BlockId(u32::MAX));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(u32::MAX), Type::BOOL),
        OperationKind::Constant(Constant::Bool(value)),
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(u32::MAX)],
    });
    let mut module = Module::new("scope");
    module.functions.push(Function::internal_helper(
        "predicate",
        Signature::new(vec![], vec![Type::BOOL]),
        vec![],
        vec![block],
    ));
    module
}

#[test]
fn inventory_only_scope_does_not_eagerly_allocate_sparse_facts() {
    let (owner, owner_storage) = admit(&Module::new("empty"));
    let floor = owner_storage + 17;
    let inventory = size_of::<CanonicalKirInventoryV1<'_>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
    let mut budget = Budget::new(&mut work, floor + inventory);
    budget.reserve_storage(floor).unwrap();
    with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
        assert!(scope.inventory().belongs_to(&owner));
        assert!(scope.sparse.is_none());
        assert!(scope.memory_ssa.is_none());
        assert_eq!(scope.budget.storage(), floor + inventory);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 6);
    assert_eq!(budget.peak_storage(), floor + inventory);
}

#[test]
fn empty_cache_has_independent_exact_and_one_under_work_and_storage_limits() {
    let (owner, owner_storage) = admit(&Module::new("empty"));
    let floor = owner_storage + 17;
    let retained_payload =
        size_of::<CanonicalKirInventoryV1<'_>>() + size_of::<CanonicalKirSparseV1<'_, '_>>();
    let peak_payload = retained_payload + sparse_engine_extra_header_bytes();
    let peak = floor + peak_payload;
    // Outer checks4 + inventory2 + first request6 + sparse8 + transfer2 + hit6.
    // An empty graph has no row or worklist allocations.
    for (allowance, storage, success) in [
        (28, peak_payload, true),
        (27, peak_payload, false),
        (28, peak_payload - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + allowance);
        work.charge_work(5).unwrap();
        let mut budget = Budget::new(&mut work, floor + storage);
        budget.reserve_storage(floor).unwrap();
        let result = with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
            scope.with_sparse_v1(|_, budget| -> ScopeResult<()> {
                assert_eq!(budget.storage(), floor + retained_payload);
                Ok(())
            })?;
            scope.with_sparse_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(budget.storage(), floor);
        match result {
            Ok(()) => {
                assert_eq!(budget.work(), 33);
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.failed_storage(), None);
            }
            Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error))) => {
                assert_eq!(error.actual(), 33);
                assert_eq!(error.limit(), 5 + allowance);
                assert_eq!(budget.work(), 27);
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.failed_storage(), None);
            }
            Err(CanonicalAnalysisScopeErrorV1::Sparse(CanonicalKirSparseErrorV1::Resource(
                Resource::Storage(error),
            ))) => {
                assert_eq!(error.actual(), peak);
                assert_eq!(error.limit(), peak - 1);
                assert_eq!(budget.work(), 25);
                assert_eq!(
                    budget.peak_storage(),
                    floor + size_of::<CanonicalKirInventoryV1<'_>>()
                );
                assert_eq!(budget.failed_storage(), Some(peak));
            }
            other => panic!("unexpected independent boundary result: {other:?}"),
        }
        drop(budget);
        assert_eq!(work.failed_work(), (allowance == 27).then_some(33));
    }
}

#[test]
fn nonempty_exact_peak_keeps_inventory_live_with_sparse_worklist_scratch() {
    let (owner, owner_storage) = admit(&boolean_module(true));
    let floor = owner_storage + 17;
    // One function, block, operation, definition, return use and each of the
    // three lookup-index rows. No edges, effects, calls or kernel contracts.
    let inventory = size_of::<CanonicalKirInventoryV1<'_>>()
        + size_of::<CanonicalKirFunctionRefV1<'_>>()
        + size_of::<CanonicalKirBlockRefV1<'_>>()
        + size_of::<CanonicalKirOperationRefV1<'_>>()
        + size_of::<CanonicalKirDefinitionRefV1<'_>>()
        + size_of::<CanonicalKirUseRefV1>()
        + size_of::<(&str, CanonicalKirFunctionCoordinateV1)>()
        + size_of::<(CanonicalKirBlockCoordinateV1, BlockId, usize)>()
        + size_of::<(CanonicalKirFunctionCoordinateV1, ValueId, usize)>();
    let sparse = size_of::<CanonicalKirSparseV1<'_, '_>>()
        + size_of::<CanonicalKirSparseValueV1>()
        + size_of::<u8>()
        + size_of::<CanonicalKirSparseExceptionV1>();
    // Sparse inverse heads1 + next1 + queue2 + closure1, plus two queue flags.
    let scratch = 5 * size_of::<usize>() + 2 * size_of::<u8>();
    let peak = floor + inventory + sparse + sparse_engine_extra_header_bytes() + scratch;
    // Census7 + eight allocations8 + fill7 + index/link6. Singleton indexes
    // need no sorting; the sole return use pays one search and comparison.
    let inventory_work = 7 + 8 + 7 + 6;
    for limit in [peak - 1, peak] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let result = with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
            assert_eq!(scope.budget.storage(), floor + inventory);
            scope.with_sparse_v1(|report, budget| -> ScopeResult<()> {
                assert_eq!(budget.storage(), floor + inventory + sparse);
                assert!(matches!(report.value(0), Some(CanonicalKirSparseValueV1::Constant(value)) if value.bits() == 1));
                Ok(())
            })
        });
        assert_eq!(budget.storage(), floor);
        if limit == peak {
            result.unwrap();
            assert_eq!(budget.peak_storage(), peak);
            // Admission8 + allocation/init18 + inverse1 + definition1 +
            // function1 + activation5 + operation6 + terminator2 + closure1.
            let sparse_work = 8 + 18 + 1 + 1 + 1 + 5 + 6 + 2 + 1;
            assert_eq!(budget.work(), 4 + inventory_work + 6 + sparse_work + 2);
            assert_eq!(budget.failed_storage(), None);
        } else {
            assert!(matches!(result,
                Err(CanonicalAnalysisScopeErrorV1::Sparse(CanonicalKirSparseErrorV1::Resource(
                    Resource::Storage(error))))
                    if error.actual() == peak && error.limit() == limit));
            // Seven preceding nonempty vectors cost16, then the final
            // allocation costs1 and fails before initializing its one element.
            assert_eq!(budget.work(), 4 + inventory_work + 6 + 8 + 16 + 1);
            assert_eq!(budget.peak_storage(), peak - size_of::<usize>());
            assert_eq!(budget.failed_storage(), Some(peak));
        }
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn same_scope_reuses_exact_report_after_a_consumer_error() {
    let (owner, owner_storage) = admit(&boolean_module(true));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(owner_storage).unwrap();
    with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
        let first = scope.with_sparse_v1(|report, _| {
            Ok::<_, CanonicalAnalysisScopeErrorV1>(std::ptr::from_ref(report))
        })?;
        let prior_work = scope.budget.work();
        let prior_storage = scope.budget.storage();
        let failed: ScopeResult<()> = scope.with_sparse_v1(|report, _| {
            assert_eq!(first, std::ptr::from_ref(report));
            Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic))
        });
        assert!(matches!(failed, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic))));
        scope.with_sparse_v1(|report, budget| -> ScopeResult<()> {
            assert_eq!(first, std::ptr::from_ref(report));
            assert!(matches!(report.value(0), Some(CanonicalKirSparseValueV1::Constant(value)) if value.bits() == 1));
            assert_eq!(budget.work(), prior_work + 12);
            assert_eq!(budget.storage(), prior_storage);
            Ok(())
        })
    })
    .unwrap();
    assert_eq!(budget.storage(), owner_storage);
}

#[test]
fn independent_equal_and_changed_owners_get_fresh_inventory_and_facts() {
    let (first, a) = admit(&boolean_module(true));
    let (same, b) = admit(&boolean_module(true));
    let (changed, c) = admit(&boolean_module(false));
    assert_eq!(first.canonical().identity(), same.canonical().identity());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(a + b + c).unwrap();
    for (owner, bits) in [(&first, 1), (&same, 1), (&changed, 0)] {
        let start = budget.work();
        with_canonical_analysis_scope_v1(owner, &mut budget, |scope| {
            assert!(scope.inventory().belongs_to(owner));
            assert_eq!(scope.inventory().belongs_to(&first), std::ptr::eq(owner, &first));
            assert!(scope.sparse.is_none());
            scope.with_sparse_v1(|report, _| -> ScopeResult<()> {
                assert!(report.inventory().belongs_to(owner));
                assert!(matches!(report.value(0), Some(CanonicalKirSparseValueV1::Constant(value)) if value.bits() == bits));
                Ok(())
            })
        })
        .unwrap();
        assert!(budget.work() > start + 1);
        assert_eq!(budget.storage(), a + b + c);
    }
}

#[test]
fn callback_errors_drop_inventory_and_cache_before_restoring_floor() {
    let (owner, owner_storage) = admit(&boolean_module(true));
    for request_sparse in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(owner_storage).unwrap();
        let result: ScopeResult<()> =
            with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
                if request_sparse {
                    scope.with_sparse_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))?;
                }
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
        assert_eq!(budget.storage(), owner_storage);
        assert!(budget.peak_storage() > owner_storage);
    }
}

#[test]
fn early_inventory_failure_preserves_floor_and_never_calls_consumer() {
    let (owner, owner_storage) = admit(&Module::new("empty"));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(owner_storage).unwrap();
    let result: ScopeResult<()> = with_canonical_analysis_scope_v1(&owner, &mut budget, |_| {
        panic!("consumer must not run after inventory failure")
    });
    assert!(matches!(
        result,
        Err(CanonicalAnalysisScopeErrorV1::Inventory(_))
    ));
    assert_eq!(budget.storage(), owner_storage);
}

#[test]
fn sparse_failure_installs_no_cache_and_preserves_prior_failure_history() {
    let (owner, owner_storage) = admit(&Module::new("empty"));
    let inventory = size_of::<CanonicalKirInventoryV1<'_>>();
    let sparse = size_of::<CanonicalKirSparseV1<'_, '_>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    work.charge_work(3).unwrap();
    assert!(work.charge_work(usize::MAX).is_err());
    {
        let mut budget = Budget::new(&mut work, owner_storage + inventory + sparse - 1);
        budget.reserve_storage(owner_storage).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
            for _ in 0..2 {
                let failed: ScopeResult<()> = scope.with_sparse_v1(|_, _| {
                    panic!("consumer must not run after sparse allocation failure")
                });
                assert!(matches!(
                    failed,
                    Err(CanonicalAnalysisScopeErrorV1::Sparse(_))
                ));
                assert!(scope.sparse.is_none());
                assert_eq!(scope.budget.storage(), owner_storage + inventory);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), owner_storage);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert_eq!(budget.work(), 3 + 4 + 2 + 2 * (6 + 8));
    }
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

#[test]
fn inventory_callback_empty_exact_and_one_under_limits_are_independent() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let header = size_of::<CanonicalKirInventoryV1<'_>>();
    let peak = floor + header;
    // Prior5 + outer4 + census1 + fill1 + request6.
    for (allowance, limit) in [(12, peak), (11, peak), (12, peak - 1)] {
        let called = Cell::new(false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + allowance);
        work.charge_work(5).unwrap();
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let result = with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
            assert!(scope.sparse.is_none() && scope.memory_ssa.is_none());
            scope.with_inventory_v1(|inventory, budget| -> ScopeResult<()> {
                called.set(true);
                assert!(inventory.belongs_to(&owner));
                assert!(inventory.functions().is_empty());
                assert_eq!(budget.storage(), peak);
                assert_eq!(budget.work(), 17);
                Ok(())
            })
        });
        assert_eq!(budget.storage(), floor);
        match (allowance, limit == peak, result) {
            (12, true, Ok(())) => {
                assert!(called.get());
                assert_eq!(budget.work(), 17);
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.failed_storage(), None);
            }
            (11, true, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error)))) => {
                assert!(!called.get());
                assert_eq!((error.actual(), error.limit(), budget.work()), (17, 16, 11));
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.failed_storage(), None);
            }
            (
                12,
                false,
                Err(CanonicalAnalysisScopeErrorV1::Inventory(
                    CanonicalKirInventoryErrorV1::Resource(Resource::Storage(error)),
                )),
            ) => {
                assert!(!called.get());
                assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
                // The inventory header fails after the initial census unit.
                assert_eq!(budget.work(), 5 + 4 + 1);
                assert_eq!(budget.peak_storage(), floor);
                assert_eq!(budget.failed_storage(), Some(peak));
            }
            (_, _, other) => panic!("wrong inventory callback boundary: {other:?}"),
        }
        drop(budget);
        assert_eq!(work.failed_work(), (allowance == 11).then_some(17));
    }
}

#[test]
fn inventory_callback_reuses_pointer_and_preserves_prior_meter_history() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let header = size_of::<CanonicalKirInventoryV1<'_>>();
    let prior_peak = floor + header + 7;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(23);
    work.charge_work(5).unwrap();
    assert!(work.charge_work(usize::MAX).is_err());
    let mut budget = Budget::new(&mut work, prior_peak);
    budget.reserve_storage(prior_peak).unwrap();
    budget.release_storage(prior_peak - floor).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let graph = with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
        let inventory_pointer = std::ptr::from_ref(scope.inventory());
        let mut graph = &owner;
        for ordinal in 0..2 {
            graph = scope.with_inventory_v1(|inventory, budget| {
                assert_eq!(std::ptr::from_ref(inventory), inventory_pointer);
                assert!(inventory.belongs_to(&owner));
                assert_eq!(budget.work(), 5 + 6 + 6 * (ordinal + 1));
                assert_eq!(budget.storage(), floor + header);
                assert_eq!(budget.peak_storage(), prior_peak);
                assert_eq!(budget.failed_storage(), Some(usize::MAX));
                Ok::<_, CanonicalAnalysisScopeErrorV1>(inventory.owner())
            })?;
            assert!(scope.sparse.is_none() && scope.memory_ssa.is_none());
        }
        // The original graph borrow may escape; the derived inventory may not.
        Ok::<_, CanonicalAnalysisScopeErrorV1>(graph)
    })
    .unwrap();
    assert!(std::ptr::eq(graph, &owner));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 23);
    assert_eq!(budget.peak_storage(), prior_peak);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    drop(budget);
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

#[test]
fn inventory_callback_nonempty_exact_work_and_storage_have_literal_census() {
    let (owner, retained) = admit(&boolean_module(true));
    let floor = retained + 17;
    let value_index = size_of::<(CanonicalKirFunctionCoordinateV1, ValueId, usize)>();
    let payload = size_of::<CanonicalKirInventoryV1<'_>>()
        + size_of::<CanonicalKirFunctionRefV1<'_>>()
        + size_of::<CanonicalKirBlockRefV1<'_>>()
        + size_of::<CanonicalKirOperationRefV1<'_>>()
        + size_of::<CanonicalKirDefinitionRefV1<'_>>()
        + size_of::<CanonicalKirUseRefV1>()
        + size_of::<(&str, CanonicalKirFunctionCoordinateV1)>()
        + size_of::<(CanonicalKirBlockCoordinateV1, BlockId, usize)>()
        + value_index;
    let peak = floor + payload;
    // One function/block/result/operation/return use and three singleton indexes:
    // census7 + allocation8 + fill7 + index/link6 = 28; outer4 + query6.
    for (limit, bytes) in [(38, peak), (37, peak), (38, peak - 1)] {
        let called = Cell::new(false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, bytes);
        budget.reserve_storage(floor).unwrap();
        let result = with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
            scope.with_inventory_v1(|inventory, budget| -> ScopeResult<()> {
                called.set(true);
                let body = owner.module().functions[0].body.as_ref().unwrap();
                assert!(std::ptr::eq(
                    inventory.operations()[0].operation,
                    &body.blocks[0].operations[0]
                ));
                assert_eq!(inventory.definitions()[0].value, Some(ValueId(u32::MAX)));
                assert_eq!(inventory.operations().len(), 1);
                assert_eq!(inventory.uses().len(), 1);
                assert!(inventory.edges().is_empty());
                assert_eq!(budget.storage(), peak);
                assert_eq!(budget.work(), 38);
                Ok(())
            })
        });
        assert_eq!(budget.storage(), floor);
        match (limit, bytes == peak, result) {
            (38, true, Ok(())) => {
                assert!(called.get());
                assert_eq!(budget.work(), 38);
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.failed_storage(), None);
            }
            (37, true, Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error)))) => {
                assert!(!called.get());
                assert_eq!((error.actual(), error.limit(), budget.work()), (38, 37, 32));
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.failed_storage(), None);
            }
            (
                38,
                false,
                Err(CanonicalAnalysisScopeErrorV1::Inventory(
                    CanonicalKirInventoryErrorV1::Resource(Resource::Storage(error)),
                )),
            ) => {
                assert!(!called.get());
                assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
                assert_eq!(budget.work(), 4 + 7 + 8);
                assert_eq!(budget.peak_storage(), peak - value_index);
                assert_eq!(budget.failed_storage(), Some(peak));
            }
            (_, _, other) => panic!("wrong nonempty callback boundary: {other:?}"),
        }
        drop(budget);
        assert_eq!(work.failed_work(), (limit == 37).then_some(38));
    }
}

#[test]
fn inventory_callback_keeps_duplicate_edges_and_exact_argument_occurrences() {
    let mut module = boolean_module(true);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(u32::MAX),
        then_target: BlockId(7),
        then_arguments: vec![ValueId(u32::MAX)],
        else_target: BlockId(7),
        else_arguments: vec![ValueId(u32::MAX)],
    });
    let mut exit = BasicBlock::new(BlockId(7));
    exit.parameters.push(ValueDef::new(ValueId(9), Type::BOOL));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(9)],
    });
    body.blocks.push(exit);
    let (owner, retained) = admit(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(retained + 17).unwrap();
    with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
        scope.with_inventory_v1(|inventory, _| -> ScopeResult<()> {
            assert_eq!(inventory.edges().len(), 2);
            assert_eq!(inventory.edge_arguments().len(), 2);
            for (ordinal, edge) in inventory.edges().iter().enumerate() {
                assert_eq!(edge.coordinate.successor as usize, ordinal);
                assert_eq!(edge.target_id, BlockId(7));
                assert_eq!(edge.arguments, &[ValueId(u32::MAX)]);
            }
            assert_ne!(
                inventory.edges()[0].coordinate,
                inventory.edges()[1].coordinate
            );
            assert!(inventory.belongs_to(&owner));
            Ok(())
        })
    })
    .unwrap();
    assert_eq!(budget.storage(), retained + 17);
}

#[test]
fn inventory_callback_reuses_both_cache_orders_without_deriving_again() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let header = size_of::<CanonicalKirInventoryV1<'_>>();
    let sparse = size_of::<CanonicalKirSparseV1<'_, '_>>();
    let memory = size_of::<CanonicalKirMemorySsaV1<'_, '_>>();
    for memory_first in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(floor).unwrap();
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
            let pointer = std::ptr::from_ref(scope.inventory());
            scope.with_inventory_v1(|inventory, budget| -> ScopeResult<()> {
                assert_eq!(std::ptr::from_ref(inventory), pointer);
                assert_eq!(budget.work(), 12);
                assert_eq!(budget.storage(), floor + header);
                Ok(())
            })?;
            assert!(scope.sparse.is_none() && scope.memory_ssa.is_none());
            for (ordinal, is_memory) in [memory_first, !memory_first].into_iter().enumerate() {
                if is_memory {
                    scope.with_memory_ssa_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))?;
                } else {
                    scope.with_sparse_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))?;
                }
                scope.with_inventory_v1(|inventory, budget| -> ScopeResult<()> {
                    assert_eq!(std::ptr::from_ref(inventory), pointer);
                    let expected = if ordinal == 1 {
                        88
                    } else if memory_first {
                        66
                    } else {
                        34
                    };
                    assert_eq!(budget.work(), expected);
                    Ok(())
                })?;
            }
            let sparse_pointer = std::ptr::from_ref(scope.sparse.as_ref().unwrap());
            let memory_pointer = std::ptr::from_ref(scope.memory_ssa.as_ref().unwrap());
            scope.with_sparse_v1(|report, budget| -> ScopeResult<()> {
                assert_eq!(std::ptr::from_ref(report), sparse_pointer);
                assert_eq!(std::ptr::from_ref(report.inventory()), pointer);
                assert_eq!(budget.storage(), floor + header + sparse + memory);
                Ok(())
            })?;
            scope.with_memory_ssa_v1(|report, budget| -> ScopeResult<()> {
                assert_eq!(std::ptr::from_ref(report), memory_pointer);
                assert_eq!(std::ptr::from_ref(report.inventory()), pointer);
                assert_eq!(budget.work(), 100);
                Ok(())
            })
        })
        .unwrap();
        let scratch_peak = floor
            + header
            + sparse
            + sparse_engine_extra_header_bytes()
            + if memory_first { memory } else { 0 };
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), 100);
        assert_eq!(
            budget.peak_storage(),
            scratch_peak.max(floor + header + sparse + memory)
        );
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}

fn inventory_callback_exit(mode: u8) -> ScopeResult<()> {
    match mode {
        0 => Ok(()),
        1 => Err(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Arithmetic,
        )),
        _ => std::panic::panic_any("inventory callback sentinel"),
    }
}

#[test]
fn inventory_callback_clean_error_and_panic_preserve_caches_and_refund_scratch() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let cache_floor = floor
        + size_of::<CanonicalKirInventoryV1<'_>>()
        + size_of::<CanonicalKirSparseV1<'_, '_>>()
        + size_of::<CanonicalKirMemorySsaV1<'_, '_>>();
    let sparse_peak = floor
        + size_of::<CanonicalKirInventoryV1<'_>>()
        + size_of::<CanonicalKirSparseV1<'_, '_>>()
        + sparse_engine_extra_header_bytes();
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(82);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(floor).unwrap();
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
            scope.with_sparse_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))?;
            scope.with_memory_ssa_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))?;
            let sparse_pointer = std::ptr::from_ref(scope.sparse.as_ref().unwrap());
            let memory_pointer = std::ptr::from_ref(scope.memory_ssa.as_ref().unwrap());
            let result = catch_unwind(AssertUnwindSafe(|| {
                scope.with_inventory_v1(|_, budget| {
                    budget.reserve_storage(19).unwrap();
                    // Logical fixture scratch has no backing surviving this callback.
                    inventory_callback_exit(mode)
                })
            }));
            match (mode, result) {
                (0, Ok(Ok(()))) => {}
                (1, Ok(Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic)))) => {}
                (2, Err(payload)) => assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"inventory callback sentinel")
                ),
                (_, other) => panic!("changed inventory callback result: {other:?}"),
            }
            assert_eq!(scope.budget.storage(), cache_floor);
            assert_eq!(
                std::ptr::from_ref(scope.sparse.as_ref().unwrap()),
                sparse_pointer
            );
            assert_eq!(
                std::ptr::from_ref(scope.memory_ssa.as_ref().unwrap()),
                memory_pointer
            );
            scope.with_inventory_v1(|_, budget| -> ScopeResult<()> {
                assert_eq!(budget.work(), 82);
                assert_eq!(budget.storage(), cache_floor);
                Ok(())
            })
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), sparse_peak.max(cache_floor + 19));
        assert_eq!(budget.work(), 82);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn inventory_callback_floor_loss_is_sticky_without_recreating_incoming_storage() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let header = size_of::<CanonicalKirInventoryV1<'_>>();
    for below_incoming in [false, true] {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(12);
            let mut budget = Budget::new(&mut work, floor + header);
            budget.reserve_storage(floor).unwrap();
            let result =
                with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
                    let failed = scope.with_inventory_v1(|_, budget| {
                        let release = if below_incoming { header + 1 } else { 1 };
                        budget.release_storage(release).unwrap();
                        inventory_callback_exit(mode)
                    });
                    assert!(matches!(
                        failed,
                        Err(CanonicalAnalysisScopeErrorV1::Resource(
                            Resource::Accounting
                        ))
                    ));
                    assert!(scope.poisoned.get());
                    assert_eq!(scope.budget.work(), 12);
                    let retry: ScopeResult<()> =
                        scope.with_inventory_v1(|_, _| panic!("poisoned callback"));
                    assert!(matches!(
                        retry,
                        Err(CanonicalAnalysisScopeErrorV1::Resource(
                            Resource::Accounting
                        ))
                    ));
                    assert_eq!(scope.budget.work(), 12);
                    Ok(())
                });
            assert!(matches!(
                result,
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert_eq!(
                budget.storage(),
                if below_incoming { floor - 1 } else { floor }
            );
            assert_eq!(budget.peak_storage(), floor + header);
            assert_eq!(budget.work(), 12);
            assert_eq!(budget.failed_storage(), None);
            drop(budget);
            assert_eq!(work.failed_work(), None);
        }
    }
}

#[test]
fn inventory_callback_slot_mismatch_and_recovery_never_reenable_the_scope() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(floor).unwrap();
    let result =
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
            let original_slot = scope.slot;
            // Private hostile slot marker tests the same check without forging a reference.
            scope.slot ^= 1;
            let failed: ScopeResult<()> = scope.with_inventory_v1(|_, _| panic!("wrong slot"));
            assert!(matches!(
                failed,
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert_eq!(scope.budget.work(), 6);
            scope.slot = original_slot;
            let retry: ScopeResult<()> =
                scope.with_inventory_v1(|_, _| panic!("restored poisoned slot"));
            assert!(matches!(
                retry,
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert_eq!(scope.budget.work(), 6);
            Ok(())
        });
    assert!(matches!(
        result,
        Err(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 6);
    drop(budget);
    assert_eq!(work.failed_work(), None);
}

fn inventory_callback_replace_ledger<'work>(
    owner: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'work>,
    foreign: &mut Budget<'work>,
    mode: u8,
) -> ScopeResult<()> {
    with_canonical_analysis_scope_v1(owner, budget, |scope| {
        scope.with_inventory_v1(|_, budget| {
            foreign.reserve_storage(budget.storage()).unwrap();
            std::mem::swap(budget, foreign);
            inventory_callback_exit(mode)
        })
    })
}

#[test]
fn inventory_callback_same_slot_foreign_ledger_is_neither_debited_nor_refunded() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let live = floor + size_of::<CanonicalKirInventoryV1<'_>>();
    for mode in 0..3 {
        let mut original_work = CanonicalKernelIrWorkBudgetV1::new(12);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = Budget::new(&mut original_work, live);
        let mut foreign = Budget::new(&mut foreign_work, live);
        budget.reserve_storage(floor).unwrap();
        let original_ledger = budget.work_ledger_identity_v1();
        let foreign_ledger = foreign.work_ledger_identity_v1();
        let result = inventory_callback_replace_ledger(&owner, &mut budget, &mut foreign, mode);
        assert!(matches!(
            result,
            Err(CanonicalAnalysisScopeErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert!(budget.work_ledger_identity_v1() == foreign_ledger);
        assert!(foreign.work_ledger_identity_v1() == original_ledger);
        assert_eq!((budget.work(), foreign.work()), (0, 12));
        assert_eq!((budget.storage(), foreign.storage()), (live, live));
        assert_eq!(
            (budget.peak_storage(), foreign.peak_storage()),
            (live, live)
        );
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(foreign.failed_storage(), None);
        // All scope owners have dropped; only this caller can restore the displaced ledger.
        std::mem::swap(&mut budget, &mut foreign);
        budget.release_storage(live - floor).unwrap();
        foreign.release_storage(live).unwrap();
        assert_eq!(budget.storage(), floor);
        drop(budget);
        drop(foreign);
        assert_eq!(original_work.failed_work(), None);
        assert_eq!(foreign_work.failed_work(), None);
    }
}

#[test]
fn inventory_callback_foreign_entry_poison_survives_restoring_original_ledger() {
    let (owner, retained) = admit(&Module::new("empty"));
    let floor = retained + 17;
    let live = floor + size_of::<CanonicalKirInventoryV1<'_>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work, live);
    let mut foreign = Budget::new(&mut foreign_work, live);
    budget.reserve_storage(floor).unwrap();
    foreign.reserve_storage(live).unwrap();
    let result =
        with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| -> ScopeResult<()> {
            std::mem::swap(scope.budget, &mut foreign);
            let failed: ScopeResult<()> = scope.with_inventory_v1(|_, _| panic!("foreign entry"));
            assert!(matches!(
                failed,
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert_eq!((scope.budget.work(), scope.budget.storage()), (0, live));
            std::mem::swap(scope.budget, &mut foreign);
            let retry: ScopeResult<()> =
                scope.with_inventory_v1(|_, _| panic!("poisoned restored ledger"));
            assert!(matches!(
                retry,
                Err(CanonicalAnalysisScopeErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert_eq!(scope.budget.work(), 6);
            Ok(())
        });
    assert!(matches!(
        result,
        Err(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!((budget.storage(), foreign.storage()), (floor, live));
    assert_eq!((budget.work(), foreign.work()), (6, 0));
    assert_eq!(foreign.failed_storage(), None);
}

struct InventoryCallbackDropBudget<'a, 'work> {
    budget: Option<Budget<'work>>,
    recovered: &'a std::cell::RefCell<Option<Budget<'work>>>,
    observed: &'a Cell<Option<(usize, usize, CanonicalKernelIrWorkLedgerIdentityV1)>>,
}

impl Drop for InventoryCallbackDropBudget<'_, '_> {
    fn drop(&mut self) {
        let budget = self.budget.take().unwrap();
        // This reads an exclusively owned displaced budget, not an aliased pointer.
        self.observed.set(Some((
            budget.storage(),
            budget.work(),
            budget.work_ledger_identity_v1(),
        )));
        assert!(self.recovered.borrow_mut().replace(budget).is_none());
    }
}

fn inventory_callback_reject_owned_budget<'work>(
    owner: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'work>,
    replacement: Budget<'work>,
    recovered: &std::cell::RefCell<Option<Budget<'work>>>,
    observed: &Cell<Option<(usize, usize, CanonicalKernelIrWorkLedgerIdentityV1)>>,
) -> ScopeResult<()> {
    with_canonical_analysis_scope_v1(owner, budget, |scope| -> ScopeResult<()> {
        let result = scope.with_inventory_v1(|_, budget| {
            let original = std::mem::replace(budget, replacement);
            Ok::<_, CanonicalAnalysisScopeErrorV1>(InventoryCallbackDropBudget {
                budget: Some(original),
                recovered,
                observed,
            })
        });
        assert!(matches!(
            result,
            Err(CanonicalAnalysisScopeErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert!(observed.get().is_some());
        assert!(scope.poisoned.get());
        Ok(())
    })
}

#[test]
fn inventory_callback_rejected_result_observes_displaced_backing_paid_during_drop() {
    let (owner, retained) = admit(&Module::new("empty"));
    let output_headers = size_of::<InventoryCallbackDropBudget<'_, '_>>()
        + size_of::<std::cell::RefCell<Option<Budget<'_>>>>()
        + size_of::<Cell<Option<(usize, usize, CanonicalKernelIrWorkLedgerIdentityV1)>>>();
    let floor = retained + 17 + output_headers;
    let live = floor + size_of::<CanonicalKirInventoryV1<'_>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(12);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work, live);
    let mut replacement = Budget::new(&mut foreign_work, live);
    budget.reserve_storage(floor).unwrap();
    replacement.reserve_storage(live).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let foreign_ledger = replacement.work_ledger_identity_v1();
    let recovered = std::cell::RefCell::new(None);
    let observed = Cell::new(None);
    let result = inventory_callback_reject_owned_budget(
        &owner,
        &mut budget,
        replacement,
        &recovered,
        &observed,
    );
    assert!(matches!(
        result,
        Err(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert!(observed.get() == Some((live, 12, ledger)));
    assert!(budget.work_ledger_identity_v1() == foreign_ledger);
    assert_eq!((budget.storage(), budget.work()), (live, 0));
    let mut original = recovered.borrow_mut().take().unwrap();
    assert_eq!((original.storage(), original.work()), (live, 12));
    // The returned value and all scope dependencies are now gone.
    std::mem::swap(&mut budget, &mut original);
    budget.release_storage(live - floor).unwrap();
    original.release_storage(live).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(original);
    drop(recovered);
    drop(budget);
    assert_eq!(work.failed_work(), None);
    assert_eq!(foreign_work.failed_work(), None);
}
