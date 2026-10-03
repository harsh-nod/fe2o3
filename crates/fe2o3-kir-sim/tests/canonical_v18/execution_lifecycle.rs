//! Synthetic owner controls; lifecycle simulation is not tile or GPU authority.
use super::*;
use fe2o3_kernel_ir::{
    BinaryOp, ComparePredicate, ExecutionOperationV15 as Op, ExecutionRoleV15 as Role,
};

fn operation(id: u32, role: Role, op: Op) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Execution(role)),
        OperationKind::Execution(op),
    )
}
fn issue() -> Operation {
    operation(10, Role::Context, Op::ContextIssue)
}
fn derive() -> Operation {
    operation(
        11,
        Role::Workgroup,
        Op::WorkgroupDerive {
            context: ValueId(10),
        },
    )
}
fn end() -> Operation {
    Operation::new(
        vec![],
        OperationKind::Execution(Op::ScopeEnd {
            workgroup: ValueId(11),
            discarded: vec![],
        }),
    )
}
fn refresh(raw: &mut Module) {
    for function in &mut raw.functions {
        function.required_capabilities = function.derived_capabilities();
    }
    raw.kernels[0].required_capabilities = raw.functions[0].required_capabilities.clone();
    raw.required_capabilities = raw.functions[0].required_capabilities.clone();
}
fn lifecycle_module(scope: bool) -> Module {
    let mut raw = super::module();
    raw.functions.truncate(1);
    let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.insert(0, issue());
    if scope {
        block.operations.insert(1, derive());
        block.operations.push(end());
    }
    refresh(&mut raw);
    raw
}
fn with_module(raw: &Module, inspect: impl FnOnce(&AdmittedSimulationModuleV1)) {
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let (canonical, owner_paid) = owner(raw, &mut budget);
    let (admitted, view_paid) = view(&canonical, &mut budget);
    assert_eq!(admitted.module(), raw);
    assert_eq!(admitted.identity().digest(), canonical.identity().digest());
    inspect(&admitted);
    drop(admitted);
    budget.release_storage(view_paid).unwrap();
    drop(canonical);
    budget.release_storage(owner_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
fn target() -> SimulationTargetV1 {
    SimulationTargetV1::amdgpu_64()
}
fn assert_refused(raw: &Module) {
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let result =
        Owner::from_module_ref_with_verification_budget_v18(raw, LAYOUT_LIMITS, &mut budget);
    assert!(
        result.is_err(),
        "a malformed lifecycle must not obtain a canonical owner"
    );
    drop(result);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn v18_context_and_scope_execute_numerically_with_exact_owner_and_replay() {
    for scope in [false, true] {
        with_module(&lifecycle_module(scope), |admitted| {
            for target in [
                SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
                target(),
            ] {
                let request = request(target);
                let original = request.clone();
                let result = admitted
                    .simulate(&request, target, SimulationLimitsV1::default())
                    .unwrap();
                assert_output(&result);
                assert_eq!(result.invocations_executed(), 64);
                assert_eq!(result.workgroups_visited(), 2);
                let scheduled = admitted
                    .simulate_scheduled(
                        &request,
                        target,
                        SimulationLimitsV1::default(),
                        SimulationScheduleRequestV1::RecordSeeded {
                            seed: 17,
                            max_decisions: 256,
                        },
                    )
                    .unwrap();
                assert_output(&scheduled);
                let replay = admitted
                    .simulate_scheduled(
                        &request,
                        target,
                        SimulationLimitsV1::default(),
                        SimulationScheduleRequestV1::Replay(scheduled.schedule_record().unwrap()),
                    )
                    .unwrap();
                assert_output(&replay);
                assert_eq!(
                    scheduled.schedule_transcript_identity(),
                    replay.schedule_transcript_identity()
                );
                assert_eq!(request, original);
            }
        });
    }
}

fn loop_module() -> Module {
    let mut raw = lifecycle_module(false);
    let body = raw.functions[0].body.as_mut().unwrap();
    let ordinary = body.blocks[0].operations.split_off(1);
    for (id, value) in [(20, 0), (21, 3), (22, 1)] {
        body.blocks[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::INDEX),
            OperationKind::Constant(Constant::Index(value)),
        ));
    }
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(20)],
    });
    let mut repeated = BasicBlock::new(BlockId(1));
    repeated
        .parameters
        .push(ValueDef::new(ValueId(23), Type::INDEX));
    repeated.operations.push(derive());
    repeated.operations.extend(ordinary);
    repeated.operations.push(end());
    repeated.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(24), Type::INDEX),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(23),
            rhs: ValueId(22),
        },
    ));
    repeated.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(25), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(24),
            rhs: ValueId(21),
        },
    ));
    repeated.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(25),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(24)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([repeated, exit]);
    refresh(&mut raw);
    raw
}

