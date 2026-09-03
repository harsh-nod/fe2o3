use std::mem::size_of;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Atomic, AtomicKind, Axis, BarrierSemantics, BasicBlock, BinaryOp,
    BlockId, CheckedBinaryOperator, ComparePredicate, Constant, Convergence, DiagnosticCode, Fence,
    Function, IndexKind, IntegerSwitchCase, IntrinsicKind, IntrinsicOperation, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, MemoryOrdering, Module, Operation, OperationKind,
    ScalarType, Signature, SwitchCase, SynchronizationScope, TargetCapability, Terminator, Type,
    UnaryOp, ValueDef, ValueId, VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV9,
    VerifiedCanonicalKernelIrV10, WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier,
    WorkgroupMemory, WorkgroupMemoryExtent, WorkgroupSize, verify_module,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    DynamicWorkgroupMemoryRequestV1, DynamicWorkgroupMemoryUnavailableV1, EventPolicyV1,
    MAX_REPORTED_UNSUPPORTED_FINDINGS_V1, MAX_REPORTED_UNSUPPORTED_IDENTIFIER_BYTES_V1,
    MAX_SCHEDULE_DECISIONS_V1, PersistedSimulationScheduleArtifactV1,
    PersistedSimulationScheduleBindingV1, PersistedSimulationScheduleCodecErrorV1,
    PersistedSimulationScheduleDocumentV1, ScalarBitsV1, SharedBufferV1,
    SimulationAdmissionErrorV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationDebugCaptureLimitsV1, SimulationDebugMemoryAccessV1, SimulationDebugRecordKindV1,
    SimulationDebugRecordV1, SimulationDebugSinkControlV1, SimulationDebugSinkV1,
    SimulationErrorV1, SimulationEventKindV1, SimulationEventSinkControlV1, SimulationEventSinkV1,
    SimulationEventV1, SimulationExecutionErrorKindV1, SimulationExecutionOutcomeV1,
    SimulationExplorationRequestV1, SimulationExplorationV1, SimulationFailureReductionLimitsV1,
    SimulationFailureReductionReportV1, SimulationFailureScheduleV1, SimulationLimitsV1,
    SimulationOutOfBoundsV2, SimulationPreflightErrorV1, SimulationRaceAssessmentV1,
    SimulationRequestV1, SimulationScheduleDecisionV1, SimulationScheduleIdentityV1,
    SimulationScheduleReplayErrorV1, SimulationScheduleRequestV1, SimulationTargetV1,
    UnsupportedFeatureV1,
};

fn reduction_limits(max_decisions: usize) -> SimulationFailureReductionLimitsV1 {
    SimulationFailureReductionLimitsV1::new(max_decisions + 2, max_decisions, max_decisions * 3)
        .unwrap()
}

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn atomic_scalar_buffer(
    value: ScalarBitsV1,
    initialized: bool,
    alignment: u32,
) -> BufferArgumentV1 {
    let width = usize::from(value.ty().bit_width().unwrap().div_ceil(8));
    BufferArgumentV1::new(
        value.ty(),
        AccessMode::ReadWrite,
        alignment,
        value.bits().to_le_bytes()[..width].to_vec(),
        vec![initialized; width],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap()
}

fn admitted(module: Module) -> AdmittedSimulationModuleV1 {
    let canonical = VerifiedCanonicalKernelIrV7::from_module(module).expect("verified fixture");
    AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default())
        .expect("admitted fixture")
}

fn dynamic_domain_1d() -> LaunchDomain {
    LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    }
}

fn u32_buffer(values: &[u32]) -> BufferArgumentV1 {
    let scalars = values
        .iter()
        .copied()
        .map(ScalarBitsV1::u32)
        .collect::<Vec<_>>();
    BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &scalars,
        SimulationTargetV1::amdgpu_64(),
    )
    .expect("u32 buffer")
}

fn i32_buffer(values: &[i32]) -> BufferArgumentV1 {
    let scalars = values
        .iter()
        .copied()
        .map(ScalarBitsV1::i32)
        .collect::<Vec<_>>();
    BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &scalars,
        SimulationTargetV1::amdgpu_64(),
    )
    .expect("i32 buffer")
}

fn byte_buffer(values: &[u8]) -> BufferArgumentV1 {
    BufferArgumentV1::new(
        ScalarType::U8,
        AccessMode::ReadWrite,
        1,
        values.to_vec(),
        vec![true; values.len()],
        SimulationTargetV1::amdgpu_64(),
    )
    .expect("u8 buffer")
}

fn u64_buffer(values: &[u64]) -> BufferArgumentV1 {
    let scalars = values
        .iter()
        .copied()
        .map(|value| {
            ScalarBitsV1::new(
                ScalarType::U64,
                u128::from(value),
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        8,
        &scalars,
        SimulationTargetV1::amdgpu_64(),
    )
    .expect("u64 buffer")
}

fn bool_buffer(values: &[bool]) -> BufferArgumentV1 {
    let scalars = values
        .iter()
        .copied()
        .map(ScalarBitsV1::boolean)
        .collect::<Vec<_>>();
    BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        1,
        &scalars,
        SimulationTargetV1::amdgpu_64(),
    )
    .expect("bool buffer")
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four bytes")))
        .collect()
}

#[derive(Default)]
struct Collector(Vec<SimulationEventV1>);

#[derive(Default)]
struct DebugCollector(
    Vec<SimulationDebugRecordV1>,
    Option<SimulationOutOfBoundsV2>,
);

impl SimulationDebugSinkV1 for DebugCollector {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        self.0.push(record);
        SimulationDebugSinkControlV1::Continue
    }

    fn terminal_out_of_bounds_v2(&mut self, detail: SimulationOutOfBoundsV2) {
        self.1 = Some(detail);
    }
}

impl SimulationEventSinkV1 for Collector {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        self.0.push(event.clone());
        Ok(())
    }
}

fn offset_helper() -> Function {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(1, Type::INDEX, OperationKind::Constant(Constant::Index(5))),
        op(
            2,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        op(
            3,
            Type::Scalar(ScalarType::U64),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Bitcast,
                value: ValueId(0),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
        op(
            4,
            Type::Scalar(ScalarType::U32),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Truncate,
                value: ValueId(3),
                to: Type::Scalar(ScalarType::U32),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(4)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(4)],
    });

    let mut below = BasicBlock::new(BlockId(1));
    below.parameters = vec![ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U32))];
    below.operations = vec![
        op(
            11,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(100)),
        ),
        op(
            12,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        ),
    ];
    below.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(12)],
    });

    let mut above = BasicBlock::new(BlockId(2));
    above.parameters = vec![ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32))];
    above.operations = vec![
        op(
            21,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(200)),
        ),
        op(
            22,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(20),
                rhs: ValueId(21),
            },
        ),
    ];
    above.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(22)],
    });

    let mut merge = BasicBlock::new(BlockId(3));
    merge.parameters = vec![ValueDef::new(ValueId(30), Type::Scalar(ScalarType::U32))];
    merge.terminator = Some(Terminator::Return {
        values: vec![ValueId(30)],
    });

    Function::internal_helper(
        "offset",
        Signature::new(vec![Type::INDEX], vec![Type::Scalar(ScalarType::U32)]),
        vec![ValueId(0)],
        vec![entry, below, above, merge],
    )
}

fn hierarchy_module() -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let slice = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let intrinsic = |kind| OperationKind::Intrinsic(IntrinsicOperation::new(kind, Type::INDEX));
    let global = |axis| {
        intrinsic(IntrinsicKind::InvocationIndex {
            kind: IndexKind::Global,
            axis,
        })
    };

    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            1,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(2, Type::INDEX, global(Axis::X)),
        op(3, Type::INDEX, global(Axis::Y)),
        op(4, Type::INDEX, global(Axis::Z)),
        op(
            5,
            Type::INDEX,
            intrinsic(IntrinsicKind::LaunchExtent { axis: Axis::X }),
        ),
        op(
            6,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(3),
                rhs: ValueId(5),
            },
        ),
        op(
            7,
            Type::INDEX,
            intrinsic(IntrinsicKind::LaunchExtent { axis: Axis::Y }),
        ),
        op(
            8,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(5),
                rhs: ValueId(7),
            },
        ),
        op(
            9,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(4),
                rhs: ValueId(8),
            },
        ),
        op(
            10,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(6),
            },
        ),
        op(
            11,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(10),
                rhs: ValueId(9),
            },
        ),
        op(
            12,
            element.clone(),
            OperationKind::Call {
                callee: "offset".into(),
                arguments: vec![ValueId(11)],
            },
        ),
        op(
            13,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(11),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(13),
                value: ValueId(12),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "hierarchy_impl",
        Signature::new(vec![slice], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut kernel = Kernel::new(
        "hierarchy",
        "hierarchy_impl",
        LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(2, 2, 1));
    let mut module = Module::new("sim-tests::hierarchy");
    module.functions = vec![entry, offset_helper()];
    module.kernels.push(kernel);
    module
}

#[test]
fn observed_sink_needs_no_request_clone_and_preserves_canonical_event_order() {
    let admitted = admitted(hierarchy_module());
    let request = SimulationRequestV1::new(
        "hierarchy",
        [3, 2, 2],
        [2, 2, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0; 12]))],
    );
    let original = request.clone();
    let mut events = Collector::default();
    let execution = admitted
        .simulate_observed_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .expect("simulation succeeds");

    assert_eq!(
        request, original,
        "borrowed caller buffers are never mutated"
    );
    assert_eq!(execution.invocations_executed(), 12);
    assert_eq!(execution.workgroups_visited(), 4);
    assert_eq!(execution.scheduled_slots_visited(), 16);
    assert!(!execution.grants_execution_authority());
    assert_eq!(
        words(execution.buffer(0).expect("output buffer").bytes()),
        vec![100, 101, 102, 103, 104, 205, 206, 207, 208, 209, 210, 211]
    );

    let invocation_order = events
        .0
        .iter()
        .filter(|event| {
            event.site.function_ordinal == 0
                && event.site.operation == Some(0)
                && event.kind == SimulationEventKindV1::OperationBegin
        })
        .map(|event| event.invocation.global)
        .collect::<Vec<_>>();
    assert_eq!(
        invocation_order,
        vec![
            [0, 0, 0],
            [1, 0, 0],
            [0, 1, 0],
            [1, 1, 0],
            [2, 0, 0],
            [2, 1, 0],
            [0, 0, 1],
            [1, 0, 1],
            [0, 1, 1],
            [1, 1, 1],
            [2, 0, 1],
            [2, 1, 1],
        ]
    );
    assert_eq!(execution.events_emitted() as usize, events.0.len());
}

fn switch_helper(typed: bool) -> Function {
    let scalar = if typed {
        ScalarType::I32
    } else {
        ScalarType::U32
    };
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(if typed {
        Terminator::IntegerSwitch {
            selector: ValueId(0),
            cases: vec![
                IntegerSwitchCase {
                    value: Constant::I32(-1),
                    target: BlockId(1),
                    arguments: vec![],
                },
                IntegerSwitchCase {
                    value: Constant::I32(4),
                    target: BlockId(2),
                    arguments: vec![],
                },
            ],
            default_target: BlockId(3),
            default_arguments: vec![],
        }
    } else {
        Terminator::Switch {
            selector: ValueId(0),
            cases: vec![SwitchCase {
                value: 7,
                target: BlockId(1),
                arguments: vec![],
            }],
            default_target: BlockId(3),
            default_arguments: vec![],
        }
    });
    let constant = |block_id, value, result| {
        let mut block = BasicBlock::new(BlockId(block_id));
        block.operations.push(op(
            result,
            Type::Scalar(scalar),
            OperationKind::Constant(if typed {
                Constant::I32(value)
            } else {
                Constant::U32(value as u32)
            }),
        ));
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(result)],
        });
        block
    };
    Function::internal_helper(
        if typed {
            "typed_switch"
        } else {
            "legacy_switch"
        },
        Signature::new(vec![Type::Scalar(scalar)], vec![Type::Scalar(scalar)]),
        vec![ValueId(0)],
        vec![
            entry,
            constant(1, 70, 1),
            constant(2, 40, 2),
            constant(3, 90, 3),
        ],
    )
}

#[test]
fn executes_legacy_and_typed_integer_switches() {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let i32_ty = Type::Scalar(ScalarType::I32);
    let u32_pointer = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let i32_pointer = Type::pointer(i32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            4,
            u32_ty.clone(),
            OperationKind::Call {
                callee: "legacy_switch".into(),
                arguments: vec![ValueId(0)],
            },
        ),
        op(
            5,
            i32_ty.clone(),
            OperationKind::Call {
                callee: "typed_switch".into(),
                arguments: vec![ValueId(1)],
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "switch_impl",
        Signature::new(vec![u32_ty, i32_ty, u32_pointer, i32_pointer], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::switch");
    module.functions = vec![entry, switch_helper(false), switch_helper(true)];
    module
        .kernels
        .push(Kernel::new("switches", "switch_impl", dynamic_domain_1d()));

    let request = SimulationRequestV1::new(
        "switches",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::i32(-1)),
            SimulationArgumentV1::Buffer(u32_buffer(&[0])),
            SimulationArgumentV1::Buffer(i32_buffer(&[0])),
        ],
    );
    let execution = admitted(module)
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .expect("switch simulation");
    assert_eq!(words(execution.buffer(2).unwrap().bytes()), vec![70]);
    assert_eq!(words(execution.buffer(3).unwrap().bytes()), vec![70]);
}

#[test]
fn executes_checked_select_unary_and_private_memory_operations() {
    let u8_ty = Type::Scalar(ScalarType::U8);
    let global_pointer = Type::pointer(u8_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let private_pointer =
        Type::pointer(u8_ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(1, u8_ty.clone(), OperationKind::Constant(Constant::U8(250))),
        op(2, u8_ty.clone(), OperationKind::Constant(Constant::U8(10))),
        Operation::checked_binary(
            ValueDef::new(ValueId(3), u8_ty.clone()),
            ValueDef::new(ValueId(4), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(2),
        ),
        op(
            5,
            u8_ty.clone(),
            OperationKind::Select {
                condition: ValueId(4),
                true_value: ValueId(3),
                false_value: ValueId(2),
            },
        ),
        op(
            6,
            u8_ty.clone(),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(5),
            },
        ),
        op(
            7,
            private_pointer,
            OperationKind::Alloca {
                element: u8_ty.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 1,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(7),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 1),
            },
        ),
        op(
            8,
            u8_ty.clone(),
            OperationKind::Load {
                pointer: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Private, 1),
            },
        ),
        op(9, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(10, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            11,
            global_pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(9),
            },
        ),
        op(
            12,
            global_pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(10),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(11),
                value: ValueId(8),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(12),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "scalar_memory_impl",
        Signature::new(vec![global_pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::scalar-memory");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "scalar_memory",
        "scalar_memory_impl",
        dynamic_domain_1d(),
    ));
    let mut request = SimulationRequestV1::new(
        "scalar_memory",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(byte_buffer(&[0, 0]))],
    );
    request.events = EventPolicyV1::Enabled;
    let mut events = Collector::default();
    let execution = admitted(module)
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .expect("scalar/private-memory simulation");
    assert_eq!(execution.buffer(0).unwrap().bytes(), &[4, 251]);
    assert!(events.0.iter().any(|event| matches!(
        event.kind,
        SimulationEventKindV1::AllocationPreexisting {
            address_space: AddressSpace::Global,
            bytes: 2,
            ..
        }
    )));
    let created = events.0.iter().find_map(|event| match event.kind {
        SimulationEventKindV1::AllocationCreated {
            allocation,
            address_space: AddressSpace::Private,
            bytes: 1,
        } => Some(allocation),
        _ => None,
    });
    assert!(created.is_some());
    assert!(events.0.iter().any(|event| matches!(
        event.kind,
        SimulationEventKindV1::AllocationReleased { allocation }
            if Some(allocation) == created
    )));
    assert!(matches!(
        events.0.first().map(|event| &event.kind),
        Some(SimulationEventKindV1::InvocationBegin)
    ));
    assert!(matches!(
        events.0.last().map(|event| &event.kind),
        Some(SimulationEventKindV1::InvocationEnd {
            outcome: SimulationExecutionOutcomeV1::Completed
        })
    ));
}

#[test]
fn reports_partial_reachable_wave_at_execution() {
    let mut wave_block = BasicBlock::new(BlockId(0));
    wave_block.operations.push(op(
        0,
        Type::Scalar(ScalarType::U32),
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::LaneId,
            WaveWidth::Wave32,
        )),
    ));
    wave_block.terminator = Some(Terminator::Return { values: vec![] });
    let mut wave = Function::internal_helper(
        "reachable_wave",
        Signature::new(vec![], vec![]),
        vec![],
        vec![wave_block],
    );
    wave.required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave32));

    let mut entry_block = BasicBlock::new(BlockId(0));
    entry_block.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: "reachable_wave".into(),
            arguments: vec![],
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "unsupported_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry_block],
    );
    let mut module = Module::new("sim-tests::unsupported");
    module.functions = vec![entry, wave];
    module.kernels.push(Kernel::new(
        "unsupported",
        "unsupported_impl",
        dynamic_domain_1d(),
    ));

    let error = admitted(module)
        .simulate(
            &SimulationRequestV1::new("unsupported", [1, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .expect_err("partial wave must fail at execution");
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::IncompleteWave(_),
            ..
        })
    ));
}

fn wave_capabilities(width: WaveWidth) -> std::collections::BTreeSet<TargetCapability> {
    std::collections::BTreeSet::from([
        TargetCapability::Subgroups,
        TargetCapability::SubgroupSize(width.lanes()),
        TargetCapability::WaveWidth(width),
    ])
}

fn wave_collective_module(width: WaveWidth) -> Module {
    let ballot_ty = match width {
        WaveWidth::Wave32 => ScalarType::U32,
        WaveWidth::Wave64 => ScalarType::U64,
    };
    let pointer = |scalar| {
        Type::pointer(
            Type::Scalar(scalar),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            5,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            6,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(7, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            8,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(6),
                rhs: ValueId(7),
            },
        ),
        op(
            9,
            Type::Scalar(ScalarType::U32),
            OperationKind::Wave(WaveOperation::full(WaveOperationKind::LaneId, width)),
        ),
        op(
            10,
            Type::Scalar(ballot_ty),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::Ballot {
                    predicate: ValueId(8),
                },
                width,
            )),
        ),
        op(
            11,
            Type::BOOL,
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::Any {
                    predicate: ValueId(8),
                },
                width,
            )),
        ),
        op(
            12,
            Type::BOOL,
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::All {
                    predicate: ValueId(8),
                },
                width,
            )),
        ),
        op(
            13,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(0)),
        ),
        op(
            14,
            Type::Scalar(ScalarType::U32),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ShuffleIndex {
                    value: ValueId(9),
                    source_lane: ValueId(13),
                    tile_width: 8,
                },
                width,
            )),
        ),
    ];
    for (base, value) in [(0_u32, 9_u32), (1, 10), (2, 11), (3, 12), (4, 14)] {
        let pointer_id = ValueId(15 + base);
        block.operations.push(op(
            pointer_id.0,
            match base {
                1 => pointer(ballot_ty),
                2 | 3 => pointer(ScalarType::Bool),
                _ => pointer(ScalarType::U32),
            },
            OperationKind::GetElementPointer {
                base: ValueId(base),
                offset: ValueId(5),
            },
        ));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: pointer_id,
                value: ValueId(value),
                access: MemoryAccess::new(
                    AddressSpace::Global,
                    match base {
                        1 if width == WaveWidth::Wave64 => 8,
                        2 | 3 => 1,
                        _ => 4,
                    },
                ),
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "wave_collectives_impl",
        Signature::new(
            vec![
                pointer(ScalarType::U32),
                pointer(ballot_ty),
                pointer(ScalarType::Bool),
                pointer(ScalarType::Bool),
                pointer(ScalarType::U32),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
        vec![block],
    );
    function.required_capabilities = wave_capabilities(width);
    let mut kernel = Kernel::new(
        "wave_collectives",
        "wave_collectives_impl",
        dynamic_domain_1d(),
    );
    kernel.required_capabilities = wave_capabilities(width);
    let mut module = Module::new("sim-tests::wave-collectives");
    module.required_capabilities = wave_capabilities(width);
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn wave_collective_request(lanes: usize, width: WaveWidth) -> SimulationRequestV1 {
    SimulationRequestV1::new(
        "wave_collectives",
        [lanes as u64, 1, 1],
        [lanes as u32, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(u32_buffer(&vec![0; lanes])),
            SimulationArgumentV1::Buffer(match width {
                WaveWidth::Wave32 => u32_buffer(&vec![0; lanes]),
                WaveWidth::Wave64 => u64_buffer(&vec![0; lanes]),
            }),
            SimulationArgumentV1::Buffer(bool_buffer(&vec![false; lanes])),
            SimulationArgumentV1::Buffer(bool_buffer(&vec![false; lanes])),
            SimulationArgumentV1::Buffer(u32_buffer(&vec![0; lanes])),
        ],
    )
}

#[test]
fn wave32_and_wave64_collectives_are_exact_and_replayable() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        let lanes = width.lanes() as usize;
        let module = admitted(wave_collective_module(width));
        let request = wave_collective_request(lanes, width);
        let execution = module
            .simulate_scheduled(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::RecordSeeded {
                    seed: 0x5eed,
                    max_decisions: 1024,
                },
            )
            .expect("full logical wave");
        assert_eq!(
            words(execution.buffer(0).unwrap().bytes()),
            (0..lanes as u32).collect::<Vec<_>>()
        );
        match width {
            WaveWidth::Wave32 => assert!(
                words(execution.buffer(1).unwrap().bytes())
                    .iter()
                    .all(|value| *value == 1)
            ),
            WaveWidth::Wave64 => assert!(
                execution
                    .buffer(1)
                    .unwrap()
                    .bytes()
                    .chunks_exact(8)
                    .all(|bytes| u64::from_le_bytes(bytes.try_into().unwrap()) == 1)
            ),
        }
        assert!(
            execution
                .buffer(2)
                .unwrap()
                .bytes()
                .iter()
                .all(|value| *value == 1)
        );
        assert!(
            execution
                .buffer(3)
                .unwrap()
                .bytes()
                .iter()
                .all(|value| *value == 0)
        );
        assert_eq!(
            words(execution.buffer(4).unwrap().bytes()),
            (0..lanes as u32)
                .map(|lane| lane / 8 * 8)
                .collect::<Vec<_>>()
        );
        let record = execution.schedule_record().unwrap().clone();
        let replay = module
            .simulate_scheduled(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::Replay(&record),
            )
            .expect("wave replay");
        assert_eq!(replay.arguments(), execution.arguments());
        let substituted = wave_collective_request(lanes - 1, width);
        assert!(matches!(
            module.simulate_scheduled(
                &substituted,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::Replay(&record)
            ),
            Err(SimulationErrorV1::Execution(
                fe2o3_kir_sim::SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::ScheduleReplay(
                        SimulationScheduleReplayErrorV1::ContextMismatch
                    ),
                    ..
                }
            ))
        ));
    }
}

