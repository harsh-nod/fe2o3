use super::storage_tests_v1::*;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1, FunctionId,
    Signature, StorageLayoutLimitsV1, VerifiedCanonicalKernelIrModuleV18,
};

fn descriptor_rows() -> Vec<StorageLayoutV1> {
    vec![
        scalar_row(),
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: StorageLayoutIdV1(0),
                value_space: AddressSpace::Private,
                encoded_space: AddressSpace::Private,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Scalar(ScalarType::Index),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Slice {
                element: StorageLayoutIdV1(0),
                value_space: AddressSpace::Private,
                access: AccessMode::ReadWrite,
                data: StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(1),
                },
                length: StorageFieldV1 {
                    offset: 8,
                    layout: StorageLayoutIdV1(2),
                },
            },
        },
    ]
}
fn expired_descriptor(object_view: bool, length: u64, access_after: bool) -> Module {
    let element = if object_view {
        object(0)
    } else {
        Type::Scalar(ScalarType::U32)
    };
    let concrete = Type::slice(
        element.clone(),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let generic = Type::slice(
        element.clone(),
        AddressSpace::Generic,
        AccessMode::ReadWrite,
    );
    let mut graph = module(
        descriptor_rows(),
        vec![
            value(
                1,
                concrete.clone(),
                OperationKind::Call {
                    callee: FunctionId::new("expired"),
                    arguments: vec![],
                },
            ),
            value(
                2,
                generic.clone(),
                OperationKind::Cast {
                    kind: CastKind::SliceToGeneric,
                    value: ValueId(1),
                    to: generic,
                },
            ),
            value(
                3,
                Type::INDEX,
                OperationKind::SliceLength { slice: ValueId(2) },
            ),
        ],
    );
    if access_after {
        let operations = &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations;
        operations.push(value(
            4,
            Type::pointer(element, AddressSpace::Generic, AccessMode::ReadWrite),
            OperationKind::SliceData { slice: ValueId(2) },
        ));
        operations.push(if object_view {
            read(5, 4, AddressSpace::Generic)
        } else {
            value(
                5,
                Type::Scalar(ScalarType::U32),
                OperationKind::Load {
                    pointer: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Generic, 4),
                },
            )
        });
        operations.push(output(5));
    }
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    let mut helper = BasicBlock::new(BlockId(0));
    helper.operations = vec![
        allocate(0, 0, AddressSpace::Private),
        constant(1, 29),
        write(0, 1, AddressSpace::Private),
        value(
            2,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(length)),
        ),
        value(
            3,
            address(3, AddressSpace::Private),
            OperationKind::Alloca {
                element: object(3),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        project(
            4,
            1,
            3,
            StorageProjectionV1::Field(0),
            AddressSpace::Private,
        ),
        project(
            5,
            2,
            3,
            StorageProjectionV1::Field(1),
            AddressSpace::Private,
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(4),
                value: ValueId(0),
                access,
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(5),
                value: ValueId(2),
                access,
            }),
        ),
        value(
            6,
            concrete.clone(),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(3),
                access,
            }),
        ),
    ];
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(6)],
    });
    graph.functions.push(Function::internal_helper(
        "expired",
        Signature::new(vec![], vec![concrete]),
        vec![],
        vec![helper],
    ));
    graph
}

#[test]
fn expired_scalar_and_object_descriptors_widen_without_eager_access_or_lifetime_revival() {
    for object_view in [false, true] {
        // Storage descriptor construction already requires a nonempty view;
        // the public kernel-slice test covers zero-length cast preservation.
        for length in [1] {
            for access_after in [false, true] {
                let graph = expired_descriptor(object_view, length, access_after);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
                let (owner, _) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                    &graph, StorageLayoutLimitsV1 { rows: 64, edges: 256, containment_depth: 32, object_bytes: 4096 },
                    &mut budget).unwrap();
                let mut request = request();
                request.events = EventPolicyV1::Enabled;
                let mut events = Events::default();
                let result = crate::simulate_canonical_storage_with_sinks_v18(
                    &owner,
                    &request,
                    None,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                    SimulationDebugCaptureLimitsV1::disabled(),
                    &mut events,
                    &mut NoopSimulationDebugSinkV1,
                );
                if access_after {
                    let error = result.unwrap_err();
                    assert!(
                        format!("{error:?}").contains("DanglingPointer"),
                        "{error:?}"
                    );
                } else {
                    result.unwrap();
                }
                // The actual cast and SliceLength have completed before any
                // subsequent access can discover the expired private frame.
                for ordinal in [1, 2] {
                    assert!(events.0.iter().any(|event| event.site.function_ordinal == 0
                        && event.site.operation == Some(ordinal)
                        && matches!(
                            event.kind,
                            SimulationEventKindV1::OperationEnd {
                                outcome: SimulationExecutionOutcomeV1::Completed
                            }
                        )));
                }
            }
        }
    }
}

