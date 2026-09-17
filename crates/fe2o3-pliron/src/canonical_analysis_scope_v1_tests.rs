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
use std::mem::size_of;

type ScopeResult<T> = Result<T, CanonicalAnalysisScopeErrorV1>;

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
    let payload =
        size_of::<CanonicalKirInventoryV1<'_>>() + size_of::<CanonicalKirSparseV1<'_, '_>>();
    // Outer checks4 + inventory2 + first request6 + sparse8 + transfer2 + hit6.
    // An empty graph has no row or worklist allocations.
    for (allowance, storage, success) in [
        (28, payload, true),
        (27, payload, false),
        (28, payload - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + allowance);
        work.charge_work(5).unwrap();
        let mut budget = Budget::new(&mut work, floor + storage);
        budget.reserve_storage(floor).unwrap();
        let result = with_canonical_analysis_scope_v1(&owner, &mut budget, |scope| {
            scope.with_sparse_v1(|_, budget| -> ScopeResult<()> {
                assert_eq!(budget.storage(), floor + payload);
                Ok(())
            })?;
            scope.with_sparse_v1(|_, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(()))
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(budget.storage(), floor);
        match result {
            Ok(()) => {
                assert_eq!(budget.work(), 33);
                assert_eq!(budget.peak_storage(), floor + payload);
            }
            Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Work(error))) => {
                assert_eq!(error.actual(), 33);
                assert_eq!(budget.work(), 27);
                assert_eq!(budget.peak_storage(), floor + payload);
            }
            Err(CanonicalAnalysisScopeErrorV1::Sparse(CanonicalKirSparseErrorV1::Resource(
                Resource::Storage(error),
            ))) => {
                assert_eq!(error.actual(), floor + payload);
                assert_eq!(budget.work(), 25);
                assert_eq!(budget.failed_storage(), Some(floor + payload));
            }
            other => panic!("unexpected independent boundary result: {other:?}"),
        }
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
    let peak = floor + inventory + sparse + scratch;
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
        } else {
            assert!(matches!(result,
                Err(CanonicalAnalysisScopeErrorV1::Sparse(CanonicalKirSparseErrorV1::Resource(
                    Resource::Storage(error)))) if error.actual() == peak));
            assert_eq!(budget.peak_storage(), peak - size_of::<usize>());
            assert_eq!(budget.failed_storage(), Some(peak));
        }
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