#[test]
fn partial_wave_collective_reports_exact_active_mask() {
    let module = admitted(wave_collective_module(WaveWidth::Wave64));
    let error = module
        .simulate(
            &wave_collective_request(33, WaveWidth::Wave64),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .expect_err("partial wave");
    let SimulationErrorV1::Execution(error) = error else {
        panic!("dynamic failure")
    };
    let SimulationExecutionErrorKindV1::IncompleteWave(detail) = error.kind else {
        panic!("partial wave detail")
    };
    assert_eq!(detail.width, WaveWidth::Wave64);
    assert_eq!(detail.wave_in_workgroup, 0);
    assert_eq!(detail.active_mask, (1_u64 << 33) - 1);
    assert_eq!(detail.required_mask, u64::MAX);
}

fn divergent_wave_module(mismatched: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            0,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(1, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            2,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let wave = |result| {
        op(
            result,
            Type::Scalar(ScalarType::U32),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave32,
            )),
        )
    };
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block.operations.push(wave(3));
    then_block.terminator = Some(if mismatched {
        Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        }
    } else {
        Terminator::Return { values: vec![] }
    });
    let mut else_block = BasicBlock::new(BlockId(2));
    if mismatched {
        else_block.operations.push(wave(4));
    }
    else_block.terminator = Some(if mismatched {
        Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        }
    } else {
        Terminator::Return { values: vec![] }
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "divergent_wave_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry, then_block, else_block, merge],
    );
    function.required_capabilities = wave_capabilities(WaveWidth::Wave32);
    let mut kernel = Kernel::new("divergent_wave", "divergent_wave_impl", dynamic_domain_1d());
    kernel.required_capabilities = wave_capabilities(WaveWidth::Wave32);
    let mut module = Module::new("sim-tests::divergent-wave");
    module.required_capabilities = wave_capabilities(WaveWidth::Wave32);
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn divergent_and_mismatched_wave_control_are_typed() {
    let request = SimulationRequestV1::new("divergent_wave", [32, 1, 1], [32, 1, 1], vec![]);
    let error = admitted(divergent_wave_module(false))
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::DivergentWave(_),
            ..
        })
    ));
    let error = admitted(divergent_wave_module(true))
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::MismatchedWave(_),
            ..
        })
    ));
}

#[test]
fn shuffle_rejects_a_dynamic_tile_source_outside_the_exact_tile() {
    let mut module = wave_collective_module(WaveWidth::Wave32);
    let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[8];
    operation.kind = OperationKind::Constant(Constant::U32(8));
    let error = admitted(module)
        .simulate(
            &wave_collective_request(32, WaveWidth::Wave32),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::WaveShuffleSourceOutOfRange {
                source_lane: 8,
                tile_width: 8
            },
            ..
        })
    ));
}

fn indexed_store_module() -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            2,
            pointer,
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
        "store_impl",
        Signature::new(
            vec![Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            )],
            vec![],
        ),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::store");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("store", "store_impl", dynamic_domain_1d()));
    module
}

#[test]
fn dynamic_failure_reports_exact_invocation_and_preserves_input_buffers() {
    let admitted = admitted(indexed_store_module());
    let request = SimulationRequestV1::new(
        "store",
        [2, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    let original = request.clone();
    let error = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .expect_err("second invocation is out of bounds");
    assert_eq!(request, original);
    let SimulationErrorV1::Execution(error) = error else {
        panic!("expected dynamic error")
    };
    assert_eq!(error.invocation.unwrap().global, [1, 0, 0]);
    assert_eq!(error.site.unwrap().operation, Some(3));
    assert!(matches!(
        error.kind,
        SimulationExecutionErrorKindV1::OutOfBounds { .. }
    ));
}

#[test]
fn target_specific_index_constants_are_rejected_by_preflight() {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(op(
        0,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(u64::from(u32::MAX) + 1)),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "wide_index_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let mut module = Module::new("sim-tests::wide-index");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "wide_index",
        "wide_index_impl",
        dynamic_domain_1d(),
    ));
    let error = admitted(module)
        .preflight(
            &SimulationRequestV1::new("wide_index", [1, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::little_endian(fe2o3_kir_sim::IndexWidthV1::Bits32),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(findings) = error else {
        panic!("expected target-constant finding")
    };
    assert!(
        findings
            .findings()
            .iter()
            .any(|finding| { finding.feature == UnsupportedFeatureV1::TargetConstantOutOfRange })
    );
}

#[test]
fn target_index_width_is_checked_before_launch_resource_products() {
    let admitted = admitted(indexed_store_module());
    let request = SimulationRequestV1::new(
        "store",
        [u64::from(u32::MAX) + 2, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let error = admitted
        .preflight(
            &request,
            SimulationTargetV1::little_endian(fe2o3_kir_sim::IndexWidthV1::Bits32),
            SimulationLimitsV1 {
                max_invocations: 1_u64 << 32,
                ..SimulationLimitsV1::default()
            },
        )
        .expect_err("coordinate does not fit 32-bit index");
    assert_eq!(
        error,
        SimulationPreflightErrorV1::InvalidLaunch(
            "launch coordinate exceeds the target index width"
        )
    );
}

#[test]
fn reaching_unreachable_is_a_site_and_invocation_bound_dynamic_error() {
    let mut block = BasicBlock::new(BlockId(7));
    block.terminator = Some(Terminator::Unreachable);
    let entry = Function::kernel_entry(
        "unreachable_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let mut module = Module::new("sim-tests::unreachable");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "unreachable",
        "unreachable_impl",
        dynamic_domain_1d(),
    ));
    let error = admitted(module)
        .simulate(
            &SimulationRequestV1::new("unreachable", [1, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .expect_err("unreachable terminator fails");
    let SimulationErrorV1::Execution(error) = error else {
        panic!("expected dynamic unreachable error")
    };
    assert_eq!(error.invocation.unwrap().global, [0, 0, 0]);
    assert_eq!(error.site.unwrap().block, BlockId(7));
    assert_eq!(
        error.kind,
        SimulationExecutionErrorKindV1::ReachedUnreachable
    );
}

fn empty_kernel_module(name: &str, signature: Signature, parameters: Vec<ValueId>) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let implementation = format!("{name}_impl");
    let entry = Function::kernel_entry(implementation.clone(), signature, parameters, vec![block]);
    let mut module = Module::new(format!("sim-tests::{name}"));
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new(name, implementation, dynamic_domain_1d()));
    module
}

fn wide_switch_module() -> Module {
    let mut switch = BasicBlock::new(BlockId(0));
    switch.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![SwitchCase {
            value: 1,
            target: BlockId(1),
            arguments: vec![],
        }],
        default_target: BlockId(2),
        default_arguments: vec![],
    });
    let mut matched = BasicBlock::new(BlockId(1));
    matched.operations.push(op(
        1,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(11)),
    ));
    matched.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let mut default = BasicBlock::new(BlockId(2));
    default.operations.push(op(
        2,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(22)),
    ));
    default.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let helper = Function::internal_helper(
        "wide_switch",
        Signature::new(
            vec![Type::Scalar(ScalarType::U128)],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0)],
        vec![switch, matched, default],
    );

    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            2,
            Type::Scalar(ScalarType::U32),
            OperationKind::Call {
                callee: "wide_switch".into(),
                arguments: vec![ValueId(0)],
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "wide_switch_impl",
        Signature::new(vec![Type::Scalar(ScalarType::U128), pointer], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::wide-switch");
    module.functions = vec![entry, helper];
    module.kernels.push(Kernel::new(
        "wide_switch_kernel",
        "wide_switch_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn legacy_switch_compares_wide_selectors_without_truncation() {
    let selector = ScalarBitsV1::new(
        ScalarType::U128,
        (1_u128 << 64) + 1,
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "wide_switch_kernel",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(selector),
            SimulationArgumentV1::Buffer(u32_buffer(&[0])),
        ],
    );
    let execution = admitted(wide_switch_module())
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(words(execution.buffer(1).unwrap().bytes()), vec![22]);
}

fn alias_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            3,
            scalar.clone(),
            OperationKind::Constant(Constant::U32(42)),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        op(
            4,
            scalar,
            OperationKind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "alias_impl",
        Signature::new(vec![pointer.clone(), pointer.clone(), pointer], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::alias");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("alias", "alias_impl", dynamic_domain_1d()));
    module
}

fn aliased_view_bounds_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(2, scalar.clone(), OperationKind::Constant(Constant::U32(1))),
        op(
            3,
            pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        op(
            4,
            scalar,
            OperationKind::Load {
                pointer: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "aliased_view_bounds_impl",
        Signature::new(vec![pointer.clone(), pointer], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::aliased-view-bounds");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "aliased_view_bounds",
        "aliased_view_bounds_impl",
        dynamic_domain_1d(),
    ));
    module
}

fn aliased_view_bounds_request() -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let backing = BufferBackingIdV1(9);
    let narrow = BufferViewArgumentV1::new(
        backing,
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        4,
        1,
        target,
    )
    .unwrap();
    let wide = BufferViewArgumentV1::new(
        backing,
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        0,
        3,
        target,
    )
    .unwrap();
    SimulationRequestV1::new(
        "aliased_view_bounds",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::BufferView(narrow),
            SimulationArgumentV1::BufferView(wide),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: backing,
        buffer: u32_buffer(&[10, 20, 30]),
    }])
}

#[test]
fn shared_backing_views_preserve_aliasing_and_copy_back_once() {
    let target = SimulationTargetV1::amdgpu_64();
    let backing_id = BufferBackingIdV1(7);
    let view = || {
        BufferViewArgumentV1::new(
            backing_id,
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            0,
            1,
            target,
        )
        .unwrap()
    };
    let request = SimulationRequestV1::new(
        "alias",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::BufferView(view()),
            SimulationArgumentV1::BufferView(view()),
            SimulationArgumentV1::Buffer(u32_buffer(&[0])),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: backing_id,
        buffer: u32_buffer(&[9]),
    }]);
    let execution = admitted(alias_module())
        .simulate(&request, target, SimulationLimitsV1::default())
        .unwrap();
    assert_eq!(words(execution.buffer(2).unwrap().bytes()), vec![42]);
    assert_eq!(
        words(execution.shared_buffer(backing_id).unwrap().bytes()),
        vec![42]
    );

    let oversized_view = BufferViewArgumentV1::new(
        backing_id,
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        0,
        2,
        target,
    )
    .unwrap();
    let invalid = SimulationRequestV1::new(
        "alias",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::BufferView(oversized_view),
            SimulationArgumentV1::BufferView(view()),
            SimulationArgumentV1::Buffer(u32_buffer(&[0])),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: backing_id,
        buffer: u32_buffer(&[9]),
    }]);
    assert!(matches!(
        admitted(alias_module()).preflight(&invalid, target, SimulationLimitsV1::default(),),
        Err(SimulationPreflightErrorV1::BufferViewBounds { argument: 0 })
    ));
}

#[test]
fn pointer_view_oob_retains_exact_argument_and_bounds_inside_shared_allocation() {
    let target = SimulationTargetV1::amdgpu_64();
    let request = aliased_view_bounds_request();

    let error = admitted(aliased_view_bounds_module())
        .simulate(&request, target, SimulationLimitsV1::default())
        .expect_err("the narrow view ends before the backing allocation");
    let SimulationErrorV1::Execution(error) = error else {
        panic!("expected execution failure");
    };
    let SimulationExecutionErrorKindV1::OutOfBounds {
        allocation,
        offset,
        bytes,
        allocation_bytes,
    } = error.kind
    else {
        panic!("expected out-of-bounds failure");
    };
    assert_eq!((allocation, offset, bytes, allocation_bytes), (1, 8, 4, 12));
}

#[test]
fn public_v1_terminal_error_shapes_remain_scalar_and_constructible() {
    let _ = SimulationExecutionErrorKindV1::OutOfBounds {
        allocation: 1,
        offset: 8,
        bytes: 4,
        allocation_bytes: 12,
    };
    let _ = fe2o3_kir_sim::DivergentWorkgroupBarrierV1 {
        phase: 0,
        waiting: fe2o3_kir_sim::WorkgroupParticipantV1 { local: [0, 0, 0] },
        exited: fe2o3_kir_sim::WorkgroupParticipantV1 { local: [1, 0, 0] },
    };
}

#[test]
fn pointer_view_oob_debug_side_record_is_exact_bounded_and_small_stack_safe() {
    std::thread::Builder::new()
        .name("sim-oob-small-stack".to_owned())
        .stack_size(256 * 1024)
        .spawn(|| {
            let target = SimulationTargetV1::amdgpu_64();
            let request = aliased_view_bounds_request();
            let admitted = admitted(aliased_view_bounds_module());
            let base_limits = SimulationLimitsV1::default();
            let resident = admitted
                .preflight(&request, target, base_limits)
                .unwrap()
                .resident_bytes();
            assert!(matches!(
                admitted.preflight(
                    &request,
                    target,
                    SimulationLimitsV1 {
                        max_resident_bytes: resident - 1,
                        ..base_limits
                    }
                ),
                Err(SimulationPreflightErrorV1::ResourceLimit {
                    resource: "resident bytes",
                    actual,
                    limit,
                }) if actual == resident as u64 && limit == (resident - 1) as u64
            ));
            let mut debug = DebugCollector::default();
            let error = admitted
                .simulate_debugged_with_sink(
                    &request,
                    target,
                    SimulationLimitsV1 {
                        max_resident_bytes: resident,
                        ..base_limits
                    },
                    SimulationDebugCaptureLimitsV1::new(16, 64, 16, 4_096).unwrap(),
                    &mut debug,
                )
                .expect_err("narrow view must fault");
            assert!(matches!(
                error,
                SimulationErrorV1::Execution(ref execution)
                    if matches!(
                        execution.kind,
                        SimulationExecutionErrorKindV1::OutOfBounds {
                            allocation: 1,
                            offset: 8,
                            bytes: 4,
                            allocation_bytes: 12,
                        }
                    )
            ));
            let detail = debug.1.expect("exact-one V2 side record");
            assert_eq!(detail.legal_lower_bound, 4);
            assert_eq!(detail.legal_upper_bound, 8);
            assert_eq!(detail.abi_view.unwrap().argument_ordinal, 0);
        })
        .unwrap()
        .join()
        .unwrap();
}

fn conflicting_store_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(1, scalar, OperationKind::Constant(Constant::U32(42))),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "conflict_impl",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::conflict");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "conflict",
        "conflict_impl",
        dynamic_domain_1d(),
    ));
    module
}

fn fenced_conflicting_store_module() -> Module {
    let mut module = conflicting_store_module();
    module.functions[0]
        .body
        .as_mut()
        .expect("entry body")
        .blocks[0]
        .operations
        .insert(
            1,
            Operation::new(
                vec![],
                OperationKind::Fence(Fence {
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Global],
                    ),
                }),
            ),
        );
    module
}

#[test]
fn fence_happens_before_with_ordinary_conflicts_is_incomplete() {
    let request = SimulationRequestV1::new(
        "conflict",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let execution = admitted(fenced_conflicting_store_module())
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::Incomplete {
            access_record_limit_reached: false,
            atomic_or_fence_happens_before_unmodeled: true,
            first: Some(_),
            ..
        }
    ));
}

#[test]
fn bounded_seed_exploration_is_deterministic_replayable_and_explicitly_exhausted() {
    let module = admitted(conflicting_store_module());
    let request = SimulationRequestV1::new(
        "conflict",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let exploration_request = SimulationExplorationRequestV1::new(41, 4, 16, 16).unwrap();
    let first = module
        .explore_seeded_schedules(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            exploration_request,
        )
        .unwrap();
    let same = module
        .explore_seeded_schedules(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            exploration_request,
        )
        .unwrap();
    assert_eq!(first, same);
    assert_eq!(first.attempted(), 4);
    assert_eq!(first.completed(), 4);
    assert_eq!(first.races_observed(), 4);
    assert!(first.requested_seed_budget_consumed());
    let witness = first.first_race().expect("first race witness");
    let replay = module
        .simulate_scheduled(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::Replay(witness.schedule()),
        )
        .unwrap();
    assert_eq!(replay.race_assessment(), witness.assessment());

    let retention_limited = module
        .explore_seeded_schedules(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationExplorationRequestV1::new(41, 1, 16, 1).unwrap(),
        )
        .unwrap();
    assert!(retention_limited.witness_retention_exhausted());
    assert!(retention_limited.first_race().is_none());

    let decision_limited = module
        .explore_seeded_schedules(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationExplorationRequestV1::new(41, 2, 1, 2).unwrap(),
        )
        .unwrap();
    assert_eq!(decision_limited.completed(), 0);
    assert_eq!(decision_limited.failures(), 2);
    assert!(matches!(
        decision_limited
            .first_failure()
            .map(|failure| &failure.kind),
        Some(SimulationExecutionErrorKindV1::ScheduleDecisionLimit {
            actual: 2,
            limit: 1
        })
    ));
}

#[test]
fn exploration_request_bounds_fail_closed() {
    assert!(SimulationExplorationRequestV1::new(0, 0, 1, 1).is_err());
    assert!(SimulationExplorationRequestV1::new(0, 1, 0, 1).is_err());
    assert!(SimulationExplorationRequestV1::new(0, 1, 1, 0).is_err());
    assert!(SimulationExplorationRequestV1::new(0, 4097, 1, 1).is_err());
}

fn one_decision_schedule_resident_bytes(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    max_decisions: usize,
) -> usize {
    let target = SimulationTargetV1::amdgpu_64();
    let plan = module
        .preflight(request, target, SimulationLimitsV1::default())
        .unwrap();
    let error = module
        .simulate_scheduled(
            request,
            target,
            SimulationLimitsV1 {
                max_resident_bytes: plan.resident_bytes(),
                ..SimulationLimitsV1::default()
            },
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 1,
                max_decisions,
            },
        )
        .unwrap_err();
    match error {
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::ScheduleResidentLimit { actual, .. },
            ..
        }) => actual,
        other => panic!("expected exact scheduled resident requirement, got {other:?}"),
    }
}

#[test]
fn one_decision_million_limit_schedule_is_compact_before_the_next_exploration_run() {
    const MAX_DECISIONS: usize = 1_000_000;
    let module = admitted(empty_kernel_module(
        "compact_schedule",
        Signature::new(vec![], vec![]),
        vec![],
    ));
    let request = SimulationRequestV1::new("compact_schedule", [1, 1, 1], [1, 1, 1], vec![]);
    let scheduled = one_decision_schedule_resident_bytes(&module, &request, MAX_DECISIONS);
    let retained_decision = size_of::<SimulationScheduleDecisionV1>();
    let exploration_inline = size_of::<SimulationExplorationV1>();
    let exploration = module
        .explore_seeded_schedules(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: scheduled + exploration_inline + retained_decision,
                ..SimulationLimitsV1::default()
            },
            SimulationExplorationRequestV1::new(9, 2, MAX_DECISIONS, 2).unwrap(),
        )
        .unwrap();
    assert_eq!(exploration.completed(), 2);
    assert_eq!(exploration.failures(), 0);
    assert_eq!(exploration.retained_decisions(), 1);
    assert_eq!(
        exploration
            .first_no_race()
            .expect("one retained no-race witness")
            .schedule()
            .decisions()
            .len(),
        1
    );
}

