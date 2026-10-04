use super::*;
#[path = "checked_integer_continuation_golden_v1_tests.rs"]
mod golden;

#[path = "checked_optimization_policy6_semantic_v1_tests.rs"]
mod semantic;

use crate::{
    checked_load_forwarding_v1::tests::{STORAGE, WORK, with_owner},
    optimize_checked_canonical_kernel_ir_policy5_v1,
};
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirOperationOriginV1 as Origin, CheckedBinaryOperator, Constant, Function, Module,
    Operation, OperationKind as Kind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
use fe2o3_pliron::{
    PlironOptimizationErrorV1, PlironOptimizationLimitsV1, PlironOptimizationPassV1 as Pass,
    PlironOptimizationPlanV1, PlironSession, ShellLimits,
};

fn identity(constant: Constant, live_flag: bool) -> Module {
    let ty = constant.ty();
    let mut entry = BasicBlock::new(BlockId(17));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), ty.clone()),
            Kind::Constant(constant),
        ),
        Operation::checked_binary(
            ValueDef::new(ValueId(3), ty.clone()),
            ValueDef::new(ValueId(4), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(2),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), ty.clone()),
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(3),
                rhs: ValueId(2),
            },
        ),
    ];
    let mut results = vec![ty.clone()];
    let mut returned = vec![ValueId(5)];
    if live_flag {
        results.push(Type::BOOL);
        returned.push(ValueId(4));
    }
    entry.terminator = Some(Terminator::Return { values: returned });
    let mut module = Module::new("policy6-identities");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty], results),
        vec![ValueId(1)],
        vec![entry],
    ));
    module
}

fn binary_count(owner: &Owner) -> usize {
    owner
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, Kind::Binary { .. }))
        .count()
}

fn finish(input: &Owner, budget: &mut Budget<'_>) -> CheckedCanonicalKernelIrOwnerPolicy6V1 {
    let prefix = optimize_checked_canonical_kernel_ir_policy5_v1(input, budget).unwrap();
    budget.reserve_storage(prefix.retained_storage()).unwrap();
    let output = continue_checked_canonical_kernel_ir_policy6_v1(input, prefix, budget).unwrap();
    budget.reserve_storage(output.retained_storage()).unwrap();
    output
}

fn assert_roster(continuation: &Continuation) {
    assert_eq!(continuation.execution().policy_version(), 6);
    assert_eq!(
        continuation
            .report()
            .passes()
            .iter()
            .map(|pass| pass.pass())
            .collect::<Vec<_>>(),
        vec![
            Pass::IntegerNeutralCanonicalization,
            Pass::DeadCodeElimination
        ]
    );
    assert!(!continuation.execution().grants_authority());
}

