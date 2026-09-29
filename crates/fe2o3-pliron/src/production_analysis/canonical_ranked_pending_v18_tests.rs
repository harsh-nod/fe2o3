use super::super::tests::{LAYOUTS, pointer_flow, with_checked};
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    ExecutionOperationV15 as E, ExecutionRoleV15 as R, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId,
};

const AMPLE: usize = 1 << 40;

#[test]
fn pending_source_descendant_refusal_suppresses_refunds_before_native_child_exists() {
    for prior_mutation in [false, true] {
        with_checked(&lifecycle(false), |checked, budget| {
            let floor = budget.storage();
            let lost = Cell::new(0usize);
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let parent = budget.storage();
                    budget.reserve_storage(16)?;
                    budget.release_storage(1)?;
                    assert!(budget.storage() > parent);
                    lost.set(budget.storage());
                    if prior_mutation {
                        assert!(matches!(pending.guard.mutation(), Failure::Mutation));
                    }
                    let refusal = pending.refuse_retained_custody();
                    if prior_mutation {
                        assert!(matches!(refusal, Failure::Mutation));
                    } else {
                        assert!(matches!(refusal, Failure::Resource(Resource::Accounting)));
                    }
                    assert!(pending.owner(budget).is_err());
                    Ok(())
                },
            );
            if prior_mutation {
                assert!(matches!(result, Err(Failure::Mutation)));
            } else {
                assert!(matches!(
                    result,
                    Err(Failure::Resource(Resource::Accounting))
                ));
            }
            assert_eq!(budget.storage(), lost.get());
            // Test-owned denied credits are drained only after every view dies.
            budget.release_storage(budget.storage() - floor).unwrap();
        });
    }
}

fn lifecycle(load: bool) -> Module {
    let mut module = Module::new("pending-lifecycle");
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(0), Type::Execution(R::Context))],
            OperationKind::Execution(E::ContextIssue),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(1), Type::Execution(R::Workgroup))],
            OperationKind::Execution(E::WorkgroupDerive {
                context: ValueId(0),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Execution(E::ScopeEnd {
                workgroup: ValueId(1),
                discarded: vec![],
            }),
        ),
    ];
    let (parameters, arguments) = if load {
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))],
            OperationKind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        (
            vec![Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )],
            vec![ValueId(2)],
        )
    } else {
        (vec![], vec![])
    };
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        arguments,
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

#[test]
fn pending_census_is_complete_and_preflight_runs_no_native_stage() {
    for load in [false, true] {
        with_checked(&lifecycle(load), |checked, budget| {
            NATIVE_STARTS.with(|count| count.set(0));
            let floor = budget.storage();
            with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    let obligations = pending.obligations(budget)?;
                    assert_eq!(obligations.len(), 3 + usize::from(load));
                    for (ordinal, row) in obligations.iter().enumerate() {
                        assert_eq!(row.coordinate().block.function.0, 0);
                        assert_eq!(row.coordinate().block.block, 0);
                        assert_eq!(row.coordinate().operation as usize, ordinal);
                        assert_eq!(
                            row.requirement(),
                            if ordinal == 3 {
                                CanonicalRankedSourceRequirementV18::Memory
                            } else {
                                CanonicalRankedSourceRequirementV18::Execution
                            }
                        );
                    }
                    assert!(!pending.source_roles_are_complete());
                    assert!(!pending.grants_artifact_or_launch_authority());
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(NATIVE_STARTS.with(Cell::get), 0);
            assert_eq!(budget.storage(), floor);
            let error = with_canonical_ranked_policy_checks_v18(
                checked,
                LAYOUTS,
                budget,
                |_, _| -> Result<(), Failure> { panic!("strict source-role gate was bypassed") },
            )
            .unwrap_err();
            assert!(matches!(error.failure(), Failure::SourceRequirementV18 {
                coordinate, requirement: CanonicalRankedSourceRequirementV18::Execution,
            } if coordinate.operation == 0));
            assert_eq!(error.observation().work_upper_bound(), 0);
            assert!(error.last_invocation().is_none());
        });
    }
}

