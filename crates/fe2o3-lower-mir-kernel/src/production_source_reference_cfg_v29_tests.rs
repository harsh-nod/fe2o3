use super::*;
use super::super::source_reference_lowering_v29_tests::cells_tests;

fn distinct_exit_loan_owner_v29(shared_successor: bool, writeback: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(if writeback { Case::Writeback } else { Case::Shared }, |_, functions| {
        let borrowed = if writeback { SemanticBorrowKindV1::Mutable } else { SemanticBorrowKindV1::Shared };
        let mut blocks = vec![block(130, vec![
            assign(place(2, REFERENCE), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                projected(1, &[(SemanticProjectionKindV1::Field(0), REFERENCE)]),
            ))),
            assign(place(3, WORD), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)]),
            ))),
            assign(place(4, WORD), fixture_word(29)),
        ], choose_word(3, 1, 2))];
        for branch in 1..=2 {
            let mut statements = vec![assign(place(2, REFERENCE), SemanticRvalueKindV1::Borrow {
                kind: borrowed,
                place: place(branch + 2, WORD),
            })];
            if writeback {
                statements.push(assign(projected(1, &[
                    (SemanticProjectionKindV1::Field(0), REFERENCE),
                    (SemanticProjectionKindV1::Dereference, WORD),
                ]), fixture_word(if branch == 1 { 17 } else { 23 })));
            }
            statements.push(unit());
            blocks.push(block((130 + branch) as u8, statements,
                if shared_successor { go(3) } else { SemanticTerminatorKindV1::Return }));
        }
        if shared_successor {
            blocks.push(block(133, vec![assign(place(4, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                    2, &[(SemanticProjectionKindV1::Dereference, WORD)],
                ))))], SemanticTerminatorKindV1::Return));
        }
        functions[2] = function(30, false, CAPTURE, vec![
            local(30, UNIT, SemanticLocalRoleV1::Return),
            local(31, CAPTURE, SemanticLocalRoleV1::Argument(0)),
            local(132, REFERENCE, SemanticLocalRoleV1::Temporary),
            local(133, WORD, SemanticLocalRoleV1::Temporary),
            local(134, WORD, SemanticLocalRoleV1::Temporary),
        ], blocks);
    })
}

#[test]
fn checked_return_drops_only_exiting_holders_and_retains_source_observations() {
    for writeback in [false, true] {
        cells_tests::run_cells(distinct_exit_loan_owner_v29(false, writeback), |plan, _| {
            let mut helpers = 0;
            for ordinal in 0..plan.instances.instances().len() {
                let instance = plan.instances.id_at(ordinal).unwrap();
                if plan.instances.instance(instance).unwrap().declaration().locals().len() != 5
                    || plan.instances.instance(instance).unwrap().declaration().blocks().len() != 3
                { continue; }
                helpers += 1;
                for branch in [1, 2] {
                    assert!(plan.blocks.iter().any(|row| row.instance == instance && row.block.index() == branch));
                    assert!(plan.accesses.iter().any(|row| row.key.site.instance == instance
                        && row.key.site.block.index() == branch && row.key.site.statement == Some(0)
                        && matches!(row.key.access, SourceReferenceAccessV29::Borrow(_))));
                    assert!(plan.boundary_values.iter().any(|row| row.site.instance == instance
                        && row.site.block.index() == branch && row.role == SourceReferenceBoundaryRoleV29::Return));
                    assert!(plan.loans.iter().any(|loan| {
                        let origin = &plan.origins[loan.origin];
                        origin.instance == instance && origin.local.index() == branch + 2
                    }));
                }
                assert_eq!(plan.nodes[plan.returns[ordinal].unwrap()].ty, UNIT);
                if writeback {
                    let incoming = plan.instances.incoming(instance).unwrap().occurrence();
                    let continuation = plan.blocks.iter().find(|row|
                        row.instance == incoming.caller && row.block.index() == 1).unwrap();
                    let node = plan.states[continuation.entry][1].node.unwrap();
                    assert!(matches!(plan.nodes[node].kind, SourceReferenceNodeKindV29::Plain(None)));
                    assert!(plan.loans.iter().any(|loan| {
                        let origin = &plan.origins[loan.origin];
                        origin.instance == incoming.caller && origin.local.index() == 1
                            && loan.effects.referent_writes == 2
                    }));
                }
            }
            assert_eq!(helpers, 2, "distinct helper instances must both retain their original exits");
            Ok(())
        }).unwrap();
    }
}

#[test]
fn checked_return_projection_does_not_relax_live_cfg_loan_joins_or_escapes() {
    let error = cells_tests::run_cells(distinct_exit_loan_owner_v29(true, false), |_, _|
        panic!("live predecessor alternatives were silently erased")).unwrap_err();
    assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
        detail: "source reference CFG merge changes loan identity", ..
    }), "{error:?}");
    let error = cells_tests::run_cells(cells_tests::return_alias_owner(true), |_, _|
        panic!("own-frame return escaped before continuation projection")).unwrap_err();
    assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
        detail: "source reference return escapes its referent call frame", ..
    }), "{error:?}");
    cells_tests::run_cells(cells_tests::return_alias_owner(false), |plan, _| {
        let mut observed = 0;
        for (ordinal, value) in plan.returns.iter().enumerate() {
            let Some(node) = value else { continue };
            if plan.nodes[*node].ty != CAPTURE { continue; }
            let instance = plan.instances.id_at(ordinal).unwrap();
            let SourceReferenceNodeKindV29::Aggregate { first, count: 1 } = plan.nodes[*node].kind
                else { panic!("ancestor return lost its original aggregate") };
            let SourceReferenceNodeKindV29::Loan(loan) = plan.nodes[plan.children[first]].kind
                else { panic!("ancestor return lost its actual loan") };
            assert_ne!(plan.origins[plan.loans[loan].origin].instance, instance);
            observed += 1;
        }
        assert_eq!(observed, 2);
        Ok(())
    }).unwrap();
}

