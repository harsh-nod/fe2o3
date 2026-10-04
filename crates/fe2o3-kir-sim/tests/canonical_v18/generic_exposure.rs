//! Synthetic genuine V18-owner simulation, not source lowering or GPU evidence.
use super::*;
use fe2o3_kernel_ir::{CastKind, WorkgroupMemory, WorkgroupMemoryExtent, WorkgroupSize};

const U32: Type = Type::Scalar(ScalarType::U32);

fn pointer(space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(U32, space, access)
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn store(address: u32, value: u32, space: AddressSpace) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(address),
            value: ValueId(value),
            access: MemoryAccess::new(space, 4),
        },
    )
}
fn refresh(raw: &mut Module) {
    for function in &mut raw.functions {
        function.required_capabilities = function.derived_capabilities();
    }
    raw.required_capabilities = raw.derived_capabilities();
    raw.kernels[0].required_capabilities = raw.required_capabilities.clone();
}
fn module(parameters: Vec<Type>, blocks: Vec<BasicBlock>, helpers: Vec<Function>) -> Module {
    let values = (0..parameters.len()).map(|id| ValueId(id as u32)).collect();
    let mut raw = Module::new("canonical-v18::generic-exposure");
    raw.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        values,
        blocks,
    ));
    raw.functions.extend(helpers);
    raw.kernels.push(Kernel::new(
        "generic",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    refresh(&mut raw);
    raw
}
fn identity_helper(ty: Type) -> Function {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    Function::internal_helper(
        "identity",
        Signature::new(vec![ty.clone()], vec![ty]),
        vec![ValueId(0)],
        vec![block],
    )
}
fn with_view(raw: &Module, inspect: impl FnOnce(&AdmittedSimulationModuleV1)) {
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let (canonical, owner_paid) = owner(raw, &mut budget);
    let (admitted, view_paid) = view(&canonical, &mut budget);
    assert_eq!(admitted.identity().wire_version(), 18);
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
fn index(value: u128, target: SimulationTargetV1) -> SimulationArgumentV1 {
    SimulationArgumentV1::Scalar(ScalarBitsV1::new(ScalarType::Index, value, target).unwrap())
}
fn buffer(values: &[u32], initialized: bool, target: SimulationTargetV1) -> SimulationArgumentV1 {
    let bytes = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let length = bytes.len();
    SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            bytes,
            vec![initialized; length],
            target,
        )
        .unwrap(),
    )
}
fn execution_error(error: SimulationErrorV1) -> SimulationExecutionErrorKindV1 {
    match error {
        SimulationErrorV1::Execution(error) => error.kind,
        other => panic!("expected execution refusal, got {other:?}"),
    }
}
fn word(execution: &SimulationExecutionV1, argument: usize, ordinal: usize) -> u32 {
    let buffer = execution.buffer(argument).unwrap();
    let offset = ordinal * 4;
    assert!(
        buffer.initialized()[offset..offset + 4]
            .iter()
            .all(|value| *value)
    );
    u32::from_le_bytes(buffer.bytes()[offset..offset + 4].try_into().unwrap())
}

