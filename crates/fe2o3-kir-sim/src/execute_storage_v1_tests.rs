use super::*;
pub(super) use fe2o3_kernel_ir::{
    StorageCopyOverlapV1, StorageFieldV1, StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1,
    StorageOperationV1, StoragePointerV1, StorageProjectionV1,
};

#[derive(Default)]
pub(super) struct Events(pub(super) Vec<SimulationEventV1>);

impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        self.0.push(event.clone());
        Ok(())
    }
}

pub(super) fn scalar_row() -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    }
}

pub(super) fn rows() -> Vec<StorageLayoutV1> {
    vec![
        scalar_row(),
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                    StorageFieldV1 {
                        offset: 4,
                        layout: StorageLayoutIdV1(0),
                    },
                ]
                .into_boxed_slice(),
            ),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: 2,
                stride: 4,
            },
        },
    ]
}

pub(super) fn object(row: u32) -> Type {
    Type::StorageObject(StorageLayoutIdV1(row))
}
pub(super) fn address(row: u32, space: AddressSpace) -> Type {
    Type::pointer(object(row), space, AccessMode::ReadWrite)
}
pub(super) fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}
pub(super) fn constant(id: u32, bits: u32) -> Operation {
    value(
        id,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(bits)),
    )
}
pub(super) fn allocate(id: u32, row: u32, space: AddressSpace) -> Operation {
    let kind = if space == AddressSpace::Workgroup {
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: object(row),
            extent: WorkgroupMemoryExtent::Static(1),
            alignment: 4,
        })
    } else {
        OperationKind::Alloca {
            element: object(row),
            count: None,
            address_space: space,
            alignment: 4,
        }
    };
    value(id, address(row, space), kind)
}
pub(super) fn project(
    id: u32,
    row: u32,
    base: u32,
    step: StorageProjectionV1,
    space: AddressSpace,
) -> Operation {
    value(
        id,
        address(row, space),
        OperationKind::Storage(StorageOperationV1::Project {
            base: ValueId(base),
            step,
        }),
    )
}
pub(super) fn write(address: u32, input: u32, space: AddressSpace) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Storage(StorageOperationV1::WriteValue {
            address: ValueId(address),
            value: ValueId(input),
            access: MemoryAccess::new(space, 4),
        }),
    )
}
pub(super) fn read(id: u32, address: u32, space: AddressSpace) -> Operation {
    value(
        id,
        Type::Scalar(ScalarType::U32),
        OperationKind::Storage(StorageOperationV1::ReadValue {
            address: ValueId(address),
            access: MemoryAccess::new(space, 4),
        }),
    )
}
pub(super) fn output(input: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(input),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}
pub(super) fn module(rows: Vec<StorageLayoutV1>, operations: Vec<Operation>) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("storage-component");
    module.storage_layouts = rows;
    module.functions.push(Function::kernel_entry(
        "entry",
        fe2o3_kernel_ir::Signature::new(
            vec![Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            )],
            vec![],
        ),
        vec![ValueId(0)],
        vec![block],
    ));
    let mut kernel = fe2o3_kernel_ir::Kernel::new(
        "entry",
        "entry",
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(1, 1, 1));
    module.kernels.push(kernel);
    module
}
pub(super) fn verified(module: &Module) -> fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_> {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let checked = fe2o3_kernel_ir::check_module_storage_v1(
        module,
        fe2o3_kernel_ir::StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .expect("real module-owned structural table");
    fe2o3_kernel_ir::verify_storage_module_ref_with_budget_v1(checked, None, &mut budget)
        .expect("real whole-module storage verifier")
}
pub(super) fn request() -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0; 4],
        vec![true; 4],
        target,
    )
    .unwrap();
    let mut request = SimulationRequestV1::new(
        "entry",
        [1, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(buffer)],
    );
    request.events = EventPolicyV1::Enabled;
    request
}
pub(super) fn run(
    module: &Module,
    events: &mut Events,
) -> Result<StorageExecutionCompletionV1, SimulationErrorV1> {
    simulate_storage_view_v1(
        verified(module),
        &request(),
        None,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        events,
    )
}
pub(super) fn output_bits(result: &StorageExecutionCompletionV1) -> u32 {
    let SimulationArgumentV1::Buffer(buffer) = &result.arguments[0] else {
        panic!("output buffer")
    };
    assert_eq!(buffer.initialized(), &[true; 4]);
    u32::from_le_bytes(buffer.bytes().try_into().unwrap())
}
pub(super) fn simple(space: AddressSpace) -> Module {
    module(
        rows(),
        vec![
            allocate(1, 1, space),
            constant(2, 0x1234_5678),
            project(3, 0, 1, StorageProjectionV1::Field(1), space),
            write(3, 2, space),
            read(4, 3, space),
            output(4),
        ],
    )
}

