//! Synthetic V18 storage semantics, not source, reference or hardware qualification.
use super::*;
use fe2o3_kernel_ir::{StorageCopyOverlapV1, StorageProjectionV1};

fn pointer(row: u32) -> Type {
    Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(row)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    )
}

fn refresh(raw: &mut Module) {
    for function in &mut raw.functions {
        function.required_capabilities = function.derived_capabilities();
    }
    raw.required_capabilities = raw.derived_capabilities();
    raw.kernels[0].required_capabilities = raw.required_capabilities.clone();
}

fn nested(packed: bool, length: u64) -> Module {
    let offset = if packed { 1 } else { 4 };
    let alignment = if packed { 1 } else { 4 };
    let mut raw = Module::new("synthetic-private-record-array");
    raw.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: length * 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length,
                stride: 4,
            },
        },
        StorageLayoutV1 {
            size: length * 4 + 8,
            alignment,
            kind: StorageLayoutKindV1::Record(vec![field(offset, 1)].into_boxed_slice()),
        },
    ];
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(3), pointer(2)),
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(2)),
                count: None,
                address_space: AddressSpace::Private,
                alignment,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), pointer(1)),
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(3),
                step: StorageProjectionV1::Field(0),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), pointer(0)),
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(4),
                step: StorageProjectionV1::ArrayIndex(ValueId(2)),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(5),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, alignment),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Private, alignment),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    raw.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
                Type::Scalar(ScalarType::U32),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    raw.kernels.push(Kernel::new(
        "nested",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    refresh(&mut raw);
    raw
}

fn request(index: u64, target: SimulationTargetV1) -> SimulationRequestV1 {
    SimulationRequestV1::new(
        "nested",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    vec![0x5a; 8],
                    vec![false; 8],
                    target,
                )
                .unwrap(),
            ),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(0xfedc_1234)),
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(ScalarType::Index, index.into(), target).unwrap(),
            ),
        ],
    )
}

fn with_view(raw: &Module, inspect: impl FnOnce(&AdmittedSimulationModuleV1)) {
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let (canonical, owner_paid) = owner(raw, &mut budget);
    let (admitted, view_paid) = view(&canonical, &mut budget);
    assert_eq!(admitted.identity().digest(), canonical.identity().digest());
    inspect(&admitted);
    drop(admitted);
    budget.release_storage(view_paid).unwrap();
    drop(canonical);
    budget.release_storage(owner_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn output(execution: &SimulationExecutionV1) {
    let buffer = execution.buffer(0).unwrap();
    assert_eq!(&buffer.bytes()[..4], &0xfedc_1234_u32.to_le_bytes());
    assert_eq!(&buffer.bytes()[4..], &[0x5a; 4]);
    assert_eq!(
        buffer.initialized(),
        &[true, true, true, true, false, false, false, false]
    );
    assert!(!execution.grants_execution_authority());
}

fn kind(
    result: Result<SimulationExecutionV1, SimulationErrorV1>,
) -> SimulationExecutionErrorKindV1 {
    match result {
        Err(SimulationErrorV1::Execution(error)) => error.kind,
        other => panic!("expected execution refusal: {other:?}"),
    }
}

#[test]
fn record_array_scalar_leaves_preserve_stride_packed_alignment_and_replay() {
    for target in [
        SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
        SimulationTargetV1::amdgpu_64(),
    ] {
        for packed in [false, true] {
            with_view(&nested(packed, 3), |admitted| {
                for index in 0..3 {
                    let request = request(index, target);
                    let before = request.clone();
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
                    output(&execution);
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
                    output(&replay);
                    assert_eq!(
                        execution.schedule_transcript_identity(),
                        replay.schedule_transcript_identity()
                    );
                    assert_eq!(request, before);
                }
            });
        }
    }
}

#[test]
fn storage_array_bounds_use_declared_length_even_with_padding_or_zero_size() {
    let target = SimulationTargetV1::amdgpu_64();
    for length in [0, 3] {
        with_view(&nested(false, length), |admitted| {
            for index in [length, length + 1, u64::MAX] {
                assert_eq!(
                    kind(admitted.simulate(
                        &request(index, target),
                        target,
                        SimulationLimitsV1::default()
                    )),
                    SimulationExecutionErrorKindV1::StorageArrayIndexOutOfBounds { index, length }
                );
            }
        });
    }
    let mut raw = nested(false, 3);
    let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
    );
    let OperationKind::Alloca { count, .. } = &mut block.operations[1].kind else {
        unreachable!()
    };
    *count = Some(ValueId(7));
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        assert!(matches!(
            kind(admitted.simulate(&request(0, target), target, SimulationLimitsV1::default())),
            SimulationExecutionErrorKindV1::OutOfBounds {
                offset: 0,
                bytes: 20,
                allocation_bytes: 0,
                ..
            }
        ));
    });
}

