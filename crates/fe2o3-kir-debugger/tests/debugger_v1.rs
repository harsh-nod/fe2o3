use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BarrierSemantics, BasicBlock, Constant, Convergence, Function,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, MemoryOrdering, Module,
    Operation, OperationKind, ScalarType, Signature, SynchronizationScope, TargetCapability,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV7, WorkgroupBarrier,
};
use fe2o3_kir_debugger::{
    DebugBreakpointV1, DebugHitConditionV1, DebugInspectionUnavailableV1, DebugInspectionV1,
    DebugKirIdentityV1, DebugNavigationV1, DebugPredicateV1, DebugScopeSelectorV1, DebugSessionV1,
    DebugSiteSelectorV1, DebugSourceCatalogV1, DebugSourceFileV1, DebugSourceResolutionV1,
    DebugSourceSiteV1, DebugSourceSpanV1, DebugTranscriptCompletenessV1, DebugWatchAccessV1,
    DebugWatchpointV1, DebugWaveWidthV1, DebuggerLimitsV1, capture_debugger_run_v1,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationDebugCaptureLimitsV1, SimulationDebugCheckpointPhaseV1,
    SimulationDebugMemoryAccessV1, SimulationDebugRecordKindV1, SimulationLimitsV1,
    SimulationRequestV1, SimulationTargetV1,
};

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn fill_module() -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    block.operations = vec![
        op(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            2,
            pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(1),
            },
        ),
        op(3, element, OperationKind::Constant(Constant::U32(42))),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "fill_impl",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("debugger-tests::fill");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "fill",
        "fill_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn helper_barrier_module() -> Module {
    let barrier = WorkgroupBarrier {
        memory_scope: SynchronizationScope::Workgroup,
        semantics: BarrierSemantics::new(MemoryOrdering::AcquireRelease, [AddressSpace::Workgroup]),
        convergence: Convergence::uniform(SynchronizationScope::Workgroup),
    };
    let mut helper_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    helper_block.operations.push(Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(barrier),
    ));
    helper_block.terminator = Some(Terminator::Return { values: vec![] });
    let mut helper = Function::internal_helper(
        "barrier_helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![helper_block],
    );
    helper
        .required_capabilities
        .insert(TargetCapability::WorkgroupBarrier);

    let mut entry_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    entry_block.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: "barrier_helper".into(),
            arguments: vec![],
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "helper_barrier_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry_block],
    );
    entry
        .required_capabilities
        .insert(TargetCapability::WorkgroupBarrier);
    let mut kernel = Kernel::new(
        "helper_barrier",
        "helper_barrier_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel
        .required_capabilities
        .insert(TargetCapability::WorkgroupBarrier);
    let mut module = Module::new("debugger-tests::helper-barrier");
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupBarrier);
    module.functions.extend([entry, helper]);
    module.kernels.push(kernel);
    module
}

fn run() -> fe2o3_kir_debugger::DebuggerRunV1 {
    run_with_buffer(&[0, 0], 2)
}

fn run_with_buffer(values: &[u32], grid: u64) -> fe2o3_kir_debugger::DebuggerRunV1 {
    run_with_buffer_and_limits(
        values,
        grid,
        DebuggerLimitsV1::new(1_024, 16_384, 1 << 20).unwrap(),
    )
}

fn run_with_buffer_and_limits(
    values: &[u32],
    grid: u64,
    debugger_limits: DebuggerLimitsV1,
) -> fe2o3_kir_debugger::DebuggerRunV1 {
    let verified = VerifiedCanonicalKernelIrV7::from_module(fill_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit(verified, SimulationLimitsV1::default()).unwrap();
    let buffer = BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &values
            .iter()
            .copied()
            .map(ScalarBitsV1::u32)
            .collect::<Vec<_>>(),
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "fill",
        [grid, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(buffer)],
    );
    capture_debugger_run_v1(
        &admitted,
        &request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        SimulationDebugCaptureLimitsV1::new(16, 256, 16, 4_096).unwrap(),
        debugger_limits,
        DebugWaveWidthV1::Wave32,
    )
}

#[test]
fn bounded_continue_with_zero_budget_is_a_no_op() {
    let run = run();
    let mut session = DebugSessionV1::new(run.transcript);
    assert_eq!(
        session.continue_forward_bounded(0),
        DebugNavigationV1::Beginning
    );
    assert!(matches!(
        session.forward_step(&DebugScopeSelectorV1::Dispatch),
        DebugNavigationV1::Stopped(_)
    ));
    let cursor = session.cursor_record_index();
    assert!(matches!(
        session.continue_forward_bounded(0),
        DebugNavigationV1::BudgetExhausted(_)
    ));
    assert_eq!(session.cursor_record_index(), cursor);
}

