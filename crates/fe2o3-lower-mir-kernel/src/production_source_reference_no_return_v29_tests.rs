use super::*;

mod execution_control_tests {
    use super::*;
    include!("production_execution_control_v29_tests.rs");
}

#[derive(Clone, Copy)]
enum Exit {
    Loop,
    Unreachable,
    MaybeReturn,
}

fn choose(local: u32, left: u32, right: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(local, WORD)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                fixture_edge(SemanticEdgeRoleV1::SwitchValue, left),
            )],
            fixture_edge(SemanticEdgeRoleV1::SwitchOtherwise, right),
        )
        .unwrap(),
    }
}

fn replace_blocks(
    functions: &mut [SemanticFunctionDeclV1],
    index: usize,
    blocks: Vec<SemanticBasicBlockV1>,
) {
    let old = &functions[index];
    let mut next = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source().clone(),
        old.abi().clone(),
        old.locals().to_vec(),
        old.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = old.kernel_entry() {
        next = next.with_kernel_entry(entry.clone());
    }
    functions[index] = next;
}

fn no_return_owner(exit: Exit, root_bypass: bool, unwind: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |_, functions| {
        let old = &functions[2];
        let mut statements = old.blocks()[0].statements().to_vec();
        statements.pop(); // Remove the real Unit assignment, not a synthetic return.
        let go =
            |target| SemanticTerminatorKindV1::Goto(fixture_edge(SemanticEdgeRoleV1::Goto, target));
        let blocks = match exit {
            Exit::Unreachable => vec![block(30, statements, SemanticTerminatorKindV1::Unreachable)],
            Exit::Loop => vec![block(30, statements, go(1)), block(31, vec![], go(1))],
            Exit::MaybeReturn => vec![
                block(30, statements, choose(3, 1, 2)),
                block(31, vec![unit()], SemanticTerminatorKindV1::Return),
                block(32, vec![], go(2)),
            ],
        };
        replace_blocks(functions, 2, blocks);
        if root_bypass {
            replace_blocks(
                functions,
                0,
                vec![
                    block(10, vec![], choose(1, 1, 2)),
                    block(
                        11,
                        vec![],
                        call(1, SemanticOperandV1::Copy(place(1, WORD)), 2),
                    ),
                    block(12, vec![unit()], SemanticTerminatorKindV1::Return),
                ],
            );
        }
        if unwind {
            let old = &functions[0];
            let SemanticTerminatorKindV1::Call(original) = old.blocks()[0].terminator().kind()
            else {
                unreachable!()
            };
            let call = SemanticDirectCallV1::new_callable(
                original.callee(),
                original.arguments().to_vec(),
                original.destination().cloned(),
                SemanticUnwindActionV1::Cleanup(fixture_edge(SemanticEdgeRoleV1::CallUnwind, 2)),
            )
            .unwrap();
            let blocks = vec![
                block(10, vec![], SemanticTerminatorKindV1::Call(call)),
                old.blocks()[1].clone(),
                old.blocks()[2].clone(),
            ];
            replace_blocks(functions, 0, blocks);
        }
    })
}

