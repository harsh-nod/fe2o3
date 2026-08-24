use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Constant, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId, VerifiedCanonicalKernelIrV7,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, IndexWidthV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationSiteV1, SimulationTargetV1,
};
use fe2o3_kir_sim_trace::{
    KirSiteCatalogV1, SimulationTraceProfileV1, simulate_with_semantic_trace_v1,
};
use fe2o3_semantic_trace::{
    AddressSpaceV1, AllocationEventV1, CaptureEndBoundaryV1, DiagnosticKindV1, DispatchEventV1,
    DispatchOutcomeV1, ExecutionLevelV1, KirSitePointV1, OperationEventV1, TraceBoundsV1,
    TraceCompletenessV1, TraceEventKindV1, WaveWidthV1, decode_trace_v1, encode_trace_v1,
};

fn op(result: u32, value: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(value)),
    )
}

fn domain(rank: u8) -> LaunchDomain {
    match rank {
        1 => LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
        2 => LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
        },
        3 => LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Dynamic,
        },
        _ => unreachable!(),
    }
}

fn module_with_blocks(rank: u8, blocks: Vec<BasicBlock>) -> Module {
    let entry = Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], blocks);
    let mut module = Module::new("trace-test");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("kernel", "entry", domain(rank)));
    module
}

fn simple_module(rank: u8) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(op(0, 7));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module_with_blocks(rank, vec![block])
}

fn empty_buffer_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("trace-empty-buffer");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("kernel", "entry", domain(1)));
    module
}

fn zero_count_alloca_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), Type::INDEX),
        OperationKind::Constant(Constant::Index(0)),
    ));
    block.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(1),
            Type::pointer(
                Type::Scalar(ScalarType::U8),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U8),
            count: Some(ValueId(0)),
            address_space: AddressSpace::Private,
            alignment: 1,
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module_with_blocks(1, vec![block])
}

fn admitted(module: Module) -> AdmittedSimulationModuleV1 {
    let canonical = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
    AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default()).unwrap()
}

fn profile(wave_width: WaveWidthV1, max_events: u64) -> SimulationTraceProfileV1 {
    SimulationTraceProfileV1 {
        wave_width,
        bounds: TraceBoundsV1::new(max_events, 4 * 1024 * 1024, 1).unwrap(),
        dispatch_occurrence: fe2o3_semantic_trace::OpaqueIdentityV1::new([7; 32]).unwrap(),
    }
}

fn run(
    module: &AdmittedSimulationModuleV1,
    grid: [u64; 3],
    workgroup: [u32; 3],
    wave: WaveWidthV1,
    max_events: u64,
) -> fe2o3_kir_sim_trace::TracedSimulationOutcomeV1 {
    simulate_with_semantic_trace_v1(
        module,
        &SimulationRequestV1::new("kernel", grid, workgroup, vec![]),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        profile(wave, max_events),
    )
    .unwrap()
}

#[test]
fn exact_claim_tail_mask_and_codec_are_deterministic() {
    let module = admitted(simple_module(1));
    let first = run(&module, [33, 1, 1], [32, 1, 1], WaveWidthV1::Wave32, 10_000);
    let second = run(&module, [33, 1, 1], [32, 1, 1], WaveWidthV1::Wave32, 10_000);
    assert!(first.execution.is_ok());
    assert_eq!(
        first.trace.header().kernel_ir_claim().digest().as_bytes(),
        module.identity().digest()
    );
    assert_eq!(
        first.trace.header().kernel_ir_claim().canonical_len(),
        module.identity().canonical_length()
    );
    assert_eq!(
        first.trace.header().dispatch(),
        second.trace.header().dispatch()
    );
    let tail = first
        .trace
        .events()
        .iter()
        .find_map(|event| match event.scope().level() {
            ExecutionLevelV1::Lane {
                logical_workitem: [32, 0, 0],
                active_mask,
                ..
            } => Some(active_mask.bits()),
            _ => None,
        })
        .unwrap();
    assert_eq!(tail, 1);
    let encoded = encode_trace_v1(&first.trace).unwrap();
    assert_eq!(decode_trace_v1(&encoded).unwrap(), first.trace);
}

