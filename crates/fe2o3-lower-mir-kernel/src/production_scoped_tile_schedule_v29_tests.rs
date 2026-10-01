use super::*;
use crate::ProductionScopeEventCandidateV29;

#[path = "production_scoped_tile_materialization_v29_tests.rs"]
mod materialization_v29_tests;

#[path = "production_scoped_tile_observation_v29_tests.rs"]
mod observation_v29_tests;

const SCHEDULE_LIMIT: usize = 100_000_000;
const SCHEDULE_FLOOR: usize = 37;

#[derive(Clone, Copy, Debug)]
enum SourceCase {
    Repeated,
    Slots,
    Shifted,
    Discard,
    Empty,
    Multidimensional,
    BranchParts,
}

fn branch_parts_blocks(helper: &SemanticFunctionDeclV1) -> Vec<SemanticBasicBlockV1> {
    let SemanticTerminatorKindV1::Call(parts) = helper.blocks()[2].terminator().kind() else {
        panic!("source Parts call");
    };
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = helper.blocks()[3].terminator().kind()
    else {
        panic!("source mask switch");
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let parts_call = || {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                parts.callee(),
                parts.arguments().to_vec(),
                Some(SemanticCallDestinationV1::new(
                    parts.destination().unwrap().place().clone(),
                    edge(SemanticEdgeRoleV1::CallReturn, 5),
                )),
                parts.unwind(),
            )
            .unwrap(),
        )
    };
    let split = SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(1, U32)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, 3),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
        )
        .unwrap(),
    };
    let active = SemanticTerminatorKindV1::SwitchInt {
        discriminant: discriminant.clone(),
        targets: SemanticSwitchTargetsV1::new(
            targets
                .values()
                .iter()
                .map(|target| {
                    SemanticSwitchTargetV1::new(
                        target.value(),
                        edge(target.edge().role(), target.edge().target().index() + 2),
                    )
                })
                .collect(),
            edge(
                targets.otherwise().role(),
                targets.otherwise().target().index() + 2,
            ),
        )
        .unwrap(),
    };
    vec![
        helper.blocks()[0].clone(),
        helper.blocks()[1].clone(),
        block(152, vec![], split),
        block(153, helper.blocks()[2].statements().to_vec(), parts_call()),
        block(154, helper.blocks()[2].statements().to_vec(), parts_call()),
        block(155, helper.blocks()[3].statements().to_vec(), active),
        block(
            156,
            helper.blocks()[4].statements().to_vec(),
            helper.blocks()[4].terminator().kind().clone(),
        ),
        block(
            157,
            helper.blocks()[5].statements().to_vec(),
            helper.blocks()[5].terminator().kind().clone(),
        ),
    ]
}

