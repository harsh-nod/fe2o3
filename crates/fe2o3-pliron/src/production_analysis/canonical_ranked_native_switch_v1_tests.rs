use super::*;
use dialect_gpu::switch_v3::{SwitchKeyKindAttrV3 as KeyKind, SwitchOpV3};
use fe2o3_kernel_ir::{IntegerSwitchCase, SwitchCase};
use pliron::op::Op;

fn switch_module(typed: bool, count: usize) -> Module {
    let mut entry = BasicBlock::new(BlockId(17));
    let arguments = |ordinal| {
        if ordinal % 2 == 0 {
            vec![ValueId(1), ValueId(2)]
        } else {
            vec![ValueId(2), ValueId(1)]
        }
    };
    entry.terminator = Some(if typed {
        Terminator::IntegerSwitch {
            selector: ValueId(0),
            cases: (0..count)
                .map(|i| IntegerSwitchCase {
                    value: Constant::I64(i as i64 - count as i64),
                    target: BlockId(23),
                    arguments: arguments(i),
                })
                .collect(),
            default_target: BlockId(23),
            default_arguments: vec![ValueId(2), ValueId(2)],
        }
    } else {
        Terminator::Switch {
            selector: ValueId(0),
            cases: (0..count)
                .map(|i| SwitchCase {
                    value: u64::MAX - i as u64,
                    target: BlockId(23),
                    arguments: arguments(i),
                })
                .collect(),
            default_target: BlockId(23),
            default_arguments: vec![ValueId(2), ValueId(2)],
        }
    });
    let mut join = BasicBlock::new(BlockId(23));
    join.parameters = vec![
        ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
    ];
    join.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("canonical-native-switch");
    module.functions.push(Function::kernel_entry(
        "dispatch",
        Signature::new(
            vec![
                Type::Scalar(if typed {
                    ScalarType::I64
                } else {
                    ScalarType::U64
                }),
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, join],
    ));
    module.kernels.push(Kernel::new(
        "dispatch",
        "dispatch",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

fn live_switch(context: &Context, root: Ptr<LiveOperation>) -> SwitchOpV3 {
    let function = first_function(context, root);
    let region = function.deref(context).get_region(0);
    let block = region.deref(context).iter(context).next().unwrap();
    let pointer = block.deref(context).get_terminator(context).unwrap();
    assert!(LiveOperation::is_op::<SwitchOpV3>(pointer, context));
    SwitchOpV3::from_operation(pointer)
}

#[test]
fn native_switch_actual_fixed_nine_reports_keep_every_source_obligation_pending() {
    for typed in [false, true] {
        for count in [0, 1, 16, 17] {
            let module = switch_module(typed, count);
            with_projection(&module, |projection, budget| {
                projection.test_live(|context, root| {
                    let switch = live_switch(context, root);
                    assert_eq!(
                        switch.kind(context),
                        Some(if typed && count == 0 {
                            KeyKind::EmptyTyped
                        } else if typed {
                            KeyKind::I64
                        } else {
                            KeyKind::LegacyU64
                        })
                    );
                    assert_eq!(switch.cases(context).unwrap().bits().len(), count);
                    let raw = switch.get_operation().deref(context);
                    assert_eq!(raw.get_num_successors(), count + 1);
                    assert_eq!(raw.get_num_operands(), 1 + 2 * (count + 1));
                    assert_eq!(raw.attributes.0.len(), 4);
                });
                projection.check(budget).unwrap();
            });
            with_checked(&module, |checked, budget| {
                let expected = checked.inventory(budget).unwrap().owner();
                with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
                    assert!(std::ptr::eq(policies.owner(budget)?, expected));
                    assert_eq!(policies.function_count(budget)?, 1);
                    let report = policies.report(0, budget)?;
                    assert_eq!(report.pass_order(), &super::super::super::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                    assert!(report.is_clean());
                    assert_eq!(policies.pending_obligations().iter().count(), 19);
                    assert!(!policies.ranked_verification_is_complete());
                    assert!(!policies.grants_artifact_or_launch_authority());
                    assert_eq!(policies.history(0, budget)?.function(), 0);
                    Ok(())
                }).unwrap();
            });
        }
    }
}

#[test]
fn native_switch_two_functions_compose_real_histories_without_cross_owner_reports() {
    let mut module = switch_module(false, 2);
    let mut second = switch_module(true, 2);
    second.functions[0].id = "second".try_into().unwrap();
    module.functions.push(second.functions.remove(0));
    module.kernels.push(Kernel::new(
        "second",
        "second",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(32),
        },
    ));
    with_checked(&module, |checked, budget| {
        with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.function_count(budget)?, 2);
            let first = policies.history(0, budget)?;
            let second = policies.history(1, budget)?;
            assert_eq!(
                second.floor().work_upper_bound(),
                first.invocation().work_upper_bound()
            );
            assert_eq!(
                second.floor().retained_storage_units(),
                first.invocation().retained_storage_units()
            );
            for ordinal in 0..2 {
                assert_eq!(policies.history(ordinal, budget)?.function(), ordinal);
                assert_eq!(policies.report(ordinal, budget)?.pass_order().len(), 9);
                assert!(policies.report(ordinal, budget)?.is_clean());
            }
            assert_eq!(
                policies.observation(budget)?.work_upper_bound(),
                first.invocation().work_upper_bound() + second.invocation().work_upper_bound()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_switch_narrow_unrepresentable_legacy_keys_refuse_at_the_source_coordinate() {
    for (ty, key) in [
        (ScalarType::U8, 256),
        (ScalarType::I8, u64::MAX),
        (ScalarType::U16, 1 << 16),
        (ScalarType::I32, u64::MAX),
    ] {
        let mut module = switch_module(false, 1);
        module.functions[0].signature.parameters[0] = Type::Scalar(ty);
        let Some(Terminator::Switch { cases, .. }) =
            &mut module.functions[0].body.as_mut().unwrap().blocks[0].terminator
        else {
            unreachable!()
        };
        cases[0].value = key;
        with_checked(&module, |checked, budget| {
            let called = Cell::new(false);
            let error = with_canonical_ranked_policy_checks_v1(checked, budget, |_, _| {
                called.set(true);
                Ok(())
            })
            .unwrap_err();
            assert!(!called.get());
            assert!(matches!(
                error.failure(),
                Failure::UnsupportedGraph {
                    function: 0,
                    block: Some(0),
                    operation: None
                }
            ));
            assert_eq!(error.observation().work_upper_bound(), 0);
        });
    }
}

#[test]
fn native_switch_complete_case_default_and_payload_semantics_are_not_template_metadata() {
    for mutation in 0..5 {
        let mut module = switch_module(false, 2);
        let mut donor = switch_module(false, 2);
        donor.functions[0].id = "donor".try_into().unwrap();
        let Some(Terminator::Switch { cases, .. }) =
            &mut donor.functions[0].body.as_mut().unwrap().blocks[0].terminator
        else {
            unreachable!()
        };
        cases[0].value = 7;
        cases[1].value = 2;
        module.functions.push(donor.functions.remove(0));
        module.kernels.push(Kernel::new(
            "donor",
            "donor",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        with_projection(&module, |projection, budget| {
            projection.test_live(|context, root| {
                let switch = live_switch(context, root);
                let pointer = switch.get_operation();
                match mutation {
                    0 => {
                        let region = root.deref(context).get_region(0);
                        let root_block = region.deref(context).iter(context).next().unwrap();
                        let donor_function =
                            root_block.deref(context).iter(context).nth(1).unwrap();
                        let region = donor_function.deref(context).get_region(0);
                        let entry = region.deref(context).iter(context).next().unwrap();
                        let donor = SwitchOpV3::from_operation(
                            entry.deref(context).get_terminator(context).unwrap(),
                        );
                        let keys = donor.cases(context).unwrap().clone();
                        switch.set_attr_gpu_switch_cases(context, keys);
                    }
                    1 => {
                        let value = pointer.deref(context).get_operand(2);
                        LiveOperation::replace_operand(pointer, context, 1, value);
                    }
                    2 => {
                        let last = pointer.deref(context).get_num_operands() - 1;
                        let first = pointer.deref(context).get_operand(1);
                        LiveOperation::replace_operand(pointer, context, last, first);
                    }
                    3 => {
                        let original = pointer.deref(context).attributes.clone();
                        pointer.deref_mut(context).attributes = original;
                    }
                    _ => pointer.deref_mut(context).attributes.set(
                        "extra".try_into().unwrap(),
                        StringAttr::new("not a switch field".into()),
                    ),
                }
            });
            assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
            projection.test_rebase_epoch();
            if mutation == 3 {
                projection.check(budget).unwrap();
            } else {
                assert!(projection.check(budget).is_err());
            }
        });
    }
}

#[test]
fn native_switch_late_typed_payload_and_changed_edge_dominance_are_rejected() {
    with_projection(&switch_module(false, 2), |projection, budget| {
        projection.test_live(|context, root| {
            let switch = live_switch(context, root);
            let raw = switch.get_operation().deref(context);
            let target = raw.get_successor(0);
            let late = target.deref(context).get_argument(0);
            drop(raw);
            LiveOperation::replace_operand(switch.get_operation(), context, 1, late);
        });
        assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
        projection.test_rebase_epoch();
        assert!(projection.check(budget).is_err());
    });
}

#[test]
fn native_switch_default_target_is_an_independent_actual_occurrence() {
    let mut module = switch_module(false, 2);
    let body = module.functions[0].body.as_mut().unwrap();
    let mut default = BasicBlock::new(BlockId(29));
    default.parameters = vec![
        ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
    ];
    default.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(default);
    let Some(Terminator::Switch { default_target, .. }) = &mut body.blocks[0].terminator else {
        unreachable!()
    };
    *default_target = BlockId(29);
    with_projection(&module, |projection, budget| {
        projection.test_live(|context, root| {
            let switch = live_switch(context, root);
            let pointer = switch.get_operation();
            let raw = pointer.deref(context);
            let target = raw.get_successor(0);
            assert_ne!(target, raw.get_successor(2));
            drop(raw);
            LiveOperation::replace_successor(pointer, context, 2, target);
        });
        assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
        projection.test_rebase_epoch();
        assert!(projection.check(budget).is_err());
    });
}

#[test]
fn native_switch_loop_dispatch_keeps_real_zero_step_progress_refusal() {
    use fe2o3_kernel_ir::ComparePredicate;
    for step in [0, 1] {
        let mut module = switch_module(false, 1);
        let function = &mut module.functions[0];
        function.signature.parameters.truncate(1);
        let body = function.body.as_mut().unwrap();
        body.parameters.truncate(1);
        body.blocks.clear();
        let ty = Type::Scalar(ScalarType::U32);
        let mut entry = BasicBlock::new(BlockId(17));
        for (id, value) in [(1, 0), (2, step), (3, 8)] {
            entry.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(id), ty.clone()),
                OperationKind::Constant(Constant::U32(value)),
            ));
        }
        entry.terminator = Some(Terminator::Switch {
            selector: ValueId(0),
            cases: vec![SwitchCase {
                value: 7,
                target: BlockId(23),
                arguments: vec![ValueId(1)],
            }],
            default_target: BlockId(23),
            default_arguments: vec![ValueId(1)],
        });
        let mut header = BasicBlock::new(BlockId(23));
        header
            .parameters
            .push(ValueDef::new(ValueId(10), ty.clone()));
        header.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(11), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(10),
                rhs: ValueId(3),
            },
        ));
        header.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(11),
            then_target: BlockId(29),
            then_arguments: vec![],
            else_target: BlockId(31),
            else_arguments: vec![],
        });
        let mut latch = BasicBlock::new(BlockId(29));
        latch.operations.push(Operation::new(
            vec![
                ValueDef::new(ValueId(12), ty),
                ValueDef::new(ValueId(13), Type::BOOL),
            ],
            OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                lhs: ValueId(10),
                rhs: ValueId(2),
            },
        ));
        latch.terminator = Some(Terminator::Branch {
            target: BlockId(23),
            arguments: vec![ValueId(12)],
        });
        let mut exit = BasicBlock::new(BlockId(31));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks = vec![entry, header, latch, exit];
        with_checked(&module, |checked, budget| {
            let called = Cell::new(false);
            let result =
                with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
                    called.set(true);
                    assert_eq!(policies.report(0, budget)?.pass_order().len(), 9);
                    assert!(policies.report(0, budget)?.is_clean());
                    Ok(())
                });
            assert_eq!(called.get(), step == 1);
            if step == 1 {
                result.unwrap();
            } else {
                let error = result.unwrap_err();
                let Failure::Analysis {
                    function: 0,
                    cause: ProductionPlironPreloweringErrorV2::Semantic(cause),
                } = error.failure()
                else {
                    panic!("expected actual progress refusal: {error:?}")
                };
                assert!(!cause.report().progress().findings().is_empty());
                assert!(
                    cause
                        .report()
                        .progress()
                        .findings()
                        .iter()
                        .all(|finding| matches!(
                            finding,
                            crate::PlironProgressFindingV1::ProgressIncomplete { .. }
                                | crate::PlironProgressFindingV1::NonTerminatingCycle { .. }
                        ))
                );
            }
        });
    }
}

mod resources {
    include!("canonical_ranked_native_switch_resources_v1_tests.rs");
}