fn install_return_projection_frames_v29(
    builder: &mut SourceReferenceBuilderV29<'_, '_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    assert_eq!(builder.frames.len(), 3);
    for ordinal in 0..3 {
        let instance = builder.plan.instances.id_at(ordinal).unwrap();
        let count = builder.plan.instances.instance(instance).unwrap().declaration().locals().len();
        let mut state = source_reference_scratch_v29(count, budget).unwrap();
        budget.charge_work(count).unwrap();
        state.resize(count, SourceReferenceLocalV29::default());
        let index = builder.plan.states.len();
        emission_push_v1(&mut builder.plan.states, state, budget).unwrap();
        builder.frames[ordinal] = Some(index);
    }
}

#[test]
fn checked_return_projection_requires_all_and_only_live_original_frames() {
    let owner = loop_owner(LoopCase::Invariant);
    let mut outer_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut outer = ArgumentBudgetV1::new(&mut outer_work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut outer, |instances, _| {
        for mutation in 0..8 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(73).unwrap();
            let mut builder = SourceReferenceBuilderV29::new(instances, &mut budget).unwrap();
            install_return_projection_frames_v29(&mut builder, &mut budget);
            let exiting = instances.id_at(2).unwrap();
            let mut captured = builder.cfg_capture(&mut budget).unwrap();
            budget.charge_work(captured.frames.len()).unwrap();
            match mutation {
                0 => { captured.frames.remove(0); }
                1 => { captured.frames.pop(); }
                2 => captured.frames[1] = captured.frames[0],
                3 => captured.frames.swap(0, 1),
                4 => captured.frames[0].1 = captured.frames[1].1,
                5 => builder.frames[0] = None,
                6 => builder.frames[2] = None,
                7 => { builder.plan.states.pop(); }
                _ => unreachable!(),
            }
            let mut before = source_reference_scratch_v29(captured.frames.len(), &mut budget).unwrap();
            budget.charge_work(captured.frames.len()).unwrap();
            before.extend_from_slice(&captured.frames);
            let capacity = captured.frames.capacity();
            let error = builder.cfg_project_return_continuation(exiting, &mut captured, &mut budget).unwrap_err();
            assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported { .. }), "{error:?}");
            assert_eq!(captured.frames, before);
            assert_eq!(captured.frames.capacity(), capacity);
            drop(before);
            drop(captured);
            drop(builder);
            budget.release_storage(budget.storage() - 73).unwrap();
            assert_eq!(budget.storage(), 73);
        }
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    }).unwrap();
}

#[test]
fn checked_return_projection_prepays_independent_exact_work_and_headers() {
    let owner = loop_owner(LoopCase::Invariant);
    let mut outer_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut outer = ArgumentBudgetV1::new(&mut outer_work, usize::MAX);
    let headers = std::mem::size_of::<Option<usize>>() + 4 * std::mem::size_of::<usize>()
        + 2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
    assert_eq!(source_reference_cfg_exit_headers_v29().unwrap(), headers);
    production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut outer, |instances, _| {
        for storage_cut in [false, true] {
            for slack in [0, 1] {
                const LIMIT: usize = 1_000_000;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
                budget.reserve_storage(73).unwrap();
                let mut builder = SourceReferenceBuilderV29::new(instances, &mut budget).unwrap();
                install_return_projection_frames_v29(&mut builder, &mut budget);
                let mut captured = builder.cfg_capture(&mut budget).unwrap();
                assert_eq!(captured.frames, [(0, 0), (1, 1), (2, 2)]);
                let capacity = captured.frames.capacity();
                // Six fixed checks, seven visits per each of all three slots,
                // and three prepaid shifts; not a measured successful budget.
                let work_needed = 30;
                if storage_cut {
                    budget.reserve_storage(LIMIT - budget.storage() - (headers - slack)).unwrap();
                } else {
                    budget.charge_work(LIMIT - budget.work() - (work_needed - slack)).unwrap();
                }
                let previous_work = budget.work();
                let previous_storage = budget.storage();
                let result = builder.cfg_project_return_continuation(instances.id_at(2).unwrap(),
                    &mut captured, &mut budget);
                if slack == 0 {
                    result.unwrap();
                    assert_eq!(captured.frames, [(0, 0), (1, 1)]);
                    assert_eq!(budget.work(), previous_work + work_needed);
                    assert_eq!(budget.storage(), previous_storage + headers);
                } else {
                    assert_eq!(captured.frames, [(0, 0), (1, 1), (2, 2)]);
                    match result.unwrap_err() {
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))
                            if storage_cut => {
                                assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
                                assert_eq!(budget.work(), previous_work);
                                assert_eq!(budget.storage(), previous_storage);
                            }
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error))
                            if !storage_cut => {
                                assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
                                assert_eq!(budget.work(), previous_work);
                                assert_eq!(budget.storage(), previous_storage + headers);
                            }
                        error => panic!("wrong projection resource refusal: {error:?}"),
                    }
                }
                assert_eq!(captured.frames.capacity(), capacity);
                assert_eq!(builder.frames, [Some(0), Some(1), Some(2)]);
                drop(captured);
                drop(builder);
                budget.release_storage(budget.storage() - 73).unwrap();
                assert_eq!(budget.storage(), 73);
            }
        }
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    }).unwrap();
}