#[test]
fn all_eight_integer_types_preserve_live_and_dead_checked_flags() {
    for zero in [
        Constant::I8(0),
        Constant::I16(0),
        Constant::I32(0),
        Constant::I64(0),
        Constant::U8(0),
        Constant::U16(0),
        Constant::U32(0),
        Constant::U64(0),
    ] {
        for live_flag in [false, true] {
            with_owner(identity(zero.clone(), live_flag), |input, budget| {
                let prefix =
                    optimize_checked_canonical_kernel_ir_policy5_v1(input, budget).unwrap();
                budget.reserve_storage(prefix.retained_storage()).unwrap();
                assert_eq!(
                    binary_count(prefix.owner()),
                    2,
                    "must mutate actual O, not pre-folded input"
                );
                let old_bytes = prefix.owner().canonical().canonical_bytes().to_vec();
                let old_record = *prefix.execution().canonical_bytes();
                let output =
                    continue_checked_canonical_kernel_ir_policy6_v1(input, prefix, budget).unwrap();
                budget.reserve_storage(output.retained_storage()).unwrap();
                assert_eq!(
                    output
                        .intermediate_policy5()
                        .owner()
                        .canonical()
                        .canonical_bytes(),
                    old_bytes
                );
                assert_eq!(
                    output.intermediate_policy5().execution().canonical_bytes(),
                    &old_record
                );
                assert_eq!(binary_count(output.owner()), 0);
                assert_roster(output.continuation());
                assert!(output.continuation().report().passes()[0].changed());
                assert!(!output.grants_authority());
                assert_eq!(output.execution().policy_version(), 6);
                assert_eq!(
                    &output.execution().canonical_bytes()[..16],
                    b"F2P6EX1\0\x01\0\x06\0\x05\0\x02\0"
                );
                let function = &output.owner().module().functions[0];
                let body = function.body.as_ref().unwrap();
                let Some(Terminator::Return { values }) = &body.blocks[0].terminator else {
                    panic!("return");
                };
                assert_eq!(values[0], body.parameters[0]);
                assert_eq!(body.blocks[0].operations.len(), usize::from(live_flag));
                if live_flag {
                    let flag = &body.blocks[0].operations[0];
                    assert_eq!(flag.kind, Kind::Constant(Constant::Bool(false)));
                    assert_eq!(values[1], flag.results[0].id);
                    assert!(
                        output
                            .continuation()
                            .occurrences()
                            .candidate()
                            .operations
                            .iter()
                            .any(|row| matches!(row.origin, Origin::ConstantFrom(_)))
                    );
                }
                output.replay(input, budget).unwrap();
                let retained = output.retained_storage();
                drop(output);
                budget.release_storage(retained).unwrap();
            });
        }
    }
}

#[test]
fn no_op_and_non_neutral_arithmetic_still_execute_the_exact_roster() {
    for module in [
        Module::new("empty-policy6"),
        identity(Constant::U32(1), true),
    ] {
        with_owner(module, |input, budget| {
            let output = finish(input, budget);
            assert_eq!(
                output.owner().canonical().canonical_bytes(),
                output
                    .intermediate_policy5()
                    .owner()
                    .canonical()
                    .canonical_bytes()
            );
            assert!(!output.continuation().report().passes()[0].changed());
            assert_roster(output.continuation());
            output.replay(input, budget).unwrap();
            let retained = output.retained_storage();
            drop(output);
            budget.release_storage(retained).unwrap();
        });
    }
}