#[test]
fn retained_witness_bytes_are_cumulative_with_each_later_scheduled_run() {
    const MAX_DECISIONS: usize = 1_000_000;
    let module = admitted(empty_kernel_module(
        "cumulative_schedule",
        Signature::new(vec![], vec![]),
        vec![],
    ));
    let request = SimulationRequestV1::new("cumulative_schedule", [1, 1, 1], [1, 1, 1], vec![]);
    let scheduled = one_decision_schedule_resident_bytes(&module, &request, MAX_DECISIONS);
    let exploration_inline = size_of::<SimulationExplorationV1>();
    let resident_limit =
        scheduled + exploration_inline + size_of::<SimulationScheduleDecisionV1>() - 1;
    let exploration = module
        .explore_seeded_schedules(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: resident_limit,
                ..SimulationLimitsV1::default()
            },
            SimulationExplorationRequestV1::new(9, 2, MAX_DECISIONS, 2).unwrap(),
        )
        .unwrap();
    assert_eq!(exploration.completed(), 1);
    assert_eq!(exploration.failures(), 1);
    assert!(matches!(
        exploration.first_failure().map(|failure| &failure.kind),
        Some(SimulationExecutionErrorKindV1::ScheduleResidentLimit { actual, limit })
            if *actual == resident_limit + 1 && *limit == resident_limit
    ));
}

fn write_then_read_or_read_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(2, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            3,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });

    let mut write = BasicBlock::new(BlockId(1));
    write.operations = vec![
        op(4, scalar.clone(), OperationKind::Constant(Constant::U32(7))),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    write.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });

    let mut read = BasicBlock::new(BlockId(2));
    read.operations = vec![op(
        5,
        scalar,
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )];
    read.terminator = Some(Terminator::Return { values: vec![] });

    let entry = Function::kernel_entry(
        "write_read_impl",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![entry, write, read],
    );
    let mut module = Module::new("sim-tests::write-then-read-or-read");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "write_read",
        "write_read_impl",
        dynamic_domain_1d(),
    ));
    module
}

fn global_barrier_order_module(include_barrier: bool) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(2, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            3,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut writer = BasicBlock::new(BlockId(1));
    writer.operations = vec![
        op(
            4,
            scalar.clone(),
            OperationKind::Constant(Constant::U32(42)),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    writer.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut peer = BasicBlock::new(BlockId(2));
    peer.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    if include_barrier {
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
        merge.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(4),
            then_arguments: vec![],
            else_target: BlockId(5),
            else_arguments: vec![],
        });
    } else {
        merge.operations.push(op(
            5,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        merge.terminator = Some(Terminator::Return { values: vec![] });
    }
    let mut blocks = vec![entry, writer, peer, merge];
    if include_barrier {
        let mut writer_exit = BasicBlock::new(BlockId(4));
        writer_exit.terminator = Some(Terminator::Return { values: vec![] });
        let mut peer_read = BasicBlock::new(BlockId(5));
        peer_read.operations.push(op(
            5,
            scalar,
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        peer_read.terminator = Some(Terminator::Return { values: vec![] });
        blocks.extend([writer_exit, peer_read]);
    }
    let mut function = Function::kernel_entry(
        "global_order_impl",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        blocks,
    );
    if include_barrier {
        function
            .required_capabilities
            .insert(TargetCapability::WorkgroupBarrier);
    }
    let mut kernel = Kernel::new("global_order", "global_order_impl", dynamic_domain_1d());
    kernel.required_capabilities = function.required_capabilities.clone();
    let mut module = Module::new("sim-tests::global-order");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn global_acquire_release_workgroup_barrier_produces_happens_before_evidence() {
    let request = SimulationRequestV1::new(
        "global_order",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let unordered = admitted(global_barrier_order_module(false))
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        unordered.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved { .. }
    ));
    let ordered = admitted(global_barrier_order_module(true))
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        ordered.race_assessment(),
        SimulationRaceAssessmentV1::NoRacesObserved {
            first_ordered_conflict: Some(fe2o3_kir_sim::SimulationOrderedMemoryConflictV1 {
                reason: fe2o3_kir_sim::SimulationHappensBeforeReasonV1::GlobalWorkgroupBarrier,
                ..
            })
        }
    ));
}

fn global_barrier_epoch_transition_module() -> Module {
    let mut module = global_barrier_order_module(true);
    let function = &mut module.functions[0];
    let body = function.body.as_mut().expect("entry body");
    let scalar = Type::Scalar(ScalarType::U32);
    let reader = &mut body.blocks[4];
    reader.operations.push(op(
        5,
        scalar.clone(),
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    reader.terminator = Some(Terminator::Return { values: vec![] });

    let writer = &mut body.blocks[5];
    writer.operations.clear();
    writer.operations = vec![
        op(6, scalar, OperationKind::Constant(Constant::U32(99))),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    writer.terminator = Some(Terminator::Return { values: vec![] });
    module
}

#[test]
fn same_invocation_access_after_a_barrier_starts_a_new_happens_before_epoch() {
    let request = SimulationRequestV1::new(
        "global_order",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let execution = admitted(global_barrier_epoch_transition_module())
        .simulate(
            &request,
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

fn ordered_read_then_cross_workgroup_read_module() -> Module {
    let mut module = global_barrier_order_module(true);
    let function = &mut module.functions[0];
    let body = function.body.as_mut().expect("entry body");
    let OperationKind::Intrinsic(intrinsic) = &mut body.blocks[0].operations[0].kind else {
        panic!("expected invocation index")
    };
    intrinsic.kind = IntrinsicKind::InvocationIndex {
        kind: IndexKind::Global,
        axis: Axis::X,
    };
    let scalar = Type::Scalar(ScalarType::U32);
    body.blocks[4].operations.push(op(
        6,
        scalar,
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    module
}

#[test]
fn ordered_epoch_reads_do_not_erase_a_write_visible_to_another_workgroup() {
    let execution = admitted(ordered_read_then_cross_workgroup_read_module())
        .simulate(
            &SimulationRequestV1::new(
                "global_order",
                [4, 1, 1],
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
            first: fe2o3_kir_sim::SimulationDataRaceV1 {
                earlier_atomic: false,
                later_atomic: false,
                ..
            },
            first_ordered_conflict: Some(fe2o3_kir_sim::SimulationOrderedMemoryConflictV1 {
                reason: fe2o3_kir_sim::SimulationHappensBeforeReasonV1::GlobalWorkgroupBarrier,
                ..
            }),
            ..
        }
    ));
    assert!(matches!(
        execution.conflict_assessment(),
        SimulationConflictAssessmentV1::ConflictsObserved {
            conflicting_bytes: 4,
            ..
        }
    ));
}

#[test]
fn cross_invocation_conflicts_are_machine_readable() {
    let request = SimulationRequestV1::new(
        "conflict",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let execution = admitted(conflicting_store_module())
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        execution.conflict_assessment(),
        SimulationConflictAssessmentV1::ConflictsObserved { .. }
    ));
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved {
            racing_bytes: 4,
            ..
        }
    ));
    assert!(!execution.grants_execution_authority());

    let incomplete = admitted(conflicting_store_module())
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_memory_access_records: 1,
                ..SimulationLimitsV1::default()
            },
        )
        .unwrap();
    assert!(matches!(
        incomplete.conflict_assessment(),
        SimulationConflictAssessmentV1::Incomplete {
            record_limit: 1,
            ..
        }
    ));
    assert!(matches!(
        incomplete.race_assessment(),
        SimulationRaceAssessmentV1::Incomplete {
            record_limit: 1,
            access_record_limit_reached: true,
            atomic_or_fence_happens_before_unmodeled: false,
            ..
        }
    ));

    let one_invocation_request = SimulationRequestV1::new(
        "conflict",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let no_retained_conflict = admitted(conflicting_store_module())
        .simulate(
            &one_invocation_request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_memory_access_records: 1,
                ..SimulationLimitsV1::default()
            },
        )
        .unwrap();
    assert!(matches!(
        no_retained_conflict.race_assessment(),
        SimulationRaceAssessmentV1::Incomplete {
            racing_bytes: 0,
            first: None,
            record_limit: 1,
            access_record_limit_reached: true,
            atomic_or_fence_happens_before_unmodeled: false,
            ..
        }
    ));
}

#[test]
fn same_invocation_read_does_not_erase_an_earlier_write() {
    let request = SimulationRequestV1::new(
        "write_read",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let execution = admitted(write_then_read_or_read_module())
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        execution.conflict_assessment(),
        SimulationConflictAssessmentV1::ConflictsObserved {
            conflicting_bytes: 4,
            ..
        }
    ));
}

#[test]
fn conflicting_bytes_count_unique_bytes_not_access_transitions() {
    let request = SimulationRequestV1::new(
        "conflict",
        [3, 1, 1],
        [3, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let execution = admitted(conflicting_store_module())
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        execution.conflict_assessment(),
        SimulationConflictAssessmentV1::ConflictsObserved {
            conflicting_bytes: 4,
            ..
        }
    ));
}

#[test]
fn event_limit_is_reserved_before_a_store_effect() {
    let mut request = SimulationRequestV1::new(
        "store",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    request.events = EventPolicyV1::Enabled;
    let mut sink = Collector::default();
    let error = admitted(indexed_store_module())
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_events: 4,
                ..SimulationLimitsV1::default()
            },
            &mut sink,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::EventLimit { .. },
            ..
        })
    ));
    assert!(
        sink.0
            .iter()
            .all(|event| !matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }))
    );
}

struct RejectWrites;

impl SimulationEventSinkV1 for RejectWrites {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        if matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }) {
            Err(fe2o3_kir_sim::SimulationEventSinkErrorV1 {
                detail: "write rejected".into(),
            })
        } else {
            Ok(())
        }
    }
}

#[test]
fn sink_failure_is_typed_and_precedes_store_commit() {
    let mut request = SimulationRequestV1::new(
        "store",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    request.events = EventPolicyV1::Enabled;
    let error = admitted(indexed_store_module())
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut RejectWrites,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::EventSinkFailure(_),
            ..
        })
    ));
}

#[test]
fn acyclic_depth_and_ssa_limits_are_preflighted() {
    let request = SimulationRequestV1::new(
        "hierarchy",
        [3, 2, 2],
        [2, 2, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0; 12]))],
    );
    let admitted = admitted(hierarchy_module());
    assert!(matches!(
        admitted.preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_call_depth: 1,
                ..SimulationLimitsV1::default()
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "acyclic call depth",
            ..
        })
    ));
    assert!(matches!(
        admitted.preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_ssa_values: 4,
                ..SimulationLimitsV1::default()
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "SSA values in one frame",
            ..
        })
    ));
}

#[test]
fn structured_loop_consumes_exact_terminator_fuel() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![],
    });
    let entry = Function::kernel_entry(
        "loop_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let mut module = Module::new("sim-tests::loop");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("loop", "loop_impl", dynamic_domain_1d()));
    let error = admitted(module)
        .simulate(
            &SimulationRequestV1::new("loop", [1, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_steps: 3,
                ..SimulationLimitsV1::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { limit: 3 },
            ..
        })
    ));
}

#[test]
fn scheduled_slots_are_target_legal_bounded_and_count_padded_tails() {
    let admitted = admitted(empty_kernel_module(
        "slots",
        Signature::new(vec![], vec![]),
        vec![],
    ));
    let oversized = SimulationRequestV1::new("slots", [1, 1, 1], [1_025, 1, 1], vec![]);
    assert!(matches!(
        admitted.preflight(
            &oversized,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "workgroup invocations",
            ..
        })
    ));

    let bounded = SimulationRequestV1::new("slots", [1_025, 1, 1], [1_024, 1, 1], vec![]);
    assert!(matches!(
        admitted.preflight(
            &bounded,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_scheduled_slots: 1_024,
                ..SimulationLimitsV1::default()
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "scheduled slots",
            ..
        })
    ));

    let tail = SimulationRequestV1::new("slots", [3, 1, 1], [2, 1, 1], vec![]);
    let plan = admitted
        .preflight(
            &tail,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(plan.invocations(), 3);
    assert_eq!(plan.scheduled_slots(), 4);
    let execution = admitted
        .simulate(
            &tail,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(execution.scheduled_slots_visited(), 4);
    assert_eq!(
        execution.schedule(),
        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1
    );
}

fn lds_exchange_module(include_barrier: bool) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let global_pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let workgroup_pointer = Type::pointer(
        scalar.clone(),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            1,
            workgroup_pointer.clone(),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: scalar.clone(),
                extent: WorkgroupMemoryExtent::Static(4),
                alignment: 4,
            }),
        ),
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(
            3,
            workgroup_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(2),
            },
        ),
        op(
            4,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            5,
            Type::Scalar(ScalarType::U64),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Bitcast,
                value: ValueId(4),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
        op(
            8,
            scalar.clone(),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Truncate,
                value: ValueId(5),
                to: scalar.clone(),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(8),
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ),
    ];
    if include_barrier {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ));
    }
    block.operations.extend([
        op(
            6,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ),
        op(
            7,
            global_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(7),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let capabilities = std::collections::BTreeSet::from([
        TargetCapability::WorkgroupMemory,
        TargetCapability::WorkgroupBarrier,
    ]);
    let mut entry = Function::kernel_entry(
        "lds_exchange_impl",
        Signature::new(
            vec![Type::pointer(
                scalar,
                AddressSpace::Global,
                AccessMode::ReadWrite,
            )],
            vec![],
        ),
        vec![ValueId(0)],
        vec![block],
    );
    entry.required_capabilities = if include_barrier {
        capabilities.clone()
    } else {
        std::collections::BTreeSet::from([TargetCapability::WorkgroupMemory])
    };
    let mut kernel = Kernel::new("lds_exchange", "lds_exchange_impl", dynamic_domain_1d());
    kernel.required_capabilities = entry.required_capabilities.clone();
    let mut module = Module::new("sim-tests::lds-exchange");
    module.required_capabilities = entry.required_capabilities.clone();
    module.functions.push(entry);
    module.kernels.push(kernel);
    module
}

fn dynamic_lds_exchange_module(include_barrier: bool) -> Module {
    let mut module = lds_exchange_module(include_barrier);
    let memory = module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::WorkgroupMemory(memory) => Some(memory),
            _ => None,
        })
        .expect("workgroup-memory declaration");
    memory.extent = WorkgroupMemoryExtent::Dynamic;
    module
        .required_capabilities
        .insert(TargetCapability::DynamicWorkgroupMemory);
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::DynamicWorkgroupMemory);
    module.kernels[0]
        .required_capabilities
        .insert(TargetCapability::DynamicWorkgroupMemory);
    module
}

fn called_dynamic_workgroup_memory_module(ambiguous: bool) -> Module {
    let mut module = unsupported_workgroup_memory_module(
        Type::Scalar(ScalarType::U32),
        WorkgroupMemoryExtent::Dynamic,
    );
    let entry = module.functions[0].body.as_mut().unwrap();
    let dynamic = entry.blocks[0].operations[0].clone();
    entry.blocks[0].operations = if ambiguous {
        vec![
            dynamic.clone(),
            Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "dynamic_lds_helper".into(),
                    arguments: vec![],
                },
            ),
        ]
    } else {
        vec![Operation::new(
            vec![],
            OperationKind::Call {
                callee: "dynamic_lds_helper".into(),
                arguments: vec![],
            },
        )]
    };
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.operations.push(dynamic);
    helper_block.terminator = Some(Terminator::Return { values: vec![] });
    let mut helper = Function::internal_helper(
        "dynamic_lds_helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![helper_block],
    );
    helper.required_capabilities.extend([
        TargetCapability::WorkgroupMemory,
        TargetCapability::DynamicWorkgroupMemory,
    ]);
    module.functions.push(helper);
    module
}

fn barrier_failure_module(mismatch: bool) -> Module {
    let barrier = |address_spaces| {
        Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(MemoryOrdering::AcquireRelease, address_spaces),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        )
    };
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(2, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            3,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    if mismatch {
        then_block
            .operations
            .push(barrier([AddressSpace::Workgroup]));
        then_block.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        });
    } else {
        then_block.terminator = Some(Terminator::Return { values: vec![] });
    }
    let mut else_block = BasicBlock::new(BlockId(2));
    else_block.operations.push(barrier(if mismatch {
        [AddressSpace::Global]
    } else {
        [AddressSpace::Workgroup]
    }));
    else_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge.terminator = Some(Terminator::Return { values: vec![] });
    let capabilities = std::collections::BTreeSet::from([TargetCapability::WorkgroupBarrier]);
    let mut function = Function::kernel_entry(
        "barrier_failure_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry, then_block, else_block, merge],
    );
    function.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new(
        "barrier_failure",
        "barrier_failure_impl",
        dynamic_domain_1d(),
    );
    kernel.required_capabilities = capabilities.clone();
    let mut module = Module::new("sim-tests::barrier-failure");
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn helper_barrier_exchange_module() -> Module {
    let mut module = lds_exchange_module(true);
    let entry_block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let barrier_index = entry_block
        .operations
        .iter()
        .position(|operation| matches!(operation.kind, OperationKind::WorkgroupBarrier(_)))
        .expect("workgroup barrier");
    let barrier = entry_block.operations.remove(barrier_index);
    entry_block.operations.insert(
        barrier_index,
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: "barrier_helper".into(),
                arguments: vec![],
            },
        ),
    );
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.operations.push(barrier);
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
    module.functions.push(helper);
    module
}

fn unsupported_workgroup_memory_module(element: Type, extent: WorkgroupMemoryExtent) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(0),
            Type::pointer(
                element.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element,
            extent,
            alignment: 8,
        }),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut capabilities = std::collections::BTreeSet::from([TargetCapability::WorkgroupMemory]);
    if extent.is_dynamic() {
        capabilities.insert(TargetCapability::DynamicWorkgroupMemory);
    }
    let mut function = Function::kernel_entry(
        "unsupported_lds_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    function.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new(
        "unsupported_lds",
        "unsupported_lds_impl",
        dynamic_domain_1d(),
    );
    kernel.required_capabilities = capabilities.clone();
    let mut module = Module::new("sim-tests::unsupported-lds");
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn uninitialized_workgroup_memory_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            0,
            Type::pointer(
                scalar.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            ),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: scalar.clone(),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 4,
            }),
        ),
        op(
            1,
            scalar,
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let capabilities = std::collections::BTreeSet::from([TargetCapability::WorkgroupMemory]);
    let mut function = Function::kernel_entry(
        "uninitialized_lds_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    function.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new(
        "uninitialized_lds",
        "uninitialized_lds_impl",
        dynamic_domain_1d(),
    );
    kernel.required_capabilities = capabilities.clone();
    let mut module = Module::new("sim-tests::uninitialized-lds");
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn dynamic_partial_write_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(
        scalar.clone(),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut module =
        unsupported_workgroup_memory_module(scalar.clone(), WorkgroupMemoryExtent::Dynamic);
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.extend([
        op(1, scalar.clone(), OperationKind::Constant(Constant::U32(7))),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ),
        op(2, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            3,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        op(
            4,
            scalar,
            OperationKind::Load {
                pointer: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
            },
        ),
    ]);
    module
}

#[test]
fn workgroup_memory_barrier_exchanges_across_full_and_partial_workgroups() {
    let admitted = admitted(lds_exchange_module(true));
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [10, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 10]))],
    );
    let mut events = Collector::default();
    let execution = admitted
        .simulate_observed_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .expect("static LDS exchange executes");
    assert_eq!(
        words(execution.buffer(0).unwrap().bytes()),
        vec![0, 0, 0, 0, 4, 4, 4, 4, 8, 8]
    );
    assert_eq!(execution.workgroups_visited(), 3);
    let releases = events
        .0
        .iter()
        .filter_map(|event| match event.kind {
            SimulationEventKindV1::WorkgroupBarrierRelease { participants, .. } => {
                Some(participants)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(releases, vec![4, 4, 2]);
    let creates = events
        .0
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                SimulationEventKindV1::AllocationCreated {
                    address_space: AddressSpace::Workgroup,
                    ..
                }
            )
        })
        .count();
    assert_eq!(creates, 3, "one static allocation per site and workgroup");
}

#[test]
fn explicit_dynamic_workgroup_memory_exchanges_across_partial_workgroups() {
    let admitted = admitted(dynamic_lds_exchange_module(true));
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [10, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 10]))],
    );
    let dynamic = DynamicWorkgroupMemoryRequestV1::new(16);
    let mut events = Collector::default();
    let execution = admitted
        .simulate_observed_with_dynamic_workgroup_memory_and_sink(
            &request,
            dynamic,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .expect("explicit dynamic LDS exchange executes");
    assert_eq!(execution.dynamic_workgroup_memory(), Some(dynamic));
    assert_eq!(
        words(execution.buffer(0).unwrap().bytes()),
        vec![0, 0, 0, 0, 4, 4, 4, 4, 8, 8]
    );
    assert_eq!(execution.workgroups_visited(), 3);
    let dynamic_allocations = events
        .0
        .iter()
        .filter_map(|event| match event.kind {
            SimulationEventKindV1::AllocationCreated {
                address_space: AddressSpace::Workgroup,
                bytes: 16,
                allocation,
            } => Some(allocation),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_allocations.len(),
        3,
        "one dynamic segment is created for each workgroup generation"
    );
    for allocation in dynamic_allocations {
        assert!(events.0.iter().any(|event| matches!(
            event.kind,
            SimulationEventKindV1::AllocationReleased { allocation: released }
                if released == allocation
        )));
    }
}

#[test]
fn explicit_dynamic_workgroup_memory_preserves_v9_and_v10_custody() {
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [4, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 4]))],
    );
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let module = dynamic_lds_exchange_module(true);
    let v9 = AdmittedSimulationModuleV1::admit_v9(
        VerifiedCanonicalKernelIrV9::from_module(module.clone()).unwrap(),
        limits,
    )
    .unwrap();
    let v10 = AdmittedSimulationModuleV1::admit_v10(
        VerifiedCanonicalKernelIrV10::from_module(module).unwrap(),
        limits,
    )
    .unwrap();
    for (wire_version, admitted) in [(9, v9), (10, v10)] {
        let execution = admitted
            .simulate_with_dynamic_workgroup_memory(
                &request,
                DynamicWorkgroupMemoryRequestV1::new(16),
                target,
                limits,
            )
            .unwrap();
        assert_eq!(execution.identity().wire_version(), wire_version);
        assert_eq!(words(execution.buffer(0).unwrap().bytes()), vec![0; 4]);
    }
}