fn source_owner(case: SourceCase) -> ProductionSemanticSsaOwnerV1 {
    use scoped_root_tests::fixtures;
    match case {
        SourceCase::Repeated => return fixtures::tile_parts_repeated_owner(),
        SourceCase::Slots => return fixtures::tile_parts_repeated_slot_owner(),
        SourceCase::Shifted => return fixtures::tile_parts_entry_slot_owner(),
        _ => {}
    }
    let template = fixtures::tile_parts_repeated_owner();
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    match case {
        SourceCase::Discard | SourceCase::Empty | SourceCase::BranchParts => {
            callables.truncate(if matches!(case, SourceCase::Discard) {
                7
            } else if matches!(case, SourceCase::BranchParts) {
                9
            } else {
                6
            });
            let helper = &functions[3];
            let return_block = helper.blocks()[5].clone();
            assert!(matches!(
                return_block.terminator().kind(),
                SemanticTerminatorKindV1::Return
            ));
            let blocks = if matches!(case, SourceCase::BranchParts) {
                branch_parts_blocks(helper)
            } else if matches!(case, SourceCase::Discard) {
                vec![helper.blocks()[0].clone(), return_block]
            } else {
                vec![return_block]
            };
            functions[3] = SemanticFunctionDeclV1::new(
                helper.identity(),
                helper.role(),
                helper.item_definition_identity(),
                helper.monomorphization_identity(),
                helper.generic_type_arguments_identity(),
                helper.const_generic_arguments_identity(),
                helper.source(),
                helper.abi().clone(),
                helper.locals().to_vec(),
                helper.entry(),
                blocks,
            )
            .unwrap();
        }
        SourceCase::Multidimensional => {
            let root = &functions[0];
            let entry = root.kernel_entry().unwrap();
            let contract = entry.source_contract();
            let dimensions = SemanticWorkgroupDimensionsV1::new([32, 2, 1]).unwrap();
            let launch = SemanticKernelLaunchBoundsV1::new(
                Some(dimensions),
                Some(dimensions),
                contract.launch().unwrap().min_workgroups_per_compute_unit(),
            )
            .unwrap();
            let contract = SemanticKernelSourceContractV1::new_with_resources(
                Some(launch),
                contract.resources(),
                contract.unsafe_assembly(),
                contract.reachable_assembly(),
            )
            .unwrap();
            functions[0] = root.clone().with_kernel_entry(SemanticKernelEntryV1::new(
                entry.export_symbol().clone(),
                entry.kernel_binding_identity(),
                contract,
            ));
        }
        _ => unreachable!(),
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn pending_source(
    case: SourceCase,
    preexisting: bool,
    launch_override: Option<ProductionSourceLaunchInputV1>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> (ProductionPendingScopedSourceOwnerV29, usize) {
    let floor = budget.storage();
    let projected = source_owner(case);
    let mut owner = source_owner(case);
    let capture = if preexisting {
        let receipt = owner
            .try_capture_occurrences_with_budget_v1(budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        receipt.retained_storage()
    } else {
        0
    };
    let input = launch_override.unwrap_or_else(|| {
        let multidimensional = matches!(case, SourceCase::Multidimensional);
        ProductionSourceLaunchInputV1::new(
            if multidimensional { 2 } else { 1 },
            Some(if multidimensional {
                [32, 2, 1]
            } else {
                [64, 1, 1]
            }),
            [2, 1, 1],
        )
    });
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            input,
        )],
    )
    .unwrap();
    let semantic = projected.source_semantic();
    let roots = [root_input(&projected)];
    let workgroup = semantic.functions()[2].abi().source_input_types()[0];
    let classes = [
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Provider {
            function: HELPER,
            identity: semantic.functions()[1].identity(),
        },
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Derive {
            binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
            operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
            context: CONTEXT,
            workgroup,
        },
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
    ];
    assert!((6..=classes.len()).contains(&semantic.callables().len()));
    let SemanticTerminatorKindV1::Call(derive) =
        semantic.functions()[1].blocks()[0].terminator().kind()
    else {
        panic!("source derive call");
    };
    let events = [
        (
            ROOT,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(1),
                kind: ProductionScopeCallKindV29::Provider,
            },
        ),
        (
            HELPER,
            0,
            semantic.functions()[1].blocks()[0].statements().len(),
            ProductionScopeEventKindV29::Call {
                callee: derive.callee(),
                kind: ProductionScopeCallKindV29::Derive,
            },
        ),
        (
            HELPER,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(2),
                kind: ProductionScopeCallKindV29::Ordinary,
            },
        ),
        (HELPER, 2, 0, ProductionScopeEventKindV29::Return),
    ]
    .map(
        |(function, block, statement_count, kind)| ProductionScopeEventCandidateV29 {
            function,
            block: SemanticBlockIdV1::from_index(block),
            statement_count,
            kind,
        },
    );
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: projected.source_semantic_sha256(),
        roots: &roots,
        classes: &classes[..semantic.callables().len()],
        events: &events,
    };
    let pending = ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
        owner,
        launch,
        input,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap();
    assert_eq!(
        budget.storage(),
        floor + capture + pending.adopted_storage()
    );
    (pending, capture)
}