#[test]
fn fixed_record_cannot_be_edited_or_joined_to_foreign_input() {
    with_owner(identity(Constant::U32(0), true), |input, budget| {
        let mut output = finish(input, budget);
        for offset in 0..POLICY6_EXECUTION_RECORD_BYTES_V1 {
            output.execution.bytes[offset] ^= 1;
            assert!(matches!(
                output.replay_continuation(input, budget),
                Err(Error::Execution)
            ));
            output.execution.bytes[offset] ^= 1;
        }
        let mut different = identity(Constant::U32(0), true);
        different.id = "foreign-policy6-input".into();
        let (foreign, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&different, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(matches!(
            output.replay_continuation(&foreign, budget),
            Err(Error::Execution)
        ));
        drop(foreign);
        budget.release_storage(receipt.retained_storage()).unwrap();
        output.replay_continuation(input, budget).unwrap();
        let retained = output.retained_storage();
        drop(output);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn actual_generated_occurrences_refuse_missing_uses_and_false_flag_origins() {
    with_owner(identity(Constant::U32(0), true), |input, budget| {
        let output = finish(input, budget);
        scoped(budget, |budget| {
            let (before, before_storage) =
                CanonicalKirInventoryV1::derive(output.intermediate_policy5().owner(), budget)
                    .unwrap();
            budget.reserve_storage(before_storage.retained_storage())?;
            let (after, after_storage) =
                CanonicalKirInventoryV1::derive(output.owner(), budget).unwrap();
            budget.reserve_storage(after_storage.retained_storage())?;
            let actual = output.continuation().occurrences().candidate();
            let mut missing = actual;
            assert!(!missing.uses.is_empty());
            missing.uses = &[];
            assert!(check_canonical_kir_transition_v1(&before, &after, missing, budget).is_err());
            let mut operations = actual.operations.to_vec();
            let synthesized = operations
                .iter_mut()
                .find(|row| matches!(row.origin, Origin::ConstantFrom(_)))
                .unwrap();
            // Replace the checked flag's actual source with the function parameter.
            synthesized.origin = Origin::ConstantFrom(actual.definitions[0].input);
            let mut hostile = actual;
            hostile.operations = &operations;
            assert!(check_canonical_kir_transition_v1(&before, &after, hostile, budget).is_err());
            let (_checked, receipt) =
                check_canonical_kir_transition_v1(&before, &after, actual, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            Ok(())
        })
        .unwrap();
        let retained = output.retained_storage();
        drop(output);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn continuation_and_replay_have_exact_work_and_storage_boundaries() {
    with_owner(identity(Constant::U32(0), true), |input, parent| {
        let floor = parent.storage();
        let execute = |work_limit, storage_limit, parent: &mut Budget<'_>| {
            let prefix = optimize_checked_canonical_kernel_ir_policy5_v1(input, parent).unwrap();
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget
                .reserve_storage(floor + prefix.retained_storage())
                .unwrap();
            let output =
                continue_checked_canonical_kernel_ir_policy6_v1(input, prefix, &mut budget);
            assert_eq!(
                budget.storage(),
                floor,
                "consumed prefix is no longer reserved"
            );
            (output, budget.work(), budget.peak_storage())
        };
        let (baseline, spent, peak) = execute(WORK, STORAGE, parent);
        let baseline = baseline.unwrap();
        assert!(peak > floor + baseline.retained_storage());
        for (work, storage, success) in [
            (spent, peak, true),
            (spent - 1, peak, false),
            (spent, peak - 1, false),
        ] {
            let (result, _, _) = execute(work, storage, parent);
            assert_eq!(result.is_ok(), success);
            if let Ok(output) = result {
                assert_eq!(
                    output.execution().canonical_bytes(),
                    baseline.execution().canonical_bytes()
                );
                assert_eq!(
                    output.continuation().execution().canonical_bytes(),
                    baseline.continuation().execution().canonical_bytes()
                );
            }
        }
        let retained = floor + baseline.retained_storage();
        let mut work = Work::new(WORK);
        let mut measured = Budget::new(&mut work, STORAGE);
        measured.reserve_storage(retained).unwrap();
        baseline.replay(input, &mut measured).unwrap();
        let spent = measured.work();
        let peak = measured.peak_storage();
        for (work, storage, success) in [
            (spent, peak, true),
            (spent - 1, peak, false),
            (spent, peak - 1, false),
        ] {
            let mut work = Work::new(work);
            let mut budget = Budget::new(&mut work, storage);
            budget.reserve_storage(retained).unwrap();
            assert_eq!(baseline.replay(input, &mut budget).is_ok(), success);
            assert_eq!(budget.storage(), retained);
        }
    });
}

#[test]
fn closed_scope_restores_floor_without_refunding_work_on_error_or_panic() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(19).unwrap();
    for panics in [false, true] {
        let spent = budget.work();
        let result: Result<(), Error> = scoped(&mut budget, |budget| {
            budget.charge_work(7)?;
            budget.reserve_storage(101)?;
            if panics {
                panic!("test-only continuation unwind");
            }
            Err(Error::Execution)
        });
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.work(), spent + 7);
        assert!(if panics {
            matches!(result, Err(Error::Panicked))
        } else {
            matches!(result, Err(Error::Execution))
        });
    }
}

#[test]
fn owned_origin_callback_panic_discards_actual_candidate() {
    with_owner(identity(Constant::U32(0), true), |input, budget| {
        let floor = budget.storage();
        let observed =
            optimize_native_neutral_kernel_ir_integer_continuation_v1(input, budget).unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let spent = budget.work();
        let result = observed.try_check_and_finish_with_v1::<(), (), _>(budget, |_, budget| {
            budget.charge_work(7).unwrap();
            panic!("test-only owned-origin callback");
        });
        assert!(matches!(
            result,
            Err(KirCheckedNeutralOptimizationErrorV1::Panicked)
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > spent);
    });
}

#[test]
fn owned_origin_cannot_replace_work_ledger_with_an_equal_storage_floor() {
    use std::cell::Cell;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    with_owner(identity(Constant::U32(0), true), |input, parent| {
        for outcome in 0..3 {
            let mut original_work = Work::new(WORK);
            let mut budget = Budget::new(&mut original_work, STORAGE);
            budget.reserve_storage(parent.storage()).unwrap();
            budget.charge_work(11).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let observed =
                optimize_native_neutral_kernel_ir_integer_continuation_v1(input, &mut budget)
                    .unwrap();
            budget
                .reserve_storage(observed.storage().retained_storage())
                .unwrap();
            let original_spent = Cell::new(0);
            let foreign_floor = Cell::new(0);
            let dropped = Arc::new(AtomicBool::new(false));
            let result = observed.try_check_and_finish_with_v1(&mut budget, |_, budget| {
                budget.charge_work(7).unwrap();
                original_spent.set(budget.work());
                foreign_floor.set(budget.storage());
                // A tiny deliberate test-only leak supplies the 'static borrow
                // an untrusted safe callback can use to replace any Work lifetime.
                *budget = Budget::new(Box::leak(Box::new(Work::new(WORK))), STORAGE);
                budget.reserve_storage(foreign_floor.get()).unwrap();
                budget.charge_work(13).unwrap();
                let owner = Dropped(dropped.clone());
                match outcome {
                    0 => Ok((owner, std::mem::size_of::<Dropped>())),
                    1 => Err(()),
                    _ => panic!("test-only foreign-ledger callback unwind"),
                }
            });
            assert!(matches!(
                result,
                Err(KirCheckedNeutralOptimizationErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert!(budget.work_ledger_identity_v1() != ledger);
            assert_eq!(
                budget.work(),
                13,
                "no post-callback charge to foreign ledger"
            );
            assert_eq!(
                budget.storage(),
                foreign_floor.get(),
                "do not refund someone else's reservation"
            );
            assert!(dropped.load(Ordering::SeqCst));
            assert_eq!(original_work.work(), original_spent.get());
            assert!(original_work.work() > 11);
        }
    });
}

fn reverse_layout_chain() -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let binary = |id, lhs, rhs| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), ty.clone()),
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    };
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), ty.clone()),
        Kind::Constant(Constant::U32(0)),
    ));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(30),
        arguments: vec![],
    });
    let mut use_block = BasicBlock::new(BlockId(20));
    use_block.operations.push(binary(4, 1, 3));
    use_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let mut producer = BasicBlock::new(BlockId(30));
    producer.operations.push(binary(3, 2, 2));
    producer.terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![],
    });
    let mut module = Module::new("policy6-physical-order");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty.clone()], vec![ty]),
        vec![ValueId(1)],
        vec![entry, use_block, producer],
    ));
    module
}