fn pointer_module(private: bool, initialize: bool) -> Module {
    let global = pointer(AddressSpace::Global, AccessMode::ReadWrite);
    let generic = pointer(AddressSpace::Generic, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(0));
    if private {
        entry.operations.push(op(
            10,
            pointer(AddressSpace::Private, AccessMode::ReadWrite),
            OperationKind::Alloca {
                element: U32,
                count: Some(ValueId(4)),
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ));
    }
    for (id, input) in [(11, if private { 10 } else { 1 }), (13, 0)] {
        entry.operations.push(op(
            id,
            generic.clone(),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(input),
                to: generic.clone(),
            },
        ));
    }
    entry.operations.push(op(
        12,
        generic.clone(),
        OperationKind::Call {
            callee: "identity".into(),
            arguments: vec![ValueId(11)],
        },
    ));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(12)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(13)],
    });
    let arms = [(1, 20), (2, 30)].map(|(block, parameter)| {
        let mut arm = BasicBlock::new(BlockId(block));
        arm.parameters
            .push(ValueDef::new(ValueId(parameter), generic.clone()));
        arm.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![ValueId(parameter)],
        });
        arm
    });
    let mut join = BasicBlock::new(BlockId(3));
    join.parameters
        .push(ValueDef::new(ValueId(40), generic.clone()));
    join.operations = vec![
        op(
            41,
            generic.clone(),
            OperationKind::Select {
                condition: ValueId(3),
                true_value: ValueId(40),
                false_value: ValueId(13),
            },
        ),
        op(
            42,
            generic.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(41),
                offset: ValueId(2),
            },
        ),
    ];
    for (id, access) in [(44, AccessMode::ReadOnly)] {
        let ty = pointer(AddressSpace::Generic, access);
        join.operations.push(op(
            id,
            ty.clone(),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(42),
                to: ty,
            },
        ));
    }
    if initialize {
        join.operations.push(store(42, 5, AddressSpace::Generic));
    }
    join.operations.push(op(
        43,
        U32,
        OperationKind::Load {
            pointer: ValueId(44),
            access: MemoryAccess::new(AddressSpace::Generic, 4),
        },
    ));
    join.operations.push(store(0, 43, AddressSpace::Global));
    join.terminator = Some(Terminator::Return { values: vec![] });
    module(
        vec![
            global.clone(),
            global,
            Type::INDEX,
            Type::BOOL,
            Type::INDEX,
            U32,
        ],
        vec![entry, arms[0].clone(), arms[1].clone(), join],
        vec![identity_helper(generic)],
    )
}
fn pointer_request(
    offset: u128,
    take_source: bool,
    count: u128,
    initialized: bool,
    target: SimulationTargetV1,
) -> SimulationRequestV1 {
    SimulationRequestV1::new(
        "generic",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            buffer(&[0x5a5a5a5a, 0x5a5a5a5a], false, target),
            buffer(&[11, 29], initialized, target),
            index(offset, target),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(ScalarType::Bool, u128::from(take_source), target).unwrap(),
            ),
            index(count, target),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(57)),
        ],
    )
}

#[test]
fn concrete_pointer_exposure_survives_helper_cfg_select_gep_and_access_restriction() {
    for target in [
        SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
        SimulationTargetV1::amdgpu_64(),
    ] {
        for private in [false, true] {
            with_view(&pointer_module(private, true), |admitted| {
                for take_source in [false, true] {
                    let request =
                        pointer_request(u128::from(take_source), take_source, 2, true, target);
                    let original = request.clone();
                    let execution = admitted
                        .simulate_scheduled(
                            &request,
                            target,
                            SimulationLimitsV1::default(),
                            SimulationScheduleRequestV1::RecordSeeded {
                                seed: 37,
                                max_decisions: 128,
                            },
                        )
                        .unwrap();
                    assert_eq!(word(&execution, 0, 0), 57);
                    assert_eq!(word(&execution, 1, 0), 11);
                    assert_eq!(
                        word(&execution, 1, 1),
                        if take_source && !private { 57 } else { 29 }
                    );
                    assert_eq!(&execution.buffer(0).unwrap().bytes()[4..], &[0x5a; 4]);
                    assert_eq!(
                        &execution.buffer(0).unwrap().initialized()[4..],
                        &[false; 4]
                    );
                    let replay = admitted
                        .simulate_scheduled(
                            &request,
                            target,
                            SimulationLimitsV1::default(),
                            SimulationScheduleRequestV1::Replay(
                                execution.schedule_record().unwrap(),
                            ),
                        )
                        .unwrap();
                    assert_eq!(word(&replay, 0, 0), 57);
                    assert_eq!(
                        execution.schedule_transcript_identity(),
                        replay.schedule_transcript_identity()
                    );
                    assert_eq!(request, original);
                    assert!(!execution.grants_execution_authority());
                }
            });
        }
    }
}

