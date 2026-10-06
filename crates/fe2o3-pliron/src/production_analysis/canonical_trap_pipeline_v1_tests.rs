//! These exercise real reports and observed receipts, not an independent oracle.
use super::super::tests::with_checked;
use super::super::traps::tests::{fixture, with_projection};
use super::super::traps::with_canonical_trap_policy_checks_v1;
use super::*;
use fe2o3_kernel_ir::{
    BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate, FunctionRole, Kernel, LaunchDomain,
    LaunchExtent, Terminator,
};
use pliron::{builtin::op_interfaces::OneRegionInterface, linked_list::ContainsLinkedList, op::Op};

#[test]
fn interleaved_declarations_never_get_native_definitions_or_synthetic_reports() {
    for declaration in 0..=2 {
        let module = fixture(declaration);
        let expected = (0..3)
            .filter(|index| *index != declaration)
            .collect::<Vec<_>>();
        with_checked(&module, |checked, budget| {
            let original = checked.inventory(budget).unwrap().owner();
            let floor = budget.storage();
            with_canonical_trap_policy_checks_v1(checked, budget, |policies, budget| {
                assert!(std::ptr::eq(policies.owner(budget)?, original));
                assert_eq!(policies.owner(budget)?.canonical().canonical_bytes(), original.canonical().canonical_bytes());
                assert_eq!(policies.owner(budget)?.module().kernels, module.kernels);
                assert_eq!(policies.module_function_count(budget)?, 3);
                assert_eq!(policies.definition_count(budget)?, 2);
                for (definition, module) in expected.iter().copied().enumerate() {
                    assert_eq!(policies.definition_coordinate(definition, budget)?, FunctionCoordinate(module as u32));
                    let report = policies.report(definition, budget)?;
                    assert_eq!(report.paired_stage_count(), 9);
                    assert_eq!(report.reports().pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                    assert!(report.reports().is_clean());
                    assert_eq!(policies.history(definition, budget)?.function(), module);
                }
                let first = policies.history(0, budget)?;
                let second = policies.history(1, budget)?;
                assert_eq!(second.floor(), first.invocation());
                assert_eq!(policies.observation(budget)?.work_upper_bound(), first.invocation().work_upper_bound() + second.invocation().work_upper_bound());
                assert_eq!(policies.pending_obligations().iter().count(), 19);
                Ok(())
            }).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn two_actual_roots_keep_their_full_order_with_an_interleaved_declaration() {
    let mut module = fixture(1);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(0);
    module.functions[2].role = FunctionRole::KernelEntry;
    module.kernels.insert(
        0,
        Kernel::new(
            "second",
            "original_helper",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(32),
            },
        ),
    );
    with_checked(&module, |checked, budget| {
        with_canonical_trap_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.owner(budget)?.module().kernels, module.kernels);
            assert_eq!(
                policies.definition_coordinate(0, budget)?,
                FunctionCoordinate(0)
            );
            assert_eq!(
                policies.definition_coordinate(1, budget)?,
                FunctionCoordinate(2)
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn ordinary_scalar_and_private_entries_keep_the_same_terminal_refusal() {
    with_checked(&fixture(0), |checked, budget| {
        let scalar =
            with_canonical_ranked_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(scalar.failure(), Failure::UnsupportedGraph { .. }));
        let private =
            with_canonical_private_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(
            private.failure(),
            Failure::PrivateRequirement {
                requirement: CanonicalPrivateRequirementV1::DefinedAcyclicCalls,
                ..
            }
        ));
        assert_eq!(private.observation().work_upper_bound(), 0);
    });
}

#[test]
fn terminal_shape_never_displaces_private_initialization_or_all_call_closure() {
    for recursive in [false, true] {
        let mut module = fixture(1);
        let helper = &mut module.functions[2].body.as_mut().unwrap().blocks[0];
        let expected = if recursive {
            helper.operations.push(fe2o3_kernel_ir::Operation::new(
                vec![],
                Kind::Call {
                    callee: "original_helper".into(),
                    arguments: vec![],
                },
            ));
            CanonicalPrivateRequirementV1::DefinedAcyclicCalls
        } else {
            helper.operations.swap(2, 3);
            CanonicalPrivateRequirementV1::CompleteCells
        };
        with_checked(&module, |checked, budget| {
            let error =
                with_canonical_trap_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
            assert!(
                matches!(error.failure(), Failure::PrivateRequirement { requirement, .. } if *requirement == expected)
            );
            assert_eq!(error.observation().work_upper_bound(), 0);
        });
    }
}

#[test]
fn every_new_borrowed_graph_query_charges_its_literal_single_work_unit() {
    with_checked(&fixture(1), |checked, budget| {
        with_canonical_trap_policy_checks_v1(checked, budget, |view, budget| {
            let before = budget.work();
            assert_eq!(view.module_function_count(budget)?, 3);
            assert_eq!(budget.work(), before + 1);
            assert_eq!(view.definition_count(budget)?, 2);
            assert_eq!(budget.work(), before + 2);
            assert_eq!(
                view.definition_coordinate(1, budget)?,
                FunctionCoordinate(2)
            );
            assert_eq!(budget.work(), before + 3);
            assert_eq!(view.pair_count(budget)?, 1);
            assert_eq!(budget.work(), before + 4);
            assert_eq!(view.pair(0, budget)?.incoming_edges(), 0..2);
            assert_eq!(budget.work(), before + 5);
            assert!(!view.incoming_edge(1, budget)?.success_when());
            assert_eq!(budget.work(), before + 6);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_occurrences_keep_trap_members_distinct_from_private_and_scalar_operations() {
    with_projection(&fixture(1), |projection, budget| {
        projection.with_function(0, budget, |input| {
            let mut counts = [0; 3];
            for block in input
                .function()
                .get_region(input.context())
                .deref(input.context())
                .iter(input.context())
            {
                for pointer in block.deref(input.context()).iter(input.context()) {
                    match input.operation(input.context(), pointer).unwrap() {
                        PrivateOperationKindV1::TrapCall => counts[0] += 1,
                        PrivateOperationKindV1::TrapEnd => counts[1] += 1,
                        PrivateOperationKindV1::Call => counts[2] += 1,
                        _ => {}
                    }
                }
            }
            assert_eq!(counts, [1, 1, 1]);
        })?;
        projection.with_function(2, budget, |input| {
            assert_eq!(input.ordinal(), 2);
        })?;
        assert!(projection.with_function(1, budget, |_| ()).is_err());
        Ok(())
    })
    .unwrap();
}

#[test]
fn trap_call_signature_and_root_schema_mutations_are_not_roundtrip_authority() {
    use dialect_gpu::optimization_v1::CallOp;
    use pliron::builtin::attributes::TypeAttr;
    use pliron::builtin::types::{FunctionType, IntegerType, Signedness};
    use pliron::operation::Operation;
    for mutation in 0..4 {
        with_projection(&fixture(1), |projection, budget| {
            projection.test_live(|context, root| {
                if mutation == 2 {
                    let original = root.deref(context).attributes.clone();
                    root.deref_mut(context).attributes = original;
                    return;
                }
                let region = root.deref(context).get_region(0);
                let block = region.deref(context).iter(context).next().unwrap();
                let function = block.deref(context).iter(context).next().unwrap();
                if mutation == 1 {
                    function.deref_mut(context).attributes.set(
                        "unexpected".try_into().unwrap(),
                        pliron::builtin::attributes::StringAttr::new("changed".to_owned()),
                    );
                    return;
                }
                let region = function.deref(context).get_region(0);
                let sink = region.deref(context).iter(context).nth(3).unwrap();
                if mutation == 3 {
                    let end = sink.deref(context).iter(context).nth(1).unwrap();
                    end.deref_mut(context).attributes.set(
                        "operand_segment_sizes".try_into().unwrap(),
                        pliron::builtin::attributes::OperandSegmentSizesAttr(vec![0]),
                    );
                    return;
                }
                let pointer = sink.deref(context).iter(context).next().unwrap();
                let call = Operation::get_op::<CallOp>(pointer, context).unwrap();
                let ty = IntegerType::get(context, 32, Signedness::Unsigned);
                let changed = FunctionType::get(context, vec![], vec![ty.into()]);
                // Deliberate test-only raw corruption bypasses the typed setter;
                // the reader must compare the actual attribute against custody.
                call.get_operation().deref_mut(context).attributes.set(
                    "gpu_call_signature".try_into().unwrap(),
                    TypeAttr::new(changed.into()),
                );
            });
            assert!(matches!(projection.check_epoch(), Err(Failure::Mutation)));
            if mutation != 2 {
                projection.test_rebase_epoch();
                let error = projection.check(budget).unwrap_err();
                if mutation == 0 {
                    assert!(matches!(
                        error,
                        Failure::PrivateRequirement {
                            requirement: CanonicalPrivateRequirementV1::NativeJoin,
                            ..
                        }
                    ));
                } else {
                    assert!(matches!(error, Failure::NativeSchema));
                }
            }
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn preserved_exit_observation_requires_the_exact_borrowed_empty_shape() {
    use crate::production_analysis::pliron_control_edges_v1::ControlViewV1;
    use dialect_gpu::optimization_v1::PreservedTerminatorKindAttr;
    use pliron::builtin::attributes::{OperandSegmentSizesAttr, StringAttr};
    use pliron::operation::Operation;
    for mutation in 0..6 {
        with_projection(&fixture(1), |projection, _budget| {
            projection.test_live(|context, root| {
                let module_region = root.deref(context).get_region(0);
                let module_block = module_region.deref(context).iter(context).next().unwrap();
                let function = module_block.deref(context).iter(context).next().unwrap();
                let region = function.deref(context).get_region(0);
                let entry = region.deref(context).iter(context).next().unwrap();
                let sink = region.deref(context).iter(context).nth(3).unwrap();
                let end = sink.deref(context).iter(context).nth(1).unwrap();
                assert_eq!(
                    ControlViewV1::observe(context, end)
                        .unwrap()
                        .successor_count(),
                    0
                );
                match mutation {
                    0 => end.deref_mut(context).attributes.set(
                        "unexpected".try_into().unwrap(),
                        StringAttr::new("changed".into()),
                    ),
                    1 => end.deref_mut(context).attributes.set(
                        "operand_segment_sizes".try_into().unwrap(),
                        OperandSegmentSizesAttr(vec![0]),
                    ),
                    2 => end.deref_mut(context).attributes.set(
                        "gpu_preserved_terminator_kind".try_into().unwrap(),
                        PreservedTerminatorKindAttr::Switch,
                    ),
                    3 => {
                        let constant = entry.deref(context).iter(context).nth(1).unwrap();
                        let value = constant.deref(context).get_result(0);
                        Operation::push_operand(end, context, value);
                    }
                    4 => {
                        let constant = entry.deref(context).iter(context).nth(1).unwrap();
                        let ty = constant.deref(context).get_type(0);
                        Operation::push_result(end, context, ty);
                    }
                    _ => {
                        Operation::push_successor(end, context, entry);
                    }
                }
                assert!(ControlViewV1::observe(context, end).is_err());
            });
            assert!(matches!(projection.check_epoch(), Err(Failure::Mutation)));
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn invalid_graph_queries_and_foreign_ledgers_poison_the_callback() {
    for kind in 0..4 {
        with_checked(&fixture(1), |checked, budget| {
            let floor = budget.storage();
            let mut work = Work::new(100);
            let mut foreign = Budget::new(&mut work, 100);
            let error = with_canonical_trap_policy_checks_v1(checked, budget, |view, budget| {
                match kind {
                    0 => {
                        assert!(view.definition_coordinate(2, budget).is_err());
                    }
                    1 => {
                        assert!(view.pair(1, budget).is_err());
                    }
                    2 => {
                        assert!(view.incoming_edge(2, budget).is_err());
                    }
                    _ => {
                        assert!(view.pair_count(&mut foreign).is_err());
                    }
                }
                Ok(())
            })
            .unwrap_err();
            assert!(matches!(
                error.failure(),
                Failure::InvalidQuery { .. } | Failure::Resource(Resource::Accounting)
            ));
            assert_eq!(foreign.work(), 0);
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn callback_error_panic_and_imbalance_drop_paid_result_before_refund() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct DropProbe(Arc<AtomicBool>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    for mode in 0..4 {
        with_checked(&fixture(1), |checked, budget| {
            let floor = budget.storage();
            let dropped = Arc::new(AtomicBool::new(false));
            let error = with_canonical_trap_policy_checks_v1(checked, budget, |_, budget| {
                budget.reserve_storage(size_of::<DropProbe>())?;
                let probe = DropProbe(Arc::clone(&dropped));
                match mode {
                    0 => {
                        drop(probe);
                        budget.release_storage(size_of::<DropProbe>())?;
                        Err(Failure::Callback("explicit"))
                    }
                    1 => {
                        drop(probe);
                        budget.release_storage(size_of::<DropProbe>())?;
                        panic!("terminal callback");
                    }
                    2 => Ok(probe),
                    _ => std::panic::panic_any(probe),
                }
            })
            .err()
            .expect("all callbacks must fail");
            assert!(dropped.load(Ordering::SeqCst));
            match mode {
                0 => assert!(matches!(error.failure(), Failure::Callback("explicit"))),
                1 => assert!(matches!(error.failure(), Failure::Panicked)),
                _ => assert!(matches!(
                    error.failure(),
                    Failure::Resource(Resource::Accounting)
                )),
            }
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn analysis_domain_zero_work_or_storage_denies_without_caller_success() {
    for limits in [Limits::new(0, usize::MAX), Limits::new(usize::MAX, 0)] {
        with_checked(&fixture(0), |checked, budget| {
            let floor = budget.storage();
            let error = with_trap_checks(checked, budget, limits, |_, _| Ok(())).unwrap_err();
            assert!(matches!(
                error.failure(),
                Failure::AnalysisLimit { .. } | Failure::Analysis { .. }
            ));
            assert!(error.observation().first_denial().is_some());
            assert_eq!(budget.storage(), floor);
        });
    }
}

fn assert_prefix(
    total: CanonicalRankedPolicyResourceObservationV1,
    history: CanonicalRankedPolicyHistoryV1,
) {
    let floor = history.floor();
    let local = history.invocation();
    assert_eq!(
        total.work_upper_bound(),
        floor.work_upper_bound() + local.work_upper_bound()
    );
    assert_eq!(
        total.retained_storage_units(),
        floor.retained_storage_units() + local.retained_storage_units()
    );
    assert_eq!(
        total.peak_storage_units(),
        floor
            .peak_storage_units()
            .max(floor.retained_storage_units() + local.peak_storage_units())
    );
}

fn is_tensor_panic(failure: &Failure, module_function: usize) -> bool {
    matches!(failure, Failure::Analysis { function, cause: ProductionPlironPreloweringErrorV2::Preservation(
        crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1::AnalysisPanicked { pass: crate::KernelCheckPassKindV1::TensorLayout }) }
        if *function == module_function)
}

#[test]
fn first_definition_real_stage_panic_keeps_one_typed_prefix_and_recovers() {
    use crate::production_analysis::pliron_pipeline::panic_after_first_production_stage_for_test_v1;
    with_checked(&fixture(0), |checked, budget| {
        let floor = budget.storage();
        let _panic = panic_after_first_production_stage_for_test_v1();
        let error =
            with_canonical_trap_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(is_tensor_panic(error.failure(), 1));
        assert!(error.observation().caught_panic());
        let history = error.last_invocation().unwrap();
        assert_eq!(history.function(), 1);
        assert!(history.invocation().work_upper_bound() > 0);
        assert_prefix(error.observation(), history);
        assert_eq!(budget.storage(), floor);
    });
    with_checked(&fixture(0), |checked, budget| {
        with_canonical_trap_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap();
    });
}

#[test]
fn second_definition_real_stage_panic_retains_prior_report_once_in_direct_state() {
    use crate::production_analysis::pliron_pipeline::panic_after_first_production_stage_for_test_v1;
    with_projection(&fixture(1), |projection, budget| {
        let mut analysis =
            private_resources::PrivateAnalysisV1::new(Limits::production_hard_ceiling());
        let first = projection.with_function(0, budget, |input| analysis.invoke(input))??;
        let floor = analysis.observation();
        assert!(first.report.reports().is_clean());
        let _panic = panic_after_first_production_stage_for_test_v1();
        let error = projection
            .with_function(2, budget, |input| analysis.invoke(input))?
            .err()
            .expect("actual second stage panic");
        assert!(is_tensor_panic(&error, 2));
        let history = analysis.last.unwrap();
        assert_eq!(history.floor(), floor);
        assert_eq!(history.function(), 2);
        assert_prefix(analysis.observation(), history);
        drop(first);
        analysis.release_reports()?;
        assert_eq!(analysis.observation().retained_storage_units(), 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn genuine_second_definition_failure_retains_completed_first_report_prefix() {
    let mut module = fixture(1);
    module.functions[2].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    with_checked(&module, |checked, budget| {
        let error =
            with_canonical_trap_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::Analysis { function: 2, .. }
        ));
        let history = error.last_invocation().unwrap();
        assert!(history.floor().work_upper_bound() > 0);
        assert!(history.invocation().work_upper_bound() > 0);
        assert_prefix(error.observation(), history);
    });
}

fn switch_trap_fixture(typed: bool) -> fe2o3_kernel_ir::Module {
    use fe2o3_kernel_ir::{
        BasicBlock, Constant, IntegerSwitchCase, Operation, ScalarType, SwitchCase, Type, ValueDef,
        ValueId,
    };
    let mut module = fixture(1);
    super::tests::add_private_switch(&mut module, typed);
    let body = module.functions[0].body.as_mut().unwrap();
    let entry = &mut body.blocks[0];
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(44), Type::Scalar(ScalarType::U32)),
        Kind::Constant(Constant::U32(19)),
    ));
    let mut guard = BasicBlock::new(BlockId(95));
    guard.parameters = vec![
        ValueDef::new(ValueId(45), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(46), Type::Scalar(ScalarType::U32)),
    ];
    guard.terminator = entry.terminator.take();
    let Some(Terminator::ConditionalBranch { then_arguments, .. }) = &mut guard.terminator else {
        unreachable!()
    };
    *then_arguments = vec![ValueId(45)];
    let payloads = [
        vec![ValueId(41), ValueId(44)],
        vec![ValueId(44), ValueId(41)],
    ];
    entry.terminator = Some(if typed {
        Terminator::IntegerSwitch {
            selector: ValueId(41),
            cases: [0, 17]
                .into_iter()
                .enumerate()
                .map(|(i, key)| IntegerSwitchCase {
                    value: Constant::U32(key),
                    target: BlockId(95),
                    arguments: payloads[i].clone(),
                })
                .collect(),
            default_target: BlockId(95),
            default_arguments: vec![ValueId(44), ValueId(44)],
        }
    } else {
        Terminator::Switch {
            selector: ValueId(41),
            cases: [0, 17]
                .into_iter()
                .enumerate()
                .map(|(i, value)| SwitchCase {
                    value,
                    target: BlockId(95),
                    arguments: payloads[i].clone(),
                })
                .collect(),
            default_target: BlockId(95),
            default_arguments: vec![ValueId(44), ValueId(44)],
        }
    });
    body.blocks.push(guard);
    module
}

#[test]
fn trap_switch_real_private_memory_and_bool_pairs_reach_all_nine_without_relabeling() {
    for typed in [false, true] {
        let module = switch_trap_fixture(typed);
        with_checked(&module, |checked, budget| {
            let original = checked.inventory(budget).unwrap().owner();
            let floor = budget.storage();
            with_canonical_trap_policy_checks_v1(checked, budget, |view, budget| {
                assert!(std::ptr::eq(view.owner(budget)?, original));
                assert_eq!(view.definition_count(budget)?, 2);
                assert_eq!(view.module_function_count(budget)?, 3);
                assert_eq!(view.pair_count(budget)?, 1);
                assert_eq!(view.pair(0, budget)?.incoming_edges().len(), 2);
                // The two exact ConditionalBranch predecessors are still the
                // only assertion facts; neither native Switch is a Bool pair.
                let mut polarities = [false; 2];
                for ordinal in 0..2 {
                    let incoming = view.incoming_edge(ordinal, budget)?;
                    assert_eq!(incoming.condition().value, fe2o3_kernel_ir::ValueId(40));
                    assert!(incoming.failure().arguments.is_empty());
                    polarities[ordinal] = incoming.success_when();
                }
                assert_ne!(polarities[0], polarities[1]);
                for definition in 0..2 {
                    let report = view.report(definition, budget)?;
                    assert_eq!(report.paired_stage_count(), 9);
                    assert_eq!(report.reports().pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                    assert!(report.reports().is_clean());
                }
                assert_eq!(view.history(1, budget)?.floor(), view.history(0, budget)?.invocation());
                assert_eq!(view.pending_obligations().iter().count(), 19);
                Ok(())
            }).unwrap();
            assert_eq!(budget.storage(), floor);
        });
        with_projection(&module, |projection, budget| {
            projection.with_function(0, budget, |input| {
                use crate::production_analysis::pliron_control_edges_v1::ControlViewV1;
                use dialect_gpu::switch_v3::SwitchOpV3;
                let context = input.context();
                let region = input.function().get_region(context).deref(context);
                let entry = region.iter(context).next().unwrap();
                let raw = entry.deref(context);
                let pointer = raw.get_terminator(context).unwrap();
                assert!(pliron::operation::Operation::is_op::<SwitchOpV3>(
                    pointer, context
                ));
                let a = raw
                    .iter(context)
                    .nth(2)
                    .unwrap()
                    .deref(context)
                    .get_result(0);
                let b = raw
                    .iter(context)
                    .nth(3)
                    .unwrap()
                    .deref(context)
                    .get_result(0);
                let control = ControlViewV1::observe(context, pointer).unwrap();
                assert_eq!(control.successor_count(), 3);
                for (ordinal, expected) in [[a, b], [b, a], [b, b]].into_iter().enumerate() {
                    let edge = control.edge(ordinal).unwrap();
                    assert_eq!(edge.argument_count(), 2);
                    for (position, value) in expected.into_iter().enumerate() {
                        assert_eq!(edge.argument_at(position).unwrap().0, value);
                    }
                }
            })?;
            projection.check(budget)?;
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn trap_switch_case_and_default_side_entries_cannot_become_bool_assertion_edges() {
    for typed in [false, true] {
        for ordinal in 0..3 {
            let mut module = switch_trap_fixture(typed);
            let terminator = module.functions[0].body.as_mut().unwrap().blocks[0]
                .terminator
                .as_mut()
                .unwrap();
            match terminator {
                Terminator::Switch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                } => {
                    if ordinal < 2 {
                        cases[ordinal].target = BlockId(94);
                        cases[ordinal].arguments.clear();
                    } else {
                        *default_target = BlockId(94);
                        default_arguments.clear();
                    }
                }
                Terminator::IntegerSwitch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                } => {
                    if ordinal < 2 {
                        cases[ordinal].target = BlockId(94);
                        cases[ordinal].arguments.clear();
                    } else {
                        *default_target = BlockId(94);
                        default_arguments.clear();
                    }
                }
                _ => unreachable!(),
            }
            with_checked(&module, |checked, budget| {
                let floor = budget.storage();
                let error = with_canonical_trap_policy_checks_v1::<()>(checked, budget, |_, _| {
                    panic!("Switch edge became a Bool pair")
                })
                .unwrap_err();
                assert!(matches!(
                    error.failure(),
                    Failure::PrivateRequirement {
                        requirement: CanonicalPrivateRequirementV1::TerminalPairs,
                        ..
                    }
                ));
                assert_eq!(error.observation().work_upper_bound(), 0);
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn trap_switch_unrepresentable_legacy_key_is_refused_at_the_actual_helper_coordinate() {
    let mut module = switch_trap_fixture(false);
    let ordinal = super::tests::make_narrow_switch_unrepresentable(
        &mut module,
        fe2o3_kernel_ir::Constant::U8(0),
        256,
    );
    assert_eq!(ordinal, 2);
    with_checked(&module, |checked, budget| {
        let floor = budget.storage();
        let error = with_canonical_trap_policy_checks_v1::<()>(checked, budget, |_, _| {
            panic!("narrow Switch reached C reports")
        })
        .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::UnsupportedGraph {
                function: 2,
                block: Some(0),
                operation: None
            }
        ));
        assert_eq!(error.observation().work_upper_bound(), 0);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn trap_switch_native_payload_and_closed_schema_mutations_refuse_even_after_epoch_rebase() {
    use pliron::operation::Operation;
    for schema in [false, true] {
        with_projection(&switch_trap_fixture(false), |projection, budget| {
            projection.test_live(|context, root| {
                let region = root.deref(context).get_region(0);
                let module_block = region.deref(context).iter(context).next().unwrap();
                let function = module_block.deref(context).iter(context).next().unwrap();
                let region = function.deref(context).get_region(0);
                let entry = region.deref(context).iter(context).next().unwrap();
                let pointer = entry.deref(context).get_terminator(context).unwrap();
                if schema {
                    pointer.deref_mut(context).attributes.set(
                        "unexpected".try_into().unwrap(),
                        pliron::builtin::attributes::StringAttr::new("foreign schema".into()),
                    );
                } else {
                    let late = pointer
                        .deref(context)
                        .get_successor(0)
                        .deref(context)
                        .get_argument(0);
                    Operation::replace_operand(pointer, context, 1, late);
                }
            });
            assert!(matches!(projection.check_epoch(), Err(Failure::Mutation)));
            projection.test_rebase_epoch();
            assert!(projection.check(budget).is_err());
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn trap_switch_real_tensor_panic_and_callback_failure_preserve_cleanup_and_typed_history() {
    use crate::production_analysis::pliron_pipeline::panic_after_first_production_stage_for_test_v1;
    let module = switch_trap_fixture(false);
    with_checked(&module, |checked, budget| {
        let floor = budget.storage();
        let _panic = panic_after_first_production_stage_for_test_v1();
        let error = with_canonical_trap_policy_checks_v1::<()>(checked, budget, |_, _| {
            panic!("stage panic reached callback")
        })
        .unwrap_err();
        assert!(is_tensor_panic(error.failure(), 0));
        assert!(error.observation().caught_panic());
        let history = error.last_invocation().unwrap();
        assert_eq!(history.function(), 0);
        assert_prefix(error.observation(), history);
        assert_eq!(budget.storage(), floor);
    });
    for panic in [false, true] {
        with_checked(&module, |checked, budget| {
            let floor = budget.storage();
            let error =
                with_canonical_trap_policy_checks_v1::<()>(checked, budget, |view, budget| {
                    assert_eq!(view.report(1, budget)?.paired_stage_count(), 9);
                    if panic {
                        panic!("trap switch callback");
                    }
                    Err(Failure::Callback("trap switch"))
                })
                .unwrap_err();
            if panic {
                assert!(matches!(error.failure(), Failure::Panicked));
            } else {
                assert!(matches!(error.failure(), Failure::Callback("trap switch")));
            }
            assert_eq!(budget.storage(), floor);
        });
    }
    with_checked(&module, |checked, budget| {
        with_canonical_trap_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap();
    });
}