#[test]
fn explicit_dynamic_workgroup_memory_validates_layout_and_resource_bounds() {
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let request = SimulationRequestV1::new("unsupported_lds", [1, 1, 1], [1, 1, 1], vec![]);
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        admitted(unsupported_workgroup_memory_module(
            Type::Scalar(scalar),
            WorkgroupMemoryExtent::Dynamic,
        ))
        .simulate_with_dynamic_workgroup_memory(
            &request,
            DynamicWorkgroupMemoryRequestV1::new(8),
            target,
            limits,
        )
        .expect("scalar dynamic segment layout executes");
    }

    let module = admitted(unsupported_workgroup_memory_module(
        Type::Scalar(ScalarType::U32),
        WorkgroupMemoryExtent::Dynamic,
    ));
    module
        .simulate_with_dynamic_workgroup_memory(
            &request,
            DynamicWorkgroupMemoryRequestV1::new(0),
            target,
            limits,
        )
        .expect("zero-byte segment is valid when never dereferenced");
    assert!(matches!(
        module.preflight_with_dynamic_workgroup_memory(
            &request,
            DynamicWorkgroupMemoryRequestV1::new(10),
            target,
            limits,
        ),
        Err(SimulationPreflightErrorV1::DynamicWorkgroupMemory(
            DynamicWorkgroupMemoryUnavailableV1::ByteExtentNotDivisible {
                byte_extent: 10,
                element_bytes: 4,
                ..
            }
        ))
    ));
    assert!(matches!(
        module.preflight_with_dynamic_workgroup_memory(
            &request,
            DynamicWorkgroupMemoryRequestV1::new(4),
            target,
            limits,
        ),
        Err(SimulationPreflightErrorV1::DynamicWorkgroupMemory(
            DynamicWorkgroupMemoryUnavailableV1::ByteExtentNotAligned {
                byte_extent: 4,
                alignment: 8,
                ..
            }
        ))
    ));
    assert_eq!(
        module
            .preflight_with_dynamic_workgroup_memory(
                &request,
                DynamicWorkgroupMemoryRequestV1::new(16),
                target,
                SimulationLimitsV1 {
                    max_allocation_bytes: 15,
                    ..limits
                },
            )
            .unwrap_err(),
        SimulationPreflightErrorV1::ResourceLimit {
            resource: "dynamic workgroup allocation bytes",
            actual: 16,
            limit: 15,
        }
    );
}

#[test]
fn explicit_dynamic_workgroup_memory_rejects_missing_ambiguous_and_undersized_bases() {
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let empty = admitted(empty_kernel_module(
        "no_dynamic_lds",
        Signature::new(vec![], vec![]),
        vec![],
    ));
    assert_eq!(
        empty
            .preflight_with_dynamic_workgroup_memory(
                &SimulationRequestV1::new("no_dynamic_lds", [1, 1, 1], [1, 1, 1], vec![]),
                DynamicWorkgroupMemoryRequestV1::new(0),
                target,
                limits,
            )
            .unwrap_err(),
        SimulationPreflightErrorV1::DynamicWorkgroupMemory(
            DynamicWorkgroupMemoryUnavailableV1::MissingReachableBase
        )
    );

    let called = admitted(called_dynamic_workgroup_memory_module(false));
    called
        .simulate_with_dynamic_workgroup_memory(
            &SimulationRequestV1::new("unsupported_lds", [2, 1, 1], [2, 1, 1], vec![]),
            DynamicWorkgroupMemoryRequestV1::new(8),
            target,
            limits,
        )
        .expect("one dynamic base in a reachable helper is unambiguous");
    assert!(matches!(
        admitted(called_dynamic_workgroup_memory_module(true))
            .preflight_with_dynamic_workgroup_memory(
                &SimulationRequestV1::new("unsupported_lds", [1, 1, 1], [1, 1, 1], vec![]),
                DynamicWorkgroupMemoryRequestV1::new(8),
                target,
                limits,
            ),
        Err(SimulationPreflightErrorV1::DynamicWorkgroupMemory(
            DynamicWorkgroupMemoryUnavailableV1::AmbiguousReachableBases { .. }
        ))
    ));

    let exchange = admitted(dynamic_lds_exchange_module(true));
    assert!(matches!(
        exchange.simulate_with_dynamic_workgroup_memory(
            &SimulationRequestV1::new(
                "lds_exchange",
                [4, 1, 1],
                [4, 1, 1],
                vec![SimulationArgumentV1::Buffer(u32_buffer(&[0; 4]))],
            ),
            DynamicWorkgroupMemoryRequestV1::new(8),
            target,
            limits,
        ),
        Err(SimulationErrorV1::Execution(
            fe2o3_kir_sim::SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::OutOfBounds { .. },
                ..
            }
        ))
    ));
}

#[test]
fn dynamic_workgroup_memory_preserves_partial_initialization_and_publication_failures() {
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let partial = admitted(dynamic_partial_write_module());
    let error = partial
        .simulate_with_dynamic_workgroup_memory(
            &SimulationRequestV1::new("unsupported_lds", [1, 1, 1], [1, 1, 1], vec![]),
            DynamicWorkgroupMemoryRequestV1::new(8),
            target,
            limits,
        )
        .expect_err("writing one element does not initialize adjacent LDS bytes");
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::UninitializedRead { .. },
            ..
        })
    ));

    let unpublished = admitted(dynamic_lds_exchange_module(false));
    let error = unpublished
        .simulate_with_dynamic_workgroup_memory(
            &SimulationRequestV1::new(
                "lds_exchange",
                [2, 1, 1],
                [2, 1, 1],
                vec![SimulationArgumentV1::Buffer(u32_buffer(&[0; 2]))],
            ),
            DynamicWorkgroupMemoryRequestV1::new(8),
            target,
            limits,
        )
        .expect_err("cross-lane dynamic LDS requires barrier publication");
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::WorkgroupUseBeforePublish { .. },
            ..
        })
    ));
}

#[test]
fn dynamic_workgroup_memory_schedule_debug_and_reducer_bind_the_exact_extent() {
    let admitted = admitted(dynamic_lds_exchange_module(true));
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [4, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 4]))],
    );
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let dynamic = DynamicWorkgroupMemoryRequestV1::new(16);
    let recorded = admitted
        .simulate_scheduled_with_dynamic_workgroup_memory(
            &request,
            dynamic,
            target,
            limits,
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 91,
                max_decisions: 32,
            },
        )
        .unwrap();
    let record = recorded.schedule_record().unwrap();
    admitted
        .simulate_scheduled_with_dynamic_workgroup_memory(
            &request,
            dynamic,
            target,
            limits,
            SimulationScheduleRequestV1::Replay(record),
        )
        .expect("same dynamic extent replays");
    assert!(matches!(
        admitted.simulate_scheduled_with_dynamic_workgroup_memory(
            &request,
            DynamicWorkgroupMemoryRequestV1::new(32),
            target,
            limits,
            SimulationScheduleRequestV1::Replay(record),
        ),
        Err(SimulationErrorV1::Execution(
            fe2o3_kir_sim::SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::ScheduleReplay(
                    SimulationScheduleReplayErrorV1::ContextMismatch
                ),
                ..
            }
        ))
    ));

    let mut debug = DebugCollector::default();
    admitted
        .simulate_debugged_with_dynamic_workgroup_memory_and_sink(
            &request,
            dynamic,
            target,
            limits,
            SimulationDebugCaptureLimitsV1::new(16, 256, 16, 1_024).unwrap(),
            &mut debug,
        )
        .expect("dynamic LDS uses the normal debugger path");
    assert!(!debug.0.is_empty());

    let exploration = admitted
        .explore_seeded_schedules_with_dynamic_workgroup_memory(
            &request,
            dynamic,
            target,
            limits,
            SimulationExplorationRequestV1::new(17, 2, 32, 64).unwrap(),
        )
        .expect("dynamic LDS uses the bounded exploration path");
    assert_eq!(exploration.attempted(), 2);
    assert_eq!(exploration.completed(), 2);
    assert_eq!(exploration.failures(), 0);
    let witness = exploration
        .first_no_race()
        .expect("exploration retains a replayable dynamic LDS witness");
    admitted
        .simulate_scheduled_with_dynamic_workgroup_memory(
            &request,
            dynamic,
            target,
            limits,
            SimulationScheduleRequestV1::Replay(witness.schedule()),
        )
        .expect("dynamic LDS exploration witness replays");

    let failing_dynamic = DynamicWorkgroupMemoryRequestV1::new(8);
    let report = admitted
        .reduce_simulation_failure_with_dynamic_workgroup_memory(
            &request,
            failing_dynamic,
            target,
            limits,
            SimulationFailureScheduleV1::Canonical,
            reduction_limits(32),
        )
        .expect("dynamic LDS memory fault reduces");
    assert_eq!(report.fingerprint().class(), "out_of_bounds");
    assert_eq!(
        admitted
            .replay_simulation_failure_reduction_with_dynamic_workgroup_memory(
                &request,
                failing_dynamic,
                target,
                limits,
                &report,
            )
            .unwrap(),
        report.fingerprint().clone()
    );
    assert!(matches!(
        admitted.replay_simulation_failure_reduction_with_dynamic_workgroup_memory(
            &request, dynamic, target, limits, &report,
        ),
        Err(fe2o3_kir_sim::SimulationFailureReductionErrorV1::ReproducerMismatch)
    ));
}

#[test]
fn helper_call_frames_resume_after_cooperative_workgroup_barriers() {
    let admitted = admitted(helper_barrier_exchange_module());
    let execution = admitted
        .simulate(
            &SimulationRequestV1::new(
                "lds_exchange",
                [4, 1, 1],
                [4, 1, 1],
                vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 4]))],
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .expect("helper barrier preserves suspended caller frames");
    assert_eq!(words(execution.buffer(0).unwrap().bytes()), vec![0; 4]);
}

#[test]
fn unpublished_cross_lane_workgroup_read_fails_with_typed_provenance() {
    let admitted = admitted(lds_exchange_module(false));
    let error = admitted
        .simulate(
            &SimulationRequestV1::new(
                "lds_exchange",
                [2, 1, 1],
                [2, 1, 1],
                vec![SimulationArgumentV1::Buffer(u32_buffer(&[0; 2]))],
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .expect_err("cross-lane LDS access requires publication");
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::WorkgroupUseBeforePublish { .. },
            ..
        })
    ));
}

#[test]
fn uninitialized_workgroup_memory_read_remains_a_distinct_failure() {
    let error = admitted(uninitialized_workgroup_memory_module())
        .simulate(
            &SimulationRequestV1::new("uninitialized_lds", [1, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::UninitializedRead { .. },
            ..
        })
    ));
}

#[test]
fn dynamic_and_non_scalar_workgroup_memory_fail_closed_in_preflight() {
    let cases = [
        (
            unsupported_workgroup_memory_module(
                Type::Scalar(ScalarType::U32),
                WorkgroupMemoryExtent::Dynamic,
            ),
            UnsupportedFeatureV1::DynamicWorkgroupMemory,
        ),
        (
            unsupported_workgroup_memory_module(
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
                WorkgroupMemoryExtent::Static(1),
            ),
            UnsupportedFeatureV1::NonScalarMemory,
        ),
    ];
    for (module, expected) in cases {
        let error = admitted(module)
            .preflight(
                &SimulationRequestV1::new("unsupported_lds", [1, 1, 1], [1, 1, 1], vec![]),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap_err();
        let SimulationPreflightErrorV1::Unsupported(report) = error else {
            panic!("expected unsupported LDS feature")
        };
        assert!(
            report
                .findings()
                .iter()
                .any(|finding| finding.feature == expected)
        );
    }
}

#[test]
fn workgroup_memory_resource_limits_have_exact_preflight_boundaries() {
    let admitted = admitted(lds_exchange_module(true));
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0; 2]))],
    );
    let limits = SimulationLimitsV1 {
        max_allocations: 2,
        max_allocation_bytes: 16,
        max_total_bytes: 24,
        ..SimulationLimitsV1::default()
    };
    let plan = admitted
        .preflight(&request, SimulationTargetV1::amdgpu_64(), limits)
        .expect("exact LDS byte and allocation limits");
    admitted
        .simulate(&request, SimulationTargetV1::amdgpu_64(), limits)
        .expect("exact preflight boundary executes");

    for (limits, resource, actual, limit) in [
        (
            SimulationLimitsV1 {
                max_allocation_bytes: 15,
                ..limits
            },
            "static workgroup allocation bytes",
            16,
            15,
        ),
        (
            SimulationLimitsV1 {
                max_total_bytes: 23,
                ..limits
            },
            "live bytes with static workgroup memory",
            24,
            23,
        ),
        (
            SimulationLimitsV1 {
                max_allocations: 1,
                ..limits
            },
            "allocations including workgroup memory",
            2,
            1,
        ),
    ] {
        assert_eq!(
            admitted
                .preflight(&request, SimulationTargetV1::amdgpu_64(), limits)
                .unwrap_err(),
            SimulationPreflightErrorV1::ResourceLimit {
                resource,
                actual,
                limit,
            }
        );
    }

    admitted
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: plan.resident_bytes(),
                ..limits
            },
        )
        .expect("exact LDS resident bound");
    assert!(matches!(
        admitted.preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: plan.resident_bytes() - 1,
                ..limits
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            ..
        })
    ));
}

#[test]
fn divergent_early_exit_at_workgroup_barrier_is_typed_and_deterministic() {
    let admitted = admitted(barrier_failure_module(false));
    let request = SimulationRequestV1::new("barrier_failure", [2, 1, 1], [2, 1, 1], vec![]);
    let run = || {
        admitted
            .simulate(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap_err()
    };
    let first = run();
    let second = run();
    assert_eq!(format!("{first:?}"), format!("{second:?}"));
    assert!(matches!(
        first,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            site: Some(fe2o3_kir_sim::SimulationSiteV1 {
                block: BlockId(2),
                operation: Some(0),
                ..
            }),
            kind: SimulationExecutionErrorKindV1::DivergentWorkgroupBarrier(_),
            ..
        })
    ));
}

#[test]
fn incompatible_same_phase_workgroup_barriers_report_exact_mismatch() {
    let admitted = admitted(barrier_failure_module(true));
    let error = admitted
        .simulate(
            &SimulationRequestV1::new("barrier_failure", [2, 1, 1], [2, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::MismatchedWorkgroupBarrier(_),
            ..
        })
    ));
}

#[test]
fn resident_memory_budget_is_checked_before_execution() {
    let admitted = admitted(empty_kernel_module(
        "resident",
        Signature::new(vec![], vec![]),
        vec![],
    ));
    let request = SimulationRequestV1::new("resident", [1, 1, 1], [1, 1, 1], vec![]);
    let base_limits = SimulationLimitsV1 {
        max_call_depth: 1,
        max_ssa_values: 1,
        max_allocations: 1,
        max_allocation_bytes: 1,
        max_total_bytes: 1,
        max_memory_access_records: 1,
        ..SimulationLimitsV1::default()
    };
    let accounted = admitted
        .preflight(&request, SimulationTargetV1::amdgpu_64(), base_limits)
        .unwrap()
        .resident_bytes();
    let error = admitted
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: accounted - 1,
                ..base_limits
            },
        )
        .unwrap_err();
    assert_eq!(
        error,
        SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual: accounted as u64,
            limit: (accounted - 1) as u64,
        }
    );
}

#[test]
fn admission_decode_peak_has_an_exact_resident_boundary() {
    let module = empty_kernel_module("admission_resident", Signature::new(vec![], vec![]), vec![]);
    let too_small = AdmittedSimulationModuleV1::admit(
        VerifiedCanonicalKernelIrV7::from_module(module.clone()).unwrap(),
        SimulationLimitsV1 {
            max_resident_bytes: 1,
            ..SimulationLimitsV1::default()
        },
    )
    .unwrap_err();
    let rejection = too_small.to_string();
    assert!(rejection.contains("successful-admission limit"));
    assert!(rejection.contains("post-decode setting"));
    let SimulationAdmissionErrorV1::ResidentBytesLimit {
        phase,
        actual,
        limit,
    } = too_small
    else {
        panic!("expected a resident admission bound");
    };
    assert_eq!(phase, "post-decode canonical admission");
    assert_eq!(limit, 1);
    AdmittedSimulationModuleV1::admit(
        VerifiedCanonicalKernelIrV7::from_module(module.clone()).unwrap(),
        SimulationLimitsV1 {
            max_resident_bytes: actual,
            ..SimulationLimitsV1::default()
        },
    )
    .expect("exact admission resident boundary");
    assert!(matches!(
        AdmittedSimulationModuleV1::admit(
            VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
            SimulationLimitsV1 {
                max_resident_bytes: actual - 1,
                ..SimulationLimitsV1::default()
            },
        ),
        Err(SimulationAdmissionErrorV1::ResidentBytesLimit {
            actual: rejected,
            limit,
            ..
        }) if rejected == actual && limit == actual - 1
    ));
}

#[test]
fn resident_accounting_includes_request_id_spare_capacity_and_unreachable_module_bulk() {
    let baseline = empty_kernel_module("resident", Signature::new(vec![], vec![]), vec![]);
    let baseline_admitted = admitted(baseline.clone());
    let limits = SimulationLimitsV1 {
        max_call_depth: 1,
        max_ssa_values: 1,
        max_allocations: 1,
        max_allocation_bytes: 1,
        max_total_bytes: 1,
        max_memory_access_records: 1,
        ..SimulationLimitsV1::default()
    };
    let normal = SimulationRequestV1::new("resident", [1, 1, 1], [1, 1, 1], vec![]);
    let normal_resident = baseline_admitted
        .preflight(&normal, SimulationTargetV1::amdgpu_64(), limits)
        .unwrap()
        .resident_bytes();

    let mut spare_kernel = String::with_capacity(32_768);
    spare_kernel.push_str("resident");
    let spare = SimulationRequestV1::new(spare_kernel, [1, 1, 1], [1, 1, 1], vec![]);
    let spare_resident = baseline_admitted
        .preflight(&spare, SimulationTargetV1::amdgpu_64(), limits)
        .unwrap()
        .resident_bytes();
    assert!(spare_resident >= normal_resident + 32_000);

    let mut unknown_kernel = String::with_capacity(65_536);
    unknown_kernel.push_str("unknown");
    let unknown = SimulationRequestV1::new(unknown_kernel, [1, 1, 1], [1, 1, 1], vec![]);
    assert!(matches!(
        baseline_admitted.preflight(
            &unknown,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: baseline_admitted.admitted_resident_bytes() + 1_024,
                ..limits
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            ..
        })
    ));

    let mut bulk = baseline;
    for ordinal in 0..256 {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        bulk.functions.push(Function::internal_helper(
            format!("unreachable_{ordinal:04}_{}", "x".repeat(128)),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
    }
    let bulk_admitted = admitted(bulk);
    assert!(
        bulk_admitted.admitted_resident_bytes()
            > baseline_admitted.admitted_resident_bytes() + 32_000
    );
    let bulk_resident = bulk_admitted
        .preflight(&normal, SimulationTargetV1::amdgpu_64(), limits)
        .unwrap()
        .resident_bytes();
    assert!(bulk_resident > normal_resident + 32_000);
    assert!(matches!(
        bulk_admitted.preflight(
            &normal,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: bulk_admitted.admitted_resident_bytes() + 1_024,
                ..limits
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            ..
        })
    ));
}

#[test]
fn many_zero_byte_arguments_have_an_exact_resident_boundary() {
    const ARGUMENTS: usize = 256;
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let admitted = admitted(empty_kernel_module(
        "zero_resident",
        Signature::new(vec![pointer; ARGUMENTS], vec![]),
        (0..ARGUMENTS).map(|index| ValueId(index as u32)).collect(),
    ));
    let request = SimulationRequestV1::new(
        "zero_resident",
        [1, 1, 1],
        [1, 1, 1],
        (0..ARGUMENTS)
            .map(|_| SimulationArgumentV1::Buffer(byte_buffer(&[])))
            .collect(),
    );
    let base_limits = SimulationLimitsV1 {
        max_call_depth: 1,
        max_ssa_values: ARGUMENTS,
        max_allocations: ARGUMENTS,
        max_allocation_bytes: 1,
        max_total_bytes: 1,
        max_memory_access_records: 1,
        ..SimulationLimitsV1::default()
    };
    let accounted = admitted
        .preflight(&request, SimulationTargetV1::amdgpu_64(), base_limits)
        .unwrap()
        .resident_bytes();
    admitted
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: accounted,
                ..base_limits
            },
        )
        .expect("the exact accounted resident boundary is admitted");
    assert_eq!(
        admitted
            .preflight(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1 {
                    max_resident_bytes: accounted - 1,
                    ..base_limits
                },
            )
            .unwrap_err(),
        SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual: accounted as u64,
            limit: (accounted - 1) as u64,
        }
    );
}