#[test]
fn generic_pointer_keeps_bounds_and_initialization_failures() {
    for private in [false, true] {
        with_view(&pointer_module(private, true), |admitted| {
            let request = pointer_request(2, true, 2, true, target());
            assert!(matches!(
                execution_error(
                    admitted
                        .simulate(&request, target(), SimulationLimitsV1::default(),)
                        .unwrap_err()
                ),
                SimulationExecutionErrorKindV1::OutOfBounds {
                    offset: 8,
                    bytes: 4,
                    ..
                }
            ));
        });
        with_view(&pointer_module(private, false), |admitted| {
            let request = pointer_request(0, true, 2, false, target());
            assert!(matches!(
                execution_error(
                    admitted
                        .simulate(&request, target(), SimulationLimitsV1::default(),)
                        .unwrap_err()
                ),
                SimulationExecutionErrorKindV1::UninitializedRead {
                    offset: 0,
                    bytes: 4,
                    ..
                }
            ));
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
fn bindings(records: &Records) -> Vec<&SimulationDebugBindingV1> {
    records
        .0
        .iter()
        .flat_map(|record| match &record.kind {
            SimulationDebugRecordKindV1::Checkpoint {
                stack: SimulationDebugCollectionV1::Captured(frames),
                ..
            } => frames.as_slice(),
            _ => &[],
        })
        .flat_map(|frame| match &frame.values {
            SimulationDebugCollectionV1::Captured(values) => values.as_slice(),
            _ => &[],
        })
        .collect()
}

#[test]
fn debug_generic_values_keep_concrete_memory_and_allocation_identity() {
    for (private, concrete) in [(false, AddressSpace::Global), (true, AddressSpace::Private)] {
        with_view(&pointer_module(private, true), |admitted| {
            let mut records = Records::default();
            let execution = admitted
                .simulate_debugged_with_sink(
                    &pointer_request(1, true, 2, true, target()),
                    target(),
                    SimulationLimitsV1::default(),
                    SimulationDebugCaptureLimitsV1::new(8, 64, 8, 128).unwrap(),
                    &mut records,
                )
                .unwrap();
            assert_eq!(word(&execution, 0, 0), 57);
            let allocation = records
                .0
                .iter()
                .find_map(|record| match &record.kind {
                    SimulationDebugRecordKindV1::Memory {
                        access: SimulationDebugMemoryAccessV1::WriteCommitted,
                        allocation,
                        address_space,
                        byte_offset: 4,
                        byte_len: 4,
                        ..
                    } if *address_space == concrete => Some(*allocation),
                    _ => None,
                })
                .expect("the generic write must retain the concrete backing");
            for value in [11, 12, 40, 41, 42, 44] {
                assert!(
                    bindings(&records).iter().any(|binding| matches!(
                        binding.observed,
                        SimulationDebugValueV1::Pointer {
                            allocation: observed, address_space: AddressSpace::Generic,
                            lower_bound: 0, upper_bound: 8, ..
                        } if binding.value == ValueId(value) && observed == allocation
                    )),
                    "missing exact generic binding {value}"
                );
            }
            assert!(records.0.iter().any(|record| matches!(&record.kind,
                SimulationDebugRecordKindV1::Checkpoint {
                    memory: SimulationDebugCollectionV1::Captured(allocations), ..
                } if allocations.iter().any(|row| row.allocation == allocation
                    && row.address_space == concrete)
            )));
            assert!(!records.0.iter().any(|record| matches!(
                &record.kind,
                SimulationDebugRecordKindV1::Memory {
                    address_space: AddressSpace::Generic,
                    ..
                }
            )));
        });
    }
}

fn slice_module() -> Module {
    let slice = Type::slice(U32, AddressSpace::Global, AccessMode::ReadWrite);
    let generic = Type::slice(U32, AddressSpace::Generic, AccessMode::ReadWrite);
    let generic_pointer = pointer(AddressSpace::Generic, AccessMode::ReadWrite);
    let index_output = Type::pointer(Type::INDEX, AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            10,
            generic.clone(),
            OperationKind::Cast {
                kind: CastKind::SliceToGeneric,
                value: ValueId(1),
                to: generic.clone(),
            },
        ),
        op(
            11,
            generic.clone(),
            OperationKind::Call {
                callee: "identity".into(),
                arguments: vec![ValueId(10)],
            },
        ),
        op(
            12,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(11) },
        ),
        op(
            13,
            generic_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(11) },
        ),
        op(
            14,
            generic_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(13),
                offset: ValueId(3),
            },
        ),
        op(
            15,
            U32,
            OperationKind::Load {
                pointer: ValueId(14),
                access: MemoryAccess::new(AddressSpace::Generic, 4),
            },
        ),
        store(0, 15, AddressSpace::Global),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(12),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    module(
        vec![
            pointer(AddressSpace::Global, AccessMode::ReadWrite),
            slice,
            index_output,
            Type::INDEX,
        ],
        vec![block],
        vec![identity_helper(generic)],
    )
}
fn slice_request(offset: u128, target: SimulationTargetV1) -> SimulationRequestV1 {
    let width = if target.index_width() == IndexWidthV1::Bits32 {
        4
    } else {
        8
    };
    let data = match buffer(&[7, 11, 29, 31], true, target) {
        SimulationArgumentV1::Buffer(buffer) => buffer,
        _ => unreachable!(),
    };
    SimulationRequestV1::new(
        "generic",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            buffer(&[0, 0], false, target),
            SimulationArgumentV1::BufferView(
                BufferViewArgumentV1::new(
                    BufferBackingIdV1(7),
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    4,
                    2,
                    target,
                )
                .unwrap(),
            ),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    ScalarType::Index,
                    AccessMode::ReadWrite,
                    4,
                    vec![0; width],
                    vec![false; width],
                    target,
                )
                .unwrap(),
            ),
            index(offset, target),
        ],
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: BufferBackingIdV1(7),
        buffer: data,
    }])
}