#[test]
fn d1_d2_d3_launches_map_to_canonical_workgroup_geometry() {
    for (rank, grid, workgroup, expected) in [
        (1, [5, 1, 1], [4, 1, 1], [2, 1, 1]),
        (2, [5, 3, 1], [4, 2, 1], [2, 2, 1]),
        (3, [5, 3, 2], [4, 2, 1], [2, 2, 2]),
    ] {
        let outcome = run(
            &admitted(simple_module(rank)),
            grid,
            workgroup,
            WaveWidthV1::Wave64,
            10_000,
        );
        assert!(outcome.execution.is_ok());
        assert_eq!(outcome.trace.header().launch().grid_workgroups(), expected);
    }
}

#[test]
fn block_and_chosen_branch_bind_to_catalog_ordinals() {
    let mut first = BasicBlock::new(BlockId(7));
    first.terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![],
    });
    let mut second = BasicBlock::new(BlockId(9));
    second.terminator = Some(Terminator::Return { values: vec![] });
    let outcome = run(
        &admitted(module_with_blocks(1, vec![first, second])),
        [1, 1, 1],
        [1, 1, 1],
        WaveWidthV1::Wave64,
        100,
    );
    let block_entries = outcome
        .trace
        .events()
        .iter()
        .filter(|event| matches!(event.kind(), TraceEventKindV1::BlockEnter))
        .map(|event| event.site().unwrap().block_ordinal())
        .collect::<Vec<_>>();
    assert_eq!(block_entries, vec![0, 1]);
    let target = outcome
        .trace
        .events()
        .iter()
        .find_map(|event| match event.kind() {
            TraceEventKindV1::Branch {
                target_block_ordinal,
            } => Some(target_block_ordinal),
            _ => None,
        })
        .unwrap();
    assert_eq!(target, 1);
}

#[test]
fn nested_call_is_closed_before_the_next_caller_operation() {
    let mut caller = BasicBlock::new(BlockId(0));
    caller.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
            OperationKind::Call {
                callee: "helper".into(),
                arguments: vec![],
            },
        ),
        op(1, 9),
    ];
    caller.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![caller],
    );
    let mut helper = BasicBlock::new(BlockId(0));
    helper.operations.push(op(0, 3));
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let helper = Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
        vec![],
        vec![helper],
    );
    let mut module = Module::new("calls");
    module.functions = vec![entry, helper];
    module
        .kernels
        .push(Kernel::new("kernel", "entry", domain(1)));
    let outcome = run(
        &admitted(module),
        [1, 1, 1],
        [1, 1, 1],
        WaveWidthV1::Wave64,
        100,
    );
    let caller_events = outcome
        .trace
        .events()
        .iter()
        .filter_map(|event| {
            let site = event.site()?;
            (site.function_ordinal() == 0).then_some((event.sequence(), site.point(), event.kind()))
        })
        .collect::<Vec<_>>();
    let call_end = caller_events
        .iter()
        .find(|(_, point, kind)| {
            *point == KirSitePointV1::Operation(0)
                && matches!(kind, TraceEventKindV1::Operation(OperationEventV1::End(_)))
        })
        .unwrap()
        .0;
    let next_begin = caller_events
        .iter()
        .find(|(_, point, kind)| {
            *point == KirSitePointV1::Operation(1)
                && matches!(
                    kind,
                    TraceEventKindV1::Operation(OperationEventV1::Begin(_))
                )
        })
        .unwrap()
        .0;
    assert!(call_end < next_begin);
    let begins = outcome
        .trace
        .events()
        .iter()
        .filter_map(|event| match (event.site()?, event.kind()) {
            (site, TraceEventKindV1::Operation(OperationEventV1::Begin(dynamic))) => {
                Some((site.function_ordinal(), site.point(), dynamic))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let caller_call = begins
        .iter()
        .find(|(function, point, _)| *function == 0 && *point == KirSitePointV1::Operation(0))
        .unwrap()
        .2;
    let helper = begins
        .iter()
        .find(|(function, _, _)| *function == 1)
        .unwrap()
        .2;
    let caller_next = begins
        .iter()
        .find(|(function, point, _)| *function == 0 && *point == KirSitePointV1::Operation(1))
        .unwrap()
        .2;
    assert_eq!(caller_call.frame(), caller_next.frame());
    assert_ne!(caller_call.frame(), helper.frame());
    assert_ne!(caller_call.occurrence(), helper.occurrence());
}

#[test]
fn dynamic_failure_has_exact_diagnostic_and_failed_dispatch_end() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Unreachable);
    let outcome = run(
        &admitted(module_with_blocks(1, vec![block])),
        [1, 1, 1],
        [1, 1, 1],
        WaveWidthV1::Wave64,
        100,
    );
    assert!(outcome.execution.is_err());
    assert!(outcome.trace.events().iter().any(|event| matches!(
        event.kind(),
        TraceEventKindV1::Diagnostic(diagnostic) if diagnostic.kind() == DiagnosticKindV1::Fault
    )));
    assert!(outcome.trace.events().iter().any(|event| {
        event.kind() == TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Failed))
    }));
}