#[test]
fn checked_return_summaries_keep_ancestor_continuations_and_replay_exact_inputs() {
    let owner = distinct_exit_loan_owner_v29(false, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let floor = budget.storage();
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        let result = builder.function(instances.root(), None, budget).unwrap();
        assert!(builder.frames.iter().all(Option::is_none));
        for (ordinal, row) in instances.instances().iter().enumerate() {
            if row.function() != SemanticFunctionIdV1::from_index(2) { continue; }
            let summary = builder.summaries[ordinal].as_ref().unwrap();
            let after = summary.after.as_ref().unwrap();
            assert_eq!(after.frames.len(), 2);
            assert!(after.frames.iter().all(|(id, _)| *id < ordinal));
            assert_eq!(after.frames.len(), summary.before.frames.len());
            budget.charge_work(after.frames.len()).unwrap();
            for ((after, _), (before, _)) in after.frames.iter().zip(&summary.before.frames) {
                assert_eq!(after, before);
            }
        }
        assert_eq!(builder.function(instances.root(), None, budget).unwrap(), result);
        assert_eq!(builder.summaries[instances.root().index()].as_ref().unwrap().reuse_count, 1);
        assert!(builder.frames.iter().all(Option::is_none));
        drop(builder);
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    }).unwrap();
}

#[test]
fn checked_return_projection_keeps_callback_error_and_panic_cleanup_bounded() {
    for panic_callback in [false, true] {
        let owner = distinct_exit_loan_owner_v29(false, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(73).unwrap();
        let mut reached = false;
        production_call_instances_v1::with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let floor = budget.storage();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                with_source_reference_plan_v29(instances, budget, |plan, _| {
                    assert_eq!(plan.instances.instances().len(), 5);
                    reached = true;
                    if panic_callback { panic!("checked return callback panic probe"); }
                    Err::<(), _>(source_reference_error_v29("checked return callback error probe"))
                })
            ));
            let result = outcome.expect("source scope must consume callback panic after bounded cleanup");
            assert!(result.is_err());
            if !panic_callback {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "checked return callback error probe", ..
                })));
            }
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        }).unwrap();
        assert!(reached);
        assert_eq!(budget.storage(), 73);
    }
}

#[derive(Clone, Copy, Debug)]
enum LoopCase {
    Ordinary,
    Invariant,
    LocalStorage,
    ChangingInitializer,
    SurvivingLoan,
    ChangingHolder,
    Uninitialized,
    MultipleLatch,
    Irreducible,
}

fn go(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(fixture_edge(SemanticEdgeRoleV1::Goto, target))
}

fn choose(left: u32, right: u32) -> SemanticTerminatorKindV1 {
    choose_word(1, left, right)
}

fn choose_word(local: u32, left: u32, right: u32) -> SemanticTerminatorKindV1 {
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

fn capture_reference() -> SemanticStatementV1 {
    assign(
        place(3, CAPTURE),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![SemanticOperandV1::Move(place(2, REFERENCE))],
            )
            .unwrap(),
        ),
    )
}

fn branch_writeback_owner(repeated_static_call: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Writeback, |_, functions| {
        functions[2] = function(
            30,
            false,
            CAPTURE,
            vec![
                local(30, UNIT, SemanticLocalRoleV1::Return),
                local(31, CAPTURE, SemanticLocalRoleV1::Argument(0)),
                local(32, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(33, WORD, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    30,
                    vec![
                        assign(
                            place(2, REFERENCE),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                                1,
                                &[(SemanticProjectionKindV1::Field(0), REFERENCE)],
                            ))),
                        ),
                        assign(
                            place(3, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                                2,
                                &[(SemanticProjectionKindV1::Dereference, WORD)],
                            ))),
                        ),
                    ],
                    choose_word(3, 1, 2),
                ),
                block(
                    31,
                    vec![assign(
                        projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)]),
                        fixture_word(11),
                    )],
                    go(3),
                ),
                block(
                    32,
                    vec![assign(
                        projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)]),
                        fixture_word(23),
                    )],
                    go(3),
                ),
                block(33, vec![dead(2), unit()], SemanticTerminatorKindV1::Return),
            ],
        );
        if repeated_static_call {
            functions[1] = function(
                20,
                false,
                WORD,
                vec![
                    local(20, UNIT, SemanticLocalRoleV1::Return),
                    local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                    local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                    local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
                ],
                vec![
                    block(20, vec![], go(1)),
                    block(21, vec![], choose(2, 4)),
                    block(
                        22,
                        vec![
                            assign(
                                place(2, REFERENCE),
                                SemanticRvalueKindV1::Borrow {
                                    kind: SemanticBorrowKindV1::Mutable,
                                    place: place(1, WORD),
                                },
                            ),
                            capture_reference(),
                            dead(2),
                        ],
                        call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 3),
                    ),
                    block(23, vec![], go(1)),
                    block(24, vec![unit()], SemanticTerminatorKindV1::Return),
                ],
            );
        }
    })
}