fn with_builder(
    owner: &ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(&mut SourceReferenceBuilderV29<'_, '_, '_>, &mut ArgumentBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let mut builder = SourceReferenceBuilderV29::new_with_storage(
                instances,
                SourceReferenceStorageV29::ScalarCells,
                budget,
            )
            .unwrap();
            consume(&mut builder, budget);
            drop(builder);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn no_return_keeps_original_coordinates_without_values_or_dead_instance_entries() {
    for exit in [Exit::Loop, Exit::Unreachable] {
        let owner = no_return_owner(exit, false, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let floor = budget.storage();
                let result = with_source_reference_storage_plan_v29(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    budget,
                    |plan, _| {
                        assert_eq!(instances.instances().len(), 5);
                        assert_eq!(
                            plan.entries.iter().filter(|entry| entry.is_some()).count(),
                            3
                        );
                        assert!(plan.returns.iter().all(Option::is_none));
                        assert!(plan.nodes.iter().all(|node| node.ty != UNIT));
                        assert_eq!(plan.loans.len(), 1);
                        assert!(plan.loans[0].effects.referent_reads > 0);
                        for (ordinal, entry) in plan.entries.iter().enumerate() {
                            let id = instances.id_at(ordinal).unwrap();
                            assert_eq!(instances.instance_reachable(id), Some(entry.is_some()));
                            for (block, _) in instances
                                .instance(id)
                                .unwrap()
                                .declaration()
                                .blocks()
                                .iter()
                                .enumerate()
                            {
                                let block = SemanticBlockIdV1::from_index(block as u32);
                                let retained = plan
                                    .blocks
                                    .iter()
                                    .filter(|row| row.instance == id && row.block == block)
                                    .count();
                                assert_eq!(
                                    retained,
                                    usize::from(instances.block_reachable(id, block).unwrap())
                                );
                            }
                        }
                        Ok(())
                    },
                );
                assert_eq!(budget.storage(), floor);
                result.unwrap();
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn no_return_summary_reuse_has_no_after_state_or_invented_unit_result() {
    let owner = no_return_owner(Exit::Loop, false, false);
    with_builder(&owner, |builder, budget| {
        let root = builder.plan.root;
        assert_eq!(
            builder.function(root, None, budget).unwrap(),
            SourceReferenceFunctionOutcomeV29::NoNormalReturn
        );
        assert!(builder.frames.iter().all(Option::is_none));
        let nodes = builder.plan.nodes.len();
        assert_eq!(
            builder.function(root, None, budget).unwrap(),
            SourceReferenceFunctionOutcomeV29::NoNormalReturn
        );
        assert_eq!(builder.plan.nodes.len(), nodes);
        let summary = builder.summaries[root.index()].as_ref().unwrap();
        assert!(summary.after.is_none());
        assert_eq!((summary.evaluations, summary.reuse_count), (1, 1));
        assert!(builder.frames.iter().all(Option::is_none));
        builder.summaries[root.index()].as_mut().unwrap().result =
            SourceReferenceFunctionOutcomeV29::Returned(0);
        assert!(builder.function(root, None, budget).is_err());
    });
}

#[test]
fn no_return_edge_does_not_remove_a_shared_target_reachable_by_another_branch() {
    let owner = no_return_owner(Exit::Loop, true, false);
    with_builder(&owner, |builder, budget| {
        let root = builder.plan.root;
        assert!(matches!(
            builder.function(root, None, budget).unwrap(),
            SourceReferenceFunctionOutcomeV29::Returned(_)
        ));
        let index = builder.cfg_index(root, budget).unwrap();
        assert_eq!(index.reachable, [true, true, true]);
        assert!(index.successors[index.successor_ranges[1].clone()].is_empty());
        let successors = &index.successors[index.successor_ranges[0].clone()];
        assert_eq!(successors.iter().map(|edge| edge.target).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(successors.iter().map(|edge| edge.ordinal).collect::<Vec<_>>(), [0, 1]);
        assert!(builder.plan.returns[root.index()].is_some());
        assert!(builder.plan.returns.iter().skip(1).all(Option::is_none));
    });
}

#[test]
fn no_return_analysis_preserves_unknown_returning_branch_and_distinct_helper_instances() {
    let owner = no_return_owner(Exit::MaybeReturn, false, false);
    with_builder(&owner, |builder, budget| {
        assert!(matches!(
            builder.function(builder.plan.root, None, budget).unwrap(),
            SourceReferenceFunctionOutcomeV29::Returned(_)
        ));
        assert!(builder.visited.iter().all(|visited| *visited));
        assert_eq!(
            builder
                .plan
                .returns
                .iter()
                .filter(|entry| entry.is_some())
                .count(),
            5
        );
        assert!(
            builder
                .summaries
                .iter()
                .all(|summary| summary.as_ref().unwrap().after.is_some())
        );
    });
}

#[test]
fn no_return_call_does_not_reuse_normal_state_as_unwind_transport() {
    let owner = no_return_owner(Exit::Unreachable, false, true);
    with_builder(&owner, |builder, budget| {
        assert_eq!(
            builder
                .plan
                .instances
                .instance_may_return(builder.plan.root),
            Some(true)
        );
        let result = builder.function(builder.plan.root, None, budget);
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference call unwind requires effect transport",
                ..
            })
        ));
        assert!(builder.summaries.iter().all(Option::is_none));
        assert!(builder.plan.returns.iter().all(Option::is_none));
    });
}

#[test]
fn no_return_cannot_be_claimed_by_removing_a_returning_calls_destination() {
    let owner = owner(Case::Shared);
    let source = owner.source_semantic();
    let mut functions = source.functions().to_vec();
    let old = &functions[0];
    let SemanticTerminatorKindV1::Call(original) = old.blocks()[0].terminator().kind() else {
        unreachable!()
    };
    let call = SemanticDirectCallV1::new_callable(
        original.callee(),
        original.arguments().to_vec(),
        None,
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let blocks = vec![
        block(10, vec![], SemanticTerminatorKindV1::Call(call)),
        old.blocks()[1].clone(),
        old.blocks()[2].clone(),
    ];
    replace_blocks(&mut functions, 0, blocks);
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap();
    assert!(matches!(
        request.admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape {
            function: ROOT,
            tail: false
        })
    ));
}

#[test]
fn no_return_source_plan_exact_and_one_short_limits_restore_the_outer_floor() {
    for exit in [Exit::Loop, Exit::Unreachable] {
        let owner = no_return_owner(exit, false, false);
        let run = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(17).unwrap();
            let result = production_call_instances_v1::with_production_call_instances_v1(
                &owner,
                ROOT,
                &mut budget,
                |instances, budget| {
                    Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                        with_source_reference_storage_plan_v29(
                            instances,
                            SourceReferenceStorageV29::ScalarCells,
                            budget,
                            |_, _| Ok(()),
                        ),
                    )
                },
            );
            assert_eq!(budget.storage(), 17);
            (result, budget.work(), budget.peak_storage())
        };
        let (result, work, storage) = run(usize::MAX, usize::MAX);
        result.unwrap().unwrap();
        run(work, storage).0.unwrap().unwrap();
        for (work, storage) in [(work - 1, storage), (work, storage - 1)] {
            let result = run(work, storage).0;
            assert!(result.is_err() || result.unwrap().is_err());
        }
    }
}