#[test]
fn generic_slice_keeps_view_length_offset_helper_return_and_debug_bounds() {
    for target in [
        SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
        SimulationTargetV1::amdgpu_64(),
    ] {
        with_view(&slice_module(), |admitted| {
            let request = slice_request(1, target);
            let original = request.clone();
            let mut records = Records::default();
            let execution = admitted
                .simulate_debugged_with_sink(
                    &request,
                    target,
                    SimulationLimitsV1::default(),
                    SimulationDebugCaptureLimitsV1::new(8, 32, 8, 128).unwrap(),
                    &mut records,
                )
                .unwrap();
            assert_eq!(word(&execution, 0, 0), 29);
            let length = execution.buffer(2).unwrap();
            let mut bytes = [0u8; 8];
            bytes[..length.bytes().len()].copy_from_slice(length.bytes());
            assert_eq!(u64::from_le_bytes(bytes), 2);
            assert!(length.initialized().iter().all(|value| *value));
            assert!(
                bindings(&records)
                    .iter()
                    .any(|binding| matches!(binding.observed,
                        SimulationDebugValueV1::Slice {
                            address_space: AddressSpace::Generic, elements: 2,
                            byte_offset: 4, byte_len: 8, ..
                        } if binding.value == ValueId(11)
                    ))
            );
            assert!(
                bindings(&records)
                    .iter()
                    .any(|binding| matches!(binding.observed,
                        SimulationDebugValueV1::Pointer {
                            address_space: AddressSpace::Generic,
                            lower_bound: 4, upper_bound: 12, byte_offset: 8, ..
                        } if binding.value == ValueId(14)
                    ))
            );
            assert_eq!(request, original);
            assert!(matches!(
                execution_error(
                    admitted
                        .simulate(
                            &slice_request(2, target),
                            target,
                            SimulationLimitsV1::default(),
                        )
                        .unwrap_err()
                ),
                SimulationExecutionErrorKindV1::OutOfBounds {
                    offset: 12,
                    bytes: 4,
                    ..
                }
            ));
        });
    }
}

