use super::super::super::super::invocations;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

const LIMIT: usize = 512 * 1024 * 1024;

#[test]
fn partial_prefix_headers_cover_retained_rows_and_replay_frames() {
    assert_eq!(
        headers(),
        2 * size_of::<Prefix<'_, '_, '_, '_, '_>>()
            + 2 * size_of::<Result<Prefix<'_, '_, '_, '_, '_>>>()
            + 2 * size_of::<Local>()
            + 2 * size_of::<ScalarDemand>()
            + size_of::<Vec<Local>>()
            + size_of::<Vec<Option<Value>>>()
            + 6 * size_of::<Vec<bool>>()
            + size_of::<Vec<u64>>()
            + 3 * size_of::<Event>()
            + 3 * size_of::<Option<Event>>()
            + 3 * size_of::<Option<Value>>()
            + 3 * size_of::<Range<usize>>()
            + size_of::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()
            + 2 * size_of::<
                std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>,
            >()
            + size_of::<
                std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaSuccessorOccurrenceV1>,
            >()
            + 2 * size_of::<std::slice::Iter<'_, fe2o3_mir_model::SsaArgumentV1>>()
            + size_of::<std::slice::Iter<'_, fe2o3_mir_model::SsaVariableIdV1>>()
            + size_of::<std::slice::Iter<'_, Demand>>()
            + size_of::<std::slice::IterMut<'_, Local>>()
            + size_of::<std::slice::Iter<'_, Local>>()
            + 5 * size_of::<Result<()>>()
            + 4 * size_of::<Result<usize>>()
            + 40 * size_of::<usize>()
            + 30 * size_of::<&()>()
    )
}

fn inspect(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let frames = FramePlan::derive(plan, slots, out)?;
    let source = plan.source(out)?;
    let semantic = source.source_semantic(out.budget)?;
    let archive = source.source_ssa(out.budget)?;
    let mut queries = 0;
    let mut terminator_demands = 0;
    for frame in &frames.frames {
        if !frame.active {
            continue;
        }
        let function = &semantic.functions()[frame.function.index() as usize];
        let rows = archive
            .occurrences_v1()
            .unwrap()
            .function(frame.function)
            .unwrap();
        for cut in &frames.cuts[frame.cuts.clone()] {
            if !cut.reachable {
                continue;
            }
            for k in 0..=function.blocks()[cut.block].statements().len() {
                let floor = out.budget.storage();
                let query =
                    frames.partial_prefix_v296(frame.root, frame.instance, cut.block, k, out)?;
                assert_eq!(
                    (query.block, query.statement, query.pc),
                    (cut.block, k, cut.pc)
                );
                if k == 0 {
                    for demand in &frames.demands[cut.demands.clone()] {
                        assert_eq!(
                            query.locals[demand.source.local].scalar,
                            ScalarDemand::Needed(demand.source.value)
                        );
                    }
                    let cache = frames.components[frame.function.index() as usize]
                        .as_ref()
                        .unwrap();
                    for row in &query.locals {
                        for leaf in 0..row.components.len() {
                            assert_eq!(
                                query.component_required(row.local, leaf, out)?,
                                cache.leaf_required(
                                    frame.function,
                                    cut.block,
                                    row.local,
                                    leaf,
                                    out
                                )?
                            );
                        }
                    }
                }
                if k == function.blocks()[cut.block].statements().len() {
                    // The first resolved terminator use is still a prefix demand.
                    if let Some(Event::Use { variable, value }) = rows
                        .events()
                        .iter()
                        .filter(|row| {
                            row.site()
                                == Site::Terminator {
                                    block: block(cut.block).unwrap(),
                                }
                        })
                        .find_map(|row| row.resolved())
                    {
                        assert_eq!(
                            query.locals[variable.get() as usize].scalar,
                            ScalarDemand::Needed(value)
                        );
                        terminator_demands += 1;
                    }
                }
                query.discard(out)?;
                assert_eq!(out.budget.storage(), floor);
                queries += 1;
            }
        }
    }
    assert!(queries > 0 && terminator_demands > 0);
    Ok(())
}

fn fixture(work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    invocations::tests::run_variant(work, storage, false, |plan, out| {
        super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
            inspect(plan, slots, out)
        })
    })
}

#[test]
fn partial_prefix_cuts_replay_actual_statements_and_keep_terminator_demands() {
    fixture(LIMIT, LIMIT).0.unwrap();
}