fn prepared_source(
    case: SourceCase,
    order: ScopedTileOrderV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> PreparedScopedTileSourceV29 {
    let (pending, capture) = pending_source(case, false, None, budget);
    assert_eq!(capture, 0);
    let mut donor = Some((pending, ScopedTileScheduleInputV29 { order }));
    let prepared = prepare_scoped_tile_source_v29(&mut donor, budget).unwrap();
    assert!(donor.is_none());
    prepared
}

fn drop_prepared(prepared: PreparedScopedTileSourceV29, budget: &mut ArgumentBudgetV1<'_>) {
    let retained = prepared.adopted_storage();
    drop(prepared);
    budget.release_storage(retained).unwrap();
}

fn assert_schedule_mismatch(
    prepared: &PreparedScopedTileSourceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    let floor = budget.storage();
    assert_eq!(
        prepared.replay_with_budget(budget),
        Err(ScopedTileFailureSummaryV29 {
            phase: ScopedTileFailurePhaseV29::Replay,
            kind: ScopedTileFailureKindV29::ReplayMismatch,
        })
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn explicit_schedule_retains_genuine_source_occurrences_and_accounting() {
    for case in [
        SourceCase::Repeated,
        SourceCase::Slots,
        SourceCase::Shifted,
        SourceCase::Discard,
        SourceCase::BranchParts,
    ] {
        let mut identities = Vec::new();
        for order in [ScopedTileOrderV29::Blocked, ScopedTileOrderV29::Striped] {
            for preexisting in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
                budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
                let (pending, capture) = pending_source(case, preexisting, None, &mut budget);
                let original = pending.adopted_storage();
                let graph = *pending.pending_identity();
                let source = *pending.source_semantic_sha256();
                let mut donor = Some((pending, ScopedTileScheduleInputV29 { order }));
                let prepared = prepare_scoped_tile_source_v29(&mut donor, &mut budget).unwrap();
                assert!(donor.is_none());
                assert_eq!(prepared.request.order, order);
                assert_eq!(prepared.pending.pending_identity(), &graph);
                assert_eq!(prepared.pending.source_semantic_sha256(), &source);
                assert_eq!(prepared.selections.len(), 2);
                let keys: Vec<_> = prepared
                    .selections
                    .iter()
                    .map(|row| {
                        assert_eq!(row.root, 0);
                        assert_eq!(row.semantic_root, ROOT);
                        assert_eq!((row.tile.lanes, row.tile.elements), (64, 2));
                        assert_eq!(row.order, order);
                        assert_eq!(row.witness.after.count, 1);
                        (row.witness.instance, row.witness.event)
                    })
                    .collect();
                assert_ne!(keys[0], keys[1]);
                if matches!(case, SourceCase::BranchParts) {
                    let mut groups = Vec::new();
                    for sidecar in &prepared.pending.inner.pending.roots[0].sidecars.rows {
                        let parts: Vec<_> = sidecar
                            .lifecycle_events
                            .as_ref()
                            .unwrap()
                            .rows
                            .iter()
                            .filter_map(|event| match event.kind {
                                DeferredLifecycleKindV29::Tile(DeferredTileEventV29 {
                                    input: DeferredTileInputV29::Parts { fragment },
                                    ..
                                }) => Some((fragment, event.block)),
                                _ => None,
                            })
                            .collect();
                        if !parts.is_empty() {
                            assert_eq!(parts.len(), 2);
                            assert_eq!(parts[0].0, parts[1].0);
                            assert_ne!(parts[0].1, parts[1].1);
                            groups.push(parts[0].0);
                        }
                    }
                    assert_eq!(groups.len(), 2);
                    assert_ne!(groups[0], groups[1]);
                }
                assert_eq!(keys[0].0 == keys[1].0, matches!(case, SourceCase::Shifted));
                if matches!(case, SourceCase::Discard) {
                    let discarded: usize = prepared
                        .pending
                        .pending_module()
                        .functions
                        .iter()
                        .filter_map(|function| function.body.as_ref())
                        .flat_map(|body| &body.blocks)
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| match &operation.kind {
                            OperationKind::Execution(
                                fe2o3_kernel_ir::ExecutionOperationV15::ScopeEnd {
                                    discarded, ..
                                },
                            ) => Some(discarded.len()),
                            _ => None,
                        })
                        .sum();
                    assert_eq!(discarded, 2);
                }
                let candidate_header = size_of::<ScopedTileScalarCandidateV29>()
                    .checked_sub(size_of::<fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>())
                    .unwrap();
                let header = size_of::<PreparedScopedTileSourceV29>()
                    .max(size_of::<ScopedTileMaterializationFailureV29>())
                    .max(candidate_header)
                    .checked_sub(size_of::<ProductionPendingScopedSourceOwnerV29>())
                    .unwrap();
                assert_eq!(
                    prepared.adopted_storage(),
                    original
                        + header
                        + prepared.selections.capacity() * size_of::<ScopedTileSelectionV29>()
                );
                let floor = budget.storage();
                assert_eq!(floor, SCHEDULE_FLOOR + capture + prepared.adopted_storage());
                prepared.replay_with_budget(&mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
                identities.push(prepared.identity);
                drop_prepared(prepared, &mut budget);
                assert_eq!(budget.storage(), SCHEDULE_FLOOR + capture);
                budget.release_storage(capture).unwrap();
            }
        }
        assert_eq!(identities[0], identities[1]);
        assert_eq!(identities[2], identities[3]);
        assert_ne!(identities[0], identities[2]);
    }
}

#[test]
fn original_request_refuses_coherent_other_order_binding_substitution() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let mut blocked = prepared_source(
        SourceCase::Repeated,
        ScopedTileOrderV29::Blocked,
        &mut budget,
    );
    let mut striped = prepared_source(
        SourceCase::Repeated,
        ScopedTileOrderV29::Striped,
        &mut budget,
    );
    assert_eq!(
        blocked.pending.pending_identity(),
        striped.pending.pending_identity()
    );
    assert_ne!(blocked.identity, striped.identity);
    assert_eq!(blocked.selections.capacity(), striped.selections.capacity());
    let floor = budget.storage();
    std::mem::swap(&mut blocked.selections, &mut striped.selections);
    std::mem::swap(&mut blocked.identity, &mut striped.identity);
    for prepared in [&blocked, &striped] {
        assert_eq!(
            prepared.replay_with_budget(&mut budget),
            Err(ScopedTileFailureSummaryV29 {
                phase: ScopedTileFailurePhaseV29::Replay,
                kind: ScopedTileFailureKindV29::ReplayMismatch,
            })
        );
        assert_eq!(budget.storage(), floor);
    }
    std::mem::swap(&mut blocked.selections, &mut striped.selections);
    std::mem::swap(&mut blocked.identity, &mut striped.identity);
    blocked.replay_with_budget(&mut budget).unwrap();
    striped.replay_with_budget(&mut budget).unwrap();
    drop_prepared(blocked, &mut budget);
    drop_prepared(striped, &mut budget);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn every_derived_occurrence_field_and_order_is_replayed() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let mut prepared = prepared_source(
        SourceCase::Shifted,
        ScopedTileOrderV29::Blocked,
        &mut budget,
    );
    let saved = prepared.selections[0];
    let floor = budget.storage();
    for fault in 0..17 {
        let row = &mut prepared.selections[0];
        match fault {
            0 => row.root += 1,
            1 => row.semantic_root = HELPER,
            2 => row.insertion += 1,
            3 => {
                row.witness.instance = ProductionCallInstanceIdV1(row.witness.instance.index() + 1)
            }
            4 => row.witness.event += 1,
            5 => row.witness.source_span += 1,
            6 => row.witness.before.first += 1,
            7 => row.witness.after.first += 1,
            8 => row.tile.producer.block = SemanticBlockIdV1::from_index(99),
            9 => row.tile.result_type = U32,
            10 => row.tile.first_result.0 += 1,
            11 => row.tile.lanes /= 2,
            12 => row.tile.elements += 1,
            13 => row.order = ScopedTileOrderV29::Striped,
            14..=16 => {
                let DeferredTileInputV29::Load {
                    workgroup,
                    input,
                    base,
                } = &mut row.tile.input
                else {
                    unreachable!()
                };
                match fault {
                    14 => workgroup.value.0 += 1,
                    15 => input.0 += 1,
                    16 => base.0 += 1,
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            prepared.replay_with_budget(&mut budget).unwrap_err().kind,
            ScopedTileFailureKindV29::ReplayMismatch,
            "{fault}"
        );
        assert_eq!(budget.storage(), floor);
        prepared.selections[0] = saved;
    }
    prepared.selections.swap(0, 1);
    assert_schedule_mismatch(&prepared, &mut budget);
    prepared.selections.swap(0, 1);
    let other = prepared.selections[1];
    prepared.selections[1] = prepared.selections[0];
    assert_schedule_mismatch(&prepared, &mut budget);
    prepared.selections[1] = other;
    let removed = prepared.selections.pop().unwrap();
    assert_schedule_mismatch(&prepared, &mut budget);
    prepared.selections.push(removed);
    prepared.identity[0] ^= 1;
    assert_schedule_mismatch(&prepared, &mut budget);
    prepared.identity[0] ^= 1;
    prepared.replay_with_budget(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    drop_prepared(prepared, &mut budget);
}

#[test]
fn genuine_equal_product_geometry_and_empty_source_preserve_donor() {
    for (case, kind) in [
        (
            SourceCase::Multidimensional,
            ScopedTileFailureKindV29::Geometry,
        ),
        (
            SourceCase::Empty,
            ScopedTileFailureKindV29::NoTileOccurrences,
        ),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let (pending, capture) = pending_source(case, true, None, &mut budget);
        pending.replay_with_budget(&mut budget).unwrap();
        let identity = *pending.pending_identity();
        let retained = pending.adopted_storage();
        let mut donor = Some((
            pending,
            ScopedTileScheduleInputV29 {
                order: ScopedTileOrderV29::Blocked,
            },
        ));
        let address = donor.as_ref().unwrap().0.pending_module() as *const Module;
        let floor = budget.storage();
        assert_eq!(
            prepare_scoped_tile_source_v29(&mut donor, &mut budget)
                .err()
                .unwrap(),
            ScopedTileFailureSummaryV29 {
                phase: ScopedTileFailurePhaseV29::Preparation,
                kind
            }
        );
        assert_eq!(donor.as_ref().unwrap().0.pending_identity(), &identity);
        assert_eq!(
            donor.as_ref().unwrap().0.pending_module() as *const Module,
            address
        );
        assert_eq!(budget.storage(), floor);
        drop(donor);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), SCHEDULE_FLOOR + capture);
        budget.release_storage(capture).unwrap();
    }
}

#[test]
fn source_launch_rank_and_grid_changes_require_fresh_schedule_identity() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let mut original = prepared_source(
        SourceCase::Repeated,
        ScopedTileOrderV29::Blocked,
        &mut budget,
    );
    for launch in [
        ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
        ProductionSourceLaunchInputV1::new(2, Some([64, 1, 1]), [2, 2, 1]),
    ] {
        let (pending, _) = pending_source(SourceCase::Repeated, false, Some(launch), &mut budget);
        let mut donor = Some((
            pending,
            ScopedTileScheduleInputV29 {
                order: ScopedTileOrderV29::Blocked,
            },
        ));
        let mut changed = prepare_scoped_tile_source_v29(&mut donor, &mut budget).unwrap();
        changed.replay_with_budget(&mut budget).unwrap();
        assert_ne!(original.identity, changed.identity);
        std::mem::swap(&mut original.identity, &mut changed.identity);
        assert_schedule_mismatch(&original, &mut budget);
        assert_schedule_mismatch(&changed, &mut budget);
        std::mem::swap(&mut original.identity, &mut changed.identity);
        drop_prepared(changed, &mut budget);
    }
    drop_prepared(original, &mut budget);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn foreign_ledger_and_missing_live_storage_refuse_before_work() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let (pending, capture) = pending_source(SourceCase::Repeated, true, None, &mut budget);
    let mut donor = Some((
        pending,
        ScopedTileScheduleInputV29 {
            order: ScopedTileOrderV29::Blocked,
        },
    ));
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, SCHEDULE_LIMIT);
    foreign.reserve_storage(budget.storage()).unwrap();
    let foreign_floor = foreign.storage();
    let error = prepare_scoped_tile_source_v29(&mut donor, &mut foreign)
        .err()
        .unwrap();
    assert_eq!(
        error.kind,
        ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Accounting)
    );
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), foreign_floor);
    assert!(donor.is_some());
    budget.release_storage(1).unwrap();
    let before = budget.work();
    assert_eq!(
        prepare_scoped_tile_source_v29(&mut donor, &mut budget)
            .err()
            .unwrap()
            .kind,
        ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Accounting)
    );
    assert_eq!(budget.work(), before);
    budget.reserve_storage(1).unwrap();
    let prepared = prepare_scoped_tile_source_v29(&mut donor, &mut budget).unwrap();
    assert_eq!(
        prepared.replay_with_budget(&mut foreign).unwrap_err().kind,
        ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Accounting)
    );
    assert_eq!(foreign.work(), 0);
    drop_prepared(prepared, &mut budget);
    assert_eq!(budget.storage(), capture);
    budget.release_storage(capture).unwrap();
}