#[test]
fn d2_padding_is_included_in_the_schedule_bound() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "d2_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let mut module = Module::new("sim-tests::d2");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "d2",
        "d2_impl",
        LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
        },
    ));
    let request = SimulationRequestV1::new("d2", [3, 3, 1], [2, 2, 1], vec![]);
    let plan = admitted(module)
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(plan.invocations(), 9);
    assert_eq!(plan.scheduled_slots(), 16);
}

#[test]
fn index_values_and_buffers_are_bound_to_their_construction_layout() {
    let scalar_module = empty_kernel_module(
        "index_scalar",
        Signature::new(vec![Type::INDEX], vec![]),
        vec![ValueId(0)],
    );
    let scalar_request = SimulationRequestV1::new(
        "index_scalar",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Scalar(
            ScalarBitsV1::index(1, SimulationTargetV1::amdgpu_64()).unwrap(),
        )],
    );
    assert_eq!(
        admitted(scalar_module)
            .preflight(
                &scalar_request,
                SimulationTargetV1::little_endian(fe2o3_kir_sim::IndexWidthV1::Bits32),
                SimulationLimitsV1::default(),
            )
            .unwrap_err(),
        SimulationPreflightErrorV1::TargetLayout { argument: 0 }
    );

    let pointer = Type::pointer(Type::INDEX, AddressSpace::Global, AccessMode::ReadWrite);
    let buffer_module = empty_kernel_module(
        "index_buffer",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
    );
    let buffer = BufferArgumentV1::new(
        ScalarType::Index,
        AccessMode::ReadWrite,
        8,
        vec![0; 8],
        vec![true; 8],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let buffer_request = SimulationRequestV1::new(
        "index_buffer",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(buffer)],
    );
    assert!(matches!(
        admitted(buffer_module).preflight(
            &buffer_request,
            SimulationTargetV1::little_endian(fe2o3_kir_sim::IndexWidthV1::Bits32),
            SimulationLimitsV1::default(),
        ),
        Err(SimulationPreflightErrorV1::TargetLayout { argument: 0 })
    ));

    let fixed_module = empty_kernel_module(
        "fixed_scalar",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
    );
    let fixed_request = SimulationRequestV1::new(
        "fixed_scalar",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7))],
    );
    admitted(fixed_module)
        .preflight(
            &fixed_request,
            SimulationTargetV1::little_endian(fe2o3_kir_sim::IndexWidthV1::Bits32),
            SimulationLimitsV1::default(),
        )
        .expect("fixed-width values are layout portable");
}

#[test]
fn bool_bitwise_and_mixed_width_shift_have_admitted_execution_semantics() {
    let u64_ty = Type::Scalar(ScalarType::U64);
    let output = Type::pointer(u64_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(1, Type::BOOL, OperationKind::Constant(Constant::Bool(true))),
        op(
            2,
            Type::BOOL,
            OperationKind::Constant(Constant::Bool(false)),
        ),
        op(
            3,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        op(4, u64_ty.clone(), OperationKind::Constant(Constant::U64(8))),
        op(
            5,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(1)),
        ),
        op(
            6,
            u64_ty,
            OperationKind::Binary {
                op: BinaryOp::ShiftRight,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 8),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "scalar_profile_impl",
        Signature::new(vec![output], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::scalar-profile");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "scalar_profile",
        "scalar_profile_impl",
        dynamic_domain_1d(),
    ));
    let output = BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        8,
        &[ScalarBitsV1::new(ScalarType::U64, 0, SimulationTargetV1::amdgpu_64()).unwrap()],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "scalar_profile",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(output)],
    );
    let execution = admitted(module)
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(
        u64::from_le_bytes(execution.buffer(0).unwrap().bytes().try_into().unwrap()),
        4
    );
}

#[derive(Default)]
struct StopAfterFirst {
    events: Vec<SimulationEventV1>,
}

#[derive(Default)]
struct DropAndStopAtFirst {
    callbacks: usize,
}

impl SimulationEventSinkV1 for DropAndStopAtFirst {
    fn record(
        &mut self,
        _event: &SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        unreachable!("the controlled callback is overridden")
    }

    fn record_controlled(
        &mut self,
        _event: &SimulationEventV1,
    ) -> Result<SimulationEventSinkControlV1, fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        self.callbacks += 1;
        Ok(SimulationEventSinkControlV1::DropAndStop)
    }
}

impl SimulationEventSinkV1 for StopAfterFirst {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        self.events.push(event.clone());
        Ok(())
    }

    fn record_controlled(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<SimulationEventSinkControlV1, fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        self.events.push(event.clone());
        Ok(SimulationEventSinkControlV1::Stop)
    }
}

#[test]
fn nonfatal_sink_stop_preserves_execution_under_a_tiny_event_budget() {
    let admitted = admitted(indexed_store_module());
    let request = SimulationRequestV1::new(
        "store",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    let limits = SimulationLimitsV1 {
        max_events: 2,
        ..SimulationLimitsV1::default()
    };
    let baseline = admitted
        .simulate(&request, SimulationTargetV1::amdgpu_64(), limits)
        .unwrap();
    let mut sink = StopAfterFirst::default();
    let observed = admitted
        .simulate_observed_with_sink(&request, SimulationTargetV1::amdgpu_64(), limits, &mut sink)
        .unwrap();

    assert_eq!(sink.events.len(), 1);
    assert!(matches!(
        sink.events[0].kind,
        SimulationEventKindV1::InvocationBegin
    ));
    assert_eq!(observed.events_emitted(), 1);
    assert_eq!(observed.arguments(), baseline.arguments());
    assert_eq!(observed.shared_buffers(), baseline.shared_buffers());
    assert_eq!(
        observed.invocations_executed(),
        baseline.invocations_executed()
    );
    assert_eq!(observed.steps_executed(), baseline.steps_executed());
    assert_eq!(observed.schedule(), baseline.schedule());
    assert_eq!(
        observed.conflict_assessment(),
        baseline.conflict_assessment()
    );
}

#[test]
fn nonretained_sink_stop_excludes_the_current_event_from_accounting() {
    let admitted = admitted(indexed_store_module());
    let request = SimulationRequestV1::new(
        "store",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    let limits = SimulationLimitsV1 {
        max_events: 2,
        ..SimulationLimitsV1::default()
    };
    let baseline = admitted
        .simulate(&request, SimulationTargetV1::amdgpu_64(), limits)
        .unwrap();
    let mut sink = DropAndStopAtFirst::default();
    let observed = admitted
        .simulate_observed_with_sink(&request, SimulationTargetV1::amdgpu_64(), limits, &mut sink)
        .unwrap();

    assert_eq!(sink.callbacks, 1);
    assert_eq!(observed.events_emitted(), 0);
    assert_eq!(observed.arguments(), baseline.arguments());
    assert_eq!(observed.invocations_executed(), 1);
    assert_eq!(observed.steps_executed(), baseline.steps_executed());
}

#[test]
fn failed_store_preparation_emits_no_completed_write_for_that_invocation() {
    let admitted = admitted(indexed_store_module());
    let mut request = SimulationRequestV1::new(
        "store",
        [2, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    request.events = EventPolicyV1::Enabled;
    let mut events = Collector::default();
    let error = admitted
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::OutOfBounds { .. },
            ..
        })
    ));
    assert!(!events.0.iter().any(|event| {
        event.invocation.global == [1, 0, 0]
            && matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. })
    }));
    assert!(events.0.iter().any(|event| {
        event.invocation.global == [1, 0, 0]
            && matches!(
                event.kind,
                SimulationEventKindV1::OperationEnd {
                    outcome: SimulationExecutionOutcomeV1::Failed
                }
            )
    }));
    assert!(matches!(
        events.0.last().map(|event| &event.kind),
        Some(SimulationEventKindV1::InvocationEnd {
            outcome: SimulationExecutionOutcomeV1::Failed
        })
    ));
}

fn recursive_module(mutual: bool) -> Module {
    fn caller(name: &str, callee: &str, kernel: bool) -> Function {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: callee.into(),
                arguments: vec![],
            },
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        if kernel {
            Function::kernel_entry(name, Signature::new(vec![], vec![]), vec![], vec![block])
        } else {
            Function::internal_helper(name, Signature::new(vec![], vec![]), vec![], vec![block])
        }
    }

    let mut module = Module::new(if mutual {
        "sim-tests::mutual-recursion"
    } else {
        "sim-tests::direct-recursion"
    });
    module.functions.push(caller("recursive_impl", "a", true));
    module
        .functions
        .push(caller("a", if mutual { "b" } else { "a" }, false));
    if mutual {
        module.functions.push(caller("b", "a", false));
    }
    module.kernels.push(Kernel::new(
        "recursive",
        "recursive_impl",
        dynamic_domain_1d(),
    ));
    module
}

fn call_graph_function(name: &str, callees: &[&str], kernel: bool) -> Function {
    let mut block = BasicBlock::new(BlockId(0));
    for callee in callees {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: (*callee).into(),
                arguments: vec![],
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    if kernel {
        Function::kernel_entry(name, Signature::new(vec![], vec![]), vec![], vec![block])
    } else {
        Function::internal_helper(name, Signature::new(vec![], vec![]), vec![], vec![block])
    }
}

fn recursive_and_acyclic_branch_module() -> Module {
    let mut module = Module::new("sim-tests::recursive-and-acyclic-branch");
    module
        .functions
        .push(call_graph_function("entry", &["cycle", "chain0"], true));
    module
        .functions
        .push(call_graph_function("cycle", &["cycle"], false));
    module
        .functions
        .push(call_graph_function("chain0", &["chain1"], false));
    module
        .functions
        .push(call_graph_function("chain1", &["chain2"], false));
    module
        .functions
        .push(call_graph_function("chain2", &[], false));
    module
        .kernels
        .push(Kernel::new("graph", "entry", dynamic_domain_1d()));
    module
}

fn long_prefix_into_cycle_module() -> Module {
    let mut module = Module::new("sim-tests::long-prefix-into-cycle");
    module
        .functions
        .push(call_graph_function("entry", &["prefix1"], true));
    module
        .functions
        .push(call_graph_function("prefix1", &["prefix2"], false));
    module
        .functions
        .push(call_graph_function("prefix2", &["cycle"], false));
    module
        .functions
        .push(call_graph_function("cycle", &["cycle"], false));
    module
        .kernels
        .push(Kernel::new("graph", "entry", dynamic_domain_1d()));
    module
}

fn repeated_call_module(repetitions: usize, external: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.reserve(repetitions);
    for _ in 0..repetitions {
        entry.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "target".into(),
                arguments: vec![],
            },
        ));
    }
    entry.terminator = Some(Terminator::Return { values: vec![] });

    let target = if external {
        Function::external_import("target", Signature::new(vec![], vec![]))
    } else {
        call_graph_function("target", &[], false)
    };
    let mut module = Module::new("sim-tests::repeated-call");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    module.functions.push(target);
    module
        .kernels
        .push(Kernel::new("repeated", "entry", dynamic_domain_1d()));
    module
}

fn repeated_long_external_call_module(repetitions: usize, identifier_bytes: usize) -> Module {
    let entry_name = format!("entry_{}", "e".repeat(identifier_bytes));
    let callee_name = format!("callee_{}", "c".repeat(identifier_bytes));
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.reserve(repetitions);
    for _ in 0..repetitions {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: callee_name.clone().into(),
                arguments: vec![],
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("sim-tests::long-unsupported");
    module.functions.push(Function::kernel_entry(
        entry_name.clone(),
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.functions.push(Function::external_import(
        callee_name,
        Signature::new(vec![], vec![]),
    ));
    module.kernels.push(Kernel::new(
        "long_unsupported",
        entry_name,
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn repeated_callees_use_a_bounded_worklist_and_preserve_occurrence_findings() {
    let request = SimulationRequestV1::new("repeated", [1, 1, 1], [1, 1, 1], vec![]);
    let plan = admitted(repeated_call_module(4_096, false))
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_reachable_functions: 2,
                ..SimulationLimitsV1::default()
            },
        )
        .expect("one repeated callee occupies one worklist slot");
    assert_eq!(plan.reachable_functions(), 2);
    assert_eq!(plan.reachable_operations(), 4_096);

    let error = admitted(repeated_call_module(32, true))
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("expected unsupported external call occurrences")
    };
    assert_eq!(report.total_findings(), 32);
    assert!(!report.is_truncated());
}

#[test]
fn long_unsupported_identifiers_are_counted_without_exceeding_retained_prefix_bounds() {
    let repetitions = MAX_REPORTED_UNSUPPORTED_FINDINGS_V1 + 257;
    let request = SimulationRequestV1::new("long_unsupported", [1, 1, 1], [1, 1, 1], vec![]);
    let error = admitted(repeated_long_external_call_module(repetitions, 2_048))
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("expected bounded unsupported report")
    };
    assert_eq!(report.total_findings(), repetitions as u64);
    assert!(report.is_truncated());
    assert!(report.findings().len() < MAX_REPORTED_UNSUPPORTED_FINDINGS_V1);
    let retained_identifier_bytes = report
        .findings()
        .iter()
        .map(|finding| {
            let callee_bytes = match &finding.feature {
                UnsupportedFeatureV1::ExternalCall(callee) => callee.as_str().len(),
                _ => 0,
            };
            finding.function.as_str().len() + callee_bytes
        })
        .sum::<usize>();
    assert!(retained_identifier_bytes <= MAX_REPORTED_UNSUPPORTED_IDENTIFIER_BYTES_V1);
}

#[test]
fn recursive_scc_does_not_hide_an_over_limit_acyclic_branch() {
    let admitted = admitted(recursive_and_acyclic_branch_module());
    let request = SimulationRequestV1::new("graph", [1, 1, 1], [1, 1, 1], vec![]);
    assert!(matches!(
        admitted.preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_call_depth: 3,
                ..SimulationLimitsV1::default()
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "acyclic call depth",
            actual: 4,
            limit: 3,
        })
    ));
}

#[test]
fn long_prefix_into_recursive_scc_is_depth_checked_without_rejecting_recursion() {
    let admitted = admitted(long_prefix_into_cycle_module());
    let request = SimulationRequestV1::new("graph", [1, 1, 1], [1, 1, 1], vec![]);
    assert!(matches!(
        admitted.preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_call_depth: 3,
                ..SimulationLimitsV1::default()
            },
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "acyclic call depth",
            actual: 4,
            limit: 3,
        })
    ));
    admitted
        .preflight(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_call_depth: 4,
                ..SimulationLimitsV1::default()
            },
        )
        .expect("the bounded recursive SCC remains admitted");
}

#[test]
fn direct_and_mutual_recursion_are_preflighted_and_fail_with_closed_lifecycles() {
    for mutual in [false, true] {
        let admitted = admitted(recursive_module(mutual));
        let mut request = SimulationRequestV1::new("recursive", [1, 1, 1], [1, 1, 1], vec![]);
        request.events = EventPolicyV1::Enabled;
        let limits = SimulationLimitsV1 {
            max_call_depth: 4,
            ..SimulationLimitsV1::default()
        };
        admitted
            .preflight(&request, SimulationTargetV1::amdgpu_64(), limits)
            .expect("bounded recursion remains admitted");
        let mut events = Collector::default();
        let error = admitted
            .simulate_with_sink(
                &request,
                SimulationTargetV1::amdgpu_64(),
                limits,
                &mut events,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::CallDepthLimit { limit: 4 },
                ..
            })
        ));
        assert!(events.0.iter().any(|event| matches!(
            event.kind,
            SimulationEventKindV1::OperationEnd {
                outcome: SimulationExecutionOutcomeV1::Failed
            }
        )));
        assert!(matches!(
            events.0.last().map(|event| &event.kind),
            Some(SimulationEventKindV1::InvocationEnd {
                outcome: SimulationExecutionOutcomeV1::Failed
            })
        ));
    }
}

#[derive(Clone, Copy)]
enum AllocationFailureShape {
    Direct,
    Nested,
    Recursive,
}

fn two_allocations(start: u32) -> Vec<Operation> {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    vec![
        op(
            start,
            pointer.clone(),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U8),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 1,
            },
        ),
        op(
            start + 1,
            pointer,
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U8),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 1,
            },
        ),
    ]
}

fn allocation_failure_module(shape: AllocationFailureShape) -> Module {
    let mut entry_block = BasicBlock::new(BlockId(0));
    entry_block.operations = match shape {
        AllocationFailureShape::Direct | AllocationFailureShape::Nested => two_allocations(0),
        AllocationFailureShape::Recursive => Vec::new(),
    };
    match shape {
        AllocationFailureShape::Direct => {
            entry_block.terminator = Some(Terminator::Unreachable);
        }
        AllocationFailureShape::Nested | AllocationFailureShape::Recursive => {
            entry_block.operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "allocator".into(),
                    arguments: vec![],
                },
            ));
            entry_block.terminator = Some(Terminator::Return { values: vec![] });
        }
    }
    let entry = Function::kernel_entry(
        "allocation_failure_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry_block],
    );
    let mut module = Module::new("sim-tests::allocation-unwind");
    module.functions.push(entry);
    if !matches!(shape, AllocationFailureShape::Direct) {
        let mut helper_block = BasicBlock::new(BlockId(0));
        helper_block.operations = two_allocations(0);
        if matches!(shape, AllocationFailureShape::Recursive) {
            helper_block.operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "allocator".into(),
                    arguments: vec![],
                },
            ));
            helper_block.terminator = Some(Terminator::Return { values: vec![] });
        } else {
            helper_block.terminator = Some(Terminator::Unreachable);
        }
        module.functions.push(Function::internal_helper(
            "allocator",
            Signature::new(vec![], vec![]),
            vec![],
            vec![helper_block],
        ));
    }
    module.kernels.push(Kernel::new(
        "allocation_failure",
        "allocation_failure_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn direct_nested_and_recursive_failure_unwind_observes_every_private_release() {
    for shape in [
        AllocationFailureShape::Direct,
        AllocationFailureShape::Nested,
        AllocationFailureShape::Recursive,
    ] {
        let admitted = admitted(allocation_failure_module(shape));
        let mut request =
            SimulationRequestV1::new("allocation_failure", [1, 1, 1], [1, 1, 1], vec![]);
        request.events = EventPolicyV1::Enabled;
        let mut events = Collector::default();
        let error = admitted
            .simulate_with_sink(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1 {
                    max_call_depth: 4,
                    ..SimulationLimitsV1::default()
                },
                &mut events,
            )
            .unwrap_err();
        assert!(matches!(
            error,
            SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::ReachedUnreachable
                    | SimulationExecutionErrorKindV1::CallDepthLimit { .. },
                ..
            })
        ));
        let created = events
            .0
            .iter()
            .filter_map(|event| match event.kind {
                SimulationEventKindV1::AllocationCreated { allocation, .. } => Some(allocation),
                _ => None,
            })
            .collect::<Vec<_>>();
        let released = events
            .0
            .iter()
            .filter_map(|event| match event.kind {
                SimulationEventKindV1::AllocationReleased { allocation } => Some(allocation),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!created.is_empty());
        assert_eq!(released, created.iter().rev().copied().collect::<Vec<_>>());
    }
}

struct RejectFailedOperationEnd {
    retained: Vec<SimulationEventV1>,
}

impl SimulationEventSinkV1 for RejectFailedOperationEnd {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        if matches!(
            event.kind,
            SimulationEventKindV1::OperationEnd {
                outcome: SimulationExecutionOutcomeV1::Failed
            }
        ) {
            return Err(fe2o3_kir_sim::SimulationEventSinkErrorV1 {
                detail: "failed operation end rejected".into(),
            });
        }
        self.retained.push(event.clone());
        Ok(())
    }
}

#[test]
fn primary_dynamic_failure_preserves_secondary_lifecycle_observation_failure() {
    let admitted = admitted(indexed_store_module());
    let mut request = SimulationRequestV1::new(
        "store",
        [2, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    request.events = EventPolicyV1::Enabled;
    let mut sink = RejectFailedOperationEnd {
        retained: Vec::new(),
    };
    let error = admitted
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut sink,
        )
        .unwrap_err();
    let SimulationErrorV1::Execution(error) = error else {
        panic!("expected execution failure")
    };
    assert!(matches!(
        error.kind,
        SimulationExecutionErrorKindV1::OutOfBounds { .. }
    ));
    assert!(matches!(
        error
            .observation_failure
            .as_ref()
            .map(|failure| &failure.kind),
        Some(SimulationExecutionErrorKindV1::EventSinkFailure(_))
    ));
    assert!(sink.retained.iter().any(|event| {
        event.invocation.global == [1, 0, 0]
            && matches!(event.kind, SimulationEventKindV1::OperationBegin)
    }));
    assert!(!sink.retained.iter().any(|event| {
        event.invocation.global == [1, 0, 0]
            && matches!(
                event.kind,
                SimulationEventKindV1::OperationEnd {
                    outcome: SimulationExecutionOutcomeV1::Failed
                }
            )
    }));
}

struct RejectFailedInvocationEnd;

impl SimulationEventSinkV1 for RejectFailedInvocationEnd {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        if matches!(
            event.kind,
            SimulationEventKindV1::InvocationEnd {
                outcome: SimulationExecutionOutcomeV1::Failed
            }
        ) {
            return Err(fe2o3_kir_sim::SimulationEventSinkErrorV1 {
                detail: "failed invocation end rejected".into(),
            });
        }
        Ok(())
    }
}