#[test]
fn storage_actual_engine_private_and_workgroup_use_one_backing_and_real_output() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        let module = simple(space);
        assert!(fe2o3_kernel_ir::verify_module_ref(&module).is_err());
        let mut events = Events::default();
        let result = run(&module, &mut events).unwrap();
        assert_eq!(output_bits(&result), 0x1234_5678);
        assert_eq!(result.invocations_executed, 1);
        assert_eq!(result.schedule_transcript_identity, None);
        let allocations: Vec<_> = events
            .0
            .iter()
            .filter_map(|event| match event.kind {
                SimulationEventKindV1::AllocationCreated {
                    allocation,
                    address_space,
                    bytes,
                } if address_space == space => Some((allocation, bytes)),
                _ => None,
            })
            .collect();
        assert_eq!(allocations.len(), 1);
        assert_eq!(allocations[0].1, 8);
        let writes: Vec<_> = events
            .0
            .iter()
            .filter_map(|event| match event.kind {
                SimulationEventKindV1::MemoryWrite {
                    allocation,
                    offset,
                    bytes,
                } if allocation == allocations[0].0 => Some((offset, bytes)),
                _ => None,
            })
            .collect();
        assert_eq!(writes, [(4, 4)]);
        assert!(events.0.iter().any(|event| matches!(event.kind,
            SimulationEventKindV1::MemoryRead { allocation, offset: 4, bytes: 4 }
                if allocation == allocations[0].0)));
    }
}

#[test]
fn storage_private_entry_does_not_relax_old_preflight_or_module_association() {
    let module = simple(AddressSpace::Private);
    let verified = verified(&module);
    assert!(std::ptr::eq(verified.module(), &module));
    assert!(matches!(
        crate::preflight::preflight(
            &module,
            0,
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default()
        ),
        Err(SimulationPreflightErrorV1::StorageProfileNotAdmitted)
    ));
    let mut foreign = simple(AddressSpace::Private);
    foreign.storage_layouts.clear();
    assert!(std::ptr::eq(verified.storage().module(), &module));
    assert!(fe2o3_kernel_ir::verify_module_ref(&foreign).is_err());
    assert!(std::ptr::eq(
        verified.storage().layouts().rows(),
        module.storage_layouts.as_slice()
    ));
}

#[test]
fn storage_actual_engine_refuses_uninitialized_before_memory_read_event() {
    let space = AddressSpace::Private;
    let module = module(
        rows(),
        vec![allocate(1, 0, space), read(2, 1, space), output(2)],
    );
    let mut events = Events::default();
    assert!(matches!(
        run(&module, &mut events),
        Err(SimulationErrorV1::Execution(_))
    ));
    assert!(
        !events
            .0
            .iter()
            .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
    );
}

