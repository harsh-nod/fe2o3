use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, CheckedBinaryOperator,
    ComparePredicate, Constant, Function, IndexKind, IntegerSwitchCase, IntrinsicKind,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation,
    OperationKind, ScalarType, Signature, SwitchCase, TargetCapability, Terminator, Type, UnaryOp,
    ValueDef, ValueId, VerifiedCanonicalKernelIrV6, WaveOperation, WaveOperationKind, WaveWidth,
    WorkgroupSize,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    EventPolicyV1, MAX_REPORTED_UNSUPPORTED_FINDINGS_V1,
    MAX_REPORTED_UNSUPPORTED_IDENTIFIER_BYTES_V1, ScalarBitsV1, SharedBufferV1,
    SimulationAdmissionErrorV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationErrorV1, SimulationEventKindV1, SimulationEventSinkControlV1, SimulationEventSinkV1,
    SimulationEventV1, SimulationExecutionErrorKindV1, SimulationExecutionOutcomeV1,
    SimulationLimitsV1, SimulationPreflightErrorV1, SimulationRequestV1,
    SimulationScheduleIdentityV1, SimulationTargetV1, UnsupportedFeatureV1,
};

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn admitted(module: Module) -> AdmittedSimulationModuleV1 {
    let canonical = VerifiedCanonicalKernelIrV6::from_module(module).expect("verified fixture");
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

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four bytes")))
        .collect()
}

#[derive(Default)]
struct Collector(Vec<SimulationEventV1>);

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
            Type::Scalar(ScalarType::U32),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Truncate,
                value: ValueId(0),
                to: Type::Scalar(ScalarType::U32),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(3)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(3)],
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
fn rejects_every_reachable_unsupported_site_before_execution() {
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
        .expect_err("wave must fail in preflight");
    let SimulationErrorV1::Preflight(SimulationPreflightErrorV1::Unsupported(findings)) = error
    else {
        panic!("expected typed unsupported preflight")
    };
    assert_eq!(findings.total_findings(), 1);
    assert!(!findings.is_truncated());
    assert_eq!(findings.findings()[0].function.as_str(), "reachable_wave");
    assert_eq!(findings.findings()[0].block, Some(BlockId(0)));
    assert_eq!(findings.findings()[0].operation, Some(0));
    assert_eq!(findings.findings()[0].feature, UnsupportedFeatureV1::Wave);
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
        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1
    );
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
        VerifiedCanonicalKernelIrV6::from_module(module.clone()).unwrap(),
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
        VerifiedCanonicalKernelIrV6::from_module(module.clone()).unwrap(),
        SimulationLimitsV1 {
            max_resident_bytes: actual,
            ..SimulationLimitsV1::default()
        },
    )
    .expect("exact admission resident boundary");
    assert!(matches!(
        AdmittedSimulationModuleV1::admit(
            VerifiedCanonicalKernelIrV6::from_module(module).unwrap(),
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
