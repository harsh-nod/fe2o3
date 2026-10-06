//! Synthetic structural V18 controls, not original-source or hardware qualification.
use super::*;
use fe2o3_kernel_ir::{CastKind, StorageProjectionV1};

fn width(scalar: ScalarType, target: SimulationTargetV1) -> usize {
    let bits = scalar.bit_width().unwrap_or(match target.index_width() {
        IndexWidthV1::Bits32 => 32,
        IndexWidthV1::Bits64 => 64,
    });
    usize::from(bits.div_ceil(8))
}

fn pointer(row: u32, access: AccessMode) -> Type {
    Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(row)),
        AddressSpace::Private,
        access,
    )
}

fn refresh(raw: &mut Module) {
    let capabilities = raw.functions[0].derived_capabilities();
    raw.functions[0].required_capabilities = capabilities.clone();
    raw.kernels[0].required_capabilities = capabilities.clone();
    raw.required_capabilities = capabilities;
}

fn scalar_module(scalar: ScalarType, target: SimulationTargetV1) -> Module {
    let bytes = width(scalar, target) as u32;
    let global = Type::pointer(
        Type::Scalar(scalar),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(3), pointer(0, AccessMode::ReadWrite)),
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(0)),
                count: Some(ValueId(2)),
                address_space: AddressSpace::Private,
                alignment: bytes,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(3),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, bytes),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(scalar)),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, bytes),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, bytes),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut raw = Module::new("synthetic-private-scalar-storage");
    raw.storage_layouts.push(StorageLayoutV1 {
        size: u64::from(bytes),
        alignment: bytes,
        kind: StorageLayoutKindV1::Scalar(scalar),
    });
    raw.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![global, Type::Scalar(scalar), Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    raw.kernels.push(Kernel::new(
        "scalar",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    refresh(&mut raw);
    raw
}

fn scalar_request(
    scalar: ScalarType,
    bits: u128,
    count: u128,
    target: SimulationTargetV1,
) -> SimulationRequestV1 {
    let bytes = width(scalar, target);
    SimulationRequestV1::new(
        "scalar",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    scalar,
                    AccessMode::ReadWrite,
                    bytes as u32,
                    vec![0x5a; bytes * 2],
                    vec![false; bytes * 2],
                    target,
                )
                .unwrap(),
            ),
            SimulationArgumentV1::Scalar(ScalarBitsV1::new(scalar, bits, target).unwrap()),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(ScalarType::Index, count, target).unwrap(),
            ),
        ],
    )
}

fn with_view<T>(raw: &Module, inspect: impl FnOnce(&AdmittedSimulationModuleV1) -> T) -> T {
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let (canonical, owner_paid) = owner(raw, &mut budget);
    let (admitted, view_paid) = view(&canonical, &mut budget);
    assert_eq!(admitted.module(), canonical.module());
    assert_eq!(admitted.identity().digest(), canonical.identity().digest());
    assert_eq!(admitted.identity().wire_version(), 18);
    let result = inspect(&admitted);
    drop(admitted);
    budget.release_storage(view_paid).unwrap();
    drop(canonical);
    budget.release_storage(owner_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}

fn output(
    execution: &SimulationExecutionV1,
    scalar: ScalarType,
    bits: u128,
    target: SimulationTargetV1,
) {
    let bytes = width(scalar, target);
    let buffer = execution.buffer(0).unwrap();
    assert_eq!(&buffer.bytes()[..bytes], &bits.to_le_bytes()[..bytes]);
    assert_eq!(&buffer.bytes()[bytes..], vec![0x5a; bytes]);
    assert!(buffer.initialized()[..bytes].iter().all(|value| *value));
    assert!(buffer.initialized()[bytes..].iter().all(|value| !*value));
    assert!(!execution.grants_execution_authority());
}

fn unsupported(error: SimulationPreflightErrorV1, expected: UnsupportedFeatureV1) {
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("expected typed unsupported report: {error:?}");
    };
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.feature == expected),
        "{report:?}"
    );
}