#[test]
fn partial_prefix_no_phi_successor_retains_dominating_live_through_locals() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |_, functions| {
            let old = functions.last_mut().unwrap();
            assert_eq!(old.blocks().len(), 1);
            let source = old.source();
            let blocks = vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([253; 32]),
                    source,
                    vec![SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Nop,
                    )],
                    SemanticTerminatorV1::new(
                        source,
                        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::Goto,
                            SemanticBlockIdV1::from_index(1),
                        )),
                    ),
                )
                .unwrap(),
                old.blocks()[0].clone(),
            ];
            *old = SemanticFunctionDeclV1::new(
                old.identity(),
                old.role(),
                old.item_definition_identity(),
                old.monomorphization_identity(),
                old.generic_type_arguments_identity(),
                old.const_generic_arguments_identity(),
                source,
                old.abi().clone(),
                old.locals().to_vec(),
                old.entry(),
                blocks,
            )
            .unwrap();
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let frames = FramePlan::derive(plan, slots, out)?;
                let source = plan.source(out)?;
                let archive = source.source_ssa(out.budget)?;
                let semantic = source.source_semantic(out.budget)?;
                let mut witnesses = 0;
                for frame in &frames.frames {
                    if !frame.active {
                        continue;
                    }
                    let function = &semantic.functions()[frame.function.index() as usize];
                    if function.blocks()[0].statements().len() != 1
                        || !matches!(
                            function.blocks()[0].statements()[0].kind(),
                            SemanticStatementKindV1::Nop
                        )
                    {
                        continue;
                    }
                    let ssa = archive.plan_for_function(frame.function).unwrap().plan();
                    let edge = fe2o3_mir_model::SsaEdgeIdV1::new(Block::new(0), 0);
                    assert!(ssa.edge_arguments(edge).unwrap().is_empty());
                    assert!(ssa.merge_variables(Block::new(1)).unwrap().is_empty());
                    for k in [0, 1] {
                        let query =
                            frames.partial_prefix_v296(frame.root, frame.instance, 0, k, out)?;
                        for local in [1, 2] {
                            assert!(matches!(
                                query.locals[local].scalar,
                                ScalarDemand::Needed(_)
                            ));
                        }
                        query.discard(out)?;
                    }
                    witnesses += 1;
                }
                assert!(witnesses > 0);
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}

#[test]
fn partial_prefix_unavailable_scalar_liveness_retains_component_demands() {
    invocations::tests::run_allocation_variant(LIMIT, LIMIT, |plan, out| {
        super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
            let frames = FramePlan::derive(plan, slots, out)?;
            let mut witnesses = 0;
            for root in 0..frames.roots.len() {
                // Genuine retained aggregate storage: constructor, field store,
                // then field load. This owner marks local 4 unpromoted.
                let query = frames.partial_prefix_v296(root, 0, 0, 1, out)?;
                let row = &query.locals[4];
                assert_eq!(row.scalar, ScalarDemand::Unavailable);
                assert_eq!(row.current, None);
                assert_eq!(row.domain, ComponentDomainV283::ScalarV42);
                assert!(query.component_required(4, 0, out)?);
                assert!(matches!(query.require_complete_local_coverage(out), Err(Error::Statement(
                    "partial prefix has unavailable all-local liveness; complete frame admission refused"))));
                // A refusal cannot mutate the retained unavailable state.
                assert_eq!(query.locals[4].scalar, ScalarDemand::Unavailable);
                query.discard(out)?;
                witnesses += 1;
            }
            assert_eq!(witnesses, 2);
            Ok(())
        })
    }).0.unwrap();
}

#[test]
fn partial_prefix_rejects_wrong_coordinates_and_foreign_owner() {
    invocations::tests::run_variant(LIMIT, LIMIT, false, |plan, out| {
        super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
            let frames = FramePlan::derive(plan, slots, out)?;
            for (root, instance, block, statement) in [
                (usize::MAX, 0, 0, 0),
                (0, usize::MAX, 0, 0),
                (0, 0, usize::MAX, 0),
                (0, 0, 0, usize::MAX),
            ] {
                assert!(matches!(
                    frames.partial_prefix_v296(root, instance, block, statement, out),
                    Err(Error::Statement(_))
                ));
            }
            let foreign = FramePlan::derive(plan, slots, out)?;
            let query = frames.partial_prefix_v296(0, 0, 0, 0, out)?;
            assert!(matches!(
                query.check(&foreign, out),
                Err(Error::Statement(_))
            ));
            query.discard(out)?;
            Ok(())
        })
    })
    .0
    .unwrap();
}