#[derive(Debug)]
struct ScheduleProbe {
    result: Result<(), ScopedTileFailureSummaryV29>,
    work: usize,
    extra: usize,
    denied_work: Option<usize>,
    denied_storage: Option<usize>,
}

fn schedule_probe(
    preexisting: bool,
    replay: bool,
    allowance: Option<(usize, usize)>,
) -> ScheduleProbe {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let (result, used, extra, denied_storage) = {
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let (pending, _) = pending_source(SourceCase::Shifted, preexisting, None, &mut budget);
        let mut donor = Some((
            pending,
            ScopedTileScheduleInputV29 {
                order: ScopedTileOrderV29::Blocked,
            },
        ));
        let prepared =
            replay.then(|| prepare_scoped_tile_source_v29(&mut donor, &mut budget).unwrap());
        let original = prepared.as_ref().map_or_else(
            || donor.as_ref().unwrap().0.adopted_storage(),
            |owner| owner.adopted_storage(),
        );
        budget
            .reserve_storage(budget.peak_storage() + 1 - budget.storage())
            .unwrap();
        if let Some((work_left, storage_left)) = allowance {
            budget
                .charge_work(SCHEDULE_LIMIT - budget.work() - work_left)
                .unwrap();
            budget
                .reserve_storage(SCHEDULE_LIMIT - budget.storage() - storage_left)
                .unwrap();
        }
        let entry = budget.storage();
        let before = budget.work();
        let result = if let Some(prepared) = prepared {
            let result = prepared.replay_with_budget(&mut budget);
            assert_eq!(budget.storage(), entry);
            drop_prepared(prepared, &mut budget);
            result
        } else {
            let graph = *donor.as_ref().unwrap().0.pending_identity();
            let address = donor.as_ref().unwrap().0.pending_module() as *const Module;
            match prepare_scoped_tile_source_v29(&mut donor, &mut budget) {
                Ok(prepared) => {
                    assert!(donor.is_none());
                    assert_eq!(prepared.pending.pending_identity(), &graph);
                    assert_eq!(
                        budget.storage(),
                        entry + prepared.adopted_storage() - original
                    );
                    drop_prepared(prepared, &mut budget);
                    Ok(())
                }
                Err(error) => {
                    assert_eq!(budget.storage(), entry);
                    assert_eq!(donor.as_ref().unwrap().0.pending_identity(), &graph);
                    assert_eq!(
                        donor.as_ref().unwrap().0.pending_module() as *const Module,
                        address
                    );
                    drop(donor);
                    budget.release_storage(original).unwrap();
                    Err(error)
                }
            }
        };
        assert_eq!(budget.storage(), entry - original);
        let used = budget.work() - before;
        let extra = budget.peak_storage() - entry;
        let denied_storage = budget.failed_storage();
        let remaining = budget.storage();
        budget.release_storage(remaining).unwrap();
        (result, used, extra, denied_storage)
    };
    ScheduleProbe {
        result,
        work: used,
        extra,
        denied_work: work.failed_work(),
        denied_storage,
    }
}