#[test]
fn storage_observer_failure_stops_before_write_commit_and_output() {
    #[derive(Default)]
    struct RejectWrite {
        rejected: usize,
        retained: Vec<SimulationEventV1>,
    }
    impl SimulationEventSinkV1 for RejectWrite {
        fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
            if matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }) {
                self.rejected += 1;
                return Err(SimulationEventSinkErrorV1 {
                    detail: "storage write observer refusal".into(),
                });
            }
            self.retained.push(event.clone());
            Ok(())
        }
    }
    let module = simple(AddressSpace::Private);
    let mut sink = RejectWrite::default();
    assert!(
        simulate_storage_view_v1(
            verified(&module),
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut sink
        )
        .is_err()
    );
    assert_eq!(sink.rejected, 1);
    assert!(!sink.retained.iter().any(|e| matches!(
        e.kind,
        SimulationEventKindV1::MemoryWrite { .. } | SimulationEventKindV1::MemoryRead { .. }
    )));
}

#[test]
fn storage_dynamic_workgroup_extent_uses_actual_layout_and_one_backing() {
    let mut module = simple(AddressSpace::Workgroup);
    let OperationKind::WorkgroupMemory(memory) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    memory.extent = WorkgroupMemoryExtent::Dynamic;
    for bytes in [8, 16] {
        let dynamic = DynamicWorkgroupMemoryRequestV1::new(bytes);
        let mut events = Events::default();
        let result = simulate_storage_view_v1(
            verified(&module),
            &request(),
            Some(dynamic),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
        assert_eq!(output_bits(&result), 0x1234_5678);
        assert_eq!(result.dynamic_workgroup_memory, Some(dynamic));
        let actual: Vec<_> = events
            .0
            .iter()
            .filter_map(|event| match event.kind {
                SimulationEventKindV1::AllocationCreated {
                    address_space: AddressSpace::Workgroup,
                    bytes,
                    ..
                } => Some(bytes),
                _ => None,
            })
            .collect();
        assert_eq!(actual, [bytes as usize]);
    }
    let mut events = Events::default();
    assert!(
        simulate_storage_view_v1(
            verified(&module),
            &request(),
            Some(DynamicWorkgroupMemoryRequestV1::new(12)),
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events
        )
        .is_err()
    );
    assert!(
        !events
            .0
            .iter()
            .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. }))
    );
}

#[test]
fn storage_actual_call_uses_parent_allocation_and_real_writeback() {
    let space = AddressSpace::Private;
    let mut module = module(
        rows(),
        vec![
            allocate(1, 0, space),
            value(
                2,
                Type::Scalar(ScalarType::U32),
                OperationKind::Call {
                    callee: "write_helper".into(),
                    arguments: vec![ValueId(1)],
                },
            ),
            read(3, 1, space),
            output(3),
        ],
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![constant(1, 41), write(0, 1, space), read(2, 0, space)];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    module.functions.push(Function::definition(
        "write_helper",
        fe2o3_kernel_ir::Signature::new(
            vec![address(0, space)],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0)],
        vec![block],
    ));
    let mut events = Events::default();
    assert_eq!(output_bits(&run(&module, &mut events).unwrap()), 41);
    assert_eq!(
        events
            .0
            .iter()
            .filter(|event| matches!(
                event.kind,
                SimulationEventKindV1::AllocationCreated {
                    address_space: AddressSpace::Private,
                    ..
                }
            ))
            .count(),
        1
    );
    assert_eq!(
        events
            .0
            .iter()
            .filter(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
            .count(),
        2
    );
    assert!(
        events
            .0
            .iter()
            .any(|event| matches!(event.kind, SimulationEventKindV1::Call { .. }))
    );
}

#[test]
fn storage_actual_returned_private_local_is_dead_before_caller_read() {
    let space = AddressSpace::Private;
    let mut module = module(
        rows(),
        vec![
            value(
                1,
                address(0, space),
                OperationKind::Call {
                    callee: "local_helper".into(),
                    arguments: vec![],
                },
            ),
            read(2, 1, space),
            output(2),
        ],
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![allocate(0, 0, space), constant(1, 7), write(0, 1, space)];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    module.functions.push(Function::definition(
        "local_helper",
        fe2o3_kernel_ir::Signature::new(vec![], vec![address(0, space)]),
        vec![],
        vec![block],
    ));
    let mut events = Events::default();
    assert!(matches!(
        run(&module, &mut events),
        Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::DanglingPointer { .. },
            ..
        }))
    ));
    let released = events
        .0
        .iter()
        .position(|event| matches!(event.kind, SimulationEventKindV1::AllocationReleased { .. }))
        .expect("actual helper frame release");
    let returned = events
        .0
        .iter()
        .position(|event| matches!(event.kind, SimulationEventKindV1::Return))
        .expect("actual helper return");
    assert!(released < returned);
    assert!(
        !events
            .0
            .iter()
            .any(|event| matches!(event.kind, SimulationEventKindV1::MemoryRead { .. }))
    );
}

