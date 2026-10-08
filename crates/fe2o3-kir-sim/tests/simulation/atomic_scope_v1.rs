//! Canonical simulator regressions, not source-import or hardware qualification.
use super::*;

fn run_scope(
    kind: AtomicKind,
    scope: SynchronizationScope,
    grid: u64,
    group: u32,
) -> fe2o3_kir_sim::SimulationExecutionV1 {
    admitted(atomic_module(
        kind,
        ScalarType::U32,
        AddressSpace::Global,
        scope,
        MemoryOrdering::Relaxed,
        (kind == AtomicKind::CompareExchange).then_some(MemoryOrdering::Relaxed),
    ))
    .simulate(
        &atomic_request(
            ScalarBitsV1::u32(0),
            ScalarBitsV1::u32(1),
            ScalarBitsV1::u32(2),
            grid,
            group,
        ),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn atomic_scope_canonical_same_group_and_cross_group_are_not_conflated() {
    use SynchronizationScope::*;
    for scope in [Workgroup, Device, System] {
        // More than two participants exercises the bounded eviction summary.
        let same = run_scope(AtomicKind::Add, scope, 8, 8);
        assert_eq!(scalar_buffer_bits(&same), 8);
        assert!(matches!(
            same.race_assessment(),
            SimulationRaceAssessmentV1::NoRacesObserved { .. }
        ));
        let cross = run_scope(AtomicKind::Add, scope, 8, 4);
        assert_eq!(scalar_buffer_bits(&cross), 8);
        match scope {
            Workgroup => assert!(matches!(
                cross.race_assessment(),
                SimulationRaceAssessmentV1::RacesObserved {
                    racing_bytes: 4,
                    first: fe2o3_kir_sim::SimulationDataRaceV1 {
                        earlier_atomic: true,
                        later_atomic: true,
                        ..
                    },
                    ..
                }
            )),
            _ => assert!(matches!(
                cross.race_assessment(),
                SimulationRaceAssessmentV1::NoRacesObserved { .. }
            )),
        }
    }
}

#[test]
fn atomic_scope_subgroup_width_is_not_invented_from_workgroup_size() {
    let same = run_scope(AtomicKind::Add, SynchronizationScope::Subgroup, 2, 2);
    assert!(matches!(
        same.race_assessment(),
        SimulationRaceAssessmentV1::Incomplete {
            atomic_or_fence_happens_before_unmodeled: true,
            first: None,
            racing_bytes: 0,
            ..
        }
    ));
    let cross = run_scope(AtomicKind::Add, SynchronizationScope::Subgroup, 2, 1);
    assert!(matches!(
        cross.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved {
            racing_bytes: 4,
            ..
        }
    ));
}

#[test]
fn atomic_scope_read_read_and_failed_compare_exchange_are_not_writes() {
    for scope in [
        SynchronizationScope::Subgroup,
        SynchronizationScope::Workgroup,
        SynchronizationScope::Device,
        SynchronizationScope::System,
    ] {
        for kind in [AtomicKind::Load, AtomicKind::CompareExchange] {
            let execution = run_scope(kind, scope, 8, 4);
            assert_eq!(scalar_buffer_bits(&execution), 0);
            assert_eq!(
                execution.conflict_assessment(),
                &SimulationConflictAssessmentV1::NoConflictsObserved
            );
            assert!(matches!(
                execution.race_assessment(),
                SimulationRaceAssessmentV1::NoRacesObserved {
                    first_ordered_conflict: None,
                }
            ));
        }
    }
}

#[test]
fn atomic_scope_one_successful_compare_exchange_keeps_its_write_conflicts() {
    let module = admitted(atomic_module(
        AtomicKind::CompareExchange,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::Workgroup,
        MemoryOrdering::Relaxed,
        Some(MemoryOrdering::Relaxed),
    ));
    let execution = module
        .simulate(
            &atomic_request(
                ScalarBitsV1::u32(0),
                ScalarBitsV1::u32(1),
                ScalarBitsV1::u32(0),
                4,
                2,
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(scalar_buffer_bits(&execution), 1);
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved {
            racing_bytes: 4,
            ..
        }
    ));
}

fn two_scope_module(first: SynchronizationScope, second: SynchronizationScope) -> Module {
    let mut module = atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::Relaxed,
        None,
    );
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let OperationKind::Atomic(atomic) = &mut block.operations[0].kind else {
        unreachable!()
    };
    atomic.scope = first;
    let mut second_op = block.operations[0].clone();
    second_op.results[0].id = ValueId(4);
    let OperationKind::Atomic(atomic) = &mut second_op.kind else {
        unreachable!()
    };
    atomic.scope = second;
    block.operations.push(second_op);
    module
}

#[test]
fn atomic_scope_same_invocation_merge_retains_the_narrowest_scope_in_both_orders() {
    use SynchronizationScope::*;
    for (first, second) in [(Workgroup, System), (System, Workgroup)] {
        let execution = admitted(two_scope_module(first, second))
            .simulate(
                &atomic_request(
                    ScalarBitsV1::u32(0),
                    ScalarBitsV1::u32(1),
                    ScalarBitsV1::u32(0),
                    2,
                    1,
                ),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        assert_eq!(scalar_buffer_bits(&execution), 4);
        assert!(matches!(
            execution.race_assessment(),
            SimulationRaceAssessmentV1::RacesObserved {
                racing_bytes: 4,
                ..
            }
        ));
    }
}

// Only invocation 0 has a narrow scope. Its representative is evicted by
// broad-scope writers 1..3 before invocation 4 in another workgroup arrives.
fn evicted_scope_module() -> Module {
    let mut module = atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::Relaxed,
        None,
    );
    let old = module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].clone();
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            4,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(5, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            6,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut narrow = BasicBlock::new(BlockId(1));
    let mut narrow_op = old.clone();
    narrow_op.results[0].id = ValueId(7);
    let OperationKind::Atomic(atomic) = &mut narrow_op.kind else {
        unreachable!()
    };
    atomic.scope = SynchronizationScope::Workgroup;
    narrow.operations.push(narrow_op);
    narrow.terminator = Some(Terminator::Return { values: vec![] });
    let mut wide = BasicBlock::new(BlockId(2));
    let mut wide_op = old;
    wide_op.results[0].id = ValueId(8);
    wide.operations.push(wide_op);
    wide.terminator = Some(Terminator::Return { values: vec![] });
    module.functions[0].body.as_mut().unwrap().blocks = vec![entry, narrow, wide];
    module
}

#[test]
fn atomic_scope_evicted_narrow_writer_prevents_false_complete_assessment() {
    let execution = admitted(evicted_scope_module())
        .simulate(
            &atomic_request(
                ScalarBitsV1::u32(0),
                ScalarBitsV1::u32(1),
                ScalarBitsV1::u32(0),
                8,
                4,
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(scalar_buffer_bits(&execution), 8);
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::Incomplete {
            access_frontier_incomplete: true,
            first: None,
            racing_bytes: 0,
            ..
        }
    ));
}

#[test]
fn atomic_scope_seeded_record_and_exact_replay_preserve_assessment() {
    let module = admitted(atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::Workgroup,
        MemoryOrdering::Relaxed,
        None,
    ));
    let request = atomic_request(
        ScalarBitsV1::u32(0),
        ScalarBitsV1::u32(1),
        ScalarBitsV1::u32(0),
        10,
        4,
    );
    for seed in [0, 1, 0xc81e] {
        let run = module
            .simulate_scheduled(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::RecordSeeded {
                    seed,
                    max_decisions: 32,
                },
            )
            .unwrap();
        assert_eq!(scalar_buffer_bits(&run), 10);
        assert!(matches!(
            run.race_assessment(),
            SimulationRaceAssessmentV1::RacesObserved { .. }
        ));
        let replay = module
            .simulate_scheduled(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::Replay(run.schedule_record().unwrap()),
            )
            .unwrap();
        assert_eq!(run.race_assessment(), replay.race_assessment());
        assert_eq!(run.arguments(), replay.arguments());
        assert_eq!(run.conflict_assessment(), replay.conflict_assessment());
        assert_eq!(run.schedule(), replay.schedule());
        assert_eq!(
            run.schedule_transcript_identity(),
            replay.schedule_transcript_identity()
        );
        assert_eq!(run.schedule_coverage(), replay.schedule_coverage());
        // Replay validates and consumes the supplied decisions without
        // allocating a second retained schedule record.
        assert!(replay.schedule_record().is_none());
    }
}