#[test]
fn seek_cannot_cross_a_truncated_transcript_boundary() {
    let run = run_with_buffer_and_limits(
        &[0, 0],
        2,
        DebuggerLimitsV1::new(1, 16_384, 1 << 20).unwrap(),
    );
    assert!(matches!(
        run.transcript.completeness(),
        DebugTranscriptCompletenessV1::Truncated(_)
    ));
    let retained = run.transcript.records().len();
    let mut session = DebugSessionV1::new(run.transcript);
    assert!(matches!(
        session.seek_record_index(retained),
        DebugNavigationV1::TranscriptTruncated(_)
    ));
    assert_eq!(session.cursor_record_index(), None);
}

#[test]
fn dynamic_failure_is_a_terminal_replay_cursor() {
    let run = run_with_buffer(&[0], 2);
    assert!(run.execution.is_err());
    assert!(run.transcript.terminal_fault().is_some());
    let mut session = DebugSessionV1::new(run.transcript);
    assert!(matches!(
        session.continue_forward(),
        DebugNavigationV1::Stopped(stop)
            if stop.reason == fe2o3_kir_debugger::DebugStopReasonV1::Fault
    ));
    assert!(matches!(
        &session.current_fault().unwrap().kind,
        fe2o3_kir_sim::SimulationExecutionErrorKindV1::OutOfBounds { .. }
    ));
}

#[test]
fn simulator_derived_transcript_contains_committed_values_and_hierarchy() {
    let run = run();
    assert!(run.execution.is_ok());
    assert_eq!(
        run.transcript.completeness(),
        DebugTranscriptCompletenessV1::Complete
    );
    let writes = run
        .transcript
        .records()
        .iter()
        .filter(|record| {
            matches!(
                &record.kind,
                SimulationDebugRecordKindV1::Memory {
                    access: SimulationDebugMemoryAccessV1::WriteCommitted,
                    value: fe2o3_kir_sim::SimulationDebugValueV1::Scalar(value),
                    ..
                } if *value == ScalarBitsV1::u32(42)
            )
        })
        .count();
    assert_eq!(writes, 2);

    let mut session = DebugSessionV1::new(run.transcript);
    let stopped = session.forward_step(&DebugScopeSelectorV1::Dispatch);
    assert!(matches!(stopped, DebugNavigationV1::Stopped(_)));
    let hierarchy = session.current_hierarchy().unwrap();
    assert_eq!(hierarchy.wave, 0);
    assert_eq!(hierarchy.lane, 0);
    assert_eq!(hierarchy.active_mask, 0b11);
}

#[test]
fn checkpoint_memory_bound_counts_bytes_and_initialization_state() {
    let verified = VerifiedCanonicalKernelIrV7::from_module(fill_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit(verified, SimulationLimitsV1::default()).unwrap();
    let buffer = BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &[ScalarBitsV1::u32(0)],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "fill",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(buffer)],
    );
    let run = capture_debugger_run_v1(
        &admitted,
        &request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        SimulationDebugCaptureLimitsV1::new(16, 256, 16, 4).unwrap(),
        DebuggerLimitsV1::new(1_024, 16_384, 1 << 20).unwrap(),
        DebugWaveWidthV1::Wave32,
    );
    let memory = run
        .transcript
        .records()
        .iter()
        .find_map(|record| match &record.kind {
            SimulationDebugRecordKindV1::Checkpoint { memory, .. } => Some(memory),
            _ => None,
        })
        .expect("checkpoint memory observation");
    assert!(matches!(
        memory,
        fe2o3_kir_sim::SimulationDebugCollectionV1::Unavailable {
            reason: fe2o3_kir_sim::SimulationDebugUnavailableReasonV1::MemoryByteLimit,
            required: 8,
        }
    ));
}