#[test]
fn private_scalar_storage_preserves_every_scalar_bit_width_and_target_index() {
    for target in [
        SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
        SimulationTargetV1::amdgpu_64(),
    ] {
        for scalar in [
            ScalarType::Bool,
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
            ScalarType::Index,
            ScalarType::F16,
            ScalarType::Bf16,
            ScalarType::F32,
            ScalarType::F64,
        ] {
            let bytes = width(scalar, target);
            let maximum = if scalar == ScalarType::Bool {
                1
            } else {
                u128::MAX >> (128 - bytes * 8)
            };
            with_view(&scalar_module(scalar, target), |admitted| {
                // Float samples are raw bits, including NaN; never host arithmetic.
                for bits in [0, maximum, maximum / 2] {
                    let request = scalar_request(scalar, bits, 3, target);
                    let before = request.clone();
                    let result = admitted
                        .simulate(&request, target, SimulationLimitsV1::default())
                        .unwrap();
                    output(&result, scalar, bits, target);
                    assert_eq!(request, before);
                }
            });
        }
    }
}

#[test]
fn private_index_storage_rejects_structurally_valid_wrong_target_width() {
    for (source, target) in [
        (IndexWidthV1::Bits32, IndexWidthV1::Bits64),
        (IndexWidthV1::Bits64, IndexWidthV1::Bits32),
    ] {
        let target = SimulationTargetV1::little_endian(target);
        let raw = scalar_module(ScalarType::Index, SimulationTargetV1::little_endian(source));
        with_view(&raw, |admitted| {
            unsupported(
                admitted
                    .preflight(
                        &scalar_request(ScalarType::Index, 1, 1, target),
                        target,
                        SimulationLimitsV1::default(),
                    )
                    .unwrap_err(),
                UnsupportedFeatureV1::InertStorage,
            );
        });
    }
}

#[test]
fn private_scalar_storage_checks_initialization_zero_extent_and_allocation_limits() {
    let target = SimulationTargetV1::amdgpu_64();
    let raw = scalar_module(ScalarType::U32, target);
    with_view(&raw, |admitted| {
        let request = scalar_request(ScalarType::U32, 37, 3, target);
        let limits = SimulationLimitsV1 {
            max_allocation_bytes: 12,
            max_total_bytes: 20,
            max_allocations: 2,
            ..SimulationLimitsV1::default()
        };
        output(
            &admitted.simulate(&request, target, limits).unwrap(),
            ScalarType::U32,
            37,
            target,
        );
        for (limits, expected) in [
            (
                SimulationLimitsV1 {
                    max_allocation_bytes: 11,
                    ..limits
                },
                SimulationExecutionErrorKindV1::AllocationBytesLimit {
                    actual: 12,
                    limit: 11,
                },
            ),
            (
                SimulationLimitsV1 {
                    max_total_bytes: 19,
                    ..limits
                },
                SimulationExecutionErrorKindV1::TotalBytesLimit {
                    actual: 20,
                    limit: 19,
                },
            ),
            (
                SimulationLimitsV1 {
                    max_allocations: 1,
                    ..limits
                },
                SimulationExecutionErrorKindV1::AllocationLimit { limit: 1 },
            ),
        ] {
            let error = admitted.simulate(&request, target, limits).unwrap_err();
            assert!(
                matches!(error, SimulationErrorV1::Execution(ref error) if error.kind == expected),
                "{error:?}"
            );
        }
        let empty = scalar_request(ScalarType::U32, 37, 0, target);
        let before = empty.clone();
        assert!(matches!(
            admitted.simulate(&empty, target, SimulationLimitsV1::default()),
            Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::OutOfBounds {
                    offset: 0,
                    bytes: 4,
                    allocation_bytes: 0,
                    ..
                },
                ..
            }))
        ));
        assert_eq!(empty, before);
    });
    let mut uninitialized = raw;
    uninitialized.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(1);
    refresh(&mut uninitialized);
    with_view(&uninitialized, |admitted| {
        assert!(matches!(
            admitted.simulate(
                &scalar_request(ScalarType::U32, 37, 1, target),
                target,
                SimulationLimitsV1::default(),
            ),
            Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::UninitializedRead {
                    offset: 0,
                    bytes: 4,
                    ..
                },
                ..
            }))
        ));
    });
}