#[test]
fn v18_balanced_loop_reuses_tokens_and_exact_existing_step_and_memory_limits() {
    with_module(&loop_module(), |admitted| {
        let request = request(target());
        let result = admitted
            .simulate(&request, target(), SimulationLimitsV1::default())
            .unwrap();
        assert_output(&result);
        let steps = result.steps_executed();
        let mut exact = SimulationLimitsV1 {
            max_steps: steps,
            ..SimulationLimitsV1::default()
        };
        let resident = admitted
            .preflight(&request, target(), exact)
            .unwrap()
            .resident_bytes();
        exact.max_resident_bytes = resident;
        assert_output(&admitted.simulate(&request, target(), exact).unwrap());
        let mut short_work = exact;
        short_work.max_steps -= 1;
        assert!(matches!(admitted.simulate(&request, target(), short_work),
            Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::StepLimit { limit }, ..
            })) if limit == steps - 1));
        let mut short_memory = exact;
        short_memory.max_resident_bytes -= 1;
        assert!(
            matches!(admitted.preflight(&request, target(), short_memory),
            Err(SimulationPreflightErrorV1::ResourceLimit { resource: "resident bytes", actual, limit })
                if actual == resident as u64 && limit == resident as u64 - 1)
        );
    });
}

#[test]
fn v18_scope_before_ordinary_helper_keeps_frame_and_result_transport() {
    let mut raw = lifecycle_module(true);
    let body = raw.functions[0].body.as_mut().unwrap();
    let store = body.blocks[0].operations.remove(4);
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U32)),
        OperationKind::Call {
            callee: "identity".into(),
            arguments: vec![ValueId(1)],
        },
    ));
    let mut store = store;
    let OperationKind::Store { value, .. } = &mut store.kind else {
        panic!()
    };
    *value = ValueId(12);
    body.blocks[0].operations.push(store);
    let mut helper = BasicBlock::new(BlockId(0));
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    raw.functions.push(Function::definition(
        "identity",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32)],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0)],
        vec![helper],
    ));
    refresh(&mut raw);
    with_module(&raw, |admitted| {
        assert_output(
            &admitted
                .simulate(&request(target()), target(), SimulationLimitsV1::default())
                .unwrap(),
        )
    });
    // Calling a helper while a scope is live still lacks the canonical exit proof.
    let body = raw.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.swap(4, 5);
    assert_refused(&raw);
}