#[test]
fn preparation_and_replay_have_exact_same_ledger_resource_boundaries() {
    for preexisting in [false, true] {
        for replay in [false, true] {
            let baseline = schedule_probe(preexisting, replay, None);
            baseline.result.unwrap();
            assert!(baseline.work > 0 && baseline.extra > 0);
            let exact = schedule_probe(preexisting, replay, Some((baseline.work, baseline.extra)));
            exact.result.unwrap();
            assert_eq!(exact.work, baseline.work);
            assert_eq!(exact.extra, baseline.extra);
            let work = schedule_probe(
                preexisting,
                replay,
                Some((baseline.work - 1, baseline.extra)),
            );
            let error = work.result.unwrap_err();
            let ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Work(limit)) = error.kind
            else {
                panic!("typed work refusal: {work:?}")
            };
            assert_eq!(work.denied_work, Some(limit.actual()));
            let storage = schedule_probe(
                preexisting,
                replay,
                Some((baseline.work, baseline.extra - 1)),
            );
            let ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Storage(limit)) =
                storage.result.unwrap_err().kind
            else {
                panic!("typed storage refusal: {storage:?}")
            };
            assert_eq!(storage.denied_storage, Some(limit.actual()));
        }
    }
}

#[test]
fn fixed_diagnostics_drop_owned_payload_and_preserve_nested_quota_kind() {
    fn requires_copy<T: Copy>() {}
    requires_copy::<ScopedTileFailureSummaryV29>();
    assert!(!std::mem::needs_drop::<ScopedTileFailureSummaryV29>());
    assert_eq!(
        ScopedTileFailureKindV29::from(ProductionSemanticKirErrorV1::RetainedLocalStorage {
            function: 0,
            retained_locals: vec![(1, 2, "test")],
            retained_count: 1,
        }),
        ScopedTileFailureKindV29::Source
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let limit = work.charge_work(1).unwrap_err();
    use fe2o3_kernel_ir::{
        CanonicalKernelIrReplayAdmissionErrorV18 as C, KernelIrDecodeError as D,
        KernelIrEncodeError as E,
    };
    for error in [
        C::Encode(E::WorkLimit(limit)),
        C::Decode(D::WorkLimit(limit)),
        C::Decode(D::Encode(E::WorkLimit(limit))),
    ] {
        assert_eq!(
            ScopedTileFailureKindV29::from(ScopedModuleErrorV29::Canonical(error)),
            ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Work(limit))
        );
    }
}