fn changing_call_input_owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |_, functions| {
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(25, WORD, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    20,
                    vec![assign(place(5, WORD), fixture_word(17))],
                    choose(1, 2),
                ),
                block(
                    21,
                    vec![borrow_reference(1), capture_reference(), dead(2)],
                    go(3),
                ),
                block(
                    22,
                    vec![borrow_reference(5), capture_reference(), dead(2)],
                    go(3),
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

fn check_branch_writeback_plan(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
    repeated_static_call: bool,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(
        plan.source,
        *plan.instances.owner().source_semantic_sha256()
    );
    assert_eq!(plan.ssa, plan.instances.owner().identity());
    assert_eq!(plan.root, plan.instances.root());
    assert_eq!(plan.instances.instances().len(), 5);
    assert_eq!((plan.loans.len(), plan.origins.len()), (2, 2));
    let block_keys = plan
        .blocks
        .iter()
        .map(|row| (row.instance.index(), row.block.index()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        block_keys.len(),
        plan.blocks.len(),
        "one retained row per static source block"
    );
    assert_eq!(block_keys.len(), if repeated_static_call { 21 } else { 15 });
    let anchor = SourceReferenceAnchorV29 {
        argument: 0,
        ty: WORD,
    };
    let root = plan.instances.instance(plan.root).unwrap();
    assert_eq!(root.function(), ROOT);
    for block in 0..3 {
        let row = plan
            .blocks
            .iter()
            .find(|row| row.instance == plan.root && row.block.index() == block)
            .unwrap();
        let node = plan.states[row.entry][1].node.unwrap();
        assert_eq!(
            plan.nodes[node].kind,
            SourceReferenceNodeKindV29::Plain(Some(anchor)),
            "worker writes cannot replace the root's copied argument"
        );
    }
    let mut root_call_blocks = BTreeSet::new();
    for (index, loan) in plan.loans.iter().enumerate() {
        let origin = &plan.origins[loan.origin];
        assert_eq!(
            (origin.local.index(), origin.generation, origin.ty),
            (1, 0, WORD)
        );
        assert!(origin.projections.is_empty());
        assert_eq!(loan.site.instance, origin.instance);
        assert_eq!(
            loan.site.block.index(),
            if repeated_static_call { 2 } else { 0 }
        );
        assert_eq!(loan.site.statement, Some(0));
        assert_eq!(loan.kind, SemanticBorrowKindV1::Mutable);
        assert_eq!(loan.source_type, REFERENCE);
        assert_eq!(loan.parent, None);
        assert_eq!(
            loan.effects,
            SourceReferenceEffectsV29 {
                referent_reads: 1,
                referent_writes: 2,
                ..SourceReferenceEffectsV29::default()
            }
        );
        assert_eq!(
            loan.representation,
            SourceReferenceRepresentationV29::NeedsAddressable(
                SourceReferenceCellNeedV29::ReferentWrite
            )
        );
        assert!(matches!(
            plan.require_promoted(index, budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference requires checked addressable storage and writeback",
                ..
            })
        ));
        assert!(std::ptr::eq(
            plan.loan_at(loan.site, budget)?.unwrap(),
            loan
        ));
        let worker = plan.instances.instance(origin.instance).unwrap();
        assert_eq!(worker.function(), SemanticFunctionIdV1::from_index(1));
        assert_eq!(
            worker.declaration().locals()[1].role(),
            SemanticLocalRoleV1::Argument(0)
        );
        let incoming = plan
            .instances
            .incoming(origin.instance)
            .unwrap()
            .occurrence();
        assert_eq!(incoming.caller, plan.root);
        assert!(root_call_blocks.insert(incoming.block.index()));
        let SemanticStatementKindV1::Assign(borrow) =
            worker.declaration().blocks()[loan.site.block.index() as usize].statements()[0].kind()
        else {
            panic!("loan must remain bound to its original borrow statement");
        };
        assert!(
            matches!(borrow.value().kind(), SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Mutable, place } if place.local() == origin.local && place.ty() == WORD && place.projections().is_empty())
        );
        let worker_entry = plan.states[plan.entries[origin.instance.index()].unwrap()][1]
            .node
            .unwrap();
        assert_eq!(
            plan.nodes[worker_entry].kind,
            SourceReferenceNodeKindV29::Plain(Some(anchor))
        );
        assert_eq!(
            origin.anchor,
            if repeated_static_call {
                None
            } else {
                Some(anchor)
            }
        );
        assert_eq!(
            plan.nodes[origin.value].kind,
            SourceReferenceNodeKindV29::Plain(origin.anchor)
        );
        let successor_blocks: &[u32] = if repeated_static_call {
            &[1, 2, 3, 4]
        } else {
            &[1]
        };
        for &block in successor_blocks {
            let row = plan
                .blocks
                .iter()
                .find(|row| row.instance == origin.instance && row.block.index() == block)
                .unwrap();
            let state = &plan.states[row.entry];
            assert_eq!(state[1].generation, 0);
            assert_eq!(
                plan.nodes[state[1].node.unwrap()].kind,
                SourceReferenceNodeKindV29::Plain(None),
                "joined writeback cannot resurrect the first-iteration scalar anchor"
            );
            assert!(
                state[2].node.is_none() && state[3].node.is_none(),
                "moved loan holders must remain dead at source successors"
            );
        }
        let helpers = plan
            .instances
            .instances()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, row)| {
                let id = plan.instances.id_at(ordinal).unwrap();
                (row.function() == SemanticFunctionIdV1::from_index(2)
                    && plan.instances.incoming(id).unwrap().occurrence().caller == origin.instance)
                    .then_some(id)
            })
            .collect::<Vec<_>>();
        assert_eq!(helpers.len(), 1);
        let helper = helpers[0];
        let call = plan.instances.incoming(helper).unwrap().occurrence();
        assert_eq!(call.caller, loan.site.instance);
        assert_eq!(call.block, loan.site.block);
        let helper_source = plan.instances.instance(helper).unwrap().declaration();
        for (block, value) in [(1, 11), (2, 23)] {
            let SemanticStatementKindV1::Assign(write) =
                helper_source.blocks()[block].statements()[0].kind()
            else {
                panic!("both original source branches must write through the reference");
            };
            assert_eq!(
                write.destination(),
                &projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)])
            );
            assert_eq!(write.value().kind(), &fixture_word(value));
        }
        for block in 0..4 {
            let row = plan
                .blocks
                .iter()
                .find(|row| row.instance == helper && row.block.index() == block)
                .unwrap();
            let state = &plan.states[row.entry];
            let node = plan.nodes[state[1].node.unwrap()];
            assert_eq!(node.ty, CAPTURE);
            let SourceReferenceNodeKindV29::Aggregate { first, count } = node.kind else {
                panic!("helper must retain its original captured reference");
            };
            assert_eq!(count, 1);
            let captured = plan.nodes[plan.children[first]];
            assert_eq!(captured.ty, REFERENCE);
            assert_eq!(captured.kind, SourceReferenceNodeKindV29::Loan(index));
            if block != 0 {
                assert_eq!(
                    plan.nodes[state[2].node.unwrap()].kind,
                    SourceReferenceNodeKindV29::Loan(index)
                );
            }
        }
    }
    assert_eq!(root_call_blocks, BTreeSet::from([0, 1]));
    assert_ne!(
        plan.origins[plan.loans[0].origin].instance,
        plan.origins[plan.loans[1].origin].instance
    );
    Ok(())
}