#[test]
fn v18_malformed_lifecycle_and_token_transport_never_gain_an_owner() {
    for mutation in 0..7 {
        let mut raw = lifecycle_module(true);
        let function = &mut raw.functions[0];
        let body = function.body.as_mut().unwrap();
        match mutation {
            0 => {
                body.blocks[0]
                    .operations
                    .insert(2, operation(30, Role::Context, Op::ContextIssue));
            }
            1 => {
                body.blocks[0].operations.pop();
            }
            2 => {
                body.blocks[0].operations.push(end());
            }
            3 => {
                let OperationKind::Execution(Op::ScopeEnd { workgroup, .. }) =
                    &mut body.blocks[0].operations.last_mut().unwrap().kind
                else {
                    panic!()
                };
                *workgroup = ValueId(10);
            }
            4 => {
                function
                    .signature
                    .parameters
                    .push(Type::Execution(Role::Context));
                body.parameters.push(ValueId(40));
            }
            5 => {
                body.blocks[0].operations.push(Operation::effect_free(
                    ValueDef::new(ValueId(41), Type::BOOL),
                    OperationKind::Constant(Constant::Bool(true)),
                ));
                body.blocks[0].operations.push(Operation::effect_free(
                    ValueDef::new(ValueId(40), Type::Execution(Role::Context)),
                    OperationKind::Select {
                        condition: ValueId(41),
                        true_value: ValueId(10),
                        false_value: ValueId(10),
                    },
                ));
            }
            6 => {
                function
                    .signature
                    .results
                    .push(Type::Execution(Role::Context));
                body.blocks[0].terminator = Some(Terminator::Return {
                    values: vec![ValueId(10)],
                });
            }
            _ => unreachable!(),
        }
        refresh(&mut raw);
        assert_refused(&raw);
    }
    let mut looped = loop_module();
    looped.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .remove(4);
    assert_refused(&looped);
}

fn tile_module(stage: u8) -> Module {
    let mut raw = lifecycle_module(false);
    let function = &mut raw.functions[0];
    function.signature.parameters.push(Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    ));
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(4));
    let operations = &mut body.blocks[0].operations;
    operations.insert(1, derive());
    operations.insert(
        2,
        Operation::effect_free(
            ValueDef::new(ValueId(12), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
    );
    operations.insert(
        3,
        operation(
            13,
            Role::MaskedTileU32 {
                lanes: 64,
                elements: 2,
            },
            Op::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(4),
                base: ValueId(12),
                lanes: 64,
                elements: 2,
            },
        ),
    );
    let mut discarded = vec![ValueId(13)];
    if stage >= 1 {
        operations.insert(
            4,
            operation(
                14,
                Role::LaneFragmentU32 {
                    lanes: 64,
                    elements: 2,
                },
                Op::TileIntoFragmentU32 {
                    tile: ValueId(13),
                    lanes: 64,
                    elements: 2,
                },
            ),
        );
        discarded = vec![ValueId(14)];
    }
    if stage >= 2 {
        operations.insert(
            5,
            Operation::new(
                vec![
                    ValueDef::new(ValueId(15), Type::Scalar(ScalarType::U32)),
                    ValueDef::new(ValueId(16), Type::Scalar(ScalarType::U32)),
                    ValueDef::new(ValueId(17), Type::BOOL),
                    ValueDef::new(ValueId(18), Type::BOOL),
                ],
                OperationKind::Execution(Op::FragmentIntoPartsU32 {
                    fragment: ValueId(14),
                    lanes: 64,
                    elements: 2,
                }),
            ),
        );
        discarded.clear();
    }
    operations.push(Operation::new(
        vec![],
        OperationKind::Execution(Op::ScopeEnd {
            workgroup: ValueId(11),
            discarded,
        }),
    ));
    refresh(&mut raw);
    raw
}

#[test]
fn v18_unscheduled_tiles_fragments_and_descendant_disposal_remain_refused() {
    for stage in 0..=2 {
        with_module(&tile_module(stage), |admitted| {
            let error = admitted
                .preflight(&request(target()), target(), SimulationLimitsV1::default())
                .unwrap_err();
            let SimulationPreflightErrorV1::Unsupported(report) = error else {
                panic!("{error:?}")
            };
            assert!(
                report
                    .findings()
                    .iter()
                    .any(|finding| finding.feature == UnsupportedFeatureV1::InertExecutionV15)
            );
        });
    }
}

#[derive(Default)]
struct Records(Vec<SimulationDebugRecordV1>);
impl SimulationDebugSinkV1 for Records {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        assert!(self.0.len() < 512);
        self.0.push(record);
        SimulationDebugSinkControlV1::Continue
    }
}