#[test]
fn scalar_storage_row_identity_survives_cfg_transport_and_replay() {
    let target = SimulationTargetV1::amdgpu_64();
    let mut raw = scalar_module(ScalarType::U64, target);
    // Distinct rows have the same shape; transport must not normalize row 1 into row 0.
    raw.storage_layouts.push(raw.storage_layouts[0].clone());
    let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations[0].results[0].ty = pointer(1, AccessMode::ReadWrite);
    let OperationKind::Alloca { element, .. } = &mut block.operations[0].kind else {
        unreachable!()
    };
    *element = Type::StorageObject(StorageLayoutIdV1(1));
    let mut next = BasicBlock::new(BlockId(1));
    next.parameters
        .push(ValueDef::new(ValueId(5), pointer(1, AccessMode::ReadWrite)));
    next.operations = block.operations.split_off(1);
    for operation in &mut next.operations {
        match &mut operation.kind {
            OperationKind::Storage(StorageOperationV1::ReadValue { address, .. })
            | OperationKind::Storage(StorageOperationV1::WriteValue { address, .. }) => {
                *address = ValueId(5)
            }
            _ => {}
        }
    }
    next.terminator = block.terminator.take();
    block.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(3)],
    });
    raw.functions[0].body.as_mut().unwrap().blocks.push(next);
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        let bits = 0xfedc_ba98_7654_3210;
        let request = scalar_request(ScalarType::U64, bits, 1, target);
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
        output(&execution, ScalarType::U64, bits, target);
        let replay = admitted
            .simulate_scheduled(
                &request,
                target,
                SimulationLimitsV1::default(),
                SimulationScheduleRequestV1::Replay(execution.schedule_record().unwrap()),
            )
            .unwrap();
        output(&replay, ScalarType::U64, bits, target);
        assert_eq!(
            execution.schedule_transcript_identity(),
            replay.schedule_transcript_identity()
        );
    });
    raw.functions[0].body.as_mut().unwrap().blocks[1].parameters[0].ty =
        pointer(0, AccessMode::ReadWrite);
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    assert!(
        Owner::from_module_ref_with_verification_budget_v18(&raw, LAYOUT_LIMITS, &mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn storage_pointer_casts_and_ordinary_scalar_loads_remain_refused() {
    let target = SimulationTargetV1::amdgpu_64();
    let request = scalar_request(ScalarType::U32, 37, 1, target);
    for (kind, to) in [
        (
            CastKind::RestrictPointerAccess,
            pointer(0, AccessMode::ReadOnly),
        ),
        (
            CastKind::PointerToGeneric,
            Type::pointer(
                Type::StorageObject(StorageLayoutIdV1(0)),
                AddressSpace::Generic,
                AccessMode::ReadWrite,
            ),
        ),
    ] {
        let mut raw = scalar_module(ScalarType::U32, target);
        raw.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .insert(
                1,
                Operation::effect_free(
                    ValueDef::new(ValueId(5), to.clone()),
                    OperationKind::Cast {
                        kind,
                        value: ValueId(3),
                        to,
                    },
                ),
            );
        refresh(&mut raw);
        with_view(&raw, |admitted| {
            unsupported(
                admitted
                    .preflight(&request, target, SimulationLimitsV1::default())
                    .unwrap_err(),
                UnsupportedFeatureV1::UnsupportedScalarOperation,
            );
        });
    }
    let mut raw = scalar_module(ScalarType::U32, target);
    let load = &mut raw.functions[0].body.as_mut().unwrap().blocks[0].operations[2];
    load.results[0].ty = Type::StorageObject(StorageLayoutIdV1(0));
    load.kind = OperationKind::Load {
        pointer: ValueId(3),
        access: MemoryAccess::new(AddressSpace::Private, 4),
    };
    raw.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .pop();
    refresh(&mut raw);
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    assert!(
        Owner::from_module_ref_with_verification_budget_v18(&raw, LAYOUT_LIMITS, &mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn scalar_storage_projects_record_fields_but_does_not_admit_readonly_writes() {
    let target = SimulationTargetV1::amdgpu_64();
    let mut raw = scalar_module(ScalarType::U32, target);
    raw.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Record(vec![field(0, 0)].into_boxed_slice()),
    });
    let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations[0].results[0].ty = pointer(1, AccessMode::ReadWrite);
    let OperationKind::Alloca { element, .. } = &mut block.operations[0].kind else {
        unreachable!()
    };
    *element = Type::StorageObject(StorageLayoutIdV1(1));
    block.operations.insert(
        1,
        Operation::effect_free(
            ValueDef::new(ValueId(5), pointer(0, AccessMode::ReadWrite)),
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(3),
                step: StorageProjectionV1::Field(0),
            }),
        ),
    );
    for operation in &mut block.operations[2..] {
        match &mut operation.kind {
            OperationKind::Storage(StorageOperationV1::ReadValue { address, .. })
            | OperationKind::Storage(StorageOperationV1::WriteValue { address, .. }) => {
                *address = ValueId(5)
            }
            _ => {}
        }
    }
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        output(
            &admitted
                .simulate(
                    &scalar_request(ScalarType::U32, 37, 1, target),
                    target,
                    SimulationLimitsV1::default(),
                )
                .unwrap(),
            ScalarType::U32,
            37,
            target,
        );
    });
    let mut raw = scalar_module(ScalarType::U32, target);
    let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.insert(
        1,
        Operation::effect_free(
            ValueDef::new(ValueId(5), pointer(0, AccessMode::ReadOnly)),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(3),
                to: pointer(0, AccessMode::ReadOnly),
            },
        ),
    );
    let OperationKind::Storage(StorageOperationV1::WriteValue { address, .. }) =
        &mut block.operations[2].kind
    else {
        unreachable!()
    };
    *address = ValueId(5);
    refresh(&mut raw);
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    assert!(
        Owner::from_module_ref_with_verification_budget_v18(&raw, LAYOUT_LIMITS, &mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), 0);
}