#[test]
fn source_reference_cfg_repeated_helpers_join_every_ancestor_writeback_branch() {
    run_owner(branch_writeback_owner(false), |plan, budget| {
        assert_eq!(plan.instances.instances().len(), 5);
        assert_eq!(plan.loans.len(), 2);
        for loan in &plan.loans {
            assert_eq!(loan.effects.referent_writes, 2);
            assert_eq!(
                loan.representation,
                SourceReferenceRepresentationV29::NeedsAddressable(
                    SourceReferenceCellNeedV29::ReferentWrite
                )
            );
            let origin = &plan.origins[loan.origin];
            let continuation = plan
                .blocks
                .iter()
                .find(|block| block.instance == origin.instance && block.block.index() == 1)
                .unwrap();
            let node = plan.states[continuation.entry][1].node.unwrap();
            assert!(matches!(
                plan.nodes[node].kind,
                SourceReferenceNodeKindV29::Plain(None)
            ));
            assert!(
                plan.require_promoted(
                    plan.loans
                        .iter()
                        .position(|candidate| std::ptr::eq(candidate, loan))
                        .unwrap(),
                    budget
                )
                .is_err()
            );
        }
        assert_ne!(
            plan.origins[plan.loans[0].origin].instance,
            plan.origins[plan.loans[1].origin].instance
        );
        check_branch_writeback_plan(plan, budget, false)?;
        Ok(())
    })
    .unwrap();
    run_owner(branch_writeback_owner(true), |plan, budget| {
        check_branch_writeback_plan(plan, budget, true)
    })
    .unwrap();
}

#[test]
fn source_reference_cfg_same_static_call_cannot_reuse_first_reference_input() {
    let owner = changing_call_input_owner();
    let semantic = owner.source_semantic();
    let worker = &semantic.functions()[1];
    assert!(matches!(
        worker.blocks()[3].terminator().kind(),
        SemanticTerminatorKindV1::Call(_)
    ));
    assert!(matches!(
        worker.blocks()[1].statements()[0].kind(),
        SemanticStatementKindV1::Assign(_)
    ));
    assert!(matches!(
        run_owner(owner, |_, _| panic!("different origins were collapsed")),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference CFG merge changes loan identity",
            ..
        }) | Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference control-flow choice requires checked addressable holder state and writeback",
            ..
        })
    ));
}

fn read_reference() -> SemanticStatementV1 {
    assign(
        place(6, WORD),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
            2,
            &[(SemanticProjectionKindV1::Dereference, WORD)],
        ))),
    )
}

fn borrow_reference(local: u32) -> SemanticStatementV1 {
    assign(
        place(2, REFERENCE),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(local, WORD),
        },
    )
}