#[test]
fn preparation_attempt_unwind_preserves_original_donor_and_floor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let (pending, capture) = pending_source(SourceCase::Repeated, true, None, &mut budget);
    let donor = Some((
        pending,
        ScopedTileScheduleInputV29 {
            order: ScopedTileOrderV29::Blocked,
        },
    ));
    let identity = *donor.as_ref().unwrap().0.pending_identity();
    let floor = budget.storage();
    let before = budget.work();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), ScopedTileFailureKindV29> =
            scoped_tile_attempt_v29(&mut budget, |budget| {
                donor
                    .as_ref()
                    .unwrap()
                    .0
                    .inner
                    .replay(budget)
                    .map_err(ScopedTileFailureKindV29::from)?;
                let _scratch = emission_vec_v1::<u64>(17, budget)?;
                std::panic::panic_any("tile schedule test unwind");
            });
    }))
    .unwrap_err();
    assert_eq!(
        panic.downcast_ref::<&'static str>(),
        Some(&"tile schedule test unwind")
    );
    assert_eq!(donor.as_ref().unwrap().0.pending_identity(), &identity);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > before);
    let retained = donor.as_ref().unwrap().0.adopted_storage();
    drop(donor);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), SCHEDULE_FLOOR + capture);
    budget.release_storage(capture).unwrap();
}