#[test]
fn primary_failure_preserves_rejected_invocation_end_as_secondary() {
    let admitted = admitted(indexed_store_module());
    let mut request = SimulationRequestV1::new(
        "store",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    request.events = EventPolicyV1::Enabled;
    let error = admitted
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_steps: 1,
                ..SimulationLimitsV1::default()
            },
            &mut RejectFailedInvocationEnd,
        )
        .unwrap_err();
    let SimulationErrorV1::Execution(error) = error else {
        panic!("expected execution failure")
    };
    assert!(matches!(
        error.kind,
        SimulationExecutionErrorKindV1::StepLimit { limit: 1 }
    ));
    let secondary = error
        .observation_failure
        .expect("rejected invocation end remains machine-readable");
    assert!(matches!(
        secondary.kind,
        SimulationExecutionErrorKindV1::EventSinkFailure(_)
    ));
    assert!(matches!(
        secondary.site,
        Some(fe2o3_kir_sim::SimulationSiteV1 {
            operation: None,
            ..
        })
    ));
}

#[test]
fn reserved_invocation_end_closes_an_event_limit_failure_exactly() {
    let admitted = admitted(empty_kernel_module(
        "event_closure",
        Signature::new(vec![], vec![]),
        vec![],
    ));
    let mut request = SimulationRequestV1::new("event_closure", [1, 1, 1], [1, 1, 1], vec![]);
    request.events = EventPolicyV1::Enabled;
    let mut events = Collector::default();
    let error = admitted
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_events: 2,
                ..SimulationLimitsV1::default()
            },
            &mut events,
        )
        .unwrap_err();
    let SimulationErrorV1::Execution(error) = error else {
        panic!("expected event limit")
    };
    assert!(matches!(
        error.kind,
        SimulationExecutionErrorKindV1::EventLimit { limit: 2 }
    ));
    assert!(error.observation_failure.is_none());
    assert_eq!(events.0.len(), 2);
    assert!(matches!(
        events.0[0].kind,
        SimulationEventKindV1::InvocationBegin
    ));
    assert!(matches!(
        events.0[1].kind,
        SimulationEventKindV1::InvocationEnd {
            outcome: SimulationExecutionOutcomeV1::Failed
        }
    ));
}

fn deep_acyclic_module(depth: usize) -> Module {
    let mut module = Module::new("sim-tests::deep-acyclic");
    for index in 0..depth {
        let mut block = BasicBlock::new(BlockId(0));
        if index + 1 < depth {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: format!("f{}", index + 1).into(),
                    arguments: vec![],
                },
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let name = format!("f{index}");
        let function = if index == 0 {
            Function::kernel_entry(name, Signature::new(vec![], vec![]), vec![], vec![block])
        } else {
            Function::internal_helper(name, Signature::new(vec![], vec![]), vec![], vec![block])
        };
        module.functions.push(function);
    }
    module
        .kernels
        .push(Kernel::new("deep", "f0", dynamic_domain_1d()));
    module
}

#[test]
fn maximum_call_chain_and_recursive_limit_run_on_a_small_native_stack() {
    let deep = admitted(deep_acyclic_module(1_024));
    let recursive = admitted(recursive_module(true));
    std::thread::Builder::new()
        .name("kir-sim-small-stack".into())
        .stack_size(128 * 1024)
        .spawn(move || {
            let limits = SimulationLimitsV1 {
                max_call_depth: 1_024,
                max_ssa_values: 1,
                ..SimulationLimitsV1::default()
            };
            let request = SimulationRequestV1::new("deep", [1, 1, 1], [1, 1, 1], vec![]);
            deep.preflight(&request, SimulationTargetV1::amdgpu_64(), limits)
                .expect("maximum acyclic depth preflights iteratively");
            deep.simulate(&request, SimulationTargetV1::amdgpu_64(), limits)
                .expect("maximum acyclic depth executes iteratively");

            let request = SimulationRequestV1::new("recursive", [1, 1, 1], [1, 1, 1], vec![]);
            let error = recursive
                .simulate(
                    &request,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1 {
                        max_call_depth: 1_024,
                        max_ssa_values: 1,
                        ..SimulationLimitsV1::default()
                    },
                )
                .unwrap_err();
            assert!(matches!(
                error,
                SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::CallDepthLimit { limit: 1_024 },
                    ..
                })
            ));
        })
        .expect("small-stack thread starts")
        .join()
        .expect("iterative simulator does not overflow the native stack");
}

#[test]
fn persisted_schedule_codec_and_replay_run_on_a_small_native_stack() {
    let canonical = VerifiedCanonicalKernelIrV7::from_module(empty_kernel_module(
        "empty_schedule",
        Signature::new(vec![], vec![]),
        vec![],
    ))
    .expect("verified fixture");
    let identity = *canonical.identity();
    let admitted = AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default())
        .expect("admitted fixture");
    std::thread::Builder::new()
        .name("kir-sim-persisted-schedule-small-stack".into())
        .stack_size(256 * 1024)
        .spawn(move || persisted_schedule_small_stack_body(&admitted, identity))
        .expect("small-stack thread starts")
        .join()
        .expect("persisted schedule stays iterative");
}

#[inline(never)]
fn persisted_schedule_small_stack_body(
    admitted: &AdmittedSimulationModuleV1,
    identity: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV7,
) {
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let request = SimulationRequestV1::new("empty_schedule", [4, 1, 1], [4, 1, 1], vec![]);
    let recorded = admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 17,
                max_decisions: 64,
            },
        )
        .unwrap();
    let binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV7,
        identity,
        [0x51; 32],
        87,
        target,
        limits,
    );
    let bytes = PersistedSimulationScheduleDocumentV1::encode_record(
        binding,
        recorded.schedule_record().unwrap(),
    )
    .unwrap();
    let persisted = PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&bytes).unwrap();
    admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::Replay(persisted.record()),
        )
        .unwrap();
}

#[test]
fn high_invocation_tiny_kernel_does_not_reserve_limit_sized_frames() {
    let admitted = admitted(empty_kernel_module(
        "tiny_many",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
    ));
    let request = SimulationRequestV1::new(
        "tiny_many",
        [1_048_576, 1, 1],
        [256, 1, 1],
        vec![SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7))],
    );
    let execution = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_call_depth: 1_024,
                max_ssa_values: 1 << 20,
                max_allocations: 1,
                max_allocation_bytes: 1,
                max_total_bytes: 1,
                max_memory_access_records: 1,
                ..SimulationLimitsV1::default()
            },
        )
        .expect("tiny invocations reserve only their actual frame state");
    assert_eq!(execution.invocations_executed(), 1_048_576);
    assert_eq!(execution.steps_executed(), 1_048_576);
}

fn late_block_loop_module(block_count: usize) -> Module {
    let mut blocks = Vec::with_capacity(block_count);
    for index in 0..block_count {
        let mut block = BasicBlock::new(BlockId(u32::try_from(index).unwrap()));
        let next = if index + 1 == block_count {
            index
        } else {
            index + 1
        };
        block.terminator = Some(Terminator::Branch {
            target: BlockId(u32::try_from(next).unwrap()),
            arguments: vec![],
        });
        blocks.push(block);
    }
    let mut module = Module::new("sim-tests::late-block-loop");
    module.functions.push(Function::kernel_entry(
        "late_block_impl",
        Signature::new(vec![], vec![]),
        vec![],
        blocks,
    ));
    module.kernels.push(Kernel::new(
        "late_block",
        "late_block_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn many_block_late_loop_uses_precomputed_block_indices() {
    let block_count = 16_384;
    let admitted = admitted(late_block_loop_module(block_count));
    let request = SimulationRequestV1::new("late_block", [1, 1, 1], [1, 1, 1], vec![]);
    let error = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_steps: block_count as u64 + 1_024,
                ..SimulationLimitsV1::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { .. },
            ..
        })
    ));
}

fn long_identifier_call_loop_module(identifier_bytes: usize) -> Module {
    let entry_name = format!("entry_{}", "e".repeat(identifier_bytes));
    let callee_name = format!("callee_{}", "c".repeat(identifier_bytes));
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: callee_name.clone().into(),
            arguments: vec![],
        },
    ));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![],
    });
    let mut callee = BasicBlock::new(BlockId(0));
    callee.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("sim-tests::long-id-call-loop");
    module.functions.push(Function::kernel_entry(
        entry_name.clone(),
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    module.functions.push(Function::internal_helper(
        callee_name,
        Signature::new(vec![], vec![]),
        vec![],
        vec![callee],
    ));
    module.kernels.push(Kernel::new(
        "long_call_loop",
        entry_name,
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn long_identifier_call_loop_uses_numeric_runtime_and_event_sites() {
    let admitted = admitted(long_identifier_call_loop_module(4_000));
    let request = SimulationRequestV1::new("long_call_loop", [1, 1, 1], [1, 1, 1], vec![]);
    let limits = SimulationLimitsV1 {
        max_steps: 8_192,
        max_events: 65_536,
        ..SimulationLimitsV1::default()
    };
    let disabled_error = admitted
        .simulate(&request, SimulationTargetV1::amdgpu_64(), limits)
        .unwrap_err();
    assert!(matches!(
        disabled_error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { .. },
            ..
        })
    ));

    let mut events = Collector::default();
    let observed_error = admitted
        .simulate_observed_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            &mut events,
        )
        .unwrap_err();
    assert!(matches!(
        observed_error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { .. },
            ..
        })
    ));
    assert!(events.0.len() > 8_192);
    assert!(events.0.iter().all(|event| event.site.function_ordinal < 2));
}

fn nonempty_helper_call_loop_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(10)],
    });
    let mut loop_block = BasicBlock::new(BlockId(1));
    loop_block.parameters = vec![ValueDef::new(ValueId(0), scalar.clone())];
    loop_block.operations.push(op(
        1,
        scalar.clone(),
        OperationKind::Call {
            callee: "echo".into(),
            arguments: vec![ValueId(0)],
        },
    ));
    loop_block.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(1)],
    });

    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(20)],
    });
    let mut module = Module::new("sim-tests::nonempty-helper-loop");
    module.functions.push(Function::kernel_entry(
        "nonempty_loop_impl",
        Signature::new(vec![scalar.clone()], vec![]),
        vec![ValueId(10)],
        vec![entry, loop_block],
    ));
    module.functions.push(Function::internal_helper(
        "echo",
        Signature::new(vec![scalar.clone()], vec![scalar]),
        vec![ValueId(20)],
        vec![helper_block],
    ));
    module.kernels.push(Kernel::new(
        "nonempty_loop",
        "nonempty_loop_impl",
        dynamic_domain_1d(),
    ));
    module
}

#[test]
fn repeated_nonempty_helper_calls_reuse_frame_and_value_scratch() {
    let admitted = admitted(nonempty_helper_call_loop_module());
    let request = SimulationRequestV1::new(
        "nonempty_loop",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7))],
    );
    let error = admitted
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_steps: 100_000,
                ..SimulationLimitsV1::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { .. },
            ..
        })
    ));
}

fn conditional_load_module(guarded: bool, volatile: bool) -> Module {
    let element = Type::Scalar(ScalarType::U8);
    let slice = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    let mut guarded_access = MemoryAccess::new(AddressSpace::Global, 1);
    guarded_access.volatile = volatile;
    block.operations = vec![
        op(
            4,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(5, pointer, OperationKind::SliceData { slice: ValueId(1) }),
        op(
            6,
            element,
            if guarded {
                OperationKind::GuardedLoad {
                    pointer: ValueId(4),
                    predicate: ValueId(2),
                    fallback: ValueId(3),
                    access: guarded_access,
                }
            } else {
                OperationKind::Load {
                    pointer: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Global, 1),
                }
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(5),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });

    let entry = Function::kernel_entry(
        "conditional_load_impl",
        Signature::new(
            vec![
                slice.clone(),
                slice,
                Type::BOOL,
                Type::Scalar(ScalarType::U8),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    let mut module = Module::new("sim-tests::conditional-load");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "conditional_load",
        "conditional_load_impl",
        dynamic_domain_1d(),
    ));
    module
}

fn empty_u8_buffer() -> BufferArgumentV1 {
    byte_buffer(&[])
}

fn conditional_load_request(predicate: bool, input: BufferArgumentV1) -> SimulationRequestV1 {
    SimulationRequestV1::new(
        "conditional_load",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(input),
            SimulationArgumentV1::Buffer(byte_buffer(&[0])),
            SimulationArgumentV1::Scalar(ScalarBitsV1::boolean(predicate)),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(ScalarType::U8, 99, SimulationTargetV1::amdgpu_64()).unwrap(),
            ),
        ],
    )
}

#[test]
fn guarded_load_false_uses_fallback_without_reading_or_emitting_a_read() {
    let admitted = admitted(conditional_load_module(true, false));
    let mut request = conditional_load_request(false, empty_u8_buffer());
    request.events = EventPolicyV1::Enabled;
    let mut events = Collector::default();
    let execution = admitted
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .expect("false guarded load is non-speculative");

    assert_eq!(execution.buffer(1).unwrap().bytes(), &[99]);
    assert_eq!(
        events
            .0
            .iter()
            .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
            .count(),
        0
    );
}

#[test]
fn volatile_guarded_load_false_does_not_read_an_empty_slice() {
    let admitted = admitted(conditional_load_module(true, true));
    let mut request = conditional_load_request(false, empty_u8_buffer());
    request.events = EventPolicyV1::Enabled;
    let mut events = Collector::default();
    let execution = admitted
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .expect("false volatile guarded load is non-speculative");

    assert_eq!(execution.buffer(1).unwrap().bytes(), &[99]);
    assert!(
        !events
            .0
            .iter()
            .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
    );
}

#[test]
fn guarded_load_true_reads_through_the_shared_load_path() {
    let admitted = admitted(conditional_load_module(true, false));
    let mut request = conditional_load_request(true, byte_buffer(&[7]));
    request.events = EventPolicyV1::Enabled;
    let mut events = Collector::default();
    let execution = admitted
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .expect("true guarded load reads memory");

    assert_eq!(execution.buffer(1).unwrap().bytes(), &[7]);
    assert_eq!(
        events
            .0
            .iter()
            .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
            .count(),
        1
    );
}

#[test]
fn volatile_guarded_load_true_reads_once() {
    let admitted = admitted(conditional_load_module(true, true));
    let mut request = conditional_load_request(true, byte_buffer(&[7]));
    request.events = EventPolicyV1::Enabled;
    let mut events = Collector::default();
    let execution = admitted
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .expect("true volatile guarded load reads memory");

    assert_eq!(execution.buffer(1).unwrap().bytes(), &[7]);
    assert_eq!(
        events
            .0
            .iter()
            .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
            .count(),
        1
    );
}

#[test]
fn guarded_load_true_reports_the_guarded_operation_site_for_an_invalid_read() {
    let error = admitted(conditional_load_module(true, false))
        .simulate(
            &conditional_load_request(true, empty_u8_buffer()),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .expect_err("true guarded load validates its pointer");
    let SimulationErrorV1::Execution(error) = error else {
        panic!("expected execution failure");
    };
    assert_eq!(error.site.unwrap().operation, Some(2));
    assert!(matches!(
        error.kind,
        SimulationExecutionErrorKindV1::OutOfBounds { .. }
    ));
}

#[test]
fn false_guarded_load_does_not_consume_a_memory_access_record() {
    let limits = SimulationLimitsV1 {
        max_memory_access_records: 1,
        ..SimulationLimitsV1::default()
    };
    let admitted = admitted(conditional_load_module(true, false));
    let false_execution = admitted
        .simulate(
            &conditional_load_request(false, empty_u8_buffer()),
            SimulationTargetV1::amdgpu_64(),
            limits,
        )
        .unwrap();
    assert_eq!(
        false_execution.conflict_assessment(),
        &SimulationConflictAssessmentV1::NoConflictsObserved
    );

    let true_execution = admitted
        .simulate(
            &conditional_load_request(true, byte_buffer(&[7])),
            SimulationTargetV1::amdgpu_64(),
            limits,
        )
        .unwrap();
    assert!(matches!(
        true_execution.conflict_assessment(),
        SimulationConflictAssessmentV1::Incomplete {
            record_limit: 1,
            ..
        }
    ));
}

#[test]
fn guarded_load_has_the_same_inline_resident_cost_as_load() {
    let guarded = admitted(conditional_load_module(true, false));
    let ordinary = admitted(conditional_load_module(false, false));
    assert_eq!(
        guarded.admitted_resident_bytes(),
        ordinary.admitted_resident_bytes()
    );
}

#[test]
fn canonical_schedule_recording_preserves_legacy_execution_exactly() {
    let admitted = admitted(helper_barrier_exchange_module());
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [10, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 10]))],
    );
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let legacy = admitted.simulate(&request, target, limits).unwrap();
    let recorded = admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 64 },
        )
        .unwrap();

    assert_eq!(recorded.arguments(), legacy.arguments());
    assert_eq!(recorded.shared_buffers(), legacy.shared_buffers());
    assert_eq!(recorded.steps_executed(), legacy.steps_executed());
    assert_eq!(recorded.conflict_assessment(), legacy.conflict_assessment());
    assert_eq!(recorded.schedule(), legacy.schedule());
    assert_eq!(
        recorded.schedule_transcript_identity(),
        legacy.schedule_transcript_identity()
    );
    assert_eq!(recorded.schedule_coverage().decisions(), 20);
    assert_eq!(recorded.schedule_coverage().workgroups(), 3);
    assert_eq!(recorded.schedule_coverage().barrier_releases(), 3);
    assert!(recorded.schedule_coverage().is_complete());

    let record = recorded.schedule_record().expect("explicit record");
    assert_eq!(record.decisions().len(), 20);
    assert_eq!(record.decisions()[0].workgroup(), [0, 0, 0]);
    assert_eq!(record.decisions()[0].phase(), 0);
    assert_eq!(record.decisions()[0].local(), [0, 0, 0]);
    assert_eq!(record.decisions()[8].workgroup(), [1, 0, 0]);
    assert_eq!(record.decisions()[16].workgroup(), [2, 0, 0]);
    assert!(
        record.decisions()[16..]
            .iter()
            .all(|decision| decision.local()[0] < 2)
    );
}

#[test]
fn seeded_schedule_is_deterministic_and_replays_helper_barriers_and_partial_groups() {
    let admitted = admitted(helper_barrier_exchange_module());
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [10, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 10]))],
    );
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let run = |seed| {
        admitted
            .simulate_scheduled(
                &request,
                target,
                limits,
                SimulationScheduleRequestV1::RecordSeeded {
                    seed,
                    max_decisions: 64,
                },
            )
            .unwrap()
    };
    let first = run(0x5eed);
    let same = run(0x5eed);
    let different = run(0x5eee);

    assert_eq!(first.schedule_record(), same.schedule_record());
    assert_ne!(first.schedule_record(), different.schedule_record());
    assert_eq!(
        first.schedule(),
        SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1
    );
    assert_eq!(
        words(first.buffer(0).unwrap().bytes()),
        vec![0, 0, 0, 0, 4, 4, 4, 4, 8, 8]
    );

    let record = first.schedule_record().unwrap();
    let replayed = admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::Replay(record),
        )
        .unwrap();
    assert_eq!(replayed.arguments(), first.arguments());
    assert_eq!(replayed.schedule(), first.schedule());
    assert_eq!(
        replayed.schedule_transcript_identity(),
        first.schedule_transcript_identity()
    );
    assert_eq!(replayed.schedule_coverage(), first.schedule_coverage());
    assert!(replayed.schedule_record().is_none());
}

#[test]
fn persisted_schedule_codec_round_trips_and_replays_the_existing_record() {
    let canonical = VerifiedCanonicalKernelIrV7::from_module(helper_barrier_exchange_module())
        .expect("verified fixture");
    let identity = *canonical.identity();
    let admitted = AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default())
        .expect("admitted fixture");
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [10, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 10]))],
    );
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let recorded = admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 0x5eed,
                max_decisions: 64,
            },
        )
        .unwrap();
    let binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::SimulationBundleV1 {
            bundle_sha256: [6; 32],
            subject_sha256: [7; 32],
        },
        identity,
        [9; 32],
        317,
        target,
        limits,
    );
    let encoded = PersistedSimulationScheduleDocumentV1::encode_record(
        binding,
        recorded.schedule_record().unwrap(),
    )
    .unwrap();
    let decoded = PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&encoded).unwrap();

    assert_eq!(decoded.binding(), binding);
    assert_eq!(decoded.to_canonical_bytes().unwrap(), encoded);
    assert_eq!(decoded.record(), recorded.schedule_record().unwrap());
    let replayed = admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::Replay(decoded.record()),
        )
        .unwrap();
    assert_eq!(replayed.arguments(), recorded.arguments());
    assert_eq!(
        replayed.schedule_transcript_identity(),
        recorded.schedule_transcript_identity()
    );

    let different_route = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV7,
        identity,
        [9; 32],
        317,
        target,
        limits,
    );
    assert_ne!(decoded.binding(), different_route);

    let mut invalid_limits = limits;
    invalid_limits.max_events = 0;
    let invalid_binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV7,
        identity,
        [9; 32],
        317,
        target,
        invalid_limits,
    );
    assert_eq!(
        PersistedSimulationScheduleDocumentV1::encode_record(
            invalid_binding,
            recorded.schedule_record().unwrap(),
        )
        .unwrap_err(),
        PersistedSimulationScheduleCodecErrorV1::InvalidLimits
    );

    let substituted_seed =
        String::from_utf8(encoded)
            .unwrap()
            .replacen("\"seed\":24301", "\"seed\":24302", 1);
    assert_eq!(
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(substituted_seed.as_bytes())
            .unwrap_err(),
        PersistedSimulationScheduleCodecErrorV1::InvalidRecordIntegrity
    );
}