#[derive(Default)]
struct DebugRecords(Vec<SimulationDebugRecordV1>);
impl SimulationDebugSinkV1 for DebugRecords {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        assert!(self.0.len() < 128);
        self.0.push(record);
        SimulationDebugSinkControlV1::Continue
    }
}

#[test]
fn scalar_storage_debug_execution_retains_memory_but_never_erases_tagged_values() {
    let target = SimulationTargetV1::amdgpu_64();
    with_view(&scalar_module(ScalarType::U32, target), |admitted| {
        let mut records = DebugRecords::default();
        let execution = admitted
            .simulate_debugged_with_sink(
                &scalar_request(ScalarType::U32, 37, 1, target),
                target,
                SimulationLimitsV1::default(),
                SimulationDebugCaptureLimitsV1::new(8, 32, 8, 128).unwrap(),
                &mut records,
            )
            .unwrap();
        output(&execution, ScalarType::U32, 37, target);
        assert!(records.0.iter().any(|record| matches!(
            &record.kind,
            SimulationDebugRecordKindV1::Checkpoint {
                stack: SimulationDebugCollectionV1::Unavailable {
                    reason: SimulationDebugUnavailableReasonV1::NotCaptured,
                    ..
                },
                memory: SimulationDebugCollectionV1::Captured(_),
                ..
            }
        )));
        for access in [
            SimulationDebugMemoryAccessV1::Read,
            SimulationDebugMemoryAccessV1::WriteCommitted,
        ] {
            assert!(records.0.iter().any(|record| matches!(&record.kind,
                SimulationDebugRecordKindV1::Memory {
                    access: observed, address_space: AddressSpace::Private,
                    byte_offset: 0, byte_len: 4, value: SimulationDebugValueV1::Scalar(value), ..
                } if *observed == access && *value == ScalarBitsV1::u32(37)
            )));
        }
        assert!(records.0.iter().any(|record| matches!(&record.kind,
            SimulationDebugRecordKindV1::Checkpoint {
                memory: SimulationDebugCollectionV1::Captured(allocations), ..
            } if allocations.iter().any(|allocation|
                allocation.address_space == AddressSpace::Private
                && allocation.bytes == 37_u32.to_le_bytes()
                && allocation.initialized == [true; 4]
            )
        )));
    });
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
fn private_scalar_storage_lifetime_closes_after_success_and_dynamic_failure() {
    let target = SimulationTargetV1::amdgpu_64();
    with_view(&scalar_module(ScalarType::U32, target), |admitted| {
        for count in [0, 1] {
            let mut events = Events::default();
            let execution = admitted.simulate_observed_with_sink(
                &scalar_request(ScalarType::U32, 37, count, target),
                target,
                SimulationLimitsV1::default(),
                &mut events,
            );
            if count == 0 {
                assert!(matches!(
                    execution,
                    Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                        kind: SimulationExecutionErrorKindV1::OutOfBounds { .. },
                        ..
                    }))
                ));
            } else {
                output(&execution.unwrap(), ScalarType::U32, 37, target);
            }
            let created = events
                .0
                .iter()
                .enumerate()
                .filter_map(|(ordinal, event)| match event {
                    SimulationEventKindV1::AllocationCreated {
                        allocation,
                        address_space: AddressSpace::Private,
                        bytes,
                    } => Some((ordinal, *allocation, *bytes)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(created.len(), 1);
            let (created_at, allocation, bytes) = created[0];
            assert_eq!(bytes, count as usize * 4);
            let released = events
                .0
                .iter()
                .enumerate()
                .filter_map(|(ordinal, event)| match event {
                    SimulationEventKindV1::AllocationReleased {
                        allocation: released,
                    } if *released == allocation => Some(ordinal),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(released.len(), 1);
            assert!(released[0] > created_at);
            if count != 0 {
                let positions = events
                    .0
                    .iter()
                    .enumerate()
                    .filter_map(|(ordinal, event)| match event {
                        SimulationEventKindV1::MemoryWrite {
                            allocation: observed,
                            ..
                        }
                        | SimulationEventKindV1::MemoryRead {
                            allocation: observed,
                            ..
                        } if *observed == allocation => Some(ordinal),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(positions.len(), 2);
                assert!(
                    created_at < positions[0]
                        && positions[0] < positions[1]
                        && positions[1] < released[0]
                );
            }
        }
    });
}

#[test]
fn private_storage_pointer_cannot_escape_through_a_helper_return() {
    let target = SimulationTargetV1::amdgpu_64();
    let mut raw = scalar_module(ScalarType::U32, target);
    raw.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind = OperationKind::Call {
        callee: fe2o3_kernel_ir::FunctionId::new("escape"),
        arguments: vec![ValueId(2)],
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), pointer(0, AccessMode::ReadWrite)),
        OperationKind::Alloca {
            element: Type::StorageObject(StorageLayoutIdV1(0)),
            count: Some(ValueId(0)),
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let mut helper = Function::definition(
        "escape",
        Signature::new(vec![Type::INDEX], vec![pointer(0, AccessMode::ReadWrite)]),
        vec![ValueId(0)],
        vec![block],
    );
    helper.required_capabilities = helper.derived_capabilities();
    raw.functions.push(helper);
    refresh(&mut raw);
    raw.required_capabilities = raw.derived_capabilities();
    raw.kernels[0].required_capabilities = raw.required_capabilities.clone();
    with_view(&raw, |admitted| {
        let error = admitted
            .preflight(
                &scalar_request(ScalarType::U32, 37, 1, target),
                target,
                SimulationLimitsV1::default(),
            )
            .unwrap_err();
        let SimulationPreflightErrorV1::Unsupported(report) = error else {
            panic!("expected storage signature refusal: {error:?}");
        };
        assert!(
            report
                .findings()
                .iter()
                .any(|finding| finding.function.as_str() == "escape"
                    && finding.block.is_none()
                    && finding.feature == UnsupportedFeatureV1::InertStorage)
        );
    });
}

#[test]
fn private_scalar_storage_preserves_runtime_alignment_and_refuses_volatile_access() {
    let target = SimulationTargetV1::amdgpu_64();
    let request = scalar_request(ScalarType::U32, 37, 1, target);
    for ordinal in [1, 2] {
        let mut raw = scalar_module(ScalarType::U32, target);
        match &mut raw.functions[0].body.as_mut().unwrap().blocks[0].operations[ordinal].kind {
            OperationKind::Storage(StorageOperationV1::ReadValue { access, .. })
            | OperationKind::Storage(StorageOperationV1::WriteValue { access, .. }) => {
                access.alignment = 8
            }
            _ => unreachable!(),
        }
        with_view(&raw, |admitted| {
            let before = request.clone();
            assert!(matches!(
                admitted.simulate(&request, target, SimulationLimitsV1::default()),
                Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::MisalignedAccess {
                        required: 8,
                        offset: 0
                    },
                    ..
                }))
            ));
            assert_eq!(request, before);
        });
    }
    let mut raw = scalar_module(ScalarType::U32, target);
    let OperationKind::Storage(StorageOperationV1::WriteValue { access, .. }) =
        &mut raw.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    access.volatile = true;
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        unsupported(
            admitted
                .preflight(&request, target, SimulationLimitsV1::default())
                .unwrap_err(),
            UnsupportedFeatureV1::InertStorage,
        );
    });
}