#[test]
fn breakpoints_watchpoints_and_reverse_navigation_share_one_typed_transcript() {
    let run = run();
    let mut session = DebugSessionV1::new(run.transcript);
    session
        .add_breakpoint(DebugBreakpointV1 {
            id: 1,
            site: DebugSiteSelectorV1 {
                function_ordinal: Some(0),
                block: Some(fe2o3_kernel_ir::BlockId(0)),
                operation: Some(2),
                phase: Some(SimulationDebugCheckpointPhaseV1::AfterOperation),
            },
            scope: DebugScopeSelectorV1::GlobalWorkitem([0, 0, 0]),
            predicate: DebugPredicateV1::ScalarEquals {
                frame_depth: 0,
                value: ValueId(3),
                expected: ScalarBitsV1::u32(42),
            },
            hit_condition: None,
            enabled: true,
        })
        .unwrap();
    session
        .add_watchpoint(DebugWatchpointV1 {
            id: 2,
            allocation: 1,
            byte_offset: 0,
            byte_len: 8,
            access: DebugWatchAccessV1::Write,
            scope: DebugScopeSelectorV1::Dispatch,
            value_equals: Some(ScalarBitsV1::u32(42)),
            enabled: true,
        })
        .unwrap();

    assert!(matches!(
        session.continue_forward(),
        DebugNavigationV1::Stopped(stop)
            if stop.reason == fe2o3_kir_debugger::DebugStopReasonV1::Breakpoint(1)
    ));
    assert_eq!(
        session.scalar(0, ValueId(3)),
        DebugInspectionV1::Available(ScalarBitsV1::u32(42))
    );
    assert!(matches!(
        session.continue_forward(),
        DebugNavigationV1::Stopped(stop)
            if stop.reason == fe2o3_kir_debugger::DebugStopReasonV1::Watchpoint(2)
    ));
    assert_eq!(
        session.stack(),
        DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NotCheckpoint)
    );
    assert!(matches!(
        session.reverse_step(&DebugScopeSelectorV1::GlobalWorkitem([0, 0, 0])),
        DebugNavigationV1::Stopped(_)
    ));
}

#[test]
fn breakpoint_hit_cache_tracks_forward_reverse_and_seek() {
    let run = run();
    let mut session = DebugSessionV1::new(run.transcript);
    session
        .add_breakpoint(DebugBreakpointV1 {
            id: 7,
            site: DebugSiteSelectorV1 {
                function_ordinal: Some(0),
                block: Some(fe2o3_kernel_ir::BlockId(0)),
                operation: Some(0),
                phase: Some(SimulationDebugCheckpointPhaseV1::BeforeOperation),
            },
            scope: DebugScopeSelectorV1::Dispatch,
            predicate: DebugPredicateV1::True,
            hit_condition: Some(DebugHitConditionV1::Equal(2)),
            enabled: true,
        })
        .unwrap();
    assert_eq!(session.breakpoint_hit_count(7), Some(0));
    assert!(matches!(
        session.continue_forward(),
        DebugNavigationV1::Stopped(stop)
            if stop.reason == fe2o3_kir_debugger::DebugStopReasonV1::Breakpoint(7)
    ));
    assert_eq!(session.breakpoint_hit_count(7), Some(2));
    let second_hit = session.cursor_record_index().unwrap();
    assert!(matches!(
        session.seek_record_index(0),
        DebugNavigationV1::Stopped(_)
    ));
    assert_eq!(session.breakpoint_hit_count(7), Some(1));
    assert!(matches!(session.seek_entry(), DebugNavigationV1::Beginning));
    assert_eq!(session.breakpoint_hit_count(7), Some(0));
    assert!(matches!(
        session.seek_record_index(second_hit),
        DebugNavigationV1::Stopped(_)
    ));
    assert_eq!(session.breakpoint_hit_count(7), Some(2));
}