#[test]
fn generic_pointer_cannot_escape_a_released_callee_allocation() {
    let mut raw = pointer_module(true, true);
    let generic = pointer(AddressSpace::Generic, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            1,
            pointer(AddressSpace::Private, AccessMode::ReadWrite),
            OperationKind::Alloca {
                element: U32,
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        op(
            2,
            generic.clone(),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(1),
                to: generic,
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    raw.functions[1].body.as_mut().unwrap().blocks = vec![block];
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        assert!(matches!(
            execution_error(
                admitted
                    .simulate(
                        &pointer_request(0, true, 2, true, target()),
                        target(),
                        SimulationLimitsV1::default(),
                    )
                    .unwrap_err()
            ),
            SimulationExecutionErrorKindV1::DanglingPointer { .. }
        ));
    });
}

fn access_module(access: AccessMode) -> Module {
    let generic = pointer(AddressSpace::Generic, access);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            2,
            generic.clone(),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(1),
                to: generic.clone(),
            },
        ),
        op(
            3,
            generic.clone(),
            OperationKind::Call {
                callee: "identity".into(),
                arguments: vec![ValueId(2)],
            },
        ),
        op(
            4,
            U32,
            if access == AccessMode::ReadOnly {
                OperationKind::Load {
                    pointer: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Generic, 4),
                }
            } else {
                OperationKind::Constant(Constant::U32(71))
            },
        ),
    ];
    if access == AccessMode::WriteOnly {
        block.operations.push(store(3, 4, AddressSpace::Generic));
    }
    block.operations.push(store(0, 4, AddressSpace::Global));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module(
        vec![
            pointer(AddressSpace::Global, AccessMode::ReadWrite),
            pointer(AddressSpace::Global, access),
        ],
        vec![block],
        vec![identity_helper(generic)],
    )
}

#[test]
fn generic_exposure_preserves_exact_readonly_and_writeonly_rights() {
    for access in [AccessMode::ReadOnly, AccessMode::WriteOnly] {
        with_view(&access_module(access), |admitted| {
            let request = SimulationRequestV1::new(
                "generic",
                [1, 1, 1],
                [1, 1, 1],
                vec![
                    buffer(&[0], false, target()),
                    SimulationArgumentV1::Buffer(
                        BufferArgumentV1::new(
                            ScalarType::U32,
                            access,
                            4,
                            29_u32.to_le_bytes().to_vec(),
                            vec![true; 4],
                            target(),
                        )
                        .unwrap(),
                    ),
                ],
            );
            let execution = admitted
                .simulate(&request, target(), SimulationLimitsV1::default())
                .unwrap();
            assert_eq!(
                word(&execution, 0, 0),
                if access == AccessMode::ReadOnly {
                    29
                } else {
                    71
                }
            );
            assert_eq!(
                word(&execution, 1, 0),
                if access == AccessMode::ReadOnly {
                    29
                } else {
                    71
                }
            );
        });
    }
}