#[test]
fn atomic_scope_access_record_limit_remains_incomplete() {
    let module = admitted(atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::Relaxed,
        None,
    ));
    let execution = module
        .simulate(
            &atomic_request(
                ScalarBitsV1::u32(0),
                ScalarBitsV1::u32(1),
                ScalarBitsV1::u32(0),
                4,
                4,
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_memory_access_records: 1,
                ..SimulationLimitsV1::default()
            },
        )
        .unwrap();
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::Incomplete {
            access_record_limit_reached: true,
            record_limit: 1,
            ..
        }
    ));
}

#[test]
fn atomic_scope_frontier_remains_charged_to_the_existing_resident_limit() {
    let module = admitted(atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::Relaxed,
        None,
    ));
    let request = atomic_request(
        ScalarBitsV1::u32(0),
        ScalarBitsV1::u32(1),
        ScalarBitsV1::u32(0),
        4,
        4,
    );
    let limits = SimulationLimitsV1::default();
    let plan = module
        .preflight(&request, SimulationTargetV1::amdgpu_64(), limits)
        .unwrap();
    assert!(plan.resident_bytes() > 1);
    assert!(
        module
            .preflight(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1 {
                    max_resident_bytes: plan.resident_bytes() - 1,
                    ..limits
                }
            )
            .is_err()
    );
}