#[test]
fn source_catalog_requires_the_exact_canonical_kir_identity() {
    let run = run();
    let identity = run.transcript.identity();
    let mut session = DebugSessionV1::new(run.transcript);
    let admitted = AdmittedSimulationModuleV1::admit(
        VerifiedCanonicalKernelIrV7::from_module(fill_module()).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    let wrong = DebugSourceCatalogV1::new(
        DebugKirIdentityV1 {
            digest: [9; 32],
            canonical_len: identity.canonical_len,
        },
        vec![],
        vec![],
    )
    .unwrap();
    assert!(session.bind_source_catalog(&admitted, wrong).is_err());

    let exact = DebugSourceCatalogV1::new(identity, vec![], vec![]).unwrap();
    session.bind_source_catalog(&admitted, exact).unwrap();
}

#[test]
fn source_catalog_distinguishes_absent_eliminated_and_many_to_one() {
    let run = run();
    let identity = run.transcript.identity();
    let site = run.transcript.records()[0].site;
    let file = [3; 32];
    let span = |byte_start, byte_end| DebugSourceSpanV1 {
        file,
        byte_start,
        byte_end,
        line: 1,
        column: u32::try_from(byte_start + 1).unwrap(),
    };
    let catalog = DebugSourceCatalogV1::new_with_eliminated(
        identity,
        vec![DebugSourceFileV1 {
            identity: file,
            byte_len: 64,
            display_path: "fixture.rs".to_owned(),
        }],
        vec![DebugSourceSiteV1 {
            site,
            spans: vec![span(0, 4), span(8, 12)],
        }],
        vec![span(16, 20)],
    )
    .unwrap();
    assert_eq!(
        catalog.resolve_site(site),
        DebugSourceResolutionV1::ManyToOne
    );
    assert!(matches!(
        catalog.resolve_source(file, 0, 4),
        DebugSourceResolutionV1::Resolved { site: actual, .. } if actual == site
    ));
    assert_eq!(
        catalog.resolve_source(file, 0, 12),
        DebugSourceResolutionV1::ManyToOne
    );
    assert_eq!(
        catalog.resolve_source(file, 16, 20),
        DebugSourceResolutionV1::Eliminated
    );
    assert_eq!(
        catalog.resolve_source(file, 24, 28),
        DebugSourceResolutionV1::Absent
    );

    assert!(
        DebugSourceCatalogV1::new_with_eliminated(
            identity,
            vec![DebugSourceFileV1 {
                identity: file,
                byte_len: 64,
                display_path: "fixture.rs".to_owned(),
            }],
            vec![DebugSourceSiteV1 {
                site,
                spans: vec![span(0, 4), span(0, 4)],
            }],
            vec![],
        )
        .is_err()
    );
}

#[test]
fn helper_frames_barriers_workgroups_and_partial_waves_share_one_replay() {
    let verified = VerifiedCanonicalKernelIrV7::from_module(helper_barrier_module()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit(verified, SimulationLimitsV1::default()).unwrap();
    let request = SimulationRequestV1::new("helper_barrier", [5, 1, 1], [4, 1, 1], vec![]);
    let run = capture_debugger_run_v1(
        &admitted,
        &request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        SimulationDebugCaptureLimitsV1::new(8, 64, 8, 1_024).unwrap(),
        DebuggerLimitsV1::new(1_024, 4_096, 1 << 20).unwrap(),
        DebugWaveWidthV1::Wave32,
    );
    assert!(run.execution.is_ok());
    let arrivals = run
        .transcript
        .records()
        .iter()
        .filter(|record| {
            matches!(
                record.kind,
                SimulationDebugRecordKindV1::WorkgroupBarrier {
                    action: fe2o3_kir_sim::SimulationDebugBarrierActionV1::Arrive,
                    participants: 1,
                    ..
                }
            )
        })
        .count();
    let releases: Vec<_> = run
        .transcript
        .records()
        .iter()
        .filter_map(|record| match record.kind {
            SimulationDebugRecordKindV1::WorkgroupBarrier {
                action: fe2o3_kir_sim::SimulationDebugBarrierActionV1::Release,
                participants,
                ..
            } => Some((record.invocation.workgroup, participants)),
            _ => None,
        })
        .collect();
    assert_eq!(arrivals, 5);
    assert_eq!(releases, vec![([0, 0, 0], 4), ([1, 0, 0], 1)]);

    let first_group = run
        .transcript
        .records()
        .iter()
        .find(|record| record.invocation.workgroup == [0, 0, 0])
        .unwrap();
    let tail_group = run
        .transcript
        .records()
        .iter()
        .find(|record| record.invocation.workgroup == [1, 0, 0])
        .unwrap();
    assert_eq!(
        fe2o3_kir_debugger::hierarchy_for_invocation_v1(
            first_group.invocation,
            DebugWaveWidthV1::Wave32,
        )
        .active_mask,
        0b1111
    );
    assert_eq!(
        fe2o3_kir_debugger::hierarchy_for_invocation_v1(
            tail_group.invocation,
            DebugWaveWidthV1::Wave32,
        )
        .active_mask,
        0b1
    );

    let mut over = DebugSessionV1::new(run.transcript.clone());
    assert!(matches!(
        over.forward_step(&DebugScopeSelectorV1::GlobalWorkitem([0, 0, 0])),
        DebugNavigationV1::Stopped(_)
    ));
    assert!(matches!(
        over.step_over(&DebugScopeSelectorV1::GlobalWorkitem([0, 0, 0])),
        DebugNavigationV1::Stopped(_)
    ));
    assert!(matches!(
        &over.current().unwrap().kind,
        SimulationDebugRecordKindV1::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
            ..
        }
    ));
    assert_eq!(over.current().unwrap().site.function_ordinal, 0);

    let mut out = DebugSessionV1::new(run.transcript);
    assert!(matches!(
        out.forward_step(&DebugScopeSelectorV1::GlobalWorkitem([0, 0, 0])),
        DebugNavigationV1::Stopped(_)
    ));
    assert!(matches!(
        out.forward_step(&DebugScopeSelectorV1::GlobalWorkitem([0, 0, 0])),
        DebugNavigationV1::Stopped(_)
    ));
    assert_eq!(out.current().unwrap().site.function_ordinal, 1);
    assert!(matches!(
        out.step_out(&DebugScopeSelectorV1::GlobalWorkitem([0, 0, 0])),
        DebugNavigationV1::Stopped(_)
    ));
    assert_eq!(out.current().unwrap().site.function_ordinal, 0);
}