#[test]
fn pending_unreachable_keeps_the_complete_operation_census_and_strict_role_gate() {
    let mut module = lifecycle(true);
    let mut terminal = BasicBlock::new(BlockId(41));
    terminal.terminator = Some(Terminator::Unreachable);
    let mut later = BasicBlock::new(BlockId(97));
    later.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32))],
        OperationKind::Load {
            pointer: ValueId(2),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    later.terminator = Some(Terminator::Unreachable);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks.extend([terminal, later]);
    assert!(Terminator::Unreachable.successors().is_empty());
    with_checked(&module, |checked, budget| {
        let exact = checked.inventory(budget).unwrap().owner();
        let floor = budget.storage();
        let completed = Cell::new(false);
        NATIVE_STARTS.with(|count| count.set(0));
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                assert!(std::ptr::eq(pending.owner(budget)?, exact));
                assert_eq!(pending.owner(budget)?.module(), &module);
                let rows = pending.obligations(budget)?;
                assert_eq!(rows.len(), 5);
                for (row, (block, operation, requirement)) in rows.iter().zip([
                    (0, 0, CanonicalRankedSourceRequirementV18::Execution),
                    (0, 1, CanonicalRankedSourceRequirementV18::Execution),
                    (0, 2, CanonicalRankedSourceRequirementV18::Execution),
                    (0, 3, CanonicalRankedSourceRequirementV18::Memory),
                    (2, 0, CanonicalRankedSourceRequirementV18::Memory),
                ]) {
                    assert_eq!(row.coordinate().block.function.0, 0);
                    assert_eq!(row.coordinate().block.block, block);
                    assert_eq!(row.coordinate().operation, operation);
                    assert_eq!(row.requirement(), requirement);
                }
                assert!(!pending.source_roles_are_complete());
                assert!(!pending.grants_artifact_or_launch_authority());
                completed.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(completed.get());
        assert_eq!(NATIVE_STARTS.with(Cell::get), 0);
        assert_eq!(budget.storage(), floor);
        let error = with_canonical_ranked_policy_checks_v18(
            checked,
            LAYOUTS,
            budget,
            |_, _| -> Result<(), Failure> { panic!("unreachable bypassed a source role") },
        )
        .unwrap_err();
        assert!(matches!(error.failure(), Failure::SourceRequirementV18 {
            coordinate, requirement: CanonicalRankedSourceRequirementV18::Execution,
        } if coordinate.block.function.0 == 0 && coordinate.block.block == 0 && coordinate.operation == 0));
        assert_eq!(error.observation().work_upper_bound(), 0);
        assert!(error.last_invocation().is_none());
        assert_eq!(NATIVE_STARTS.with(Cell::get), 0);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn pending_unreachable_native_observation_preserves_the_exact_terminal_graph() {
    for reachable in [false, true] {
        let mut module = Module::new("pending-scalar-unreachable");
        let mut entry = BasicBlock::new(BlockId(17));
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(22),
            then_arguments: vec![],
            else_target: BlockId(22),
            else_arguments: vec![],
        });
        let mut end = BasicBlock::new(BlockId(22));
        end.terminator = Some(if reachable {
            Terminator::Unreachable
        } else {
            Terminator::Return { values: vec![] }
        });
        let mut blocks = vec![entry, end];
        if !reachable {
            let mut terminal = BasicBlock::new(BlockId(97));
            terminal.terminator = Some(Terminator::Unreachable);
            blocks.push(terminal);
        }
        module.functions.push(Function::internal_helper(
            "entry",
            Signature::new(vec![Type::BOOL], vec![]),
            vec![ValueId(0)],
            blocks,
        ));
        with_checked(&module, |checked, budget| {
            let exact = checked.inventory(budget).unwrap().owner();
            let floor = budget.storage();
            let completed = Cell::new(false);
            NATIVE_STARTS.with(|count| count.set(0));
            with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    assert!(pending.obligations(budget)?.is_empty());
                    let phase_floor = budget.storage();
                    assert!(std::ptr::eq(pending.owner(budget)?, exact));
                    assert_eq!(pending.owner(budget)?.module(), &module);
                    let attempt = pending
                        .with_native_observations(budget, |observed, budget| {
                            assert!(std::ptr::eq(observed.owner(budget)?, exact));
                            assert_eq!(observed.owner(budget)?.module(), &module);
                            assert!(observed.obligations(budget)?.is_empty());
                            assert_eq!(observed.function_count(budget)?, 1);
                            assert!(observed.report(0, budget)?.unwrap().is_clean());
                            assert!(observed.history(0, budget)?.is_some());
                            assert!(observed.observation(budget)?.work_upper_bound() > 0);
                            assert!(!observed.source_roles_are_complete());
                            assert!(!observed.ranked_verification_is_complete());
                            assert!(!observed.grants_artifact_or_launch_authority());
                            completed.set(true);
                            Ok(())
                        });
                    if reachable {
                        attempt.unwrap();
                    } else {
                        // Exact identity preserves the dead block too. The
                        // unchanged bounds policy independently refuses it.
                        let error = attempt.unwrap_err();
                        let Failure::Analysis { function: 0,
                            cause: ProductionPlironPreloweringErrorV2::Bounds(bounds) } = error.failure()
                        else { panic!("disconnected terminal lost its exact bounds refusal: {:?}", error.failure()); };
                        assert!(matches!(bounds.report().findings(),
                            [crate::production_analysis::pliron_ranked_bounds::RankedBoundsFindingV1::UnreachableBlock { block: 2 }]));
                        assert!(error.last_invocation().is_some());
                        assert!(error.observation().work_upper_bound() > 0);
                        assert!(!completed.get());
                        assert!(std::ptr::eq(pending.owner(budget)?, exact));
                        assert_eq!(pending.owner(budget)?.module(), &module);
                    }
                    assert_eq!(budget.storage(), phase_floor);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(completed.get(), reachable, "reachable={reachable}");
            assert_eq!(NATIVE_STARTS.with(Cell::get), 1);
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn pending_unreachable_does_not_admit_a_missing_terminator() {
    let mut module = pointer_flow(false);
    module.functions[0].body.as_mut().unwrap().blocks[1].terminator = None;
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    budget.reserve_storage(23).unwrap();
    NATIVE_STARTS.with(|count| count.set(0));
    let error = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &module,
        LAYOUTS,
        &mut budget,
    )
    .unwrap_err();
    let fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Verification(
        fe2o3_kernel_ir::BorrowedKernelIrVerificationErrorV1::Verification(errors),
    ) = error
    else {
        panic!("missing terminator changed its verifier refusal: {error:?}");
    };
    assert!(errors.diagnostics().iter().any(|diagnostic|
        diagnostic.code == fe2o3_kernel_ir::DiagnosticCode::MissingTerminator));
    assert_eq!(NATIVE_STARTS.with(Cell::get), 0);
    assert_eq!(budget.storage(), 23);
}

#[test]
fn pending_native_observations_run_actual_fixed_stages_but_remain_pending() {
    with_checked(&pointer_flow(false), |checked, budget| {
        let exact = checked.inventory(budget).unwrap().owner();
        NATIVE_STARTS.with(|count| count.set(0));
        let floor = budget.storage();
        with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
            assert!(pending.obligations(budget)?.is_empty());
            let phase_floor = budget.storage();
            pending.with_native_observations(budget, |observed, budget| {
                assert!(std::ptr::eq(observed.owner(budget)?, exact));
                assert!(observed.obligations(budget)?.is_empty());
                assert_eq!(observed.function_count(budget)?, 1);
                let report = observed.report(0, budget)?.unwrap();
                assert!(report.is_clean());
                assert_eq!(report.pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                assert!(observed.history(0, budget)?.is_some());
                assert!(observed.observation(budget)?.work_upper_bound() > 0);
                assert!(!observed.source_roles_are_complete());
                assert!(!observed.ranked_verification_is_complete());
                assert!(!observed.grants_artifact_or_launch_authority());
                Ok(())
            }).unwrap();
            assert_eq!(budget.storage(), phase_floor);
            Ok(())
        }).unwrap();
        assert_eq!(NATIVE_STARTS.with(Cell::get), 1);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn pending_epoch_change_between_phases_or_during_native_is_terminal() {
    for during_native in [false, true] {
        with_checked(&pointer_flow(false), |checked, budget| {
            NATIVE_STARTS.with(|count| count.set(0));
            let completed = Cell::new(false);
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    if during_native {
                        MUTATE_AFTER_NATIVE.with(|flag| flag.set(true));
                    } else {
                        pending.graph.test_ranked_mutate_and_restore_v18();
                    }
                    let error = pending
                        .with_native_observations(budget, |_, _| -> Result<(), Failure> {
                            panic!("changed epoch reached report consumer")
                        })
                        .unwrap_err();
                    assert!(matches!(error.failure(), Failure::Mutation));
                    assert_eq!(error.last_invocation().is_some(), during_native);
                    let work = budget.work();
                    assert!(matches!(pending.owner(budget), Err(Failure::Mutation)));
                    assert_eq!(
                        budget.work(),
                        work,
                        "latched epoch failure has no new debit"
                    );
                    completed.set(true);
                    Ok(())
                },
            );
            assert!(matches!(result, Err(Failure::Mutation)));
            assert!(completed.get());
            assert_eq!(NATIVE_STARTS.with(Cell::get), usize::from(during_native));
        });
    }
}

#[test]
fn pending_phase_two_checks_foreign_custody_before_any_debit() {
    with_checked(&pointer_flow(false), |checked, budget| {
        NATIVE_STARTS.with(|count| count.set(0));
        let completed = Cell::new(false);
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, _| {
                let mut foreign_work = Work::new(AMPLE);
                let mut foreign = Budget::new(&mut foreign_work, AMPLE);
                foreign.reserve_storage(AMPLE / 2).unwrap();
                let before = (foreign.work(), foreign.storage());
                let error = pending
                    .with_native_observations(&mut foreign, |_, _| -> Result<(), Failure> {
                        panic!("foreign budget reached native consumer")
                    })
                    .unwrap_err();
                assert!(matches!(
                    error.failure(),
                    Failure::Resource(Resource::Accounting)
                ));
                assert_eq!((foreign.work(), foreign.storage()), before);
                assert_eq!(error.observation().work_upper_bound(), 0);
                assert!(error.last_invocation().is_none());
                completed.set(true);
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(Failure::Resource(Resource::Accounting))
        ));
        assert!(completed.get());
        assert_eq!(NATIVE_STARTS.with(Cell::get), 0);
    });
}

#[test]
fn pending_phase_two_first_header_denial_stays_latched_after_padding_settles() {
    with_checked(&pointer_flow(false), |checked, budget| {
        NATIVE_STARTS.with(|count| count.set(0));
        let completed = Cell::new(false);
        let result = with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                // The first phase-two envelope is its extra immutable slice plus
                // a zero-sized consumer capture and two one-byte alignments.
                let first_header = size_of::<&[CanonicalRankedSourceObligationV18]>() + 2;
                let padding = AMPLE - budget.storage() - first_header + 1;
                budget.reserve_storage(padding)?;
                let before = budget.work();
                let error = pending
                    .with_native_observations(budget, |_, _| -> Result<(), Failure> {
                        panic!("one-short phase-two header reached native")
                    })
                    .unwrap_err();
                assert!(
                    matches!(error.failure(), Failure::Resource(Resource::Storage(error)) if error.actual() == AMPLE + 1)
                );
                assert_eq!(
                    budget.work(),
                    before + 3,
                    "one owner query plus the existing protected entry"
                );
                assert_eq!(budget.failed_storage(), Some(AMPLE + 1));
                budget.release_storage(padding)?;
                budget.charge_work(AMPLE - budget.work())?;
                let work = budget.work();
                let error = pending
                    .with_native_observations(budget, |_, _| -> Result<(), Failure> {
                        panic!("latched refusal reached native")
                    })
                    .unwrap_err();
                assert!(
                    matches!(error.failure(), Failure::Resource(Resource::Storage(error)) if error.actual() == AMPLE + 1)
                );
                assert_eq!(budget.work(), work);
                assert_eq!(budget.failed_work(), None);
                completed.set(true);
                Ok(())
            },
        );
        assert!(
            matches!(result, Err(Failure::Resource(Resource::Storage(error))) if error.actual() == AMPLE + 1)
        );
        assert!(completed.get());
        assert_eq!(NATIVE_STARTS.with(Cell::get), 0);
    });
}

#[test]
fn pending_native_panic_and_owned_report_error_preserve_real_history() {
    with_checked(&pointer_flow(false), |checked, budget| {
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                let floor = budget.storage();
                let error = pending
                    .with_native_observations(budget, |_, _| -> Result<(), Failure> {
                        panic!("pending actual native callback panic")
                    })
                    .unwrap_err();
                assert!(matches!(error.failure(), Failure::Panicked));
                assert!(error.last_invocation().is_some());
                assert!(error.observation().work_upper_bound() > 0);
                assert_eq!(budget.storage(), floor);
                assert!(
                    pending.owner(budget).is_ok(),
                    "semantic callback errors do not poison unrelated queries"
                );
                Ok(())
            },
        )
        .unwrap();
    });
    let mut cycle = super::super::super::tests::noop();
    cycle.functions[0].signature.parameters = vec![Type::BOOL];
    let body = cycle.functions[0].body.as_mut().unwrap();
    body.parameters = vec![ValueId(0)];
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(92),
        then_arguments: vec![],
        else_target: BlockId(93),
        else_arguments: vec![],
    });
    let mut loop_block = BasicBlock::new(BlockId(92));
    loop_block.terminator = Some(Terminator::Branch {
        target: BlockId(92),
        arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(93));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([loop_block, exit]);
    with_checked(&cycle, |checked, budget| {
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                let error = pending
                    .with_native_observations(budget, |_, _| -> Result<(), Failure> {
                        panic!("the actual reachable cycle cannot produce a clean native report")
                    })
                    .unwrap_err();
                let Failure::Analysis {
                    cause: ProductionPlironPreloweringErrorV2::Semantic(cause),
                    ..
                } = error.failure()
                else {
                    panic!("expected the actual semantic progress refusal: {error:?}");
                };
                assert!(
                    cause
                        .report()
                        .progress()
                        .findings()
                        .iter()
                        .any(|finding| matches!(
                            finding,
                            crate::PlironProgressFindingV1::ProgressIncomplete { .. }
                        ))
                );
                assert!(
                    error.observation().retained_storage_units() > 0,
                    "owned diagnostic keeps the accepted analysis-domain storage envelope"
                );
                assert!(error.last_invocation().is_some());
                drop(error);
                Ok(())
            },
        )
        .unwrap();
    });
}