#[test]
fn storage_projection_does_not_initialize_bytes_or_hide_alignment_failures() {
    let target = SimulationTargetV1::amdgpu_64();
    let mut raw = nested(false, 3);
    raw.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(3);
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        assert!(matches!(
            kind(admitted.simulate(&request(2, target), target, SimulationLimitsV1::default())),
            SimulationExecutionErrorKindV1::UninitializedRead {
                offset: 12,
                bytes: 4,
                ..
            }
        ));
    });
    let mut raw = nested(true, 3);
    let OperationKind::Storage(StorageOperationV1::WriteValue { access, .. }) =
        &mut raw.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    access.alignment = 4;
    with_view(&raw, |admitted| {
        assert_eq!(
            kind(admitted.simulate(&request(2, target), target, SimulationLimitsV1::default())),
            SimulationExecutionErrorKindV1::MisalignedAccess {
                required: 4,
                offset: 9
            }
        );
    });
}

#[test]
fn aggregate_storage_allocation_obeys_exact_existing_byte_and_count_limits() {
    let target = SimulationTargetV1::amdgpu_64();
    with_view(&nested(false, 3), |admitted| {
        let request = request(2, target);
        let limits = SimulationLimitsV1 {
            max_allocation_bytes: 20,
            max_total_bytes: 28,
            max_allocations: 2,
            ..SimulationLimitsV1::default()
        };
        output(&admitted.simulate(&request, target, limits).unwrap());
        for (limits, expected) in [
            (
                SimulationLimitsV1 {
                    max_allocation_bytes: 19,
                    ..limits
                },
                SimulationExecutionErrorKindV1::AllocationBytesLimit {
                    actual: 20,
                    limit: 19,
                },
            ),
            (
                SimulationLimitsV1 {
                    max_total_bytes: 27,
                    ..limits
                },
                SimulationExecutionErrorKindV1::TotalBytesLimit {
                    actual: 28,
                    limit: 27,
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
            assert_eq!(kind(admitted.simulate(&request, target, limits)), expected);
        }
    });
}

#[test]
fn helper_borrows_exact_private_subobject_without_escaping_its_frame() {
    let target = SimulationTargetV1::amdgpu_64();
    let mut raw = nested(false, 3);
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.operations = vec![
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    helper_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    raw.functions.push(Function::definition(
        "leaf",
        Signature::new(
            vec![pointer(0), Type::Scalar(ScalarType::U32)],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![helper_block],
    ));
    let block = &mut raw.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.splice(
        3..5,
        [Operation::effect_free(
            ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
            OperationKind::Call {
                callee: fe2o3_kernel_ir::FunctionId::new("leaf"),
                arguments: vec![ValueId(5), ValueId(1)],
            },
        )],
    );
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        output(
            &admitted
                .simulate(&request(2, target), target, SimulationLimitsV1::default())
                .unwrap(),
        )
    });
}

#[test]
fn storage_copy_and_volatile_leaf_access_remain_explicitly_unsupported() {
    let target = SimulationTargetV1::amdgpu_64();
    for copy in [false, true] {
        let mut raw = nested(false, 3);
        if copy {
            raw.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .insert(
                    1,
                    Operation::new(
                        vec![],
                        OperationKind::Storage(StorageOperationV1::CopyObject {
                            source: ValueId(3),
                            destination: ValueId(3),
                            source_access: MemoryAccess::new(AddressSpace::Private, 4),
                            destination_access: MemoryAccess::new(AddressSpace::Private, 4),
                            overlap: StorageCopyOverlapV1::MayOverlap,
                        }),
                    ),
                );
        } else {
            let OperationKind::Storage(StorageOperationV1::WriteValue { access, .. }) =
                &mut raw.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
            else {
                unreachable!()
            };
            access.volatile = true;
        }
        refresh(&mut raw);
        with_view(&raw, |admitted| {
            let error = admitted
                .preflight(&request(0, target), target, SimulationLimitsV1::default())
                .unwrap_err();
            let SimulationPreflightErrorV1::Unsupported(report) = error else {
                panic!("{error:?}")
            };
            assert!(
                report
                    .findings()
                    .iter()
                    .any(|finding| finding.feature == UnsupportedFeatureV1::InertStorage)
            );
        });
    }
}

#[test]
fn storage_projection_cannot_substitute_equal_shape_layout_rows() {
    let mut raw = nested(false, 3);
    raw.storage_layouts.push(raw.storage_layouts[0].clone());
    raw.functions[0].body.as_mut().unwrap().blocks[0].operations[2].results[0].ty = pointer(3);
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    assert!(
        Owner::from_module_ref_with_verification_budget_v18(&raw, LAYOUT_LIMITS, &mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), 0);
}

#[derive(Default)]
struct Records(Vec<SimulationDebugRecordV1>);
impl SimulationDebugSinkV1 for Records {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        assert!(self.0.len() < 128);
        self.0.push(record);
        SimulationDebugSinkControlV1::Continue
    }
}

#[test]
fn projected_write_preserves_all_other_private_bytes_and_initialization_bits() {
    let target = SimulationTargetV1::amdgpu_64();
    with_view(&nested(false, 3), |admitted| {
        let mut records = Records::default();
        let execution = admitted
            .simulate_debugged_with_sink(
                &request(1, target),
                target,
                SimulationLimitsV1::default(),
                SimulationDebugCaptureLimitsV1::new(8, 64, 8, 128).unwrap(),
                &mut records,
            )
            .unwrap();
        output(&execution);
        let mut bytes = vec![0; 20];
        let mut initialized = vec![false; 20];
        bytes[8..12].copy_from_slice(&0xfedc_1234_u32.to_le_bytes());
        initialized[8..12].fill(true);
        assert!(records.0.iter().any(|record| matches!(&record.kind,
            SimulationDebugRecordKindV1::Checkpoint {
                memory: SimulationDebugCollectionV1::Captured(allocations), ..
            } if allocations.iter().any(|allocation|
                allocation.address_space == AddressSpace::Private
                && allocation.bytes == bytes && allocation.initialized == initialized
            )
        )));
    });
}

#[test]
fn zero_sized_record_elements_form_views_but_still_enforce_array_length() {
    let target = SimulationTargetV1::amdgpu_64();
    let mut raw = nested(true, 3);
    raw.storage_layouts[0] = StorageLayoutV1 {
        size: 0,
        alignment: 1,
        kind: StorageLayoutKindV1::Record(Box::new([])),
    };
    raw.storage_layouts[1] = StorageLayoutV1 {
        size: 0,
        alignment: 1,
        kind: StorageLayoutKindV1::Array {
            element: StorageLayoutIdV1(0),
            length: 3,
            stride: 0,
        },
    };
    raw.storage_layouts[2].size = 8;
    raw.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .truncate(3);
    refresh(&mut raw);
    with_view(&raw, |admitted| {
        for index in [0, 2] {
            let executed = admitted
                .simulate(
                    &request(index, target),
                    target,
                    SimulationLimitsV1::default(),
                )
                .unwrap();
            assert_eq!(executed.buffer(0).unwrap().bytes(), &[0x5a; 8]);
            assert_eq!(executed.buffer(0).unwrap().initialized(), &[false; 8]);
        }
        assert_eq!(
            kind(admitted.simulate(&request(3, target), target, SimulationLimitsV1::default())),
            SimulationExecutionErrorKindV1::StorageArrayIndexOutOfBounds {
                index: 3,
                length: 3
            }
        );
    });
}