fn loop_owner(case: LoopCase) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |_, functions| {
        let mut entry = vec![];
        if !matches!(
            case,
            LoopCase::Ordinary
                | LoopCase::LocalStorage
                | LoopCase::ChangingInitializer
                | LoopCase::SurvivingLoan
                | LoopCase::Uninitialized
        ) {
            entry.push(borrow_reference(1));
        }
        if matches!(case, LoopCase::ChangingHolder) {
            entry.push(assign(place(5, WORD), fixture_word(29)));
        }
        let mut body = match case {
            LoopCase::Ordinary => vec![assign(place(6, WORD), fixture_word(7))],
            LoopCase::Invariant | LoopCase::MultipleLatch | LoopCase::Irreducible => {
                vec![read_reference()]
            }
            LoopCase::ChangingHolder => vec![borrow_reference(5), read_reference()],
            // Retain the address-taken local so this case reaches reference CFG
            // analysis instead of failing earlier at an undefined scalar SSA edge.
            LoopCase::Uninitialized => vec![
                assign(place(5, WORD), fixture_word(19)),
                borrow_reference(5),
                read_reference(),
                dead(2),
            ],
            LoopCase::LocalStorage | LoopCase::ChangingInitializer | LoopCase::SurvivingLoan => {
                vec![
                    statement(SemanticStatementKindV1::StorageLive(
                        SemanticLocalIdV1::from_index(5),
                    )),
                    assign(
                        place(5, WORD),
                        if matches!(case, LoopCase::ChangingInitializer) {
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD)))
                        } else {
                            fixture_word(13)
                        },
                    ),
                    borrow_reference(5),
                    read_reference(),
                    dead(2),
                    dead(5),
                ]
            }
        };
        if matches!(case, LoopCase::ChangingInitializer) {
            body.push(assign(place(1, WORD), fixture_word(31)));
        }
        if matches!(case, LoopCase::SurvivingLoan) {
            body.truncate(4);
        }
        let mut exit = vec![];
        if matches!(case, LoopCase::Uninitialized) {
            exit.push(assign(
                place(6, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(5, WORD))),
            ));
        }
        if !matches!(
            case,
            LoopCase::Ordinary
                | LoopCase::Uninitialized
                | LoopCase::LocalStorage
                | LoopCase::ChangingInitializer
                | LoopCase::SurvivingLoan
        ) {
            exit.push(dead(2));
        }
        exit.push(unit());
        let blocks = if matches!(case, LoopCase::Irreducible) {
            vec![
                block(20, entry, choose(1, 2)),
                block(21, body.clone(), choose(2, 3)),
                block(22, body, choose(1, 3)),
                block(23, exit, SemanticTerminatorKindV1::Return),
            ]
        } else if matches!(case, LoopCase::MultipleLatch) {
            vec![
                block(20, entry, go(1)),
                block(21, vec![], choose(2, 4)),
                block(22, body, choose(1, 3)),
                block(23, vec![read_reference()], go(1)),
                block(24, exit, SemanticTerminatorKindV1::Return),
            ]
        } else {
            vec![
                block(20, entry, go(1)),
                block(21, vec![], choose(2, 3)),
                block(22, std::mem::take(&mut body), go(1)),
                block(23, exit, SemanticTerminatorKindV1::Return),
            ]
        };
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(25, WORD, SemanticLocalRoleV1::Temporary),
                local(26, WORD, SemanticLocalRoleV1::Temporary),
            ],
            blocks,
        );
    })
}