#[test]
fn persisted_schedule_codec_rejects_noncanonical_and_corrupt_documents() {
    let canonical = VerifiedCanonicalKernelIrV7::from_module(helper_barrier_exchange_module())
        .expect("verified fixture");
    let identity = *canonical.identity();
    let admitted = AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default())
        .expect("admitted fixture");
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [4, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 4]))],
    );
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let recorded = admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 16 },
        )
        .unwrap();
    let binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV7,
        identity,
        [3; 32],
        99,
        target,
        limits,
    );
    let encoded = PersistedSimulationScheduleDocumentV1::encode_record(
        binding,
        recorded.schedule_record().unwrap(),
    )
    .unwrap();

    let mut whitespace = encoded.clone();
    whitespace.push(b'\n');
    assert_eq!(
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&whitespace).unwrap_err(),
        PersistedSimulationScheduleCodecErrorV1::NonCanonical
    );

    let text = String::from_utf8(encoded).unwrap();
    for corrupt in [
        text.replacen("{\"schema\":", "{\"unknown\":0,\"schema\":", 1),
        text.replacen(
            "\"schema\":\"fe2o3-simulation-schedule-v1\"",
            "\"schema\":null",
            1,
        ),
        text.replacen(
            "\"schema\":\"fe2o3-simulation-schedule-v1\"",
            "\"schema\":\"fe2o3-simulation-schedule-v1\",\"schema\":\"fe2o3-simulation-schedule-v1\"",
            1,
        ),
        text.replacen(
            "{\"schema\":",
            "{\"fe2o3:schedule_decision_limit\":0,\"schema\":",
            1,
        ),
        text.replacen(
            "{\"schema\":",
            "{\"fe2o3:schedule_allocation_failure\":0,\"schema\":",
            1,
        ),
        text.replacen(
            "{\"schema\":",
            "{\"fe2o3:schedule_schema_unsupported\":0,\"schema\":",
            1,
        ),
        text.replacen("\"sha256\":\"0303", "\"sha256\":\"A303", 1),
        text.replacen(
            "\"artifact\":{\"kind\":",
            "\"artifact\":{\"unknown\":0,\"kind\":",
            1,
        ),
        text.replacen("\"limits\":{", "\"limits\":{\"max_events\":1,", 1),
        text.replacen("\"kind\":\"canonical_kir_v7\"", "\"kind\":null", 1),
    ] {
        assert!(matches!(
            PersistedSimulationScheduleDocumentV1::from_canonical_bytes(corrupt.as_bytes()),
            Err(PersistedSimulationScheduleCodecErrorV1::JsonStructure)
        ));
    }

    let unsupported_schema = text.replacen(
        "fe2o3-simulation-schedule-v1",
        "fe2o3-simulation-schedule-v2",
        1,
    );
    assert_eq!(
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(unsupported_schema.as_bytes(),)
            .unwrap_err(),
        PersistedSimulationScheduleCodecErrorV1::UnsupportedSchema
    );

    let hostile_long_key = format!("{{\"{}\":0}}", "x".repeat(129));
    assert_eq!(
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(hostile_long_key.as_bytes(),)
            .unwrap_err(),
        PersistedSimulationScheduleCodecErrorV1::StringTokenLimit {
            actual: 129,
            limit: 128,
        }
    );

    let corrupt_integrity = text.replacen("\"record_sha256\":\"", "\"record_sha256\":\"0", 1);
    assert!(
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(corrupt_integrity.as_bytes())
            .is_err()
    );
}

#[test]
fn debug_records_expose_seeded_semantic_schedule_and_decision_prefix() {
    let admitted = admitted(helper_barrier_exchange_module());
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [4, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 4]))],
    );
    let mut debug = DebugCollector::default();
    let execution = admitted
        .simulate_debugged_scheduled_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 7,
                max_decisions: 16,
            },
            SimulationDebugCaptureLimitsV1::new(16, 256, 16, 1_024).unwrap(),
            &mut debug,
        )
        .unwrap();

    assert!(!debug.0.is_empty());
    assert!(debug.0.iter().all(|record| {
        record.schedule.identity
            == SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1
            && record.schedule.decision_ordinal < execution.schedule_coverage().decisions()
    }));
    assert!(debug.0.windows(2).all(|records| {
        records[0].schedule.decision_ordinal <= records[1].schedule.decision_ordinal
    }));
}

#[test]
fn schedule_replay_rejects_stale_inputs_and_limits_before_execution() {
    let admitted = admitted(helper_barrier_exchange_module());
    let request = SimulationRequestV1::new(
        "lds_exchange",
        [4, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[u32::MAX; 4]))],
    );
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let recorded = admitted
        .simulate_scheduled(
            &request,
            target,
            limits,
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 16 },
        )
        .unwrap();
    let record = recorded.schedule_record().unwrap();

    let stale_request = SimulationRequestV1::new(
        "lds_exchange",
        [4, 1, 1],
        [4, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0; 4]))],
    );
    for result in [
        admitted.simulate_scheduled(
            &stale_request,
            target,
            limits,
            SimulationScheduleRequestV1::Replay(record),
        ),
        admitted.simulate_scheduled(
            &request,
            target,
            SimulationLimitsV1 {
                max_events: limits.max_events - 1,
                ..limits
            },
            SimulationScheduleRequestV1::Replay(record),
        ),
    ] {
        assert!(matches!(
            result,
            Err(SimulationErrorV1::Execution(
                fe2o3_kir_sim::SimulationExecutionErrorV1 {
                    invocation: None,
                    site: None,
                    kind: SimulationExecutionErrorKindV1::ScheduleReplay(
                        SimulationScheduleReplayErrorV1::ContextMismatch
                    ),
                    ..
                }
            ))
        ));
    }
}

#[test]
fn schedule_record_bounds_are_fail_closed_without_changing_default_step_failures() {
    let admitted = admitted(empty_kernel_module(
        "empty_schedule",
        Signature::new(vec![], vec![]),
        vec![],
    ));
    let request = SimulationRequestV1::new("empty_schedule", [4, 1, 1], [4, 1, 1], vec![]);
    let target = SimulationTargetV1::amdgpu_64();
    let plan = admitted
        .preflight(&request, target, SimulationLimitsV1::default())
        .unwrap();

    let resident_limit = plan.resident_bytes();
    let resident = admitted
        .simulate_scheduled(
            &request,
            target,
            SimulationLimitsV1 {
                max_resident_bytes: resident_limit,
                ..SimulationLimitsV1::default()
            },
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 4 },
        )
        .unwrap_err();
    assert!(matches!(
        resident,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            invocation: None,
            kind: SimulationExecutionErrorKindV1::ScheduleResidentLimit {
                actual,
                limit
            },
            ..
        }) if actual > limit && limit == resident_limit
    ));

    let oversized = admitted
        .simulate_scheduled(
            &request,
            target,
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordCanonical {
                max_decisions: MAX_SCHEDULE_DECISIONS_V1 + 1,
            },
        )
        .unwrap_err();
    assert!(matches!(
        oversized,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            invocation: None,
            kind: SimulationExecutionErrorKindV1::ScheduleDecisionLimit { .. },
            ..
        })
    ));

    let too_short = admitted
        .simulate_scheduled(
            &request,
            target,
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 1 },
        )
        .unwrap_err();
    assert!(matches!(
        too_short,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::ScheduleDecisionLimit {
                actual: 2,
                limit: 1
            },
            ..
        })
    ));

    let legacy = admitted
        .simulate(
            &request,
            target,
            SimulationLimitsV1 {
                max_steps: 2,
                ..SimulationLimitsV1::default()
            },
        )
        .unwrap_err();
    assert!(matches!(
        legacy,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::StepLimit { limit: 2 },
            ..
        })
    ));
}

fn atomic_capability(
    scalar: ScalarType,
    address_space: AddressSpace,
    scope: SynchronizationScope,
) -> TargetCapability {
    TargetCapability::Atomic {
        width_bits: scalar.bit_width().unwrap(),
        address_space,
        max_scope: scope,
    }
}

fn atomic_module(
    kind: AtomicKind,
    scalar: ScalarType,
    address_space: AddressSpace,
    scope: SynchronizationScope,
    ordering: MemoryOrdering,
    failure_ordering: Option<MemoryOrdering>,
) -> Module {
    let scalar_ty = Type::Scalar(scalar);
    let pointer_ty = Type::pointer(scalar_ty.clone(), address_space, AccessMode::ReadWrite);
    let results = match kind {
        AtomicKind::Store => vec![],
        AtomicKind::CompareExchange => vec![
            ValueDef::new(ValueId(3), scalar_ty.clone()),
            ValueDef::new(ValueId(4), Type::BOOL),
        ],
        _ => vec![ValueDef::new(ValueId(3), scalar_ty.clone())],
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        results,
        OperationKind::Atomic(Atomic {
            kind,
            pointer: ValueId(0),
            value: (kind != AtomicKind::Load).then_some(ValueId(1)),
            compare: (kind == AtomicKind::CompareExchange).then_some(ValueId(2)),
            access: MemoryAccess::new(
                address_space,
                u32::from(scalar.bit_width().unwrap().div_ceil(8)),
            ),
            scope,
            ordering,
            failure_ordering,
        }),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let capabilities = if matches!(scalar.bit_width(), Some(8 | 16 | 32 | 64)) {
        std::collections::BTreeSet::from([atomic_capability(scalar, address_space, scope)])
    } else {
        // V7 represents i128/u128 atomics, but its hardware capability vocabulary
        // deliberately has no 128-bit atomic capability to declare.
        std::collections::BTreeSet::new()
    };
    let mut entry = Function::kernel_entry(
        "atomic_impl",
        Signature::new(vec![pointer_ty, scalar_ty.clone(), scalar_ty], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    entry.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new("atomic", "atomic_impl", dynamic_domain_1d());
    kernel.required_capabilities = capabilities.clone();
    let mut module = Module::new("sim-tests::atomic");
    module.required_capabilities = capabilities;
    module.functions.push(entry);
    module.kernels.push(kernel);
    module
}

fn atomic_request(
    initial: ScalarBitsV1,
    operand: ScalarBitsV1,
    compare: ScalarBitsV1,
    grid: u64,
    workgroup: u32,
) -> SimulationRequestV1 {
    SimulationRequestV1::new(
        "atomic",
        [grid, 1, 1],
        [workgroup, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(atomic_scalar_buffer(
                initial,
                true,
                u32::from(initial.ty().bit_width().unwrap().div_ceil(8)),
            )),
            SimulationArgumentV1::Scalar(operand),
            SimulationArgumentV1::Scalar(compare),
        ],
    )
}

fn atomic_then_non_atomic_conflict_module(ordering: MemoryOrdering) -> Module {
    let mut module = atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        ordering,
        None,
    );
    let function = &mut module.functions[0];
    let block = &mut function.body.as_mut().expect("entry body").blocks[0];
    block.operations.push(Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    ));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    function
        .required_capabilities
        .insert(TargetCapability::WorkgroupBarrier);
    module.kernels[0].required_capabilities = function.required_capabilities.clone();
    module.required_capabilities = function.required_capabilities.clone();
    module
}

fn atomic_store_then_atomic_and_ordinary_load_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        op(3, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });

    let atomic_access = MemoryAccess::new(AddressSpace::Global, 4);
    let mut store = BasicBlock::new(BlockId(1));
    store.operations.push(Operation::new(
        vec![],
        OperationKind::Atomic(Atomic {
            kind: AtomicKind::Store,
            pointer: ValueId(0),
            value: Some(ValueId(1)),
            compare: None,
            access: atomic_access,
            scope: SynchronizationScope::System,
            ordering: MemoryOrdering::Relaxed,
            failure_ordering: None,
        }),
    ));
    store.terminator = Some(Terminator::Return { values: vec![] });

    let mut select_load = BasicBlock::new(BlockId(2));
    select_load.operations = vec![
        op(5, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            6,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(2),
                rhs: ValueId(5),
            },
        ),
    ];
    select_load.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(3),
        then_arguments: vec![],
        else_target: BlockId(4),
        else_arguments: vec![],
    });

    let mut atomic_load = BasicBlock::new(BlockId(3));
    atomic_load.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(7), scalar.clone())],
        OperationKind::Atomic(Atomic {
            kind: AtomicKind::Load,
            pointer: ValueId(0),
            value: None,
            compare: None,
            access: atomic_access,
            scope: SynchronizationScope::System,
            ordering: MemoryOrdering::Relaxed,
            failure_ordering: None,
        }),
    ));
    atomic_load.terminator = Some(Terminator::Return { values: vec![] });

    let mut ordinary_load = BasicBlock::new(BlockId(4));
    ordinary_load.operations.push(op(
        8,
        scalar.clone(),
        OperationKind::Load {
            pointer: ValueId(0),
            access: atomic_access,
        },
    ));
    ordinary_load.terminator = Some(Terminator::Return { values: vec![] });

    let capability = atomic_capability(
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
    );
    let mut function = Function::kernel_entry(
        "mixed_atomic_frontier_impl",
        Signature::new(vec![pointer, scalar], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry, store, select_load, atomic_load, ordinary_load],
    );
    function.required_capabilities.insert(capability);
    let mut kernel = Kernel::new(
        "mixed_atomic_frontier",
        "mixed_atomic_frontier_impl",
        dynamic_domain_1d(),
    );
    kernel.required_capabilities = function.required_capabilities.clone();
    let mut module = Module::new("sim-tests::mixed-atomic-frontier");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn atomic_load_does_not_erase_a_store_visible_to_an_ordinary_load() {
    let execution = admitted(atomic_store_then_atomic_and_ordinary_load_module())
        .simulate(
            &SimulationRequestV1::new(
                "mixed_atomic_frontier",
                [3, 1, 1],
                [3, 1, 1],
                vec![
                    SimulationArgumentV1::Buffer(u32_buffer(&[0])),
                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7)),
                ],
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved {
            first: fe2o3_kir_sim::SimulationDataRaceV1 {
                earlier_atomic: true,
                later_atomic: false,
                ..
            },
            first_ordered_conflict: Some(fe2o3_kir_sim::SimulationOrderedMemoryConflictV1 {
                reason: fe2o3_kir_sim::SimulationHappensBeforeReasonV1::AtomicSerialization,
                ..
            }),
            ..
        }
    ));
}

fn ordered_atomic_store_replacement_module() -> Module {
    let mut module = atomic_store_then_atomic_and_ordinary_load_module();
    let block = &mut module.functions[0]
        .body
        .as_mut()
        .expect("entry body")
        .blocks[3];
    let atomic = &mut block.operations[0];
    atomic.results.clear();
    let OperationKind::Atomic(atomic) = &mut atomic.kind else {
        panic!("expected atomic load")
    };
    atomic.kind = AtomicKind::Store;
    atomic.value = Some(ValueId(1));
    block.operations.push(op(
        9,
        Type::Scalar(ScalarType::U32),
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    module
}

#[test]
fn ordered_atomic_writer_replacement_cannot_hide_a_race_with_the_replacement_writer() {
    let execution = admitted(ordered_atomic_store_replacement_module())
        .simulate(
            &SimulationRequestV1::new(
                "mixed_atomic_frontier",
                [2, 1, 1],
                [2, 1, 1],
                vec![
                    SimulationArgumentV1::Buffer(u32_buffer(&[0])),
                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(7)),
                ],
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved {
            first: fe2o3_kir_sim::SimulationDataRaceV1 {
                earlier_atomic: true,
                later_atomic: false,
                ..
            },
            first_ordered_conflict: Some(fe2o3_kir_sim::SimulationOrderedMemoryConflictV1 {
                reason: fe2o3_kir_sim::SimulationHappensBeforeReasonV1::AtomicSerialization,
                ..
            }),
            ..
        }
    ));
}

#[test]
fn ordered_atomic_conflicts_do_not_consume_the_unique_racing_byte_marker() {
    let execution = admitted(atomic_then_non_atomic_conflict_module(
        MemoryOrdering::Relaxed,
    ))
    .simulate(
        &atomic_request(
            ScalarBitsV1::u32(0),
            ScalarBitsV1::u32(1),
            ScalarBitsV1::u32(0),
            3,
            3,
        ),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::RacesObserved {
            racing_bytes: 4,
            first_ordered_conflict: Some(fe2o3_kir_sim::SimulationOrderedMemoryConflictV1 {
                reason: fe2o3_kir_sim::SimulationHappensBeforeReasonV1::AtomicSerialization,
                ..
            }),
            ..
        }
    ));
}