#[test]
fn tiny_trace_limit_is_nonfatal_and_marks_a_valid_prefix() {
    let module = admitted(simple_module(1));
    let outcome = run(&module, [128, 1, 1], [1, 1, 1], WaveWidthV1::Wave64, 2);
    assert!(outcome.execution.is_ok());
    assert_eq!(outcome.trace.events().len(), 2);
    assert!(matches!(
        outcome.trace.header().completeness(),
        TraceCompletenessV1::Truncated {
            reason: fe2o3_semantic_trace::TruncationReasonV1::EventLimit,
            ..
        }
    ));
    assert_eq!(
        outcome.trace.header().boundaries().end(),
        CaptureEndBoundaryV1::DispatchEndIncluded
    );
    let untraced = module
        .simulate(
            &SimulationRequestV1::new("kernel", [128, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    let traced = outcome.execution.unwrap();
    assert_eq!(traced.identity(), untraced.identity());
    assert_eq!(traced.arguments(), untraced.arguments());
    assert_eq!(traced.shared_buffers(), untraced.shared_buffers());
    assert_eq!(
        traced.invocations_executed(),
        untraced.invocations_executed()
    );
    assert_eq!(traced.workgroups_visited(), untraced.workgroups_visited());
    assert_eq!(
        traced.scheduled_slots_visited(),
        untraced.scheduled_slots_visited()
    );
    assert_eq!(traced.steps_executed(), untraced.steps_executed());
    assert_eq!(traced.schedule(), untraced.schedule());
    assert_eq!(traced.conflict_assessment(), untraced.conflict_assessment());
    assert_eq!(traced.events_emitted(), 0);
    assert_eq!(untraced.events_emitted(), 0);
    assert!(encode_trace_v1(&outcome.trace).is_ok());
}

#[test]
fn small_byte_budget_does_not_preallocate_declared_event_limit() {
    let module = admitted(simple_module(1));
    let outcome = simulate_with_semantic_trace_v1(
        &module,
        &SimulationRequestV1::new("kernel", [1, 1, 1], [1, 1, 1], vec![]),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        SimulationTraceProfileV1 {
            wave_width: WaveWidthV1::Wave64,
            bounds: TraceBoundsV1::new(500_000, 4 * 1_024, 1).unwrap(),
            dispatch_occurrence: fe2o3_semantic_trace::OpaqueIdentityV1::new([8; 32]).unwrap(),
        },
    )
    .unwrap();
    assert!(outcome.trace.events().len() < 500_000);
    assert!(encode_trace_v1(&outcome.trace).is_ok());
}

#[test]
fn collector_growth_is_geometric_under_many_invocations() {
    let module = admitted(simple_module(1));
    let outcome = simulate_with_semantic_trace_v1(
        &module,
        &SimulationRequestV1::new("kernel", [2_048, 1, 1], [1, 1, 1], vec![]),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        SimulationTraceProfileV1 {
            wave_width: WaveWidthV1::Wave64,
            bounds: TraceBoundsV1::new(20_000, 16 * 1_024 * 1_024, 1).unwrap(),
            dispatch_occurrence: fe2o3_semantic_trace::OpaqueIdentityV1::new([10; 32]).unwrap(),
        },
    )
    .unwrap();
    let execution = outcome.execution.as_ref().unwrap();
    assert_eq!(execution.invocations_executed(), 2_048);
    assert_eq!(execution.events_emitted(), 7 * 2_048);
    assert!(outcome.trace.events().len() > 10_000);
    assert!(matches!(
        outcome.trace.header().completeness(),
        TraceCompletenessV1::Complete
    ));
    assert!(encode_trace_v1(&outcome.trace).is_ok());
}

#[test]
fn zero_length_preexisting_and_private_allocations_round_trip() {
    let empty_buffer = BufferArgumentV1::new(
        ScalarType::U8,
        AccessMode::ReadWrite,
        1,
        vec![],
        vec![],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let global = simulate_with_semantic_trace_v1(
        &admitted(empty_buffer_module()),
        &SimulationRequestV1::new(
            "kernel",
            [1, 1, 1],
            [1, 1, 1],
            vec![SimulationArgumentV1::Buffer(empty_buffer)],
        ),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        profile(WaveWidthV1::Wave64, 100),
    )
    .unwrap();
    assert!(global.trace.events().iter().any(|event| matches!(
        event.kind(),
        TraceEventKindV1::Allocation(AllocationEventV1::Preexisting {
            byte_len: 0,
            address_space: AddressSpaceV1::Global,
            ..
        })
    )));
    assert_eq!(
        decode_trace_v1(&encode_trace_v1(&global.trace).unwrap()).unwrap(),
        global.trace
    );

    let private = run(
        &admitted(zero_count_alloca_module()),
        [1, 1, 1],
        [1, 1, 1],
        WaveWidthV1::Wave64,
        100,
    );
    let created = private
        .trace
        .events()
        .iter()
        .find_map(|event| match event.kind() {
            TraceEventKindV1::Allocation(AllocationEventV1::Create {
                allocation,
                byte_len: 0,
                address_space: AddressSpaceV1::Private,
            }) => Some(allocation),
            _ => None,
        });
    let created = created.expect("zero-byte private allocation was observed");
    assert!(private.trace.events().iter().any(|event| {
        event.kind()
            == TraceEventKindV1::Allocation(AllocationEventV1::Release {
                allocation: created,
            })
    }));
    assert_eq!(
        decode_trace_v1(&encode_trace_v1(&private.trace).unwrap()).unwrap(),
        private.trace
    );
}

#[test]
fn hard_simulation_limits_do_not_drive_trace_preallocation() {
    let limits = SimulationLimitsV1 {
        max_call_depth: 1_024,
        max_allocations: 1 << 20,
        max_resident_bytes: 1 << 34,
        ..SimulationLimitsV1::default()
    };
    let outcome = simulate_with_semantic_trace_v1(
        &admitted(simple_module(1)),
        &SimulationRequestV1::new("kernel", [1, 1, 1], [1, 1, 1], vec![]),
        SimulationTargetV1::amdgpu_64(),
        limits,
        SimulationTraceProfileV1 {
            wave_width: WaveWidthV1::Wave64,
            bounds: TraceBoundsV1::new_with_resident(2, 4 * 1_024, 64 * 1_024, 1).unwrap(),
            dispatch_occurrence: fe2o3_semantic_trace::OpaqueIdentityV1::new([10; 32]).unwrap(),
        },
    )
    .unwrap();
    assert!(outcome.execution.is_ok());
    assert_eq!(outcome.trace.events().len(), 2);
}

#[test]
fn large_reverse_named_catalog_resolves_by_sorted_index() {
    let mut module = simple_module(1);
    for index in (0..2_048).rev() {
        module.functions.push(Function::declaration(
            format!("declaration_{index:04}"),
            Signature::new(vec![], vec![]),
        ));
    }
    let admitted = admitted(module);
    let catalog = KirSiteCatalogV1::from_admitted(&admitted).unwrap();
    let site = SimulationSiteV1 {
        function: "entry".into(),
        block: BlockId(0),
        operation: Some(0),
    };
    for _ in 0..8_192 {
        assert_eq!(
            catalog.claim(&site).unwrap().point(),
            KirSitePointV1::Operation(0)
        );
    }
}

#[test]
fn zero_workgroup_axes_are_typed_adapter_errors() {
    let module = admitted(simple_module(1));
    for axis in 0..3 {
        let mut workgroup = [1, 1, 1];
        workgroup[axis] = 0;
        let error = simulate_with_semantic_trace_v1(
            &module,
            &SimulationRequestV1::new("kernel", [1, 1, 1], workgroup, vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            profile(WaveWidthV1::Wave64, 100),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            fe2o3_kir_sim_trace::TraceAdapterErrorV1::ZeroWorkgroupDimension { axis: actual }
                if actual == axis
        ));
    }
}

#[test]
fn exact_byte_boundary_preserves_cause_and_dispatch_footer() {
    let module = admitted(simple_module(1));
    let request = SimulationRequestV1::new("kernel", [4, 1, 1], [1, 1, 1], vec![]);
    let mut selected = None;
    for byte_limit in 256..2_048 {
        let bounds = TraceBoundsV1::new(100, byte_limit, 1).unwrap();
        let result = simulate_with_semantic_trace_v1(
            &module,
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationTraceProfileV1 {
                wave_width: WaveWidthV1::Wave64,
                bounds,
                dispatch_occurrence: fe2o3_semantic_trace::OpaqueIdentityV1::new([9; 32]).unwrap(),
            },
        );
        if let Ok(outcome) = result
            && matches!(
                outcome.trace.header().completeness(),
                TraceCompletenessV1::Truncated {
                    reason: fe2o3_semantic_trace::TruncationReasonV1::ByteLimit,
                    ..
                }
            )
        {
            selected = Some((byte_limit, outcome));
            break;
        }
    }
    let (byte_limit, outcome) = selected.expect("a bounded byte-truncated trace exists");
    let encoded = encode_trace_v1(&outcome.trace).unwrap();
    assert!(encoded.len() as u64 <= byte_limit);
    assert_eq!(
        outcome.trace.header().boundaries().end(),
        CaptureEndBoundaryV1::DispatchEndIncluded
    );
    assert!(matches!(
        outcome.trace.events().last().unwrap().kind(),
        TraceEventKindV1::Dispatch(DispatchEventV1::End(_))
    ));
    assert_eq!(outcome.trace.events().len(), 2);
    assert_eq!(outcome.execution.unwrap().events_emitted(), 0);
}

#[test]
fn configuration_digest_binds_target_and_values_but_not_wave_visualization() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut raw = Module::new("configuration");
    raw.functions.push(entry);
    raw.kernels.push(Kernel::new("kernel", "entry", domain(1)));
    let module = admitted(raw);
    let make_request = |value| {
        SimulationRequestV1::new(
            "kernel",
            [1, 1, 1],
            [1, 1, 1],
            vec![SimulationArgumentV1::Scalar(ScalarBitsV1::u32(value))],
        )
    };
    let run_profile = |target, wave, value| {
        simulate_with_semantic_trace_v1(
            &module,
            &make_request(value),
            target,
            SimulationLimitsV1::default(),
            profile(wave, 100),
        )
        .unwrap()
    };
    let baseline = run_profile(SimulationTargetV1::amdgpu_64(), WaveWidthV1::Wave32, 7);
    let wave64 = run_profile(SimulationTargetV1::amdgpu_64(), WaveWidthV1::Wave64, 7);
    let changed_value = run_profile(SimulationTargetV1::amdgpu_64(), WaveWidthV1::Wave32, 8);
    let changed_target = run_profile(
        SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
        WaveWidthV1::Wave32,
        7,
    );
    assert_eq!(
        baseline.configuration_identity,
        wave64.configuration_identity
    );
    assert_ne!(
        baseline.configuration_identity,
        changed_value.configuration_identity
    );
    assert_ne!(
        baseline.configuration_identity,
        changed_target.configuration_identity
    );
    assert_eq!(
        baseline.trace.header().dispatch(),
        wave64.trace.header().dispatch()
    );
}
