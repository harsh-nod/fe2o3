use super::super::storage_v18::tests::{Input, LIMITS, SPACE, WORK, add, constant, input};
use super::*;
use fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Module, OperationKind,
    ScalarType, Signature, Terminator, Type, ValueId,
};

macro_rules! early_adoption_capture_refusal {
    ($name:ident, $optimize:path) => {
        #[test]
        fn $name() {
            use std::{cell::Cell, rc::Rc};
            struct Hostile(Rc<Cell<usize>>);
            impl Drop for Hostile {
                fn drop(&mut self) {
                    self.0.set(self.0.get() + 1);
                    panic!("uncalled integer-policy capture destructor");
                }
            }
            let source = input(&reverse_chain(3));
            for fault in 0..3 {
                let mut work = Work::new(WORK);
                let mut budget = Budget::new(&mut work, SPACE);
                budget.reserve_storage(source.storage + 71).unwrap();
                let floor = budget.storage();
                let value = $optimize(&source.owner, LIMITS, &mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget
                    .reserve_storage(value.storage().retained_storage())
                    .unwrap();
                let dropped = Rc::new(Cell::new(0));
                let called = Rc::new(Cell::new(0));
                let observed_call = Rc::clone(&called);
                let hostile = Hostile(Rc::clone(&dropped));
                let result = match fault {
                    0 => {
                        let original = (budget.storage(), budget.work());
                        let mut foreign_work = Work::new(WORK);
                        let mut foreign = Budget::new(&mut foreign_work, SPACE);
                        foreign.reserve_storage(original.0).unwrap();
                        let before = (foreign.storage(), foreign.work());
                        let result =
                            value.try_check_and_finish_with_v18(&mut foreign, move |_, _| {
                                let _capture = hostile;
                                observed_call.set(observed_call.get() + 1);
                                Ok::<_, Infallible>(((), 0))
                            });
                        assert_eq!((foreign.storage(), foreign.work()), before);
                        assert_eq!((budget.storage(), budget.work()), original);
                        result
                    }
                    1 => {
                        budget.release_storage(1).unwrap();
                        let before = (budget.storage(), budget.work());
                        let result =
                            value.try_check_and_finish_with_v18(&mut budget, move |_, _| {
                                let _capture = hostile;
                                observed_call.set(observed_call.get() + 1);
                                Ok::<_, Infallible>(((), 0))
                            });
                        assert_eq!((budget.storage(), budget.work()), before);
                        result
                    }
                    _ => {
                        let original_slot = std::ptr::from_ref(&budget) as usize;
                        let mut moved = Box::new(budget);
                        assert_ne!(original_slot, std::ptr::from_ref(&*moved) as usize);
                        let before = (moved.storage(), moved.work());
                        let result =
                            value.try_check_and_finish_with_v18(&mut moved, move |_, _| {
                                let _capture = hostile;
                                observed_call.set(observed_call.get() + 1);
                                Ok::<_, Infallible>(((), 0))
                            });
                        assert_eq!((moved.storage(), moved.work()), before);
                        result
                    }
                };
                assert!(matches!(
                    result,
                    Err(KirCheckedNeutralOptimizationErrorV1::Resource(
                        Resource::Accounting
                    ))
                ));
                assert_eq!(called.get(), 0);
                assert_eq!(dropped.get(), 1);
            }
        }
    };
}

early_adoption_capture_refusal!(
    scalar_policy3_uncalled_capture_preserves_early_accounting_refusal,
    crate::optimize_neutral_kernel_ir_v18
);
early_adoption_capture_refusal!(
    integer_policy6_uncalled_capture_preserves_early_accounting_refusal,
    crate::optimize_neutral_kernel_ir_integer_continuation_v18
);
early_adoption_capture_refusal!(
    integer_policy9_uncalled_capture_preserves_early_accounting_refusal,
    crate::optimize_neutral_kernel_ir_integer_worklist_v18
);

fn reverse_chain(depth: u32) -> Module {
    assert!(depth > 0);
    let mut module = Module::new("integer-worklist");
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(constant(1, 0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(depth + 1),
        arguments: vec![],
    });
    let mut use_block = BasicBlock::new(BlockId(1));
    use_block.operations.push(add(2, 0, 3));
    use_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut blocks = vec![entry, use_block];
    for i in 0..depth {
        let mut block = BasicBlock::new(BlockId(i + 2));
        let operand = if i + 1 == depth { 1 } else { i + 4 };
        block.operations.push(add(i + 3, operand, operand));
        block.terminator = Some(Terminator::Branch {
            target: BlockId(i + 1),
            arguments: vec![],
        });
        blocks.push(block);
    }
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32)],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0)],
        blocks,
    ));
    module
}

