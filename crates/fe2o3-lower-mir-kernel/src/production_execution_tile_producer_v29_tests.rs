mod discard_tests {
    use super::*;
    use fe2o3_kernel_ir::{ExecutionOperationV15 as Ex, ExecutionRoleV15 as Role};
    type Fixture = (FunctionBody, Vec<PreparedLifecycleEventV29>);
    const FLOOR: usize = 47;
    const ROOM: usize = 100_000_000;

    fn role(id: u32, role: Role, operation: Ex) -> Operation {
        Operation::new(
            vec![ValueDef::new(ValueId(id), Type::Execution(role))],
            OperationKind::Execution(operation),
        )
    }
    fn issue() -> Operation {
        role(0, Role::Context, Ex::ContextIssue)
    }
    fn derive(id: u32) -> Operation {
        role(
            id,
            Role::Workgroup,
            Ex::WorkgroupDerive {
                context: ValueId(0),
            },
        )
    }
    fn load(id: u32, workgroup: u32) -> Operation {
        role(
            id,
            Role::MaskedTileU32 {
                lanes: 64,
                elements: 2,
            },
            Ex::MaskedTileLoadU32 {
                workgroup: ValueId(workgroup),
                input: ValueId(1_000_000),
                base: ValueId(1_000_001),
                lanes: 64,
                elements: 2,
            },
        )
    }
    fn fragment(id: u32, tile: u32) -> Operation {
        role(
            id,
            Role::LaneFragmentU32 {
                lanes: 64,
                elements: 2,
            },
            Ex::TileIntoFragmentU32 {
                tile: ValueId(tile),
                lanes: 64,
                elements: 2,
            },
        )
    }
    fn parts(fragment: u32, first: u32) -> Operation {
        Operation::new(
            (0..4)
                .map(|i| {
                    ValueDef::new(
                        ValueId(first + i),
                        Type::Scalar(if i < 2 {
                            ScalarType::U32
                        } else {
                            ScalarType::Bool
                        }),
                    )
                })
                .collect(),
            OperationKind::Execution(Ex::FragmentIntoPartsU32 {
                fragment: ValueId(fragment),
                lanes: 64,
                elements: 2,
            }),
        )
    }
    fn end(workgroup: u32) -> Operation {
        Operation::new(
            vec![],
            OperationKind::Execution(Ex::ScopeEnd {
                workgroup: ValueId(workgroup),
                discarded: vec![],
            }),
        )
    }
    fn event(block: usize, instance: usize, operation: Operation) -> PreparedLifecycleEventV29 {
        let span = InstancePhysicalSpanV1 {
            block: BlockId(block as u32),
            first: 0,
            count: 0,
        };
        PreparedLifecycleEventV29 {
            block,
            witness: LifecycleInsertionV29 {
                instance: ProductionCallInstanceIdV1(instance),
                event: 0,
                source_span: block,
                before: span,
                after: InstancePhysicalSpanV1 { count: 1, ..span },
            },
            operation: Some(operation),
        }
    }
    fn branch(target: u32) -> Terminator {
        Terminator::Branch {
            target: BlockId(target),
            arguments: vec![],
        }
    }
    fn split() -> Terminator {
        Terminator::ConditionalBranch {
            condition: ValueId(1_000_002),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        }
    }
    fn ret() -> Terminator {
        Terminator::Return { values: vec![] }
    }
    fn body(terms: Vec<Terminator>) -> FunctionBody {
        FunctionBody {
            parameters: vec![],
            blocks: terms
                .into_iter()
                .enumerate()
                .map(|(i, term)| {
                    let mut block = BasicBlock::new(BlockId(i as u32));
                    block.terminator = Some(term);
                    block
                })
                .collect(),
        }
    }
    fn chain(stage: u8) -> Fixture {
        let mut events = vec![
            event(0, 0, issue()),
            event(0, 1, derive(1)),
            event(0, 2, load(2, 1)),
        ];
        if stage >= 1 {
            events.push(event(0, 2, fragment(3, 2)));
        }
        if stage >= 2 {
            events.push(event(0, 2, parts(3, 20)));
        }
        events.push(event(0, 1, end(1)));
        (body(vec![ret()]), events)
    }
    struct Observed {
        result: Result<(), ProductionSemanticKirErrorV1>,
        ends: Vec<Vec<ValueId>>,
        end_bytes: usize,
        work: usize,
        peak: usize,
    }
    fn run_discard(
        (body, mut events): Fixture,
        work_limit: usize,
        storage_limit: usize,
    ) -> Observed {
        let original_body = body.clone();
        let witnesses: Vec<_> = events.iter().map(|row| row.witness).collect();
        let original_ops: Vec<_> = events.iter().map(|row| row.operation.clone()).collect();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(3).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = prepare_tile_discards_v29(&body, &mut events, &mut budget);
        assert_eq!(body, original_body);
        let mut ends = Vec::new();
        let mut end_bytes = 0;
        for ((row, witness), original) in events.iter().zip(witnesses).zip(original_ops) {
            assert_eq!(row.witness, witness);
            match &row.operation.as_ref().unwrap().kind {
                OperationKind::Execution(Ex::ScopeEnd { discarded, .. }) => {
                    end_bytes += discarded.capacity() * size_of::<ValueId>();
                    ends.push(discarded.clone());
                }
                _ => assert_eq!(row.operation, original),
            }
        }
        if result.is_ok() {
            assert_eq!(budget.storage(), FLOOR + end_bytes);
        }
        let (used, peak) = (budget.work(), budget.peak_storage());
        // The enclosing insertion attempt drops prepared owners before rollback.
        drop(events);
        budget.release_storage(budget.storage() - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert!(
            budget.work_ledger_identity_v1() == ledger,
            "work ledger identity changed"
        );
        Observed {
            result,
            ends,
            end_bytes,
            work: used,
            peak,
        }
    }
    fn refused(observed: Observed) {
        assert!(matches!(
            observed.result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "execution lifecycle differs from its retained source instance",
                ..
            })
        ));
    }

    #[test]
    fn discard_population_tracks_only_live_affine_descendants() {
        for (stage, expected) in [(0, vec![ValueId(2)]), (1, vec![ValueId(3)]), (2, vec![])] {
            let observed = run_discard(chain(stage), ROOM, ROOM);
            observed.result.unwrap();
            assert_eq!(observed.ends, vec![expected]);
        }
    }

    #[test]
    fn discard_population_crosses_helper_instances_without_moving_source_events() {
        let (mut body, mut events) = chain(1);
        assert_ne!(
            events[2].witness.instance,
            events.last().unwrap().witness.instance
        );
        body.blocks[0].operations.push(Operation::new(
            vec![ValueDef::new(ValueId(900), Type::Scalar(ScalarType::U32))],
            OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(9)),
        ));
        for row in &mut events[2..] {
            row.witness.before.first = 1;
            row.witness.after.first = 1;
        }
        let observed = run_discard((body, events), ROOM, ROOM);
        observed.result.unwrap();
        assert_eq!(observed.ends, vec![vec![ValueId(3)]]);
    }

    #[test]
    fn disjoint_branch_local_scopes_rejoin_after_each_scope_ends() {
        let events = vec![
            event(0, 0, issue()),
            event(1, 1, derive(1)),
            event(1, 2, load(2, 1)),
            event(1, 1, end(1)),
            event(2, 3, derive(10)),
            event(2, 4, load(11, 10)),
            event(2, 4, fragment(12, 11)),
            event(2, 3, end(10)),
        ];
        let observed = run_discard(
            (body(vec![split(), branch(3), branch(3), ret()]), events),
            ROOM,
            ROOM,
        );
        observed.result.unwrap();
        assert_eq!(observed.ends, vec![vec![ValueId(2)], vec![ValueId(12)]]);
    }

    #[test]
    fn discard_population_rejects_invalid_lifecycle_and_cfg() {
        for fault in 0..10 {
            let (mut body, mut events) = chain(2);
            match fault {
                0 => events.swap(1, 2),
                1 => events.insert(events.len() - 1, event(0, 2, parts(3, 30))),
                2 => events.push(event(0, 1, end(1))),
                3 => {
                    events.pop();
                }
                4 => body.blocks[0].terminator = Some(branch(0)),
                5 => {
                    let mut unreachable = BasicBlock::new(BlockId(1));
                    unreachable.terminator = Some(ret());
                    body.blocks.push(unreachable);
                    events.push(event(1, 2, load(40, 1)));
                }
                6 => {
                    let mut unreachable = BasicBlock::new(BlockId(1));
                    unreachable.terminator = Some(branch(1));
                    body.blocks.push(unreachable);
                    events.push(event(1, 2, load(40, 1)));
                }
                7 => events.last_mut().unwrap().witness.before.first = 1,
                8 => events.last_mut().unwrap().operation = Some(end(2)),
                9 => {
                    events[4].operation.as_mut().unwrap().results[2].ty =
                        Type::Scalar(ScalarType::U32)
                }
                _ => unreachable!(),
            }
            refused(run_discard((body, events), ROOM, ROOM));
        }
    }

    #[test]
    fn discard_population_rejects_unequal_join_before_scope_end() {
        let events = vec![
            event(0, 0, issue()),
            event(0, 1, derive(1)),
            event(1, 2, load(2, 1)),
            event(3, 1, end(1)),
        ];
        refused(run_discard(
            (body(vec![split(), branch(3), branch(3), ret()]), events),
            ROOM,
            ROOM,
        ));
    }

    #[test]
    fn late_lifecycle_error_preserves_retained_payload_until_outer_cleanup() {
        let (body, mut events) = chain(0);
        events.push(event(0, 1, end(1)));
        let observed = run_discard((body, events), ROOM, ROOM);
        assert_eq!(observed.ends, vec![vec![ValueId(2)], vec![]]);
        assert!(observed.end_bytes >= size_of::<ValueId>());
        refused(observed);
    }

    #[test]
    fn discard_population_sorts_live_descendants_by_value_id() {
        let events = vec![
            event(0, 0, issue()),
            event(0, 1, derive(1)),
            event(0, 2, load(9, 1)),
            event(0, 3, load(2, 1)),
            event(0, 1, end(1)),
        ];
        let observed = run_discard((body(vec![ret()]), events), ROOM, ROOM);
        observed.result.unwrap();
        assert_eq!(observed.ends, vec![vec![ValueId(2), ValueId(9)]]);
    }

    #[test]
    fn discard_population_obeys_measured_exact_and_one_short_budgets() {
        for stage in [1, 2] {
            let measured = run_discard(chain(stage), ROOM, ROOM);
            measured.result.unwrap();
            let exact = run_discard(chain(stage), measured.work, measured.peak);
            exact.result.unwrap();
            assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
            assert_eq!(exact.ends, measured.ends);
            let work_short = run_discard(chain(stage), measured.work - 1, measured.peak);
            assert!(matches!(
                work_short.result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
            let storage_short = run_discard(chain(stage), measured.work, measured.peak - 1);
            assert!(matches!(
                storage_short.result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
        }
    }

    #[test]
    fn discard_argument_bound_is_checked_before_payload_allocation() {
        let count = fe2o3_kernel_ir::MAX_VALUE_ARGUMENTS_V1;
        let mut events = vec![event(0, 0, issue()), event(0, 1, derive(1))];
        for i in 0..count {
            let block = usize::from(i >= count / 2);
            events.push(event(block, 2, load(u32::try_from(i + 2).unwrap(), 1)));
        }
        events.push(event(1, 1, end(1)));
        let observed = run_discard((body(vec![branch(1), ret()]), events), ROOM, ROOM);
        assert_eq!(observed.ends, vec![Vec::<ValueId>::new()]);
        assert_eq!(observed.end_bytes, 0);
        refused(observed);
    }

    include!("production_execution_tile_discard_cycles_v29_tests.rs");
}

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

fn result_fixture(parts: bool, elements: u16) -> DeferredTileEventV29 {
    let occurrence = ProductionCallOccurrenceV1 {
        caller: ProductionCallInstanceIdV1(2),
        block: SemanticBlockIdV1::from_index(3),
    };
    let identity = SemanticExecutionIdentityV29 {
        semantic_type: SemanticTypeIdV1::from_index(0),
        type_identity: SemanticTypeIdentityV1::from_sha256([77; 32]),
        producer: occurrence,
        value: ValueId(9),
    };
    DeferredTileEventV29 {
        input: if parts {
            DeferredTileInputV29::Parts { fragment: identity }
        } else {
            DeferredTileInputV29::Fragment { tile: identity }
        },
        producer: occurrence,
        result_type: SemanticTypeIdV1::from_index(1),
        first_result: ValueId(100),
        lanes: 64,
        elements,
    }
}

#[test]
fn tile_result_roster_preserves_geometry_order_and_exact_ids() {
    for lanes in [1, 64, 256] {
        for elements in [1, 2, 125] {
            for parts in [false, true] {
                let mut event = result_fixture(parts, elements);
                event.lanes = lanes;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
                let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
                with_canonical_call_scratch_v1(&mut budget, |budget| {
                    let operation = event.operation(budget)?;
                    let count = if parts { usize::from(elements) * 2 } else { 1 };
                    assert_eq!(operation.results.len(), count);
                    for (offset, value) in operation.results.iter().enumerate() {
                        assert_eq!(value.id, ValueId(100 + offset as u32));
                        let expected = if parts {
                            Type::Scalar(if offset < usize::from(elements) {
                                ScalarType::U32
                            } else {
                                ScalarType::Bool
                            })
                        } else {
                            Type::Execution(ExecutionRoleV15::LaneFragmentU32 { lanes, elements })
                        };
                        assert_eq!(value.ty, expected);
                    }
                    let OperationKind::Execution(kind) = operation.kind else {
                        panic!()
                    };
                    assert!(kind.validate_payload().is_ok());
                    Ok(())
                })
                .unwrap();
                assert_eq!(budget.storage(), 0);
            }
        }
    }
}

#[test]
fn tile_result_roster_refuses_bad_geometry_and_id_overflow() {
    for (lanes, elements, first) in [
        (0, 2, 100),
        (257, 2, 100),
        (64, 0, 100),
        (64, 126, 100),
        (64, 2, u32::MAX - 2),
    ] {
        let mut event = result_fixture(true, elements);
        event.lanes = lanes;
        event.first_result = ValueId(first);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        assert!(
            with_canonical_call_scratch_v1(&mut budget, |budget| { event.definitions(budget) })
                .is_err()
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn tile_result_roster_requires_exact_storage_and_work() {
    let event = result_fixture(true, 125);
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let outcome = with_canonical_call_scratch_v1(&mut budget, |budget| {
            let values = event.definitions(budget)?;
            assert_eq!(values.len(), 250);
            Ok(())
        });
        let peak = budget.peak_storage();
        assert_eq!(budget.storage(), 17);
        drop(budget);
        (outcome.is_ok(), work.work(), peak)
    };
    let (ok, work, peak) = run(1_000_000, 1_000_000);
    assert!(ok);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}

fn range_prepared(definitions: Vec<ValueDef>) -> PreparedLifecycleEventV29 {
    let span = InstancePhysicalSpanV1 {
        block: BlockId(0),
        first: 0,
        count: 0,
    };
    PreparedLifecycleEventV29 {
        block: 0,
        witness: LifecycleInsertionV29 {
            instance: ProductionCallInstanceIdV1(0),
            event: 0,
            source_span: 0,
            before: span,
            after: InstancePhysicalSpanV1 { count: 1, ..span },
        },
        operation: Some(Operation::new(
            definitions,
            OperationKind::Execution(fe2o3_kernel_ir::ExecutionOperationV15::ContextIssue),
        )),
    }
}

#[test]
fn tile_result_range_checks_every_physical_and_deferred_definition() {
    let scalar = |id| ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32));
    let plain = || FunctionBody {
        parameters: vec![],
        blocks: vec![BasicBlock::new(BlockId(0))],
    };
    for case in 0..5 {
        let mut body = plain();
        let mut prepared = Vec::new();
        match case {
            0 => body.parameters.push(ValueId(103)),
            1 => body.blocks[0].parameters.push(scalar(103)),
            2 => body.blocks[0].operations.push(Operation::new(
                vec![scalar(102), scalar(103)],
                OperationKind::Execution(fe2o3_kernel_ir::ExecutionOperationV15::ContextIssue),
            )),
            3 => prepared.push(range_prepared(vec![scalar(102), scalar(103)])),
            _ => prepared.push(range_prepared(vec![scalar(200), scalar(201)])),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        let outcome = check_lifecycle_result_range_v29(100..104, &body, &prepared, &mut budget);
        assert_eq!(outcome.is_ok(), case == 4, "case {case}");
        assert_eq!(budget.storage(), 0);
        assert!(check_lifecycle_result_range_v29(100..100, &body, &prepared, &mut budget).is_err());
    }
}
