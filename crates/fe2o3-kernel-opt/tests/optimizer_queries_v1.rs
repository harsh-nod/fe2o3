use std::error::Error;

use fe2o3_kernel_analysis::{
    PresburgerAffineExprV1, PresburgerBoxV1, PresburgerConstraintV1, PresburgerFailureV1,
    PresburgerSetV1,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, ComparePredicate, Constant, Function, Kernel, LaunchDomain, LaunchExtent,
    Module, Operation, OperationKind, Signature, Terminator, Type, ValueDef, ValueId,
    verify_module,
};
use fe2o3_kernel_opt::{
    CheckedOptimizerQuerySessionV1, OptimizerQueryDecisionV1, OptimizerQueryFailureV1,
    OptimizerQueryLimitsV1,
};

fn query_module() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("optimizer-query-v1");
    module.functions.push(Function::kernel_entry(
        "kernel_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "kernel_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn guarded_query_module(predicate: ComparePredicate, false_reconverges: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::BOOL),
            OperationKind::Compare {
                predicate,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut accepted = BasicBlock::new(BlockId(1));
    accepted.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut rejected = BasicBlock::new(BlockId(2));
    rejected.terminator = Some(if false_reconverges {
        Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        }
    } else {
        Terminator::Return { values: vec![] }
    });
    let mut destination = BasicBlock::new(BlockId(3));
    destination.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("optimizer-query-guard-v1");
    module.functions.push(Function::kernel_entry(
        "kernel_impl",
        Signature::new(vec![Type::INDEX], vec![]),
        vec![ValueId(0)],
        vec![entry, accepted, rejected, destination],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "kernel_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    verify_module(&module).unwrap();
    module
}

#[test]
fn presburger_queries_prove_refute_and_bind_the_graph_epoch() {
    let module = query_module();
    let session =
        CheckedOptimizerQuerySessionV1::new(&module, 41, OptimizerQueryLimitsV1::default())
            .unwrap();
    let domain = PresburgerBoxV1::new(vec![0], vec![4]).unwrap();
    let empty = PresburgerSetV1::new(
        domain.clone(),
        vec![PresburgerConstraintV1::LessEqualZero(
            PresburgerAffineExprV1::new(5, vec![-1]).unwrap(),
        )],
    )
    .unwrap();
    let OptimizerQueryDecisionV1::Proved(receipt) = session.prove_presburger_set_empty(&empty)
    else {
        panic!("expected a complete emptiness proof")
    };
    assert_eq!(receipt.graph_epoch(), 41);
    assert_eq!(receipt.work_units(), 4);
    assert!(!receipt.grants_semantic_authority());

    assert_eq!(
        session.prove_presburger_set_empty(&PresburgerSetV1::box_only(domain)),
        OptimizerQueryDecisionV1::Refuted { witness: vec![0] }
    );
}

#[test]
fn presburger_preflight_budget_exhaustion_is_incomplete_and_deterministic() {
    let module = query_module();
    let limits = OptimizerQueryLimitsV1::new(16, 8).unwrap();
    let session = CheckedOptimizerQuerySessionV1::new(&module, 0, limits).unwrap();
    let set = PresburgerSetV1::box_only(PresburgerBoxV1::new(vec![0], vec![9]).unwrap());
    let expected =
        OptimizerQueryDecisionV1::Incomplete(OptimizerQueryFailureV1::PresburgerPointLimit {
            required: 9,
            limit: 8,
        });
    assert_eq!(session.prove_presburger_set_empty(&set), expected);
    assert_eq!(session.prove_presburger_set_empty(&set), expected);
}

#[test]
fn presburger_failure_without_error_trait_is_not_exposed_as_a_source() {
    let failure = OptimizerQueryFailureV1::Presburger(PresburgerFailureV1::Unsupported {
        detail: "hostile test",
    });
    assert!(failure.source().is_none());
    assert!(failure.to_string().contains("hostile test"));
}

#[test]
fn guard_queries_require_the_true_edge_and_reject_false_reconvergence() {
    for (predicate, equality) in [
        (ComparePredicate::Equal, true),
        (ComparePredicate::LessThan, false),
    ] {
        let safe = guarded_query_module(predicate, false);
        let safe = CheckedOptimizerQuerySessionV1::new(&safe, 9, OptimizerQueryLimitsV1::default())
            .unwrap();
        let proved = if equality {
            safe.prove_dominating_index_equals_constant(
                &"kernel_impl".into(),
                BlockId(3),
                ValueId(0),
            )
        } else {
            safe.prove_dominating_unsigned_less_than(
                &"kernel_impl".into(),
                BlockId(3),
                ValueId(0),
                ValueId(1),
            )
        };
        assert!(matches!(proved, OptimizerQueryDecisionV1::Proved(_)));

        let hostile = guarded_query_module(predicate, true);
        let hostile =
            CheckedOptimizerQuerySessionV1::new(&hostile, 9, OptimizerQueryLimitsV1::default())
                .unwrap();
        let rejected = if equality {
            hostile.prove_dominating_index_equals_constant(
                &"kernel_impl".into(),
                BlockId(3),
                ValueId(0),
            )
        } else {
            hostile.prove_dominating_unsigned_less_than(
                &"kernel_impl".into(),
                BlockId(3),
                ValueId(0),
                ValueId(1),
            )
        };
        assert_eq!(
            rejected,
            OptimizerQueryDecisionV1::Incomplete(OptimizerQueryFailureV1::Unsupported)
        );
    }
}