fn observed<'a>(
    source: &'a Input,
    budget: &mut Budget<'_>,
) -> KirNeutralOptimizationOutputIntegerWorklistV18<'a> {
    let floor = budget.storage();
    let value =
        optimize_neutral_kernel_ir_integer_worklist_v18(&source.owner, LIMITS, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(value.storage().retained_storage())
        .unwrap();
    value
}

fn binary_count(owner: &Owner) -> usize {
    owner
        .module()
        .functions
        .iter()
        .flat_map(|f| f.body.iter())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
        .filter(|op| matches!(op.kind, OperationKind::Binary { .. }))
        .count()
}

#[test]
fn actual_v18_worklist_closes_cross_block_exposure_without_changing_historical_policy6() {
    for depth in [1, 4, 16] {
        let source = input(&reverse_chain(depth));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        let floor = 29 + source.storage;
        budget.reserve_storage(floor).unwrap();
        let old = crate::optimize_neutral_kernel_ir_integer_continuation_v18(
            &source.owner,
            LIMITS,
            &mut budget,
        )
        .unwrap();
        assert!(binary_count(old.owner()) > 0);
        assert_eq!(old.execution().policy_version(), 6);
        drop(old);
        let value = observed(&source, &mut budget);
        assert_eq!(binary_count(value.owner()), 0);
        assert_eq!(value.execution().policy_version(), 9);
        assert_eq!(value.execution().graph_schema(), 18);
        assert!(!value.execution().grants_authority());
        assert_eq!(
            value
                .report()
                .passes()
                .iter()
                .map(|p| p.pass())
                .collect::<Vec<_>>(),
            crate::fixed_policy_v3::POLICY9_PASSES
        );
        assert!(value.map().matches_execution(value.report()));
        value
            .map()
            .check_against(&source.owner, value.owner(), &mut budget)
            .unwrap();
        let adopted = value.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(binary_count(adopted.owner()), 0);
        assert_eq!(adopted.input_audit_bytes(), source.owner.canonical_bytes());
        assert!(!adopted.grants_authority());
        assert_eq!(budget.storage(), floor);
        let body = adopted.owner().module().functions[0].body.as_ref().unwrap();
        assert_eq!(
            body.blocks[1].terminator,
            Some(Terminator::Return {
                values: vec![ValueId(0)]
            })
        );
    }
}

