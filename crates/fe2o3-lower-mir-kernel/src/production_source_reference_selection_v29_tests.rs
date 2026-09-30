use super::*;
use production_call_instances_v1::with_production_call_instances_v1;

include!("production_source_reference_selection_actual_v30_tests.rs");
include!("production_source_reference_selection_bind_v30_tests.rs");
include!("production_source_reference_selection_memory_v30_tests.rs");

fn selection_owner(parallel: bool, recurrence: bool) -> ProductionSemanticSsaOwnerV1 {
    let prior = distinct_origin_join_owner();
    let source = prior.source_semantic();
    let root = &source.functions()[0];
    let mut blocks = root.blocks().to_vec();
    // The second issuer borrows the other original allocation owner.
    blocks[4] = block(
        4,
        vec![assign(
            3,
            BORROW,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(2, CARRIER),
            },
        )],
        blocks[4].terminator().kind().clone(),
    );
    if parallel {
        blocks[3] = block(
            3,
            blocks[3].statements().to_vec(),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(6, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 8),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 8),
                )
                .unwrap(),
            },
        );
    }
    if recurrence {
        let SemanticTerminatorKindV1::Call(previous) = blocks[8].terminator().kind() else {
            panic!("original helper call");
        };
        blocks[8] = block(
            8,
            blocks[8].statements().to_vec(),
            call(1, previous.arguments().to_vec(), 0, UNIT, 10),
        );
        blocks.push(block(
            10,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(6, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 8),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 9),
                )
                .unwrap(),
            },
        ));
    }
    admitted_owner(
        source.types().to_vec(),
        vec![
            rebuild(root, root.locals().to_vec(), blocks),
            source.functions()[1].clone(),
        ],
        source.callables().to_vec(),
    )
}

fn entry_loop_selection_owner() -> ProductionSemanticSsaOwnerV1 {
    let prior = helper_owner(true, false, false);
    let source = prior.source_semantic();
    let helper = &source.functions()[1];
    let mut statements = helper.blocks()[0].statements().to_vec();
    statements.push(assign(
        1,
        REFERENCE,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(3, REFERENCE))),
    ));
    let blocks = vec![
        block(
            100,
            statements,
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(2, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 0),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                )
                .unwrap(),
            },
        ),
        block(101, vec![], SemanticTerminatorKindV1::Return),
    ];
    admitted_owner(
        source.types().to_vec(),
        vec![
            source.functions()[0].clone(),
            rebuild(helper, helper.locals().to_vec(), blocks),
        ],
        source.callables().to_vec(),
    )
}

fn with_plan(
    owner: &ProductionSemanticSsaOwnerV1,
    inspect: impl FnOnce(&SourceReferencePlanV29<'_, '_>, &mut ArgumentBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    with_production_call_instances_v1(owner, ROOT, &mut budget, |instances, budget| {
        let builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        inspect(&builder.plan, budget);
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
}

fn original_borrow<'a>(
    plan: &'a SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    block: u32,
) -> &'a SemanticPlaceV1 {
    let SemanticStatementKindV1::Assign(assignment) = plan
        .instances
        .instance(instance)
        .unwrap()
        .declaration()
        .blocks()[block as usize]
        .statements()[0]
        .kind()
    else {
        panic!("original reference assignment");
    };
    let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
        panic!("original typed reborrow");
    };
    place
}

fn site(block: u32) -> ExecutionSiteV29 {
    execution_site_v29(SemanticBlockIdV1::from_index(block), Some(0))
}

fn assert_selection_error(result: Result<(), ProductionSemanticKirErrorV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "reference selection differs from its original typed SSA edges",
            })
        ),
        "{result:?}"
    );
}