#[test]
fn partial_prefix_foreign_or_refunded_account_latches_before_observation() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let mut reached = false;
        let result = invocations::tests::run_variant(LIMIT, LIMIT, false, |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let frames = FramePlan::derive(plan, slots, out)?;
                let query = frames.partial_prefix_v296(0, 0, 0, 0, out)?;
                reached = true;
                let error = if foreign {
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, LIMIT);
                    budget.reserve_storage(query.required)?;
                    let mut other = Writer::new(&mut budget)?;
                    query.check(&frames, &mut other).unwrap_err()
                } else {
                    out.budget.release_storage(1)?;
                    query.check(&frames, out).unwrap_err()
                };
                assert!(matches!(
                    error,
                    Error::Resource(Resource::Accounting)
                        | Error::Source(SourceError::Resource(Resource::Accounting))
                ));
                assert!(query.check(&frames, out).is_err());
                Err(error)
            })
        });
        assert!(reached && result.0.is_err());
    }
}

#[test]
fn partial_prefix_replay_has_exact_and_one_short_resource_boundaries() {
    let baseline = fixture(LIMIT, LIMIT);
    baseline.0.unwrap();
    let exact = fixture(baseline.1, baseline.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (baseline.1, baseline.2, baseline.3)
    );
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    assert!(matches!(fixture(baseline.1 - 1, baseline.3).0,
        Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
    assert!(matches!(fixture(baseline.1, baseline.3 - 1).0,
        Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
}

#[test]
fn partial_prefix_failure_only_moves_keep_scalar_and_component_success_demands() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            super::super::super::paired::aggregate_tests::checked_transform(
                types,
                functions,
                SemanticCheckedBinaryOpV1::Add,
                true,
                true,
            );
            let old = functions.last_mut().unwrap();
            let source = old.source();
            let mut blocks = old.blocks().to_vec();
            let SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                target,
                unwind,
            } = blocks[1].terminator().kind()
            else {
                panic!("checked assert")
            };
            let SemanticAssertMessageV1::DivisionByZero(component) = message else {
                panic!("failure-only component move")
            };
            let message = SemanticAssertMessageV1::Overflow {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Move(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(1),
                        vec![],
                        SemanticTypeIdV1::from_index(0),
                    )
                    .unwrap(),
                ),
                right: component.clone(),
            };
            blocks[1] = SemanticBasicBlockV1::new(
                blocks[1].identity(),
                source,
                vec![],
                SemanticTerminatorV1::new(
                    source,
                    SemanticTerminatorKindV1::Assert {
                        condition: condition.clone(),
                        expected: *expected,
                        message,
                        target: *target,
                        unwind: *unwind,
                    },
                ),
            )
            .unwrap();
            *old = SemanticFunctionDeclV1::new(
                old.identity(),
                old.role(),
                old.item_definition_identity(),
                old.monomorphization_identity(),
                old.generic_type_arguments_identity(),
                old.const_generic_arguments_identity(),
                source,
                old.abi().clone(),
                old.locals().to_vec(),
                old.entry(),
                blocks,
            )
            .unwrap();
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let frames = FramePlan::derive(plan, slots, out)?;
                let source = plan.source(out)?.source_semantic(out.budget)?;
                let mut witnesses = 0;
                for frame in &frames.frames {
                    if !frame.active {
                        continue;
                    }
                    let function = &source.functions()[frame.function.index() as usize];
                    for (block, body) in function.blocks().iter().enumerate() {
                        if matches!(body.terminator().kind(), Terminator::Assert { .. }) {
                            let query = frames.partial_prefix_v296(
                                frame.root,
                                frame.instance,
                                block,
                                body.statements().len(),
                                out,
                            )?;
                            assert!(query.component_required(4, 0, out)?);
                            assert!(query.component_required(4, 1, out)?);
                            assert!(matches!(query.locals[1].scalar, ScalarDemand::Needed(_)));
                            query.discard(out)?;
                            witnesses += 1;
                        }
                    }
                }
                assert!(witnesses > 0);
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}