#[test]
fn v18_lifecycle_debug_keeps_memory_but_explicitly_loses_the_whole_value_stack() {
    with_module(&lifecycle_module(true), |admitted| {
        let mut request = request(target());
        request.grid = GridShapeV1([1, 1, 1]);
        request.workgroup = WorkgroupShapeV1([1, 1, 1]);
        let mut records = Records::default();
        let result = admitted
            .simulate_debugged_with_sink(
                &request,
                target(),
                SimulationLimitsV1::default(),
                SimulationDebugCaptureLimitsV1::new(8, 64, 8, 128).unwrap(),
                &mut records,
            )
            .unwrap();
        assert_eq!(
            &result.buffer(0).unwrap().bytes()[..4],
            &37_u32.to_le_bytes()
        );
        assert!(records.0.iter().any(|record| matches!(&record.kind,
            SimulationDebugRecordKindV1::Checkpoint { phase: SimulationDebugCheckpointPhaseV1::BeforeOperation,
                stack: SimulationDebugCollectionV1::Captured(frames), .. }
                if record.site.operation == 0 && !frames.is_empty())));
        let after_issue: Vec<_> = records
            .0
            .iter()
            .filter(|record| {
                record.site.operation != 0
                    || matches!(
                        &record.kind,
                        SimulationDebugRecordKindV1::Checkpoint {
                            phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
                            ..
                        }
                    )
            })
            .filter_map(|record| match &record.kind {
                SimulationDebugRecordKindV1::Checkpoint { stack, .. } => Some(stack),
                _ => None,
            })
            .collect();
        assert!(!after_issue.is_empty());
        assert!(after_issue.iter().all(|stack| matches!(
            stack,
            SimulationDebugCollectionV1::Unavailable {
                reason: SimulationDebugUnavailableReasonV1::NotCaptured,
                ..
            }
        )));
        assert!(records.0.iter().any(|record| matches!(&record.kind,
            SimulationDebugRecordKindV1::Checkpoint { memory: SimulationDebugCollectionV1::Captured(allocations), .. }
                if allocations.iter().any(|allocation| allocation.address_space == AddressSpace::Global
                    && allocation.bytes[..4] == 37_u32.to_le_bytes()))));
        assert!(records.0.iter().any(|record| matches!(
            &record.kind,
            SimulationDebugRecordKindV1::Memory {
                address_space: AddressSpace::Global,
                access: SimulationDebugMemoryAccessV1::WriteCommitted,
                byte_offset: 0,
                byte_len: 4,
                ..
            }
        )));
        assert!(
            records
                .0
                .iter()
                .all(|record| record.invocation.global == [0, 0, 0]
                    && record.invocation.local == [0, 0, 0]
                    && record.invocation.workgroup_size == [1, 1, 1]
                    && record.invocation.launch_extent == [1, 1, 1])
        );
        assert!(!result.grants_execution_authority());
    });
}

#[test]
fn v18_lifecycle_capability_is_partial_and_old_profiles_stay_closed() {
    let matrix = semantic_capability_matrix_v1();
    let mut rows = 0;
    for row in matrix
        .top_level_rows
        .iter()
        .filter(|row| row.operation == SimulationOperationSurfaceV1::Execution)
    {
        if row.kir_wire_version == SimulationKirWireVersionV1::V18 {
            assert_eq!(
                row.capability,
                SimulationCapabilityDispositionV1::Owned {
                    owner: SimulationSemanticOwnerV1::ControlFlow,
                    typed_rejections: &[SimulationUnsupportedReasonCodeV1::InertExecutionV15],
                }
            );
            rows += 1;
        } else {
            assert!(matches!(
                row.capability,
                SimulationCapabilityDispositionV1::Unsupported { .. }
            ));
        }
    }
    assert_eq!(rows, 4);
}