#[test]
fn malformed_generic_type_access_and_call_correspondence_never_form_an_owner() {
    for fault in 0..4 {
        let mut raw = if fault == 1 {
            access_module(AccessMode::WriteOnly)
        } else {
            pointer_module(false, true)
        };
        match fault {
            0 => {
                let operation =
                    &mut raw.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
                let changed = pointer(AddressSpace::Generic, AccessMode::ReadOnly);
                operation.results[0].ty = changed.clone();
                let OperationKind::Cast { to, .. } = &mut operation.kind else {
                    unreachable!()
                };
                *to = changed;
            }
            1 => {
                let operations = &mut raw.functions[0].body.as_mut().unwrap().blocks[0].operations;
                operations[2].kind = OperationKind::Load {
                    pointer: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Generic, 4),
                };
            }
            2 => {
                let operations = &mut raw.functions[0].body.as_mut().unwrap().blocks[3].operations;
                let OperationKind::Store { pointer, .. } = &mut operations[3].kind else {
                    unreachable!()
                };
                *pointer = ValueId(44);
            }
            _ => {
                raw.functions[1].signature.results[0] =
                    pointer(AddressSpace::Global, AccessMode::ReadWrite);
            }
        }
        let mut work = Work::new(BOUND);
        let mut budget = Budget::new(&mut work, BOUND);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(
            Owner::from_module_ref_with_verification_budget_v18(&raw, LAYOUT_LIMITS, &mut budget,)
                .is_err(),
            "malformed case {fault}"
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn generic_and_constant_entry_buffers_do_not_gain_root_abi_authority() {
    for space in [AddressSpace::Generic, AddressSpace::Constant] {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let raw = module(
            vec![pointer(space, AccessMode::ReadOnly)],
            vec![block],
            vec![],
        );
        with_view(&raw, |admitted| {
            let request = SimulationRequestV1::new(
                "generic",
                [1, 1, 1],
                [1, 1, 1],
                vec![buffer(&[11], true, target())],
            );
            let error = admitted
                .preflight(&request, target(), SimulationLimitsV1::default())
                .unwrap_err();
            if space == AddressSpace::Generic {
                let SimulationPreflightErrorV1::Unsupported(report) = error else {
                    panic!("expected generic root refusal: {error:?}");
                };
                assert!(report.findings().iter().any(|finding| finding.feature
                    == UnsupportedFeatureV1::UnsupportedAddressSpace(AddressSpace::Generic)));
            } else {
                assert!(matches!(
                    error,
                    SimulationPreflightErrorV1::ArgumentType { argument: 0, .. }
                ));
            }
        });
    }
}

#[test]
fn generic_atomic_and_memory_intrinsic_accesses_remain_preflight_refusals() {
    use fe2o3_kernel_ir::{
        Atomic, AtomicKind, MemoryElementType, MemoryIntrinsicOperation, MemoryLayout,
        MemoryOrdering, SynchronizationScope, VolatileAccessContract,
    };
    for atomic in [false, true] {
        let mut raw = access_module(AccessMode::ReadOnly);
        raw.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind = if atomic {
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Load,
                pointer: ValueId(3),
                value: None,
                compare: None,
                access: MemoryAccess::new(AddressSpace::Generic, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Relaxed,
                failure_ordering: None,
            })
        } else {
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(3),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space: AddressSpace::Generic,
                layout: MemoryLayout::new(4, 4),
                contract: VolatileAccessContract::rust_allocation_load(),
            })
        };
        refresh(&mut raw);
        with_view(&raw, |admitted| {
            let request = SimulationRequestV1::new(
                "generic",
                [1, 1, 1],
                [1, 1, 1],
                vec![
                    buffer(&[0], false, target()),
                    SimulationArgumentV1::Buffer(
                        BufferArgumentV1::new(
                            ScalarType::U32,
                            AccessMode::ReadOnly,
                            4,
                            29_u32.to_le_bytes().to_vec(),
                            vec![true; 4],
                            target(),
                        )
                        .unwrap(),
                    ),
                ],
            );
            let before = request.clone();
            let error = admitted
                .preflight(&request, target(), SimulationLimitsV1::default())
                .unwrap_err();
            let SimulationPreflightErrorV1::Unsupported(report) = error else {
                panic!("expected operation-family refusal: {error:?}");
            };
            assert!(
                report.findings().iter().any(|finding| finding.feature
                    == UnsupportedFeatureV1::UnsupportedAddressSpace(AddressSpace::Generic)),
                "generic access was not refused for atomic={atomic}: {report:?}"
            );
            assert_eq!(request, before);
        });
    }
}

#[derive(Default)]
struct Events(Vec<SimulationEventKindV1>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        assert!(self.0.len() < 128);
        self.0.push(event.kind.clone());
        Ok(())
    }
}

