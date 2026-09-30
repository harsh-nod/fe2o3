use super::super::storage_v18::tests::{
    Input, LIMITS, SPACE, WORK, constant, diamond, fixture, input,
};
use super::*;
use fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, CheckedBinaryOperator,
    Constant, Function, Module, Operation, OperationKind as Op, ScalarType, Signature, Terminator,
    Type, ValueDef, ValueId,
};

const U32: Type = Type::Scalar(ScalarType::U32);

fn observed<'a>(
    source: &'a Input,
    budget: &mut Budget<'_>,
) -> KirNeutralOptimizationOutputMixedFixedpointV18<'a> {
    let floor = budget.storage();
    let value =
        optimize_neutral_kernel_ir_mixed_fixedpoint_v18(&source.owner, LIMITS, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(value.storage().retained_storage())
        .unwrap();
    value
}

fn operations(owner: &Owner) -> impl Iterator<Item = &Operation> {
    owner
        .module()
        .functions
        .iter()
        .flat_map(|f| f.body.iter())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
}

fn binary(id: u32, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), U32),
        Op::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn select(id: u32, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), U32),
        Op::Select {
            condition: ValueId(0),
            true_value: ValueId(lhs),
            false_value: ValueId(rhs),
        },
    )
}

fn cascade() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        constant(2, 0),
        constant(3, 0),
        select(4, 2, 3),
        binary(5, BinaryOp::Add, 1, 4),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    let mut module = Module::new("mixed-fixedpoint-cascade");
    module.functions.push(Function::internal_helper(
        "cascade",
        Signature::new(vec![Type::BOOL, U32], vec![U32]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

// Each Select unlocks exactly one following integer identity. Alternating
// zero-add and one-multiply prevents the worklist from shortcutting the chain.
fn dependent_rounds(levels: usize) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![constant(1, 0), constant(2, 0), constant(3, 1)];
    let mut previous = 2;
    for level in 0..levels {
        let id = 4 + 2 * u32::try_from(level).unwrap();
        let (neutral, other, op) = if level % 2 == 0 {
            (1, 3, BinaryOp::Add)
        } else {
            (3, 1, BinaryOp::Multiply)
        };
        block.operations.push(select(id, previous, neutral));
        block.operations.push(binary(id + 1, op, other, id));
        previous = id + 1;
    }
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(previous)],
    });
    let mut module = Module::new("mixed-fixedpoint-dependent-rounds");
    module.functions.push(Function::internal_helper(
        "cascade",
        Signature::new(vec![Type::BOOL], vec![U32]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

#[test]
fn mixed_fixedpoint_cse_enables_select_then_integer_and_checks_terminal_round() {
    let source = input(&cascade());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    let floor = source.storage + 17;
    budget.reserve_storage(floor).unwrap();
    let historical =
        crate::optimize_neutral_kernel_ir_mixed_pure_cse_v18(&source.owner, LIMITS, &mut budget)
            .unwrap();
    assert_eq!(
        &historical.execution().canonical_bytes()[..8],
        &[10, 0, 1, 0, 4, 0, 18, 0]
    );
    assert_eq!(historical.execution().canonical_bytes().len(), 576);
    assert!(operations(historical.owner()).any(|op| matches!(op.kind, Op::Select { .. })));
    assert!(operations(historical.owner()).any(|op| matches!(
        op.kind,
        Op::Binary {
            op: BinaryOp::Add,
            ..
        }
    )));
    drop(historical);
    let value = observed(&source, &mut budget);
    assert_eq!(value.execution().policy_version(), 11);
    assert_eq!(value.execution().graph_schema(), 18);
    assert_eq!(value.execution().rounds(), 3);
    assert!(!value.execution().grants_authority());
    assert_eq!(
        &value.execution().canonical_bytes()[..8],
        &[11, 0, 1, 0, 15, 0, 18, 0]
    );
    assert_eq!(value.execution().canonical_bytes().len(), 352 + 15 * 60);
    assert!(operations(value.owner()).next().is_none());
    let passes = value.report().passes();
    assert!(!passes[0].changed() && !passes[1].changed() && passes[2].changed());
    assert!(passes[5].changed() && passes[6].changed());
    assert!(passes[10..].iter().all(|p| !p.changed()
        && p.input_epoch() == p.output_epoch()
        && p.input_graph_work() == p.output_graph_work()));
    for round in passes.chunks_exact(5) {
        assert_eq!(
            round.iter().map(|p| p.pass()).collect::<Vec<_>>(),
            crate::fixed_policy_v3::POLICY11_PASSES
        );
    }
    assert!(value.map().matches_execution(value.report()));
    value
        .map()
        .check_against(&source.owner, value.owner(), &mut budget)
        .unwrap();
    let checked = value.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(checked.input_audit_bytes(), source.owner.canonical_bytes());
    assert!(!checked.grants_authority());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn mixed_fixedpoint_requires_terminal_round_within_exact_iteration_cap() {
    use crate::PlironOptimizationErrorV1 as E;
    for (levels, accepted) in [(30, true), (31, false)] {
        let source = input(&dependent_rounds(levels));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        let floor = source.storage + 19;
        budget.reserve_storage(floor).unwrap();
        let result =
            optimize_neutral_kernel_ir_mixed_fixedpoint_v18(&source.owner, LIMITS, &mut budget);
        assert_eq!(budget.storage(), floor);
        if accepted {
            let value = result.unwrap();
            assert_eq!(value.execution().rounds(), 32);
            assert_eq!(value.report().passes().len(), 160);
            budget
                .reserve_storage(value.storage().retained_storage())
                .unwrap();
            value.try_check_and_finish_v18(&mut budget).unwrap();
        } else {
            assert!(matches!(
                result,
                Err(KirNeutralOptimizationErrorV18::Pass(
                    E::FixedpointRoundLimitExceeded {
                        completed: 32,
                        limit: 32
                    }
                ))
            ));
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn mixed_fixedpoint_equal_work_checked_neutralization_is_not_convergence() {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        constant(1, 0),
        Operation::checked_binary(
            ValueDef::new(ValueId(2), U32),
            ValueDef::new(ValueId(3), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(0),
            ValueId(1),
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2), ValueId(3)],
    });
    let mut module = Module::new("mixed-fixedpoint-checked-neutral");
    module.functions.push(Function::internal_helper(
        "checked",
        Signature::new(vec![U32], vec![U32, Type::BOOL]),
        vec![ValueId(0)],
        vec![block],
    ));
    let source = input(&module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(source.storage).unwrap();
    let value = observed(&source, &mut budget);
    let integer = value.report().passes()[1];
    assert!(integer.changed());
    assert_eq!(integer.input_graph_work(), integer.output_graph_work());
    assert_eq!(
        integer.output_epoch().sequence(),
        integer.input_epoch().sequence() + 1
    );
    assert_eq!(value.execution().rounds(), 2);
    assert!(
        operations(value.owner()).any(|op| matches!(op.kind, Op::Constant(Constant::Bool(false))))
    );
    value.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), source.storage);
}

#[test]
fn mixed_fixedpoint_is_deterministic_across_fresh_contexts() {
    let source = input(&cascade());
    let run = || {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(source.storage).unwrap();
        let value = observed(&source, &mut budget);
        let evidence = (
            value.owner().canonical_bytes().to_vec(),
            *value.map().digest(),
            value.report().clone(),
        );
        value.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(budget.storage(), source.storage);
        evidence
    };
    assert_eq!(run(), run());
}

#[test]
fn mixed_fixedpoint_preserves_memory_execution_and_loop_cfg() {
    for module in [fixture(), diamond(false), diamond(true)] {
        let source = input(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(source.storage).unwrap();
        let value = observed(&source, &mut budget);
        for predicate in [
            (|op: &Operation| matches!(op.kind, Op::Storage(_))) as fn(&Operation) -> bool,
            |op: &Operation| matches!(op.kind, Op::Execution(_)),
        ] {
            assert_eq!(
                operations(&source.owner).filter(|op| predicate(op)).count(),
                operations(value.owner()).filter(|op| predicate(op)).count()
            );
        }
        let cfg = |owner: &Owner| {
            owner
                .module()
                .functions
                .iter()
                .flat_map(|f| f.body.iter())
                .flat_map(|b| &b.blocks)
                .map(|b| b.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(cfg(&source.owner), cfg(value.owner()));
        assert!(
            value
                .occurrences()
                .candidate()
                .edges
                .iter()
                .all(|e| e.input == e.output)
        );
        value.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(budget.storage(), source.storage);
    }
}

#[test]
fn mixed_fixedpoint_one_round_has_exact_and_one_short_execution_limits() {
    let source = input(&Module::new("empty-mixed-fixedpoint-boundary"));
    let floor = source.storage + 23;
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result =
            optimize_neutral_kernel_ir_mixed_fixedpoint_v18(&source.owner, LIMITS, &mut budget);
        if let Ok(value) = &result {
            assert_eq!(value.execution().rounds(), 1);
        }
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
    let (full, failed) = run(WORK, SPACE);
    assert!(full.0);
    assert_eq!(failed, None);
    // One terminal round must not prepay the 32-round maximum.
    assert!(full.1 < 4 * crate::fixed_policy_v3::POLICY3_SESSION_WORK_CAP);
    assert_eq!(run(full.1, full.2).0, full);
    let short_work = run(full.1 - 1, full.2);
    assert!(!short_work.0.0 && short_work.1.is_some());
    assert_eq!(short_work.0.3, None);
    let short_storage = run(full.1, full.2 - 1);
    assert!(!short_storage.0.0 && short_storage.0.3.is_some());
    assert_eq!(short_storage.1, None);
}

#[test]
fn mixed_fixedpoint_multiround_adoption_has_exact_and_one_short_headroom() {
    let source = input(&cascade());
    let floor = source.storage + 29;
    let run = |work_headroom: usize, storage_headroom: usize| {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(floor).unwrap();
        let value = observed(&source, &mut budget);
        assert_eq!(value.execution().rounds(), 3);
        // Normalize only adoption headroom; live pointer-index probes can vary.
        budget
            .charge_work(WORK - budget.work() - work_headroom)
            .unwrap();
        let padding = SPACE - budget.storage() - storage_headroom;
        let old_peak = budget.peak_storage();
        budget.reserve_storage(padding).unwrap();
        assert!(budget.storage() > old_peak);
        let start = (budget.work(), budget.storage());
        let result = value.try_check_and_finish_v18(&mut budget);
        let kind = match &result {
            Ok(_) => 0,
            Err(KirCheckedNeutralOptimizationErrorV1::Resource(Resource::Work(_)))
            | Err(KirCheckedNeutralOptimizationErrorV1::Inventory(
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(Resource::Work(_)),
            ))
            | Err(KirCheckedNeutralOptimizationErrorV1::Transition(
                CanonicalKirTransitionErrorV1::Resource(Resource::Work(_)),
            )) => 1,
            Err(KirCheckedNeutralOptimizationErrorV1::Resource(Resource::Storage(_)))
            | Err(KirCheckedNeutralOptimizationErrorV1::Inventory(
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(Resource::Storage(_)),
            ))
            | Err(KirCheckedNeutralOptimizationErrorV1::Transition(
                CanonicalKirTransitionErrorV1::Resource(Resource::Storage(_)),
            )) => 2,
            Err(error) => panic!("unexpected adoption refusal: {error:?}"),
        };
        drop(result);
        assert_eq!(budget.storage(), floor + padding);
        let state = (
            kind,
            budget.work() - start.0,
            budget.peak_storage() - start.1,
            budget.failed_storage(),
        );
        budget.release_storage(padding).unwrap();
        assert_eq!(budget.storage(), floor);
        drop(budget);
        (state, work.failed_work())
    };
    let (full, failed) = run(WORK / 2, SPACE / 2);
    assert_eq!(full.0, 0);
    assert_eq!(failed, None);
    assert_eq!(run(full.1, full.2).0, full);
    let short_work = run(full.1 - 1, full.2);
    assert_eq!(short_work.0.0, 1);
    assert!(short_work.1.is_some());
    assert_eq!(short_work.0.3, None);
    let short_storage = run(full.1, full.2 - 1);
    assert_eq!(short_storage.0.0, 2);
    assert!(short_storage.0.3.is_some());
    assert_eq!(short_storage.1, None);
}

#[test]
fn mixed_fixedpoint_lost_or_foreign_custody_never_calls_adoption_callback() {
    use std::cell::Cell;
    let source = input(&cascade());
    for foreign in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(source.storage + 31).unwrap();
        let value = observed(&source, &mut budget);
        let called = Cell::new(false);
        let result = if foreign {
            let mut other_work = Work::new(WORK);
            let mut other = Budget::new(&mut other_work, SPACE);
            other.reserve_storage(budget.storage()).unwrap();
            value.try_check_and_finish_with_v18(&mut other, |_, _| {
                called.set(true);
                Ok::<_, Infallible>(((), 0))
            })
        } else {
            budget.release_storage(1).unwrap();
            value.try_check_and_finish_with_v18(&mut budget, |_, _| {
                called.set(true);
                Ok::<_, Infallible>(((), 0))
            })
        };
        assert!(matches!(
            result,
            Err(KirCheckedNeutralOptimizationErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert!(!called.get());
    }
}