#[test]
fn real_canonical_comparison_quota_keeps_its_exact_resource_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let (pending, _) = pending_source(SourceCase::Repeated, false, None, &mut budget);
    let graph = &pending.inner.pending.graph;
    let floor = budget.storage();
    budget
        .charge_work(SCHEDULE_LIMIT - budget.work() - 1)
        .unwrap();
    let raw = graph
        .matches_module_with_budget_v18(graph.module(), &mut budget)
        .unwrap_err();
    assert!(matches!(
        &raw,
        fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Encode(
            fe2o3_kernel_ir::KernelIrEncodeError::WorkLimit(_)
        )
    ));
    let summary = ScopedTileFailureKindV29::from(ScopedModuleErrorV29::Canonical(raw));
    assert!(matches!(
        summary,
        ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Work(_))
    ));
    assert_eq!(budget.storage(), floor);
    let retained = pending.adopted_storage();
    drop(pending);
    budget.release_storage(retained).unwrap();
}

#[test]
fn missing_donor_and_changed_source_refuse_without_taking_custody() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    assert_eq!(
        prepare_scoped_tile_source_v29(&mut None, &mut budget)
            .err()
            .unwrap(),
        ScopedTileFailureSummaryV29 {
            phase: ScopedTileFailurePhaseV29::Preparation,
            kind: ScopedTileFailureKindV29::MissingDonor,
        }
    );
    assert_eq!(budget.work(), 0);
    let (mut pending, _) = pending_source(SourceCase::Repeated, false, None, &mut budget);
    pending.inner.source.input.semantic_sha256[0] ^= 1;
    let identity = *pending.pending_identity();
    let retained = pending.adopted_storage();
    let mut donor = Some((
        pending,
        ScopedTileScheduleInputV29 {
            order: ScopedTileOrderV29::Blocked,
        },
    ));
    let floor = budget.storage();
    assert_eq!(
        prepare_scoped_tile_source_v29(&mut donor, &mut budget)
            .err()
            .unwrap(),
        ScopedTileFailureSummaryV29 {
            phase: ScopedTileFailurePhaseV29::Preparation,
            kind: ScopedTileFailureKindV29::Source,
        }
    );
    assert_eq!(donor.as_ref().unwrap().0.pending_identity(), &identity);
    assert_eq!(donor.as_ref().unwrap().1.order, ScopedTileOrderV29::Blocked);
    assert_eq!(budget.storage(), floor);
    drop(donor);
    budget.release_storage(retained).unwrap();
}