#[test]
fn generic_workgroup_access_retains_distinct_concrete_allocations() {
    let mut block = BasicBlock::new(BlockId(0));
    let concrete = Type::pointer(Type::INDEX, AddressSpace::Workgroup, AccessMode::ReadWrite);
    let generic = Type::pointer(Type::INDEX, AddressSpace::Generic, AccessMode::ReadWrite);
    let output = Type::pointer(Type::INDEX, AddressSpace::Global, AccessMode::ReadWrite);
    block.operations = vec![
        op(
            1,
            concrete,
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::INDEX,
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 8,
            }),
        ),
        op(
            2,
            generic.clone(),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(1),
                to: generic,
            },
        ),
        op(
            3,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Generic, 8),
            },
        ),
        op(
            4,
            Type::INDEX,
            OperationKind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Generic, 8),
            },
        ),
        op(
            5,
            output.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(3),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(5),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 8),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut raw = module(vec![output], vec![block], vec![]);
    raw.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    with_view(&raw, |admitted| {
        let request = SimulationRequestV1::new(
            "generic",
            [2, 1, 1],
            [1, 1, 1],
            vec![SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    ScalarType::Index,
                    AccessMode::ReadWrite,
                    8,
                    vec![0xff; 16],
                    vec![false; 16],
                    target(),
                )
                .unwrap(),
            )],
        );
        let mut events = Events::default();
        let execution = admitted
            .simulate_observed_with_sink(
                &request,
                target(),
                SimulationLimitsV1::default(),
                &mut events,
            )
            .unwrap();
        let output = execution.buffer(0).unwrap();
        assert_eq!(
            output.bytes(),
            [0u64.to_le_bytes(), 1u64.to_le_bytes()].concat()
        );
        assert!(output.initialized().iter().all(|value| *value));
        let created = events
            .0
            .iter()
            .filter_map(|event| match event {
                SimulationEventKindV1::AllocationCreated {
                    allocation,
                    address_space: AddressSpace::Workgroup,
                    bytes: 8,
                } => Some(*allocation),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(created.len(), 2);
        assert_ne!(created[0], created[1]);
        for allocation in created {
            assert_eq!(events.0.iter().filter(|event| matches!(event,
                SimulationEventKindV1::AllocationReleased { allocation: released } if *released == allocation
            )).count(), 1);
        }
    });
}

#[test]
fn guarded_generic_access_does_not_dereference_an_inactive_one_past_pointer() {
    let mut raw = pointer_module(false, true);
    let operations = &mut raw.functions[0].body.as_mut().unwrap().blocks[3].operations;
    operations[3].kind = OperationKind::GuardedStore {
        pointer: ValueId(42),
        predicate: ValueId(3),
        value: ValueId(5),
        access: MemoryAccess::new(AddressSpace::Generic, 4),
    };
    operations[4].kind = OperationKind::GuardedLoad {
        pointer: ValueId(44),
        predicate: ValueId(3),
        fallback: ValueId(5),
        access: MemoryAccess::new(AddressSpace::Generic, 4),
    };
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        for active in [false, true] {
            let request = pointer_request(if active { 1 } else { 2 }, active, 2, true, target());
            let mut events = Events::default();
            let execution = admitted
                .simulate_observed_with_sink(
                    &request,
                    target(),
                    SimulationLimitsV1::default(),
                    &mut events,
                )
                .unwrap();
            assert_eq!(word(&execution, 0, 0), 57);
            assert_eq!(word(&execution, 1, 1), if active { 57 } else { 29 });
            assert_eq!(
                events
                    .0
                    .iter()
                    .filter(|event| matches!(event, SimulationEventKindV1::MemoryRead { .. },))
                    .count(),
                usize::from(active)
            );
            assert_eq!(
                events
                    .0
                    .iter()
                    .filter(|event| matches!(event, SimulationEventKindV1::MemoryWrite { .. },))
                    .count(),
                1 + usize::from(active)
            );
        }
    });
}