// Only workgroup 0 executes the initial narrow access. Its local barriers
// do not order the other workgroup. In the read case the peer goes directly to
// the final write, so no intervening peer read evicts the intended witness.
fn epoch_scope_module(first_kind: AtomicKind, barriers: usize) -> Module {
    assert!((1..=2).contains(&barriers));
    let mut module = atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::Relaxed,
        None,
    );
    let prototype = module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].clone();
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            4,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(5, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            6,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let access = |id, scope, kind| {
        let mut operation = prototype.clone();
        operation.results[0].id = ValueId(id);
        let OperationKind::Atomic(atomic) = &mut operation.kind else {
            unreachable!()
        };
        atomic.kind = kind;
        atomic.scope = scope;
        atomic.value = (kind != AtomicKind::Load).then_some(ValueId(1));
        operation
    };
    let mut narrow = BasicBlock::new(BlockId(1));
    narrow
        .operations
        .push(access(7, SynchronizationScope::Workgroup, first_kind));
    narrow.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut peer = BasicBlock::new(BlockId(2));
    peer.terminator = Some(Terminator::Branch {
        target: if first_kind == AtomicKind::Load {
            BlockId(5)
        } else {
            BlockId(3)
        },
        arguments: vec![],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    for epoch in 0..barriers {
        merge.operations.push(Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Global],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ));
        merge.operations.push(access(
            8 + epoch as u32,
            SynchronizationScope::System,
            first_kind,
        ));
    }
    merge.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(4),
        then_arguments: vec![],
        else_target: BlockId(5),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(4));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut writer = BasicBlock::new(BlockId(5));
    writer
        .operations
        .push(access(20, SynchronizationScope::System, AtomicKind::Add));
    writer.terminator = Some(Terminator::Return { values: vec![] });
    let function = &mut module.functions[0];
    function.body.as_mut().unwrap().blocks = vec![entry, narrow, peer, merge, exit, writer];
    function
        .required_capabilities
        .insert(TargetCapability::WorkgroupBarrier);
    module.kernels[0].required_capabilities = function.required_capabilities.clone();
    module.required_capabilities = function.required_capabilities.clone();
    module
}

fn assert_epoch_history(first_kind: AtomicKind) {
    for barriers in [1, 2] {
        let execution = admitted(epoch_scope_module(first_kind, barriers))
            .simulate(
                &atomic_request(
                    ScalarBitsV1::u32(0),
                    ScalarBitsV1::u32(1),
                    ScalarBitsV1::u32(0),
                    2,
                    1,
                ),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        let expected = if first_kind == AtomicKind::Load {
            1
        } else {
            2 * barriers as u128 + 2
        };
        assert_eq!(scalar_buffer_bits(&execution), expected);
        if barriers == 1 {
            assert!(matches!(
                execution.race_assessment(),
                SimulationRaceAssessmentV1::RacesObserved {
                    racing_bytes: 4,
                    first: fe2o3_kir_sim::SimulationDataRaceV1 {
                        earlier_atomic: true,
                        later_atomic: true,
                        ..
                    },
                    ..
                }
            ));
        } else {
            // The exact old site was evicted, but its scope is still a duty.
            // An unproved summary cannot invent a concrete race or a pass.
            assert!(matches!(
                execution.race_assessment(),
                SimulationRaceAssessmentV1::Incomplete {
                    access_frontier_incomplete: true,
                    racing_bytes: 0,
                    first: None,
                    ..
                }
            ));
        }
    }
}

#[test]
fn atomic_scope_prior_epoch_write_survives_two_wider_epoch_replacements() {
    assert_epoch_history(AtomicKind::Add);
}

#[test]
fn atomic_scope_prior_epoch_read_survives_until_a_later_cross_group_write() {
    assert_epoch_history(AtomicKind::Load);
}

#[test]
fn atomic_scope_epoch_retention_does_not_order_a_post_barrier_ordinary_race() {
    let execution = admitted(global_barrier_epoch_transition_module())
        .simulate(
            &SimulationRequestV1::new(
                "global_order",
                [2, 1, 1],
                [2, 1, 1],
                vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved {
            racing_bytes: 4,
            ..
        }
    ));
}