#[test]
fn source_reference_cfg_feature_free_loop_uses_finite_initialized_state() {
    run_owner(loop_owner(LoopCase::Ordinary), |plan, _| {
        assert!(plan.loans.is_empty());
        assert_eq!(plan.instances.instances().len(), 3);
        assert_eq!(plan.blocks.len(), 11);
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_cfg_absence_census_covers_expanded_callees_with_exact_work_cut() {
    for (case, expected) in [(LoopCase::Ordinary, false), (LoopCase::Invariant, true)] {
        let owner = loop_owner(case);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, _| {
                let required: usize = instances
                    .instances()
                    .iter()
                    .map(|row| {
                        1 + row.declaration().blocks().len()
                            + row
                                .declaration()
                                .blocks()
                                .iter()
                                .enumerate()
                                .filter(|(block, _)| {
                                    row.ssa()
                                        .plan()
                                        .is_reachable(SsaBlockIdV1::new(*block as u32))
                                })
                                .map(|(_, block)| 1 + block.statements().len())
                                .sum::<usize>()
                    })
                    .sum();
                for (limit, accepted) in [(required, true), (required - 1, false)] {
                    let mut census_work = CanonicalKernelIrWorkBudgetV1::new(limit);
                    let mut census_budget = ArgumentBudgetV1::new(&mut census_work, 0);
                    let result =
                        source_reference_borrows_present_v29(instances, &mut census_budget);
                    if accepted {
                        assert_eq!(result.unwrap(), expected);
                    } else {
                        assert!(result.is_err());
                    }
                    assert_eq!(census_budget.storage(), 0);
                }
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
    }
}

#[test]
fn source_reference_cfg_invariant_loop_records_each_static_read_once() {
    for case in [
        LoopCase::Invariant,
        LoopCase::MultipleLatch,
        LoopCase::Irreducible,
    ] {
        run_owner(loop_owner(case), |plan, budget| {
            assert_eq!(plan.loans.len(), 2);
            for (index, loan) in plan.loans.iter().enumerate() {
                assert_eq!(loan.site.block.index(), 0);
                let origin = &plan.origins[loan.origin];
                assert_eq!(origin.local.index(), 1);
                assert_eq!(origin.generation, 0);
                assert_eq!(
                    loan.effects.referent_reads,
                    if matches!(case, LoopCase::Invariant) {
                        1
                    } else {
                        2
                    }
                );
                assert_eq!(loan.effects.referent_writes, 0);
                assert_eq!(
                    plan.require_promoted(index, budget)?,
                    SourceReferenceRepresentationV29::StableReferent
                );
            }
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn source_reference_cfg_loop_local_storage_reuses_epoch_only_after_loan_death() {
    run_owner(loop_owner(LoopCase::LocalStorage), |plan, budget| {
        assert_eq!(plan.loans.len(), 2);
        for (index, loan) in plan.loans.iter().enumerate() {
            let origin = &plan.origins[loan.origin];
            assert_eq!(origin.local.index(), 5);
            assert_ne!(origin.generation, 0);
            assert_ne!(origin.generation, u32::MAX);
            assert_eq!(loan.effects.referent_reads, 1);
            assert_eq!(loan.effects.referent_writes, 0);
            assert_eq!(
                plan.require_promoted(index, budget)?,
                SourceReferenceRepresentationV29::StableReferent
            );
            let header = plan
                .blocks
                .iter()
                .find(|block| block.instance == origin.instance && block.block.index() == 1)
                .unwrap();
            assert!(plan.states[header.entry][2].node.is_none());
            assert!(plan.states[header.entry][5].node.is_none());
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_cfg_repeated_initializer_does_not_keep_first_iteration_anchor() {
    run_owner(loop_owner(LoopCase::ChangingInitializer), |plan, budget| {
        assert_eq!(plan.loans.len(), 2);
        for (index, loan) in plan.loans.iter().enumerate() {
            assert_eq!(loan.site.block.index(), 2);
            assert_eq!(loan.site.statement, Some(2));
            let origin = &plan.origins[loan.origin];
            assert_eq!(origin.local.index(), 5);
            assert_eq!(origin.anchor, None);
            assert!(matches!(
                plan.nodes[origin.value].kind,
                SourceReferenceNodeKindV29::Plain(None)
            ));
            assert_eq!(loan.effects.referent_reads, 1);
            assert_eq!(loan.effects.referent_writes, 0);
            assert_eq!(
                plan.require_promoted(index, budget)?,
                SourceReferenceRepresentationV29::StableReferent
            );
            let header = plan
                .blocks
                .iter()
                .find(|block| block.instance == origin.instance && block.block.index() == 1)
                .unwrap();
            let counter = plan.states[header.entry][1].node.unwrap();
            assert!(matches!(
                plan.nodes[counter].kind,
                SourceReferenceNodeKindV29::Plain(None)
            ));
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_cfg_live_loan_cannot_alias_a_later_same_site_storage_epoch() {
    assert!(matches!(
        run_owner(loop_owner(LoopCase::SurvivingLoan), |_, _| panic!(
            "live loan crossed StorageLive"
        )),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference CFG storage generations differ",
            ..
        }) | Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference CFG holder liveness differs",
            ..
        }) | Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference referent storage dies with a live loan",
            ..
        })
    ));
}

#[test]
fn source_reference_cfg_zero_trip_does_not_invent_backedge_initialization() {
    assert!(matches!(
        run_owner(loop_owner(LoopCase::Uninitialized), |_, _| panic!(
            "zero-trip read is undefined"
        )),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference reads a dead or undefined holder",
            ..
        })
    ));
}

#[test]
fn source_reference_cfg_changing_holder_is_not_an_arbitrary_predecessor_snapshot() {
    assert!(matches!(
        run_owner(loop_owner(LoopCase::ChangingHolder), |_, _| panic!(
            "runtime holder requires a cell"
        )),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference CFG merge changes loan identity",
            ..
        }) | Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference control-flow choice requires checked addressable holder state and writeback",
            ..
        })
    ));
}

#[test]
fn source_reference_cfg_worklist_and_index_exhaustion_never_publish_partial_owner() {
    let owner = loop_owner(LoopCase::Invariant);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            let worker = instances.id_at(1).unwrap();
            let index = builder.cfg_index(worker, budget).unwrap();
            assert_eq!(index.successor_ranges.len(), 4);
            assert_eq!(index.successors.iter().map(|edge| edge.target).collect::<Vec<_>>(), [1, 2, 3, 1]);
            assert_eq!(
                index.predecessors[index.predecessor_ranges[1].clone()],
                [0, 2]
            );
            drop(index);
            drop(builder);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
    // Independent small bounds, not a measured successful envelope.
    for limit in [0, 1, 8, 32] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4096);
        let mut published = false;
        let result = production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                    with_source_reference_plan_v29(instances, budget, |_, _| {
                        published = true;
                        Ok(())
                    }),
                )
            },
        );
        assert!(!published);
        assert!(result.is_err() || result.unwrap().is_err());
    }
}

#[test]
fn source_reference_cfg_constructor_headers_have_independent_exact_storage_cuts() {
    use std::mem::size_of;
    let index = 2 * size_of::<Result<SourceReferenceCfgIndexV29, ProductionSemanticKirErrorV1>>();
    let state = 2 * size_of::<Result<SourceReferenceCfgStateV29, ProductionSemanticKirErrorV1>>();
    let call = size_of::<SourceReferenceCallSummaryV29>()
        + size_of::<Option<SourceReferenceCallSummaryV29>>()
        + 2 * size_of::<Option<Vec<usize>>>();
    let run = 2 * size_of::<Option<SourceReferenceCfgStateV29>>();
    assert_eq!(
        source_reference_cfg_return_headers_v29::<SourceReferenceCfgIndexV29>().unwrap(),
        index
    );
    assert_eq!(
        source_reference_cfg_return_headers_v29::<SourceReferenceCfgStateV29>().unwrap(),
        state
    );
    assert_eq!(source_reference_cfg_call_headers_v29().unwrap(), call);
    assert_eq!(source_reference_cfg_run_headers_v29().unwrap(), run);
    for required in [index, state, call, run] {
        for slack in [0, 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = ArgumentBudgetV1::new(&mut work, 17 + required - slack);
            budget.reserve_storage(17).unwrap();
            let result = budget.reserve_storage(required);
            assert_eq!(result.is_ok(), slack == 0);
            assert_eq!(
                budget.storage(),
                if slack == 0 { 17 + required } else { 17 }
            );
            budget.release_storage(budget.storage() - 17).unwrap();
            assert_eq!(budget.storage(), 17);
        }
    }
}