#[test]
fn single_sweep_is_not_a_hidden_fixpoint_or_dominance_traversal() {
    // Directly qualify the fixed continuation on O. The Policy5 CFG prefix would
    // otherwise merge this deliberately adverse but legal physical block layout.
    with_owner(reverse_layout_chain(), |input, budget| {
        assert_eq!(binary_count(input), 2);
        let observed =
            optimize_native_neutral_kernel_ir_integer_continuation_v1(input, budget).unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let first = observed.try_check_and_finish_v1(budget).unwrap();
        budget
            .reserve_storage(first.storage().retained_storage())
            .unwrap();
        assert_eq!(binary_count(first.owner()), 1);
        assert_roster(&first);
        let observed =
            optimize_native_neutral_kernel_ir_integer_continuation_v1(first.owner(), budget)
                .unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let second = observed.try_check_and_finish_v1(budget).unwrap();
        budget
            .reserve_storage(second.storage().retained_storage())
            .unwrap();
        assert_eq!(binary_count(second.owner()), 0);
        assert_roster(&second);
        assert_ne!(
            first.owner().canonical().canonical_bytes(),
            second.owner().canonical().canonical_bytes()
        );
        assert!(first.report().passes()[0].changed());
        assert!(second.report().passes()[0].changed());
        let retained = second.storage().retained_storage();
        drop(second);
        budget.release_storage(retained).unwrap();
        let retained = first.storage().retained_storage();
        drop(first);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn an_earlier_rewrite_can_expose_a_later_identity_in_the_same_sweep() {
    let mut module = reverse_layout_chain();
    // This legal layout visits the constant producer before its dependent use.
    module.functions[0].body.as_mut().unwrap().blocks.swap(1, 2);
    with_owner(module, |input, budget| {
        assert_eq!(binary_count(input), 2);
        let observed =
            optimize_native_neutral_kernel_ir_integer_continuation_v1(input, budget).unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let output = observed.try_check_and_finish_v1(budget).unwrap();
        budget
            .reserve_storage(output.storage().retained_storage())
            .unwrap();
        assert_eq!(binary_count(output.owner()), 0);
        assert_roster(&output);
        let retained = output.storage().retained_storage();
        drop(output);
        budget.release_storage(retained).unwrap();
    });
}

#[test]
fn zero_work_failure_consumes_the_prefix_without_refunding_prior_charges() {
    with_owner(identity(Constant::U32(0), true), |input, parent| {
        let prefix = optimize_checked_canonical_kernel_ir_policy5_v1(input, parent).unwrap();
        let floor = parent.storage();
        let mut work = Work::new(11);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(11).unwrap();
        budget
            .reserve_storage(floor + prefix.retained_storage())
            .unwrap();
        assert!(matches!(
            continue_checked_canonical_kernel_ir_policy6_v1(input, prefix, &mut budget),
            Err(Error::Resource(_))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), 11);
    });
}

#[test]
fn fresh_sessions_produce_identical_final_bytes_map_and_execution() {
    let run = || {
        let mut result = None;
        with_owner(identity(Constant::U64(0), true), |input, budget| {
            let output = finish(input, budget);
            result = Some((
                output.owner().canonical().canonical_bytes().to_vec(),
                *output.continuation().map().digest(),
                *output.execution().canonical_bytes(),
                *output.continuation().execution().canonical_bytes(),
            ));
            let retained = output.retained_storage();
            drop(output);
            budget.release_storage(retained).unwrap();
        });
        result.unwrap()
    };
    assert_eq!(run(), run());
}

#[test]
fn caller_selected_historical_plan_cannot_invoke_the_integer_step() {
    let mut session = PlironSession::new(ShellLimits::default(), []).unwrap();
    let root = session
        .import_operation_text_v1(
            r#"builtin.module @custody {
        ^entry():
        dead = builtin.constant <builtin.integer <7: i64>> : builtin.integer i64
    }"#,
        )
        .unwrap();
    let before = session
        .analyze_operation_graph_v1(&root)
        .unwrap()
        .replay_identity();
    let plan = PlironOptimizationPlanV1::new(
        vec![Pass::IntegerNeutralCanonicalization],
        PlironOptimizationLimitsV1::default(),
    )
    .unwrap();
    assert!(matches!(
        session.execute_optimization_v1(&root, &plan),
        Err(PlironOptimizationErrorV1::PassRejected(
            Pass::IntegerNeutralCanonicalization
        ))
    ));
    assert_eq!(
        session
            .analyze_operation_graph_v1(&root)
            .unwrap()
            .replay_identity(),
        before
    );
    assert!(!session.is_poisoned());
    assert_eq!(Pass::DeadCodeElimination as usize, 0);
    assert_eq!(
        Pass::DominancePureCommonSubexpressionElimination as usize,
        5
    );
    assert_eq!(Pass::IntegerNeutralCanonicalization as usize, 6);
}