#[test]
fn original_distinct_reference_diamond_retains_ordered_edges_and_both_issuers() {
    let owner = selection_owner(false, false);
    with_plan(&owner, |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        let floor = budget.storage();
        with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, budget| {
                let SourceReferenceSelectionStepV29::Parameter {
                    block,
                    variable,
                    first,
                    count,
                    ..
                } = graph.nodes[0].step
                else {
                    panic!("distinct references must remain an original parameter selection");
                };
                assert_eq!((block.get(), variable.get(), count), (8, 7, 2));
                assert_eq!(
                    graph.edges[first].edge,
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(3), 0)
                );
                assert_eq!(
                    graph.edges[first + 1].edge,
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(7), 0)
                );
                assert_ne!(graph.edges[first].input, graph.edges[first + 1].input);
                let leaves = graph
                    .nodes
                    .iter()
                    .filter_map(|node| match node.step {
                        SourceReferenceSelectionStepV29::Leaf(
                            SourceExternalReferenceOriginV29::Issued { instance, recipe },
                        ) => {
                            assert_eq!(instance, plan.root);
                            assert_eq!(
                                (recipe.pointer_type, recipe.form, recipe.access),
                                (
                                    REFERENCE,
                                    SourceIssuedFormV29::Pointer,
                                    AccessMode::ReadWrite
                                )
                            );
                            Some(recipe.block.index())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(leaves, [1, 5]);
                check_source_reference_selection_rows_v29(
                    plan,
                    plan.root,
                    site(8),
                    ExecutionOperandV29::RvaluePlace,
                    source,
                    graph,
                    budget,
                )
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn original_reference_selection_preserves_parallel_edges_and_seeded_loop_recurrence() {
    let owner = selection_owner(true, true);
    with_plan(&owner, |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        let floor = budget.storage();
        with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, budget| {
                let SourceReferenceSelectionStepV29::Parameter { first, count, .. } =
                    graph.nodes[0].step
                else {
                    panic!("original loop parameter");
                };
                assert_eq!(count, 4);
                let incoming = &graph.edges[first..first + count];
                assert_eq!(
                    incoming
                        .iter()
                        .map(|edge| (edge.edge.source().get(), edge.edge.ordinal()))
                        .collect::<Vec<_>>(),
                    [(3, 0), (3, 1), (7, 0), (10, 0)]
                );
                assert_eq!(incoming[0].input, incoming[1].input);
                assert_ne!(incoming[0].input, incoming[2].input);
                assert!(graph.edges.iter().any(|edge| edge.input == 0));
                assert_eq!(
                    graph
                        .nodes
                        .iter()
                        .filter(|node| matches!(
                            node.step,
                            SourceReferenceSelectionStepV29::Leaf(_)
                        ))
                        .count(),
                    2
                );
                check_source_reference_selection_rows_v29(
                    plan,
                    plan.root,
                    site(8),
                    ExecutionOperandV29::RvaluePlace,
                    source,
                    graph,
                    budget,
                )
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn original_entry_reference_loop_retains_invocation_separately_from_its_backedge() {
    let owner = entry_loop_selection_owner();
    with_plan(&owner, |plan, budget| {
        let child = ProductionCallInstanceIdV1(1);
        let source = original_borrow(plan, child, 0);
        with_source_reference_selection_v29(
            plan,
            child,
            site(0),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, budget| {
                let SourceReferenceSelectionStepV29::Parameter {
                    block,
                    variable,
                    invocation: Some(input),
                    first,
                    count,
                    ..
                } = graph.nodes[0].step
                else {
                    panic!("original entry loop must retain its invocation selection");
                };
                assert_eq!((block.get(), variable.get(), count), (0, 1, 1));
                assert_eq!(
                    graph.edges[first].edge,
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0)
                );
                assert!(matches!(
                    graph.nodes[input].step,
                    SourceReferenceSelectionStepV29::CallArgument { .. }
                ));
                assert_ne!(graph.edges[first].input, input);
                check_source_reference_selection_rows_v29(
                    plan,
                    child,
                    site(0),
                    ExecutionOperandV29::RvaluePlace,
                    source,
                    graph,
                    budget,
                )?;
                for forged in [None, Some(graph.edges[first].input)] {
                    let mut nodes = graph.nodes.clone();
                    let SourceReferenceSelectionStepV29::Parameter { invocation, .. } =
                        &mut nodes[0].step
                    else {
                        panic!("retained original entry parameter");
                    };
                    *invocation = forged;
                    let forged = SourceReferenceSelectionGraphV29 {
                        subject: graph.subject,
                        nodes,
                        edges: graph.edges.clone(),
                    };
                    assert_selection_error(check_source_reference_selection_rows_v29(
                        plan,
                        child,
                        site(0),
                        ExecutionOperandV29::RvaluePlace,
                        source,
                        &forged,
                        budget,
                    ));
                }
                check_source_reference_selection_rows_v29(
                    plan,
                    child,
                    site(0),
                    ExecutionOperandV29::RvaluePlace,
                    source,
                    graph,
                    budget,
                )
            },
        )
        .unwrap();
    });
}

#[test]
fn single_origin_helper_selection_keeps_exact_call_argument_and_replay_is_unchanged() {
    let owner = helper_owner(true, false, false);
    with_plan(&owner, |plan, budget| {
        let child = ProductionCallInstanceIdV1(1);
        let source = original_borrow(plan, child, 0);
        let floor = budget.storage();
        for _ in 0..2 {
            with_source_reference_selection_v29(
                plan,
                child,
                site(0),
                ExecutionOperandV29::RvaluePlace,
                source,
                budget,
                |graph, budget| {
                    let SourceReferenceSelectionStepV29::CallArgument {
                        call,
                        local,
                        source_argument,
                        input,
                        ..
                    } = graph.nodes[0].step
                    else {
                        panic!("original helper argument edge");
                    };
                    assert_eq!(
                        (
                            call.caller,
                            call.block.index(),
                            local.index(),
                            source_argument
                        ),
                        (plan.root, 3, 1, 0)
                    );
                    assert!(matches!(
                        graph.nodes[input].step,
                        SourceReferenceSelectionStepV29::Leaf(
                            SourceExternalReferenceOriginV29::Issued { .. }
                        )
                    ));
                    assert!(graph.edges.is_empty());
                    check_source_reference_selection_rows_v29(
                        plan,
                        child,
                        site(0),
                        ExecutionOperandV29::RvaluePlace,
                        source,
                        graph,
                        budget,
                    )
                },
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn source_selection_replay_rejects_forged_missing_reordered_duplicate_and_wrong_typed_rows() {
    let owner = selection_owner(false, false);
    with_plan(&owner, |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        with_source_reference_selection_v29(plan, plan.root, site(8), ExecutionOperandV29::RvaluePlace,
            source, budget, |graph, budget| with_canonical_call_scratch_v1(budget, |budget| {
                source_reference_owned_prepay_v29::<SourceReferenceSelectionGraphV29>(plan, budget)?;
                let mut nodes = emission_vec_v1(graph.nodes.len(), budget)?;
                let mut edges = emission_vec_v1(graph.edges.len() + 1, budget)?;
                for fault in 0..8 {
                    nodes.clear(); edges.clear();
                    budget.charge_work(graph.nodes.len() + graph.edges.len())?;
                    nodes.extend_from_slice(&graph.nodes);
                    edges.extend_from_slice(&graph.edges);
                    let mut changed = SourceReferenceSelectionGraphV29 { subject: graph.subject, nodes, edges };
                    match fault {
                        0 => changed.edges.clear(),
                        1 => changed.edges.swap(0, 1),
                        2 => changed.edges.push(changed.edges[0]),
                        3 => changed.edges[0].edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(3), 1),
                        4 => changed.edges[1].input = changed.edges[0].input,
                        5 => changed.nodes[0].value.pointer_type = BORROW,
                        6 => changed.subject.source ^= 1,
                        7 => changed.nodes[1].step = changed.nodes[2].step,
                        _ => unreachable!(),
                    }
                    assert_selection_error(check_source_reference_selection_rows_v29(plan, plan.root, site(8), ExecutionOperandV29::RvaluePlace, source, &changed, budget));
                    nodes = changed.nodes; edges = changed.edges;
                    check_source_reference_selection_rows_v29(plan, plan.root, site(8), ExecutionOperandV29::RvaluePlace, source, graph, budget)?;
                }
                let copied = source.clone();
                let refused = with_source_reference_selection_v29(plan, plan.root, site(8), ExecutionOperandV29::RvaluePlace, &copied, budget, |_, _| Ok(()));
                assert!(matches!(refused, Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0, block: None, statement: None,
                    detail: "source issued pointer differs from its original issuer or actual guard",
                })));
                check_source_reference_selection_rows_v29(plan, plan.root, site(8), ExecutionOperandV29::RvaluePlace, source, graph, budget)
            })).unwrap();
    });
}

#[test]
fn selection_fixed_headers_and_callback_envelopes_have_an_independent_size_oracle() {
    use std::mem::{align_of_val, size_of, size_of_val};
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let expected = h::<SourceReferenceSelectionBuilderV29<'_, '_, '_>>()
        + h::<SourceReferenceSelectionGraphV29>()
        + h::<SourceReferenceSelectionSubjectV29>()
        + h::<SourceReferenceSelectionValueV29>()
        + h::<SourceReferenceSelectionNodeV29>()
        + h::<SourceReferenceSelectionStepV29>()
        + h::<SourceReferenceSelectionEdgeV29>()
        + h::<Option<SourceExternalReferenceOriginV29>>()
        + h::<Option<SemanticLocalIdV1>>()
        + h::<Option<usize>>()
        + h::<&mut SourceIssuedSemanticV29<'_, '_, '_>>()
        + h::<Vec<bool>>()
        + h::<usize>()
        + h::<bool>()
        + h::<()>();
    assert_eq!(source_reference_selection_headers_v29().unwrap(), expected);
    let captured = [7usize; 3];
    let callback = move |_: &SourceReferenceSelectionGraphV29, _: &mut ArgumentBudgetV1<'_>| {
        assert_eq!(captured, [7; 3]);
        Ok::<_, ProductionSemanticKirErrorV1>(())
    };
    let envelope = 2 * size_of_val(&callback)
        + 2 * align_of_val(&callback)
        + h::<()>()
        + size_of::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<&SourceReferencePlanV29<'_, '_>>()
        + 2 * size_of::<&SemanticPlaceV1>()
        + 2 * size_of::<&mut ArgumentBudgetV1<'_>>();
    assert_eq!(
        source_reference_selection_call_headers_v29::<()>(&callback).unwrap(),
        envelope
    );
}

#[test]
fn selection_seed_check_rejects_an_unseeded_cycle_without_choosing_an_unrelated_leaf() {
    let owner = selection_owner(false, false);
    with_plan(&owner, |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, budget| {
                with_canonical_call_scratch_v1(budget, |budget| {
                    source_reference_owned_prepay_v29::<
                        SourceReferenceSelectionBuilderV29<'_, '_, '_>,
                    >(plan, budget)?;
                    let mut nodes = emission_vec_v1(graph.nodes.len(), budget)?;
                    let mut edges = emission_vec_v1(graph.edges.len(), budget)?;
                    budget.charge_work(graph.nodes.len() + graph.edges.len())?;
                    nodes.extend_from_slice(&graph.nodes);
                    edges.extend_from_slice(&graph.edges);
                    nodes[0].step = SourceReferenceSelectionStepV29::Alias {
                        site: site(8),
                        role: ExecutionOperandV29::RvaluePlace,
                        source: source as *const SemanticPlaceV1 as usize,
                        reborrow: true,
                        input: 0,
                    };
                    let hostile = SourceReferenceSelectionBuilderV29 {
                        plan,
                        graph: SourceReferenceSelectionGraphV29 {
                            subject: graph.subject,
                            nodes,
                            edges,
                        },
                        indices: BTreeMap::new(),
                        originals: BTreeMap::new(),
                        node_bound: graph.nodes.len(),
                    };
                    assert_selection_error(hostile.check_seeded(budget));
                    check_source_reference_selection_rows_v29(
                        plan,
                        plan.root,
                        site(8),
                        ExecutionOperandV29::RvaluePlace,
                        source,
                        graph,
                        budget,
                    )
                })
            },
        )
        .unwrap();
    });
}

#[test]
fn selection_callback_cannot_undercut_retained_graph_storage_or_erase_failure() {
    let owner = selection_owner(false, false);
    with_plan(&owner, |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        let floor = budget.storage();
        let mut callbacks = 0;
        let denied = with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, budget| {
                assert!(!graph.nodes.is_empty());
                callbacks += 1;
                budget.release_storage(1)?;
                Ok(())
            },
        );
        assert!(matches!(
            denied,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((callbacks, budget.storage()), (1, floor));
        let before = (budget.work(), budget.storage());
        let retry = with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |_, _| {
                callbacks += 1;
                Ok(())
            },
        );
        assert!(matches!(
            retry,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(
            (callbacks, budget.work(), budget.storage()),
            (1, before.0, before.1)
        );
    });
}

fn resource_run(
    owner: &ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionSemanticKirErrorV1>,
    usize,
    usize,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let mut result = None;
    let mut callbacks = 0;
    let mut before_peak = 0;
    let mut accepted_work = 0;
    let mut accepted_peak = 0;
    with_production_call_instances_v1(owner, ROOT, &mut budget, |instances, budget| {
        let builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        let source = original_borrow(&builder.plan, builder.plan.root, 8);
        let floor = budget.storage();
        before_peak = budget.peak_storage();
        let query = with_source_reference_selection_v29(
            &builder.plan, builder.plan.root, site(8), ExecutionOperandV29::RvaluePlace,
            source, budget, |graph, budget| {
                assert!(!graph.edges.is_empty());
                callbacks += 1;
                accepted_work = budget.work();
                accepted_peak = budget.peak_storage();
                Ok(())
            },
        );
        assert_eq!(budget.storage(), floor);
        if let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first)) = &query {
            let (work, storage) = (budget.work(), budget.storage());
            assert!(matches!(builder.plan.check_owner(instances, budget),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(again)) if again == *first));
            assert_eq!((budget.work(), budget.storage()), (work, storage));
        }
        result = Some(query);
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    }).unwrap();
    (
        result.unwrap(),
        callbacks,
        before_peak,
        accepted_work,
        accepted_peak,
    )
}

#[test]
fn selection_complete_query_has_exact_and_one_short_work_storage_and_sticky_refusal() {
    let owner = selection_owner(true, true);
    let (result, callbacks, before, work, peak) = resource_run(&owner, usize::MAX, usize::MAX);
    result.unwrap();
    assert_eq!(callbacks, 1);
    assert!(peak > before);
    let exact = resource_run(&owner, work, peak);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3, exact.4), (1, work, peak));
    let work_short = resource_run(&owner, work - 1, peak);
    assert!(matches!(work_short.0,
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
            if error.actual() == work && error.limit() == work - 1));
    assert_eq!(work_short.1, 0);
    let storage_short = resource_run(&owner, work, peak - 1);
    assert!(matches!(storage_short.0,
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error)))
            if error.actual() == peak && error.limit() == peak - 1));
    assert_eq!(storage_short.1, 0);
}

#[test]
fn selection_query_rejects_fully_funded_foreign_ledger_without_consuming_a_callback() {
    let owner = selection_owner(false, false);
    with_plan(&owner, |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        let before = (budget.work(), budget.storage());
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
        foreign.reserve_storage(before.1).unwrap();
        let mut callbacks = 0;
        let denied = with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            &mut foreign,
            |_, _| {
                callbacks += 1;
                Ok(())
            },
        );
        assert!(matches!(
            denied,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(
            (callbacks, foreign.work(), foreign.storage()),
            (0, 0, before.1)
        );
        let denied = with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |_, _| {
                callbacks += 1;
                Ok(())
            },
        );
        assert!(matches!(
            denied,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(
            (callbacks, budget.work(), budget.storage()),
            (0, before.0, before.1)
        );
        foreign.release_storage(before.1).unwrap();
        assert_eq!(foreign.storage(), 0);
    });
}