#[test]
fn whole_slice_exposure_keeps_variant_generation_and_exact_payload_offset() {
    for object_view in [false, true] {
        for stale in [false, true] {
            for access_after in [false, true] {
                let mut graph = super::storage_views_tests_v1::variant_module(false, false);
                assert_eq!(graph.storage_layouts.len(), 6);
                let descriptor = descriptor_rows();
                graph.storage_layouts.extend([
                    descriptor[1].clone(),
                    descriptor[2].clone(),
                    StorageLayoutV1 {
                        size: 16,
                        alignment: 8,
                        kind: StorageLayoutKindV1::Slice {
                            element: StorageLayoutIdV1(0),
                            value_space: AddressSpace::Private,
                            access: AccessMode::ReadWrite,
                            data: StorageFieldV1 {
                                offset: 0,
                                layout: StorageLayoutIdV1(6),
                            },
                            length: StorageFieldV1 {
                                offset: 8,
                                layout: StorageLayoutIdV1(7),
                            },
                        },
                    },
                ]);
                let element = if object_view {
                    object(0)
                } else {
                    Type::Scalar(ScalarType::U32)
                };
                let concrete = Type::slice(
                    element.clone(),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                );
                let generic = Type::slice(
                    element.clone(),
                    AddressSpace::Generic,
                    AccessMode::ReadWrite,
                );
                let access = MemoryAccess::new(AddressSpace::Private, 8);
                let operations =
                    &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations;
                operations.truncate(operations.len() - 2);
                operations.extend([
                    value(20, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
                    value(
                        21,
                        address(8, AddressSpace::Private),
                        OperationKind::Alloca {
                            element: object(8),
                            count: None,
                            address_space: AddressSpace::Private,
                            alignment: 8,
                        },
                    ),
                    project(
                        22,
                        6,
                        21,
                        StorageProjectionV1::Field(0),
                        AddressSpace::Private,
                    ),
                    project(
                        23,
                        7,
                        21,
                        StorageProjectionV1::Field(1),
                        AddressSpace::Private,
                    ),
                    Operation::new(
                        vec![],
                        OperationKind::Storage(StorageOperationV1::WriteValue {
                            address: ValueId(22),
                            value: ValueId(6),
                            access,
                        }),
                    ),
                    Operation::new(
                        vec![],
                        OperationKind::Storage(StorageOperationV1::WriteValue {
                            address: ValueId(23),
                            value: ValueId(20),
                            access,
                        }),
                    ),
                    value(
                        24,
                        concrete,
                        OperationKind::Storage(StorageOperationV1::ReadValue {
                            address: ValueId(21),
                            access,
                        }),
                    ),
                ]);
                if stale {
                    operations.push(write(3, 2, AddressSpace::Private));
                }
                let cast_ordinal = operations.len() as u32;
                operations.extend([
                    value(
                        25,
                        generic.clone(),
                        OperationKind::Cast {
                            kind: CastKind::SliceToGeneric,
                            value: ValueId(24),
                            to: generic,
                        },
                    ),
                    value(
                        26,
                        Type::INDEX,
                        OperationKind::SliceLength { slice: ValueId(25) },
                    ),
                ]);
                if access_after {
                    operations.push(value(
                        27,
                        Type::pointer(element, AddressSpace::Generic, AccessMode::ReadWrite),
                        OperationKind::SliceData { slice: ValueId(25) },
                    ));
                    operations.push(if object_view {
                        read(28, 27, AddressSpace::Generic)
                    } else {
                        value(
                            28,
                            Type::Scalar(ScalarType::U32),
                            OperationKind::Load {
                                pointer: ValueId(27),
                                access: MemoryAccess::new(AddressSpace::Generic, 4),
                            },
                        )
                    });
                    operations.push(output(28));
                }
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
                let (owner, _) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                    &graph, StorageLayoutLimitsV1 { rows: 64, edges: 256, containment_depth: 32, object_bytes: 4096 },
                    &mut budget).unwrap();
                let mut input = request();
                input.events = EventPolicyV1::Enabled;
                let mut events = Events::default();
                let result = crate::simulate_canonical_storage_with_sinks_v18(
                    &owner,
                    &input,
                    None,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                    SimulationDebugCaptureLimitsV1::disabled(),
                    &mut events,
                    &mut NoopSimulationDebugSinkV1,
                );
                assert_eq!(result.is_ok(), !(stale && access_after), "{result:?}");
                if access_after && !stale {
                    assert_eq!(
                        u32::from_le_bytes(
                            result
                                .unwrap()
                                .buffer(0)
                                .unwrap()
                                .bytes()
                                .try_into()
                                .unwrap()
                        ),
                        99
                    );
                }
                assert!(events.0.iter().any(|event| event.site.function_ordinal == 0
                    && event.site.operation == Some(cast_ordinal)
                    && matches!(
                        event.kind,
                        SimulationEventKindV1::OperationEnd {
                            outcome: SimulationExecutionOutcomeV1::Completed
                        }
                    )));
            }
        }
    }
}