fn storage_restricted_pointer_fixture(write_through_readonly: bool) -> Module {
    let space = AddressSpace::Private;
    let readonly = Type::pointer(object(0), space, AccessMode::ReadOnly);
    let mut operations = vec![
        allocate(1, 0, space),
        constant(2, 0x1234_5678),
        write(1, 2, space),
        value(
            3,
            readonly.clone(),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(1),
                to: readonly,
            },
        ),
    ];
    if write_through_readonly {
        operations.push(write(3, 2, space));
    }
    operations.extend([read(4, 3, space), output(4)]);
    module(rows(), operations)
}

#[test]
fn storage_access_restriction_preserves_allocation_and_allows_read() {
    let module = storage_restricted_pointer_fixture(false);
    assert!(fe2o3_kernel_ir::verify_module_ref(&module).is_err());
    let mut events = Events::default();
    // run() derives the genuine module-bound storage verifier view and enters
    // the same interpreter used by ordinary execution.
    let result = run(&module, &mut events).expect("restricted storage pointer remains readable");
    assert_eq!(output_bits(&result), 0x1234_5678);
    assert_eq!(result.invocations_executed, 1);
    assert_eq!(result.schedule_transcript_identity, None);
    let allocations: Vec<_> = events
        .0
        .iter()
        .filter_map(|event| match event.kind {
            SimulationEventKindV1::AllocationCreated {
                allocation,
                address_space: AddressSpace::Private,
                bytes,
            } => Some((allocation, bytes)),
            _ => None,
        })
        .collect();
    assert_eq!(allocations.len(), 1);
    let (allocation, bytes) = allocations[0];
    assert_eq!(bytes, 4);
    let accesses: Vec<_> = events
        .0
        .iter()
        .filter_map(|event| match event.kind {
            SimulationEventKindV1::MemoryWrite {
                allocation: found,
                offset,
                bytes,
            } if found == allocation => Some(("write", offset, bytes)),
            SimulationEventKindV1::MemoryRead {
                allocation: found,
                offset,
                bytes,
            } if found == allocation => Some(("read", offset, bytes)),
            _ => None,
        })
        .collect();
    assert_eq!(accesses, [("write", 0, 4), ("read", 0, 4)]);
}

#[test]
fn storage_access_restriction_rejects_write_through_readonly_before_execution() {
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1, CanonicalKernelIrVerificationResourceBudgetV1,
        CanonicalKernelIrWorkBudgetV1, DiagnosticCode, StorageLayoutLimitsV1,
    };
    let module = storage_restricted_pointer_fixture(true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    let checked = fe2o3_kernel_ir::check_module_storage_v1(
        &module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .expect("layout remains structurally valid");
    let result =
        fe2o3_kernel_ir::verify_storage_module_ref_with_budget_v1(checked, None, &mut budget);
    assert_eq!(budget.storage(), 19);
    match result.expect_err("read-only storage pointer cannot authorize WriteValue") {
        BorrowedKernelIrVerificationErrorV1::Verification(errors) => {
            assert!(
                errors.contains(DiagnosticCode::InvalidMemoryAccess),
                "{errors:?}"
            );
        }
        BorrowedKernelIrVerificationErrorV1::Resource(error) => {
            panic!("unexpected resource refusal: {error}");
        }
    }
}