#[test]
fn select_preserves_independent_scalar_cells_and_never_initializes_the_other_cell() {
    let target = SimulationTargetV1::amdgpu_64();
    for (initialize_second, choose_first, expected) in [
        (false, true, Some(37)),
        (false, false, None),
        (true, true, Some(37)),
        (true, false, Some(91)),
    ] {
        let mut raw = scalar_module(ScalarType::U32, target);
        let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
        let mut tail = block.operations.split_off(2);
        let mut second = block.operations[0].clone();
        second.results[0].id = ValueId(5);
        block.operations.push(second);
        if initialize_second {
            block.operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
                    OperationKind::Constant(Constant::U32(91)),
                ),
                Operation::new(
                    vec![],
                    OperationKind::Storage(StorageOperationV1::WriteValue {
                        address: ValueId(5),
                        value: ValueId(8),
                        access: MemoryAccess::new(AddressSpace::Private, 4),
                    }),
                ),
            ]);
        }
        block.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(6), Type::BOOL),
                OperationKind::Constant(Constant::Bool(choose_first)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(7), pointer(0, AccessMode::ReadWrite)),
                OperationKind::Select {
                    condition: ValueId(6),
                    true_value: ValueId(3),
                    false_value: ValueId(5),
                },
            ),
        ]);
        let OperationKind::Storage(StorageOperationV1::ReadValue { address, .. }) =
            &mut tail[0].kind
        else {
            unreachable!()
        };
        *address = ValueId(7);
        block.operations.extend(tail);
        refresh(&mut raw);
        with_view(&raw, |admitted| {
            let request = scalar_request(ScalarType::U32, 37, 1, target);
            let before = request.clone();
            let result = admitted.simulate(&request, target, SimulationLimitsV1::default());
            match expected {
                Some(bits) => output(&result.unwrap(), ScalarType::U32, bits, target),
                None => assert!(matches!(
                    result,
                    Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                        kind: SimulationExecutionErrorKindV1::UninitializedRead {
                            offset: 0,
                            bytes: 4,
                            ..
                        },
                        ..
                    }))
                )),
            }
            assert_eq!(request, before);
        });
        // Identical scalar shapes still do not authorize a different row at Select.
        raw.storage_layouts.push(raw.storage_layouts[0].clone());
        let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
        block.operations[2].results[0].ty = pointer(1, AccessMode::ReadWrite);
        let OperationKind::Alloca { element, .. } = &mut block.operations[2].kind else {
            unreachable!()
        };
        *element = Type::StorageObject(StorageLayoutIdV1(1));
        let mut work = Work::new(BOUND);
        let mut budget = Budget::new(&mut work, BOUND);
        assert!(
            Owner::from_module_ref_with_verification_budget_v18(&raw, LAYOUT_LIMITS, &mut budget)
                .is_err()
        );
        assert_eq!(budget.storage(), 0);
    }
}