#[test]
fn source_reference_cfg_refusal_probes_reach_real_index_and_worklist_boundaries() {
    let owner = loop_owner(LoopCase::Invariant);
    let mut outer_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut outer_budget = ArgumentBudgetV1::new(&mut outer_work, usize::MAX);
    let mut entered = 0;
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut outer_budget,
        |instances, _| {
            for work_cut in [true, false] {
                const LIMIT: usize = 1_000_000;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
                let mut builder = SourceReferenceBuilderV29::new(instances, &mut budget).unwrap();
                let worker = instances.id_at(1).unwrap();
                let declaration = instances.instance(worker).unwrap().declaration();
                assert_eq!(declaration.blocks().len(), 4);
                assert_eq!(declaration.locals().len(), 7);
                assert_eq!(instances.instances().len(), 3);
                let argument = builder
                    .node(
                        WORD,
                        SourceReferenceNodeKindV29::Plain(Some(SourceReferenceAnchorV29 {
                            argument: 0,
                            ty: WORD,
                        })),
                        &mut budget,
                    )
                    .unwrap();
                let mut state = source_reference_scratch_v29(7, &mut budget).unwrap();
                state.resize(7, SourceReferenceLocalV29::default());
                state[1].node = Some(argument);
                builder.plan.states = emission_vec_v1(4, &mut budget).unwrap();
                builder.plan.states.push(state);
                entered += 1;
                if work_cut {
                    // Four edges fit the first four-slot growth. States have four
                    // prepaid slots, so the two prelude clones cannot grow them.
                    let b = declaration.blocks().len();
                    let e = 4;
                    let locals = declaration.locals().len();
                    let frames = instances.instances().len();
                    let index = 8 * 3 + 6 * b + 9 * e + 3;
                    let capture = 3 + frames;
                    let clone = 3 + 1 + 3 + locals + 2;
                    let prelude = index + 3 * 3 + 3 * b + capture + clone;
                    let first_statement = 5 + clone + frames + 1 + 1;
                    let allowance = prelude + first_statement;
                    budget
                        .charge_work(LIMIT - budget.work() - allowance)
                        .unwrap();
                    let result = builder.run_cfg(worker, 0, &mut budget);
                    let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error),
                    )) = result
                    else {
                        panic!("not worklist work refusal")
                    };
                    assert_eq!(error.actual(), LIMIT + 4);
                    assert_eq!(error.limit(), LIMIT);
                    assert_eq!(
                        builder.effect_site,
                        Some(SourceReferenceSiteV29 {
                            instance: worker,
                            block: SemanticBlockIdV1::from_index(0),
                            statement: Some(0)
                        })
                    );
                    assert_eq!(builder.epochs[worker.index()].len(), b);
                    assert_eq!(budget.work(), LIMIT);
                } else {
                    budget.reserve_storage(LIMIT - budget.storage()).unwrap();
                    let result = builder.run_cfg(worker, 0, &mut budget);
                    let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(error),
                    )) = result
                    else {
                        panic!("not CFG header storage refusal")
                    };
                    assert_eq!(
                        error.actual(),
                        LIMIT + 2 * std::mem::size_of::<Option<SourceReferenceCfgStateV29>>()
                    );
                    assert_eq!(error.limit(), LIMIT);
                    assert!(builder.effect_site.is_none());
                }
                assert!(builder.plan.blocks.is_empty());
                assert!(builder.summaries.iter().all(Option::is_none));
                drop(builder);
                budget.release_storage(budget.storage()).unwrap();
                assert_eq!(budget.storage(), 0);
            }
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
    assert_eq!(entered, 2);
}

#[test]
fn source_reference_cfg_invariant_static_call_reuses_checked_summary_after_real_backedge() {
    let owner = owner_with(Case::Shared, |_, functions| {
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(25, WORD, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    20,
                    vec![
                        unit(),
                        assign(
                            place(4, REFERENCE),
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                place: place(1, WORD),
                            },
                        ),
                        assign(
                            place(5, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
                        ),
                    ],
                    go(1),
                ),
                block(21, vec![], choose(2, 3)),
                block(
                    22,
                    vec![
                        assign(place(5, WORD), fixture_word(17)),
                        assign(
                            place(3, CAPTURE),
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::Tuple,
                                    vec![SemanticOperandV1::Copy(place(4, REFERENCE))],
                                )
                                .unwrap(),
                            ),
                        ),
                    ],
                    call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 4),
                ),
                block(23, vec![dead(4), unit()], SemanticTerminatorKindV1::Return),
                block(24, vec![], go(1)),
            ],
        );
    });
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            builder.function(instances.root(), None, budget).unwrap();
            builder.finish_effects(budget).unwrap();
            assert_eq!(instances.instances().len(), 5);
            assert_eq!(builder.plan.loans.len(), 2);
            let mut reused = 0;
            for (index, row) in instances.instances().iter().enumerate() {
                if row.function() == SemanticFunctionIdV1::from_index(2) {
                    assert!(builder.summaries[index].as_ref().unwrap().reuse_count >= 1);
                    reused += 1;
                }
            }
            assert_eq!(reused, 2);
            for loan in &builder.plan.loans {
                assert_eq!(loan.effects.referent_reads, 1);
                assert_eq!(loan.effects.referent_writes, 0);
                assert_eq!(
                    loan.representation,
                    SourceReferenceRepresentationV29::StableReferent
                );
            }
            drop(builder);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}