#[test]
fn pending_post_native_snapshot_resource_refusal_keeps_accepted_history() {
    for storage_short in [false, true] {
        with_checked(&pointer_flow(false), |checked, budget| {
            let floor = budget.storage();
            NATIVE_STARTS.with(|count| count.set(0));
            let completed = Cell::new(false);
            let result = with_pending_canonical_ranked_source_roles_v18(
                checked,
                LAYOUTS,
                budget,
                |pending, budget| {
                    pending
                        .with_native_observations(budget, |observed, budget| {
                            assert!(observed.report(0, budget)?.unwrap().is_clean());
                            assert!(observed.last_invocation(budget)?.is_some());
                            Ok(())
                        })
                        .unwrap();
                    EXHAUST_AFTER_NATIVE.with(|cut| cut.set(Some((storage_short, AMPLE))));
                    let error = pending
                        .with_native_observations(budget, |_, _| -> Result<(), Failure> {
                            panic!("post-invocation snapshot cut reached the consumer")
                        })
                        .unwrap_err();
                    match error.failure() {
                        Failure::Resource(Resource::Storage(_)) if storage_short => (),
                        Failure::Resource(Resource::Work(_)) if !storage_short => (),
                        other => panic!("wrong post-native resource refusal: {other:?}"),
                    }
                    assert!(error.last_invocation().is_some());
                    assert!(error.observation().work_upper_bound() > 0);
                    let work = budget.work();
                    assert!(matches!(pending.owner(budget), Err(Failure::Resource(_))));
                    assert_eq!(budget.work(), work);
                    completed.set(true);
                    Ok(())
                },
            );
            assert!(matches!(result, Err(Failure::Resource(_))));
            assert!(
                completed.get(),
                "latched resource cannot mask a failed history assertion"
            );
            assert_eq!(
                NATIVE_STARTS.with(Cell::get),
                2,
                "same retained graph's positive then hostile invocation"
            );
            assert_eq!(budget.storage(), floor);
        });
    }
}
