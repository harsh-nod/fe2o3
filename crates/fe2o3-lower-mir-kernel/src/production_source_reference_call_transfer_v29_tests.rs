use super::*;

const LIMIT: usize = 10_000_000;

fn transfer_owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Writeback, |_, functions| {
        let old = &functions[1];
        let mut locals = old.locals().to_vec();
        locals.extend([
            local(250, WORD, SemanticLocalRoleV1::Temporary),
            local(251, WORD, SemanticLocalRoleV1::Temporary),
        ]);
        let mut statements = vec![assign(
            place(5, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
        )];
        statements.extend_from_slice(old.blocks()[0].statements());
        functions[1] = function(
            20,
            false,
            WORD,
            locals,
            vec![
                block(20, statements, old.blocks()[0].terminator().kind().clone()),
                old.blocks()[1].clone(),
            ],
        );
    })
}

fn with_builder(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(&mut SourceReferenceBuilderV29<'_, '_, '_>, &mut ArgumentBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
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
            builder.function(instances.root(), None, budget).unwrap();
            consume(&mut builder, budget);
            drop(builder);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

fn helper_index(builder: &SourceReferenceBuilderV29<'_, '_, '_>, ordinal: usize) -> usize {
    builder
        .plan
        .instances
        .instances()
        .iter()
        .enumerate()
        .filter(|(_, row)| row.function() == SemanticFunctionIdV1::from_index(2))
        .nth(ordinal)
        .unwrap()
        .0
}

fn restore_input(
    builder: &mut SourceReferenceBuilderV29<'_, '_, '_>,
    ordinal: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> (
    ProductionCallInstanceIdV1,
    ProductionCallInstanceIdV1,
    Vec<usize>,
) {
    let index = helper_index(builder, ordinal);
    let helper = builder.plan.instances.id_at(index).unwrap();
    let worker = builder
        .plan
        .instances
        .incoming(helper)
        .unwrap()
        .occurrence()
        .caller;
    let arguments = restore_instance_input(builder, helper, budget);
    (helper, worker, arguments)
}

fn restore_instance_input(
    builder: &mut SourceReferenceBuilderV29<'_, '_, '_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Vec<usize> {
    let index = instance.index();
    let summary = builder.summaries[index].as_ref().unwrap();
    let mut frames = source_reference_scratch_v29(summary.before.frames.len(), budget).unwrap();
    budget.charge_work(summary.before.frames.len()).unwrap();
    frames.extend_from_slice(&summary.before.frames);
    let source = summary.arguments.as_ref().unwrap();
    let mut arguments = source_reference_scratch_v29(source.len(), budget).unwrap();
    budget.charge_work(source.len()).unwrap();
    arguments.extend_from_slice(source);
    let input = builder
        .cfg_clone(&SourceReferenceCfgStateV29 { frames }, budget)
        .unwrap();
    builder.cfg_install(&input, budget).unwrap();
    arguments
}

#[test]
fn changed_scalar_arguments_reexecute_nested_transfers_and_join_entry_views() {
    with_builder(transfer_owner(), |builder, budget| {
        let helper = builder
            .plan
            .instances
            .id_at(helper_index(builder, 0))
            .unwrap();
        let worker = builder
            .plan
            .instances
            .incoming(helper)
            .unwrap()
            .occurrence()
            .caller;
        let mut arguments = restore_instance_input(builder, worker, budget);
        arguments[0] = builder.plain(WORD, budget).unwrap();
        let blocks = builder.plan.blocks.len();
        builder.function(worker, Some(&arguments), budget).unwrap();
        let entry = builder.plan.entries[worker.index()].unwrap();
        let argument = builder.plan.states[entry][1].node.unwrap();
        assert!(matches!(
            builder.plan.nodes[argument].kind,
            SourceReferenceNodeKindV29::Plain(None)
        ));
        assert_eq!(
            builder.summaries[worker.index()]
                .as_ref()
                .unwrap()
                .evaluations,
            2
        );
        assert_eq!(
            builder.summaries[helper.index()]
                .as_ref()
                .unwrap()
                .evaluations,
            2
        );
        assert_eq!(builder.plan.blocks.len(), blocks);
        assert_eq!(builder.plan.loans.len(), 2);
        for loan in &builder.plan.loans {
            assert_eq!(loan.effects.referent_reads, 1);
            assert_eq!(loan.effects.referent_writes, 1);
        }
        builder.finish_effects(budget).unwrap();
        builder.plan_scalar_cells(budget).unwrap();
        assert_eq!(builder.plan.cells.rows.len(), 2);
    });
}

fn set_plain(
    builder: &mut SourceReferenceBuilderV29<'_, '_, '_>,
    instance: ProductionCallInstanceIdV1,
    local: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> usize {
    let id = SemanticLocalIdV1::from_index(local);
    let mut value = builder.local(instance, id).unwrap();
    let node = builder.plain(WORD, budget).unwrap();
    value.node = Some(node);
    builder.set_local(instance, id, value).unwrap();
    node
}

#[test]
fn exact_call_inputs_reuse_without_reexecuting_or_duplicating_source_rows() {
    with_builder(transfer_owner(), |builder, budget| {
        let (helper, worker, arguments) = restore_input(builder, 0, budget);
        let blocks = builder.plan.blocks.len();
        let accesses = builder.plan.accesses.len();
        let marker = SourceReferenceSiteV29 {
            instance: worker,
            block: SemanticBlockIdV1::from_index(0),
            statement: None,
        };
        builder.effect_site = Some(marker);
        builder.effect_ordinal = 7;
        builder.function(helper, Some(&arguments), budget).unwrap();
        let summary = builder.summaries[helper.index()].as_ref().unwrap();
        assert_eq!(summary.evaluations, 1);
        assert_eq!(summary.reuse_count, 1);
        assert_eq!(builder.plan.blocks.len(), blocks);
        assert_eq!(builder.plan.accesses.len(), accesses);
        assert_eq!(builder.effect_site, Some(marker));
        assert_eq!(builder.effect_ordinal, 7);
        assert!(builder.frames[helper.index()].is_none());
    });
}

#[test]
fn changed_call_input_preserves_unrelated_current_values_and_initialization() {
    with_builder(transfer_owner(), |builder, budget| {
        let (helper, worker, arguments) = restore_input(builder, 0, budget);
        let unrelated = set_plain(builder, worker, 5, budget);
        let initialized = set_plain(builder, worker, 6, budget);
        let blocks = builder.plan.blocks.len();
        let accesses = builder.plan.accesses.len();
        builder.function(helper, Some(&arguments), budget).unwrap();
        assert_eq!(
            builder
                .local(worker, SemanticLocalIdV1::from_index(5))
                .unwrap()
                .node,
            Some(unrelated)
        );
        assert_eq!(
            builder
                .local(worker, SemanticLocalIdV1::from_index(6))
                .unwrap()
                .node,
            Some(initialized)
        );
        let referent = builder
            .local(worker, SemanticLocalIdV1::from_index(1))
            .unwrap()
            .node
            .unwrap();
        assert!(matches!(
            builder.plan.nodes[referent].kind,
            SourceReferenceNodeKindV29::Plain(None)
        ));
        let summary = builder.summaries[helper.index()].as_ref().unwrap();
        assert_eq!(summary.evaluations, 2);
        assert_eq!(summary.reuse_count, 1);
        assert_eq!(builder.plan.blocks.len(), blocks);
        assert_eq!(builder.plan.accesses.len(), accesses);
        for loan in &builder.plan.loans {
            assert_eq!(loan.effects.referent_reads, 1);
            assert_eq!(loan.effects.referent_writes, 1);
        }
        let (helper, _, arguments) = restore_input(builder, 0, budget);
        builder.function(helper, Some(&arguments), budget).unwrap();
        let summary = builder.summaries[helper.index()].as_ref().unwrap();
        assert_eq!((summary.evaluations, summary.reuse_count), (2, 2));
    });
}

#[test]
fn current_dead_or_reincarnated_referent_is_rechecked_before_summary_publication() {
    for reincarnated in [false, true] {
        with_builder(transfer_owner(), |builder, budget| {
            let (helper, worker, arguments) = restore_input(builder, 0, budget);
            let id = SemanticLocalIdV1::from_index(1);
            let mut state = builder.local(worker, id).unwrap();
            if reincarnated {
                state.generation += 1;
            } else {
                state.node = None;
            }
            builder.set_local(worker, id, state).unwrap();
            assert!(matches!(
                builder.function(helper, Some(&arguments), budget),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference referent is dead or replaced",
                    ..
                })
            ));
            let summary = builder.summaries[helper.index()].as_ref().unwrap();
            assert_eq!((summary.evaluations, summary.reuse_count), (1, 0));
        });
    }
}

#[test]
fn changed_call_argument_cannot_substitute_another_occurrence_loan() {
    with_builder(transfer_owner(), |builder, budget| {
        let other = helper_index(builder, 1);
        let substituted = builder.summaries[other]
            .as_ref()
            .unwrap()
            .arguments
            .as_ref()
            .unwrap()
            .clone();
        let (helper, _, _) = restore_input(builder, 0, budget);
        assert!(matches!(
            builder.function(helper, Some(&substituted), budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference CFG merge changes loan identity",
                ..
            })
        ));
        assert_eq!(
            builder.summaries[helper.index()]
                .as_ref()
                .unwrap()
                .evaluations,
            1
        );
    });
}

#[test]
fn a_live_call_frame_cannot_be_reentered_using_a_completed_summary() {
    with_builder(transfer_owner(), |builder, budget| {
        let (helper, _, arguments) = restore_input(builder, 0, budget);
        builder.frames[helper.index()] = builder.plan.entries[helper.index()];
        assert!(matches!(
            builder.reuse_call_summary(helper, Some(&arguments), budget),
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        assert_eq!(
            builder.summaries[helper.index()]
                .as_ref()
                .unwrap()
                .evaluations,
            1
        );
    });
}

fn loop_owner(exit_first: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Writeback, |_, functions| {
        let old = &functions[1];
        let mut locals = old.locals().to_vec();
        locals.extend([
            local(240, WORD, SemanticLocalRoleV1::Temporary),
            local(241, WORD, SemanticLocalRoleV1::Temporary),
        ]);
        let mut start = vec![assign(
            place(5, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
        )];
        start.extend_from_slice(old.blocks()[0].statements());
        let referent = || {
            projected(
                3,
                &[
                    (SemanticProjectionKindV1::Field(0), REFERENCE),
                    (SemanticProjectionKindV1::Dereference, WORD),
                ],
            )
        };
        let (first, otherwise) = if exit_first { (3, 2) } else { (2, 3) };
        functions[1] = function(
            20,
            false,
            WORD,
            locals,
            vec![
                block(
                    20,
                    start,
                    SemanticTerminatorKindV1::Goto(fixture_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    21,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(place(5, WORD)),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                fixture_edge(SemanticEdgeRoleV1::SwitchValue, first),
                            )],
                            fixture_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    22,
                    vec![
                        assign(referent(), fixture_word(17)),
                        assign(
                            place(6, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(referent())),
                        ),
                        assign(place(5, WORD), fixture_word(0)),
                    ],
                    SemanticTerminatorKindV1::Goto(fixture_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    23,
                    vec![],
                    call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 4),
                ),
                block(24, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    })
}

#[test]
fn source_loop_rechecks_both_orderings_without_duplicate_calls_accesses_or_blocks() {
    for exit_first in [false, true] {
        with_builder(loop_owner(exit_first), |builder, budget| {
            builder.finish_effects(budget).unwrap();
            builder.plan_scalar_cells(budget).unwrap();
            assert_eq!(builder.plan.cells.rows.len(), 2);
            let mut calls = 0;
            for (index, row) in builder.plan.instances.instances().iter().enumerate() {
                if row.function() == SemanticFunctionIdV1::from_index(2) {
                    let summary = builder.summaries[index].as_ref().unwrap();
                    assert!(summary.evaluations >= 2);
                    assert!(summary.reuse_count >= 1);
                    calls += 1;
                }
            }
            assert_eq!(calls, 2);
            let expected: usize = builder
                .plan
                .instances
                .instances()
                .iter()
                .map(|row| row.declaration().blocks().len())
                .sum();
            assert_eq!(builder.plan.blocks.len(), expected);
            assert_eq!(builder.block_sites.len(), expected);
            assert_eq!(builder.plan.access_sites.len(), builder.plan.accesses.len());
            for loan in &builder.plan.loans {
                assert_eq!(loan.effects.referent_reads, 2);
                assert_eq!(loan.effects.referent_writes, 2);
            }
        });
    }
}

#[test]
fn transfer_headers_have_independent_exact_and_one_short_storage_limits() {
    use std::mem::size_of;
    let required = size_of::<Result<usize, ProductionSemanticKirErrorV1>>()
        + 2 * size_of::<usize>()
        + size_of::<bool>();
    assert_eq!(
        source_reference_call_transfer_headers_v29().unwrap(),
        required
    );
    for short in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, 13 + required - short);
        budget.reserve_storage(13).unwrap();
        assert_eq!(budget.reserve_storage(required).is_ok(), short == 0);
        assert_eq!(
            budget.storage(),
            if short == 0 { 13 + required } else { 13 }
        );
        budget.release_storage(budget.storage() - 13).unwrap();
        assert_eq!(budget.storage(), 13);
    }
}

#[test]
fn transfer_resource_refusal_never_installs_a_cached_after_state() {
    for work_cut in [false, true] {
        with_builder(transfer_owner(), |builder, budget| {
            let (helper, worker, arguments) = restore_input(builder, 0, budget);
            let current = set_plain(builder, worker, 6, budget);
            if work_cut {
                budget.charge_work(LIMIT - budget.work()).unwrap();
                let result = builder.reuse_call_summary(helper, Some(&arguments), budget);
                let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error),
                )) = result
                else {
                    panic!("expected transfer work refusal")
                };
                assert_eq!((error.actual(), error.limit()), (LIMIT + 2, LIMIT));
            } else {
                let headers = source_reference_call_transfer_headers_v29().unwrap();
                budget
                    .reserve_storage(LIMIT - budget.storage() - headers + 1)
                    .unwrap();
                let result = builder.reuse_call_summary(helper, Some(&arguments), budget);
                let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error),
                )) = result
                else {
                    panic!("expected transfer storage refusal")
                };
                assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
            }
            assert_eq!(
                builder
                    .local(worker, SemanticLocalIdV1::from_index(6))
                    .unwrap()
                    .node,
                Some(current)
            );
            assert_eq!(
                builder.summaries[helper.index()]
                    .as_ref()
                    .unwrap()
                    .evaluations,
                1
            );
            assert!(builder.frames[helper.index()].is_none());
        });
    }
}