#[test]
fn actual_worklist_noop_and_second_execution_remain_nominal_and_exact() {
    for module in [Module::new("empty"), reverse_chain(8)] {
        let source = input(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        let floor = source.storage + 31;
        budget.reserve_storage(floor).unwrap();
        let first = observed(&source, &mut budget)
            .try_check_and_finish_v18(&mut budget)
            .unwrap();
        let bytes = first.owner().canonical_bytes().to_vec();
        let retained = first.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        let second =
            optimize_neutral_kernel_ir_integer_worklist_v18(first.owner(), LIMITS, &mut budget)
                .unwrap();
        budget
            .reserve_storage(second.storage().retained_storage())
            .unwrap();
        assert_eq!(second.owner().canonical_bytes(), bytes);
        assert!(second.report().passes().iter().all(|p| !p.changed()));
        let second = second.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(second.execution().policy_version(), 9);
        drop(second);
        drop(first);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn worklist_map_and_independent_checker_reject_foreign_owner_and_missing_rows() {
    let source = input(&reverse_chain(3));
    let foreign = input(&reverse_chain(4));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget
        .reserve_storage(source.storage + foreign.storage + 37)
        .unwrap();
    let value = observed(&source, &mut budget);
    assert_eq!(
        value
            .map()
            .check_against(&foreign.owner, value.owner(), &mut budget),
        Err(crate::KirOptimizationMapErrorV12::Identity)
    );
    let (a, a_credit) = CanonicalKirInventoryV18::derive_v18(&source.owner, &mut budget).unwrap();
    budget.reserve_storage(a_credit.retained_storage()).unwrap();
    let (b, b_credit) = CanonicalKirInventoryV18::derive_v18(value.owner(), &mut budget).unwrap();
    budget.reserve_storage(b_credit.retained_storage()).unwrap();
    let mut candidate = value.occurrences().candidate();
    candidate.functions = &[];
    let result = check_canonical_kir_transition_v18(&a, &b, candidate, &mut budget);
    assert!(matches!(
        result,
        Err(CanonicalKirTransitionErrorV1::IncompleteRows)
    ));
    drop(b);
    drop(a);
    budget
        .release_storage(a_credit.retained_storage() + b_credit.retained_storage())
        .unwrap();
    value.try_check_and_finish_v18(&mut budget).unwrap();
}

#[test]
fn worklist_actual_execution_retains_exact_work_and_storage_boundaries() {
    // Isolate the fixed entry/capture/adoption resource profile from the
    // context-dependent pointer-index probe count exercised by chain tests.
    let source = input(&Module::new("empty-worklist-boundary"));
    let floor = source.storage + 41;
    let run = |work_limit: usize, storage_limit: usize| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result =
            optimize_neutral_kernel_ir_integer_worklist_v18(&source.owner, LIMITS, &mut budget);
        let ok = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        let state = (
            ok,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        );
        drop(budget);
        (state, work.failed_work())
    };
    let (full, failure) = run(WORK, SPACE);
    assert!(full.0);
    assert_eq!(failure, None);
    assert_eq!(run(full.1, full.2).0, full);
    let short_work = run(full.1 - 1, full.2);
    assert!(!short_work.0.0);
    assert!(short_work.1.is_some());
    assert_eq!(short_work.0.3, None);
    let short_storage = run(full.1, full.2 - 1);
    assert!(!short_storage.0.0);
    assert!(short_storage.0.3.is_some());
    assert_eq!(short_storage.1, None);
}

#[test]
fn worklist_adoption_does_not_accept_a_foreign_ledger() {
    let source = input(&reverse_chain(2));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(43 + source.storage).unwrap();
    let value = observed(&source, &mut budget);
    let incoming = budget.storage();
    let mut other_work = Work::new(WORK);
    let mut other = Budget::new(&mut other_work, SPACE);
    other.reserve_storage(incoming).unwrap();
    assert!(matches!(
        value.try_check_and_finish_v18(&mut other),
        Err(KirCheckedNeutralOptimizationErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(budget.storage(), incoming);
    assert_eq!(other.storage(), incoming);
}

#[test]
fn worklist_execution_witness_rejects_the_actual_historical_two_pass_report() {
    let source = input(&Module::new("same-noop-endpoints"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(source.storage + 47).unwrap();
    let new = observed(&source, &mut budget);
    let old = crate::optimize_neutral_kernel_ir_integer_continuation_v18(
        &source.owner,
        LIMITS,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(old.storage().retained_storage())
        .unwrap();
    assert_eq!(old.owner().canonical_bytes(), new.owner().canonical_bytes());
    assert!(!new.map().matches_execution(old.report()));
    let table = source
        .owner
        .storage_table_identity_with_budget_v18(&mut budget)
        .unwrap();
    // This deliberately wrong profile cannot grant anything: the exact actual
    // Policy6 pass roster must be rejected before any profile is serialized.
    let (resources, _) = crate::optimization_v12::policy3_execution_resources_v1(
        source.owner.canonical_bytes().len(),
        1,
    )
    .unwrap();
    let before = budget.work();
    let result = IntegerWorklistExecutionWitnessV18::from_execution(
        &source.owner,
        new.owner(),
        table,
        old.report(),
        new.map(),
        crate::fixed_policy_v3::ExecutionProfileV1 {
            resources,
            registered_nodes: 1,
            cse_work: 0,
        },
        &mut budget,
    );
    assert!(matches!(result, Err(Resource::Accounting)));
    assert_eq!(budget.work() - before, 8);
}