#[test]
fn release_acquire_atomic_happens_before_with_ordinary_conflicts_is_incomplete() {
    let execution = admitted(atomic_then_non_atomic_conflict_module(
        MemoryOrdering::AcquireRelease,
    ))
    .simulate(
        &atomic_request(
            ScalarBitsV1::u32(0),
            ScalarBitsV1::u32(1),
            ScalarBitsV1::u32(0),
            3,
            3,
        ),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    assert!(matches!(
        execution.race_assessment(),
        SimulationRaceAssessmentV1::Incomplete {
            access_record_limit_reached: false,
            atomic_or_fence_happens_before_unmodeled: true,
            first: Some(_),
            ..
        }
    ));
}

fn scalar_buffer_bits(execution: &fe2o3_kir_sim::SimulationExecutionV1) -> u128 {
    let bytes = execution.buffer(0).unwrap().bytes();
    let mut raw = [0_u8; 16];
    raw[..bytes.len()].copy_from_slice(bytes);
    u128::from_le_bytes(raw)
}

#[test]
fn every_integer_atomic_kind_has_exact_outcomes_and_one_semantic_event() {
    let cases: [(AtomicKind, u32, u32, Option<bool>); 11] = [
        (AtomicKind::Load, 0xf0, 0xf0, None),
        (AtomicKind::Store, 0xf0, 0x0f, None),
        (AtomicKind::Exchange, 0xf0, 0x0f, None),
        (AtomicKind::CompareExchange, 0xf0, 0x0f, Some(true)),
        (AtomicKind::Add, 0xf0, 0xff, None),
        (AtomicKind::Subtract, 0xf0, 0xe1, None),
        (AtomicKind::Min, 0xf0, 0x0f, None),
        (AtomicKind::Max, 0xf0, 0xf0, None),
        (AtomicKind::BitAnd, 0xf0, 0x00, None),
        (AtomicKind::BitOr, 0xf0, 0xff, None),
        (AtomicKind::BitXor, 0xf0, 0xff, None),
    ];
    for (kind, initial, expected, expected_cas) in cases {
        let module = atomic_module(
            kind,
            ScalarType::U32,
            AddressSpace::Global,
            SynchronizationScope::System,
            MemoryOrdering::Relaxed,
            (kind == AtomicKind::CompareExchange).then_some(MemoryOrdering::Relaxed),
        );
        let request = atomic_request(
            ScalarBitsV1::u32(initial),
            ScalarBitsV1::u32(0x0f),
            ScalarBitsV1::u32(initial),
            1,
            1,
        );
        let mut events = Collector::default();
        let execution = admitted(module)
            .simulate_observed_with_sink(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                &mut events,
            )
            .unwrap();
        assert_eq!(
            scalar_buffer_bits(&execution),
            u128::from(expected),
            "{kind:?}"
        );
        let atomic_events = events
            .0
            .iter()
            .filter_map(|event| match &event.kind {
                SimulationEventKindV1::MemoryAtomic {
                    kind: actual,
                    compare_exchange_success,
                    ..
                } => Some((*actual, *compare_exchange_success)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(atomic_events, vec![(kind, expected_cas)]);
    }

    let module = atomic_module(
        AtomicKind::CompareExchange,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::AcquireRelease,
        Some(MemoryOrdering::Acquire),
    );
    let request = atomic_request(
        ScalarBitsV1::u32(7),
        ScalarBitsV1::u32(9),
        ScalarBitsV1::u32(8),
        1,
        1,
    );
    let mut events = Collector::default();
    let execution = admitted(module)
        .simulate_observed_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    assert_eq!(scalar_buffer_bits(&execution), 7);
    assert!(events.0.iter().any(|event| matches!(
        event.kind,
        SimulationEventKindV1::MemoryAtomic {
            kind: AtomicKind::CompareExchange,
            committed: None,
            compare_exchange_success: Some(false),
            ..
        }
    )));
}

#[test]
fn atomics_cover_every_fixed_integer_width_signed_order_and_wrapping() {
    let types = [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::I128,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::U128,
    ];
    for scalar in types {
        let width = scalar.bit_width().unwrap();
        let all_ones = if width == 128 {
            u128::MAX
        } else {
            (1_u128 << width) - 1
        };
        let initial = ScalarBitsV1::new(scalar, all_ones, SimulationTargetV1::amdgpu_64()).unwrap();
        let one = ScalarBitsV1::new(scalar, 1, SimulationTargetV1::amdgpu_64()).unwrap();
        let module = atomic_module(
            AtomicKind::Add,
            scalar,
            AddressSpace::Global,
            SynchronizationScope::Device,
            MemoryOrdering::SequentiallyConsistent,
            None,
        );
        let execution = admitted(module)
            .simulate(
                &atomic_request(initial, one, one, 1, 1),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        assert_eq!(scalar_buffer_bits(&execution), 0, "{scalar:?} wrapping add");
    }

    for (kind, expected) in [(AtomicKind::Min, -9_i32), (AtomicKind::Max, -2_i32)] {
        let initial = ScalarBitsV1::i32(-2);
        let operand = ScalarBitsV1::i32(-9);
        let execution = admitted(atomic_module(
            kind,
            ScalarType::I32,
            AddressSpace::Global,
            SynchronizationScope::Device,
            MemoryOrdering::AcquireRelease,
            None,
        ))
        .simulate(
            &atomic_request(initial, operand, operand, 1, 1),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(scalar_buffer_bits(&execution) as u32 as i32, expected);
    }
}

#[test]
fn all_legal_atomic_scopes_orderings_and_compare_exchange_failures_execute() {
    let scopes = [
        SynchronizationScope::Subgroup,
        SynchronizationScope::Workgroup,
        SynchronizationScope::Device,
        SynchronizationScope::System,
    ];
    let orderings = [
        MemoryOrdering::Relaxed,
        MemoryOrdering::Acquire,
        MemoryOrdering::Release,
        MemoryOrdering::AcquireRelease,
        MemoryOrdering::SequentiallyConsistent,
    ];
    for scope in scopes {
        for ordering in orderings {
            let execution = admitted(atomic_module(
                AtomicKind::Exchange,
                ScalarType::U32,
                AddressSpace::Global,
                scope,
                ordering,
                None,
            ))
            .simulate(
                &atomic_request(
                    ScalarBitsV1::u32(1),
                    ScalarBitsV1::u32(2),
                    ScalarBitsV1::u32(0),
                    1,
                    1,
                ),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
            assert_eq!(scalar_buffer_bits(&execution), 2, "{scope:?} {ordering:?}");
        }
    }

    let failure_pairs = [
        (MemoryOrdering::Relaxed, MemoryOrdering::Relaxed),
        (MemoryOrdering::Acquire, MemoryOrdering::Relaxed),
        (MemoryOrdering::Acquire, MemoryOrdering::Acquire),
        (MemoryOrdering::Release, MemoryOrdering::Relaxed),
        (MemoryOrdering::AcquireRelease, MemoryOrdering::Relaxed),
        (MemoryOrdering::AcquireRelease, MemoryOrdering::Acquire),
        (
            MemoryOrdering::SequentiallyConsistent,
            MemoryOrdering::Relaxed,
        ),
        (
            MemoryOrdering::SequentiallyConsistent,
            MemoryOrdering::Acquire,
        ),
        (
            MemoryOrdering::SequentiallyConsistent,
            MemoryOrdering::SequentiallyConsistent,
        ),
    ];
    for (success, failure) in failure_pairs {
        let execution = admitted(atomic_module(
            AtomicKind::CompareExchange,
            ScalarType::U32,
            AddressSpace::Global,
            SynchronizationScope::System,
            success,
            Some(failure),
        ))
        .simulate(
            &atomic_request(
                ScalarBitsV1::u32(3),
                ScalarBitsV1::u32(9),
                ScalarBitsV1::u32(7),
                1,
                1,
            ),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(scalar_buffer_bits(&execution), 3, "{success:?}/{failure:?}");
    }
}

fn workgroup_atomic_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(
        scalar.clone(),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            2,
            pointer,
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: scalar.clone(),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 4,
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Store,
                pointer: ValueId(2),
                value: Some(ValueId(0)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
                scope: SynchronizationScope::Workgroup,
                ordering: MemoryOrdering::Release,
                failure_ordering: None,
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ),
        op(
            3,
            scalar,
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Add,
                pointer: ValueId(2),
                value: Some(ValueId(1)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Workgroup, 4),
                scope: SynchronizationScope::Subgroup,
                ordering: MemoryOrdering::AcquireRelease,
                failure_ordering: None,
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let capabilities = std::collections::BTreeSet::from([
        TargetCapability::WorkgroupMemory,
        TargetCapability::WorkgroupBarrier,
        atomic_capability(
            ScalarType::U32,
            AddressSpace::Workgroup,
            SynchronizationScope::Workgroup,
        ),
    ]);
    let mut entry = Function::kernel_entry(
        "workgroup_atomic_impl",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new(
        "workgroup_atomic",
        "workgroup_atomic_impl",
        dynamic_domain_1d(),
    );
    kernel.required_capabilities = capabilities.clone();
    let mut module = Module::new("sim-tests::workgroup-atomic");
    module.required_capabilities = capabilities;
    module.functions.push(entry);
    module.kernels.push(kernel);
    module
}

#[test]
fn workgroup_atomics_publish_across_full_and_partial_workgroups() {
    let request = SimulationRequestV1::new(
        "workgroup_atomic",
        [5, 1, 1],
        [4, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(1)),
        ],
    );
    let mut debug = DebugCollector::default();
    let execution = admitted(workgroup_atomic_module())
        .simulate_debugged_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationDebugCaptureLimitsV1::new(16, 256, 16, 1_024).unwrap(),
            &mut debug,
        )
        .unwrap();
    assert_eq!(execution.workgroups_visited(), 2);
    assert_eq!(execution.invocations_executed(), 5);
    assert_eq!(
        debug
            .0
            .iter()
            .filter(|record| matches!(
                record.kind,
                SimulationDebugRecordKindV1::Memory {
                    access: SimulationDebugMemoryAccessV1::AtomicReadWriteCommitted,
                    ..
                }
            ))
            .count(),
        5
    );
}

fn fence_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    for scope in [
        SynchronizationScope::Subgroup,
        SynchronizationScope::Workgroup,
        SynchronizationScope::Device,
        SynchronizationScope::System,
    ] {
        for ordering in [
            MemoryOrdering::Acquire,
            MemoryOrdering::Release,
            MemoryOrdering::AcquireRelease,
            MemoryOrdering::SequentiallyConsistent,
        ] {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::Fence(Fence {
                    memory_scope: scope,
                    semantics: BarrierSemantics::new(
                        ordering,
                        [AddressSpace::Global, AddressSpace::Generic],
                    ),
                }),
            ));
        }
    }
    for scope in [
        SynchronizationScope::Subgroup,
        SynchronizationScope::Workgroup,
    ] {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Fence(Fence {
                memory_scope: scope,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
            }),
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "fence_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let mut module = Module::new("sim-tests::fence");
    module.functions.push(entry);
    module
        .kernels
        .push(Kernel::new("fence", "fence_impl", dynamic_domain_1d()));
    module
}

#[test]
fn every_legal_fence_scope_order_and_address_space_is_an_explicit_order_point() {
    let mut debug = DebugCollector::default();
    admitted(fence_module())
        .simulate_debugged_with_sink(
            &SimulationRequestV1::new("fence", [1, 1, 1], [1, 1, 1], vec![]),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationDebugCaptureLimitsV1::new(16, 256, 16, 1_024).unwrap(),
            &mut debug,
        )
        .unwrap();
    let fences = debug
        .0
        .iter()
        .filter_map(|record| match record.kind {
            SimulationDebugRecordKindV1::Fence {
                memory_scope,
                ordering,
                address_space_mask,
            } => Some((memory_scope, ordering, address_space_mask)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fences.len(), 18);
    assert!(
        fences
            .iter()
            .all(|(_, ordering, mask)| { *ordering != MemoryOrdering::Relaxed && *mask != 0 })
    );
}

#[test]
fn atomic_memory_failures_are_typed_and_store_initializes_without_reading() {
    let module = admitted(atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::Device,
        MemoryOrdering::Relaxed,
        None,
    ));
    for (buffer, expected) in [
        (
            atomic_scalar_buffer(ScalarBitsV1::u32(1), true, 1),
            "misaligned",
        ),
        (
            BufferArgumentV1::new(
                ScalarType::U32,
                AccessMode::ReadWrite,
                4,
                vec![],
                vec![],
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap(),
            "out-of-bounds",
        ),
        (
            atomic_scalar_buffer(ScalarBitsV1::u32(1), false, 4),
            "uninitialized",
        ),
    ] {
        let request = SimulationRequestV1::new(
            "atomic",
            [1, 1, 1],
            [1, 1, 1],
            vec![
                SimulationArgumentV1::Buffer(buffer),
                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(1)),
                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(1)),
            ],
        );
        let error = module
            .simulate(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .expect_err(expected);
        assert!(matches!(
            (expected, error),
            (
                "misaligned",
                SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::MisalignedAccess { .. },
                    ..
                })
            ) | (
                "out-of-bounds",
                SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::OutOfBounds { .. },
                    ..
                })
            ) | (
                "uninitialized",
                SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::UninitializedRead { .. },
                    ..
                })
            )
        ));
    }

    let store = admitted(atomic_module(
        AtomicKind::Store,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::Device,
        MemoryOrdering::Release,
        None,
    ));
    let request = SimulationRequestV1::new(
        "atomic",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(atomic_scalar_buffer(ScalarBitsV1::u32(0), false, 4)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(17)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0)),
        ],
    );
    let execution = store
        .simulate(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap();
    assert_eq!(scalar_buffer_bits(&execution), 17);
    assert!(
        execution
            .buffer(0)
            .unwrap()
            .initialized()
            .iter()
            .all(|byte| *byte)
    );
}

#[test]
fn unsupported_atomic_scalar_and_generic_space_remain_exact_typed_preflight_states() {
    let generic = admitted(atomic_module(
        AtomicKind::Exchange,
        ScalarType::U32,
        AddressSpace::Generic,
        SynchronizationScope::Device,
        MemoryOrdering::Relaxed,
        None,
    ));
    let wrong_arguments = vec![
        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0)),
        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0)),
        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0)),
    ];
    let report = |module: &AdmittedSimulationModuleV1| match module
        .preflight(
            &SimulationRequestV1::new("atomic", [1, 1, 1], [1, 1, 1], wrong_arguments.clone()),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        )
        .unwrap_err()
    {
        SimulationPreflightErrorV1::Unsupported(report) => report,
        other => panic!("expected typed unsupported report, found {other:?}"),
    };
    for scalar in [
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        let float = admitted(atomic_module(
            AtomicKind::Add,
            scalar,
            AddressSpace::Global,
            SynchronizationScope::Device,
            MemoryOrdering::Relaxed,
            None,
        ));
        assert!(report(&float).findings().iter().any(|finding| matches!(
            finding.feature,
            UnsupportedFeatureV1::FloatType(actual) if actual == scalar
        )));
    }
    assert!(report(&generic).findings().iter().any(|finding| {
        finding.feature == UnsupportedFeatureV1::UnsupportedAddressSpace(AddressSpace::Generic)
    }));
}

#[test]
fn invalid_atomic_and_fence_metadata_never_reaches_simulation() {
    let mut invalid = atomic_module(
        AtomicKind::Load,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::Invocation,
        MemoryOrdering::Release,
        None,
    );
    let body = invalid.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(Operation::new(
        vec![],
        OperationKind::Fence(Fence {
            memory_scope: SynchronizationScope::Invocation,
            semantics: BarrierSemantics::new(MemoryOrdering::Relaxed, []),
        }),
    ));
    let errors = verify_module(&invalid).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidAtomic));
    assert!(errors.contains(DiagnosticCode::InvalidFence));

    for address_space in [AddressSpace::Private, AddressSpace::Constant] {
        let errors = verify_module(&atomic_module(
            AtomicKind::Add,
            ScalarType::U32,
            address_space,
            SynchronizationScope::System,
            MemoryOrdering::Relaxed,
            None,
        ))
        .unwrap_err();
        assert!(errors.contains(DiagnosticCode::InvalidAtomic));
    }
    let errors = verify_module(&atomic_module(
        AtomicKind::Add,
        ScalarType::Bool,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::Relaxed,
        None,
    ))
    .unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidAtomic));

    let mut index = atomic_module(
        AtomicKind::Add,
        ScalarType::U64,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::Relaxed,
        None,
    );
    index.required_capabilities.clear();
    index.kernels[0].required_capabilities.clear();
    index.functions[0].required_capabilities.clear();
    index.functions[0].signature.parameters = vec![
        Type::pointer(Type::INDEX, AddressSpace::Global, AccessMode::ReadWrite),
        Type::INDEX,
        Type::INDEX,
    ];
    let operation = &mut index.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
    operation.results[0].ty = Type::INDEX;
    let OperationKind::Atomic(atomic) = &mut operation.kind else {
        unreachable!()
    };
    atomic.access.alignment = 8;
    let errors = verify_module(&index).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidAtomic));

    for address_space in [AddressSpace::Private, AddressSpace::Constant] {
        let mut fence = fence_module();
        fence.functions[0].body.as_mut().unwrap().blocks[0].operations[0] = Operation::new(
            vec![],
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::System,
                semantics: BarrierSemantics::new(MemoryOrdering::AcquireRelease, [address_space]),
            }),
        );
        let errors = verify_module(&fence).unwrap_err();
        assert!(errors.contains(DiagnosticCode::InvalidFence));
    }
}

#[test]
fn scoped_atomic_add_is_deterministic_across_partial_multi_workgroup_schedules_and_replay() {
    let canonical = VerifiedCanonicalKernelIrV7::from_module(atomic_module(
        AtomicKind::Add,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::Relaxed,
        None,
    ))
    .unwrap();
    let identity = *canonical.identity();
    let module =
        AdmittedSimulationModuleV1::admit(canonical, SimulationLimitsV1::default()).unwrap();
    let request = atomic_request(
        ScalarBitsV1::u32(0),
        ScalarBitsV1::u32(1),
        ScalarBitsV1::u32(0),
        10,
        4,
    );
    let run = |seed| {
        module
            .simulate_scheduled(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::RecordSeeded {
                    seed,
                    max_decisions: 64,
                },
            )
            .unwrap()
    };
    let first = run(0x000a_701c);
    let same = run(0x000a_701c);
    assert_eq!(scalar_buffer_bits(&first), 10);
    assert_eq!(first.schedule_record(), same.schedule_record());
    assert!(matches!(
        first.conflict_assessment(),
        SimulationConflictAssessmentV1::ConflictsObserved { .. }
    ));
    assert!(matches!(
        first.race_assessment(),
        SimulationRaceAssessmentV1::NoRacesObserved {
            first_ordered_conflict: Some(fe2o3_kir_sim::SimulationOrderedMemoryConflictV1 {
                reason: fe2o3_kir_sim::SimulationHappensBeforeReasonV1::AtomicSerialization,
                ..
            })
        }
    ));
    let binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV7,
        identity,
        [0xa7; 32],
        401,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
    );
    let persisted = PersistedSimulationScheduleDocumentV1::from_canonical_bytes(
        &PersistedSimulationScheduleDocumentV1::encode_record(
            binding,
            first.schedule_record().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let replayed = module
        .simulate_scheduled(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::Replay(persisted.record()),
        )
        .unwrap();
    assert_eq!(scalar_buffer_bits(&replayed), 10);
    assert_eq!(
        first.schedule_transcript_identity(),
        replayed.schedule_transcript_identity()
    );
}

struct RejectAtomics;

impl SimulationEventSinkV1 for RejectAtomics {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        if matches!(event.kind, SimulationEventKindV1::MemoryAtomic { .. }) {
            Err(fe2o3_kir_sim::SimulationEventSinkErrorV1 {
                detail: "atomic rejected".into(),
            })
        } else {
            Ok(())
        }
    }
}

#[test]
fn atomic_event_budget_and_sink_rejection_fail_before_a_committed_observation() {
    let module = admitted(atomic_module(
        AtomicKind::Exchange,
        ScalarType::U32,
        AddressSpace::Global,
        SynchronizationScope::System,
        MemoryOrdering::SequentiallyConsistent,
        None,
    ));
    let mut request = atomic_request(
        ScalarBitsV1::u32(3),
        ScalarBitsV1::u32(9),
        ScalarBitsV1::u32(0),
        1,
        1,
    );
    request.events = EventPolicyV1::Enabled;
    let mut retained = Collector::default();
    let error = module
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_events: 4,
                ..SimulationLimitsV1::default()
            },
            &mut retained,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::EventLimit { limit: 4 },
            ..
        })
    ));
    assert!(
        retained
            .0
            .iter()
            .all(|event| !matches!(event.kind, SimulationEventKindV1::MemoryAtomic { .. }))
    );

    let error = module
        .simulate_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut RejectAtomics,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SimulationErrorV1::Execution(fe2o3_kir_sim::SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::EventSinkFailure(_),
            ..
        })
    ));
}

#[test]
fn failure_reducer_minimizes_and_replays_memory_fault() {
    let admitted = admitted(indexed_store_module());
    let request = SimulationRequestV1::new(
        "store",
        [2, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    let limits = SimulationLimitsV1::default();
    let report = admitted
        .reduce_simulation_failure(
            &request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            SimulationFailureScheduleV1::Canonical,
            reduction_limits(16),
        )
        .unwrap();
    assert_eq!(report.fingerprint().class(), "out_of_bounds");
    assert!(report.coverage().is_locally_minimal());
    assert!(report.minimized_prefix().is_empty());
    assert!(!report.reproducer_schedule().is_empty());
    assert_eq!(
        admitted
            .replay_simulation_failure_reduction(
                &request,
                SimulationTargetV1::amdgpu_64(),
                limits,
                &report,
            )
            .unwrap(),
        report.fingerprint().clone()
    );
    assert!(!report.grants_execution_authority());
    assert!(!report.predicts_hardware_timing());
}

#[test]
fn failure_reducer_never_emits_a_report_for_a_successful_schedule() {
    let module = admitted(empty_kernel_module(
        "successful_reduction_input",
        Signature::new(vec![], vec![]),
        vec![],
    ));
    let result = module.reduce_simulation_failure(
        &SimulationRequestV1::new("successful_reduction_input", [1, 1, 1], [1, 1, 1], vec![]),
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        SimulationFailureScheduleV1::Canonical,
        reduction_limits(8),
    );
    assert!(matches!(
        result,
        Err(fe2o3_kir_sim::SimulationFailureReductionErrorV1::OriginalScheduleDidNotFail)
    ));
}

#[test]
fn failure_reducer_covers_barrier_and_seeded_race_failures() {
    let limits = SimulationLimitsV1::default();
    let barrier = admitted(barrier_failure_module(false));
    let barrier_request = SimulationRequestV1::new("barrier_failure", [2, 1, 1], [2, 1, 1], vec![]);
    let barrier_report = barrier
        .reduce_simulation_failure(
            &barrier_request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            SimulationFailureScheduleV1::Seeded { seed: 17 },
            reduction_limits(16),
        )
        .unwrap();
    assert_eq!(
        barrier_report.fingerprint().class(),
        "divergent_workgroup_barrier"
    );
    assert!(barrier_report.coverage().is_locally_minimal());

    let race = admitted(conflicting_store_module());
    let race_request = SimulationRequestV1::new(
        "conflict",
        [2, 1, 1],
        [2, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[0]))],
    );
    let race_report = race
        .reduce_simulation_failure(
            &race_request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            SimulationFailureScheduleV1::Seeded { seed: 9 },
            reduction_limits(16),
        )
        .unwrap();
    assert_eq!(race_report.fingerprint().class(), "data_race");
    assert!(race_report.fingerprint().related_site().is_some());
    assert!(race_report.coverage().is_locally_minimal());
    let recorded = race
        .simulate_scheduled(
            &race_request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 9,
                max_decisions: 16,
            },
        )
        .unwrap();
    assert_eq!(
        recorded.schedule_record().unwrap().decisions(),
        race_report.original_decisions()
    );
    assert_eq!(
        race.replay_simulation_failure_reduction(
            &race_request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            &race_report,
        )
        .unwrap(),
        race_report.fingerprint().clone()
    );
}

#[test]
fn failure_reduction_report_is_canonical_and_identity_bound() {
    let admitted = admitted(indexed_store_module());
    let request = SimulationRequestV1::new(
        "store",
        [2, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    let limits = SimulationLimitsV1::default();
    let report = admitted
        .reduce_simulation_failure(
            &request,
            SimulationTargetV1::amdgpu_64(),
            limits,
            SimulationFailureScheduleV1::Seeded { seed: u64::MAX },
            reduction_limits(16),
        )
        .unwrap();
    let bytes = report.to_canonical_bytes().unwrap();
    let decoded = SimulationFailureReductionReportV1::from_canonical_bytes(&bytes).unwrap();
    assert_eq!(decoded, report);
    let mut whitespace = bytes.clone();
    whitespace.insert(0, b' ');
    assert!(SimulationFailureReductionReportV1::from_canonical_bytes(&whitespace).is_err());
    let mut mutated = bytes;
    let marker = b"\"report_sha256\":\"";
    let position = mutated
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap()
        + marker.len();
    mutated[position] = if mutated[position] == b'0' {
        b'1'
    } else {
        b'0'
    };
    assert!(SimulationFailureReductionReportV1::from_canonical_bytes(&mutated).is_err());

    let different_request = SimulationRequestV1::new(
        "store",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(u32_buffer(&[9]))],
    );
    assert!(
        admitted
            .replay_simulation_failure_reduction(
                &different_request,
                SimulationTargetV1::amdgpu_64(),
                limits,
                &report,
            )
            .is_err()
    );

    let tiny_resident = SimulationLimitsV1 {
        max_resident_bytes: 1,
        ..limits
    };
    assert!(matches!(
        admitted.reduce_simulation_failure(
            &request,
            SimulationTargetV1::amdgpu_64(),
            tiny_resident,
            SimulationFailureScheduleV1::Canonical,
            reduction_limits(16),
        ),
        Err(fe2o3_kir_sim::SimulationFailureReductionErrorV1::ResidentLimit { limit: 1, .. })
    ));
}
