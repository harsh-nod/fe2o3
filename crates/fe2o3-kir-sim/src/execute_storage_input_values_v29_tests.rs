use super::storage_inputs_tests_v29::*;
use super::*;

#[test]
fn inline_record_copy_reads_exact_fields_and_preserves_uninitialized_padding() {
    let rows = vec![
        scalar(),
        StorageLayoutV1 {
            size: 12,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                    StorageFieldV1 {
                        offset: 8,
                        layout: StorageLayoutIdV1(0),
                    },
                ]
                .into_boxed_slice(),
            ),
        },
    ];
    let operations = vec![
        value(
            2,
            object_pointer(1, AddressSpace::Private, AccessMode::ReadWrite),
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(1)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::CopyObject {
                source: ValueId(1),
                destination: ValueId(2),
                source_access: MemoryAccess::new(AddressSpace::Constant, 4),
                destination_access: MemoryAccess::new(AddressSpace::Private, 4),
                overlap: StorageCopyOverlapV1::NonOverlapping,
            }),
        ),
        project(
            3,
            2,
            0,
            StorageProjectionV1::Field(1),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        ),
        read(
            4,
            3,
            Type::Scalar(ScalarType::U32),
            AddressSpace::Private,
            4,
        ),
        output(4),
    ];
    let owner = admit(&graph(
        rows,
        vec![output_type(), inline_type(1)],
        operations,
    ));
    for target in targets() {
        let mut bytes = vec![0x5a; 12];
        bytes[8..12].copy_from_slice(&0x12345678u32.to_le_bytes());
        let mut mask = vec![true; 12];
        mask[4..8].fill(false);
        let request = request(vec![
            SimulationStorageArgumentV29::Existing(SimulationArgumentV1::Buffer(buffer(0, target))),
            SimulationStorageArgumentV29::InlineObject(image(
                1,
                4,
                bytes.clone(),
                mask.clone(),
                vec![],
            )),
        ]);
        let original = request.clone();
        let result =
            simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()).unwrap();
        assert_eq!(output_bits(&result), 0x12345678);
        let SimulationStorageArgumentObservationV29::InlineObject(observed) =
            &result.arguments()[1]
        else {
            panic!("inline observation")
        };
        assert_eq!(observed.bytes, bytes);
        assert_eq!(observed.initialized, mask);
        assert!(observed.relocations.is_empty());
        assert_eq!(request, original);
    }
}

#[test]
fn actual_array_index_and_overlapping_scalar_object_views_use_one_allocation() {
    let rows = vec![
        scalar(),
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: 2,
                stride: 4,
            },
        },
    ];
    let owner = admit(&graph(
        rows,
        vec![
            output_type(),
            object_pointer(1, AddressSpace::Global, AccessMode::ReadWrite),
            object_pointer(0, AddressSpace::Global, AccessMode::ReadWrite),
        ],
        vec![
            constant(3, 0x3210),
            Operation::new(
                vec![],
                OperationKind::Storage(StorageOperationV1::WriteValue {
                    address: ValueId(2),
                    value: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                }),
            ),
            value(
                4,
                Type::Scalar(ScalarType::Index),
                OperationKind::Constant(Constant::Index(1)),
            ),
            project(
                5,
                1,
                0,
                StorageProjectionV1::ArrayIndex(ValueId(4)),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            read(6, 5, Type::Scalar(ScalarType::U32), AddressSpace::Global, 4),
            output(6),
        ],
    ));
    for target in targets() {
        let mut request = request(vec![
            SimulationStorageArgumentV29::Existing(SimulationArgumentV1::Buffer(buffer(0, target))),
            SimulationStorageArgumentV29::ObjectView(view(42, 1, vec![])),
            SimulationStorageArgumentV29::ObjectView(view(
                42,
                0,
                vec![SimulationObjectComponentV29::Index(1)],
            )),
        ]);
        request.shared_storage.push(SimulationSharedStorageV29 {
            id: BufferBackingIdV1(42),
            storage: SimulationStorageBackingV29::Object {
                image: image(1, 4, vec![0; 8], vec![true; 8], vec![]),
                access: AccessMode::ReadWrite,
            },
        });
        let result =
            simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()).unwrap();
        assert_eq!(output_bits(&result), 0x3210);
        let SimulationStorageBackingObservationV29::Object { image, .. } =
            &result.shared_storage()[0].storage
        else {
            panic!("object backing")
        };
        assert_eq!(&image.bytes[4..], &0x3210u32.to_le_bytes());
        let origins = result.arguments()[1..]
            .iter()
            .map(|argument| match argument {
                SimulationStorageArgumentObservationV29::ObjectView { pointer, .. } => {
                    pointer.origin
                }
                _ => panic!("object views"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            origins,
            [SimulationInputOriginV29::Backing(BufferBackingIdV1(42)); 2]
        );
    }
}

#[test]
fn initialized_component_reads_do_not_initialize_missing_siblings() {
    let rows = vec![
        scalar(),
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
    ];
    for field in [0, 1] {
        let owner = admit(&graph(
            rows.clone(),
            vec![output_type(), inline_type(1)],
            vec![
                project(
                    2,
                    1,
                    0,
                    StorageProjectionV1::Field(field),
                    AddressSpace::Constant,
                    AccessMode::ReadOnly,
                ),
                read(
                    3,
                    2,
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Constant,
                    4,
                ),
                output(3),
            ],
        ));
        let mut mask = vec![true; 8];
        mask[4..].fill(false);
        let request = request(vec![
            SimulationStorageArgumentV29::Existing(SimulationArgumentV1::Buffer(buffer(
                0,
                targets()[0],
            ))),
            SimulationStorageArgumentV29::InlineObject(image(1, 4, vec![7; 8], mask, vec![])),
        ]);
        let result =
            simulate_canonical_storage_inputs_v29(&owner, &request, targets()[0], limits());
        if field == 0 {
            assert_eq!(output_bits(&result.unwrap()), 0x07070707);
        } else {
            assert!(matches!(
                result,
                Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::UninitializedRead { .. },
                    ..
                }))
            ));
        }
    }
}

#[test]
fn helper_transport_reads_the_same_typed_inline_region() {
    let pointer = inline_type(0);
    let mut graph = graph(
        vec![scalar()],
        vec![output_type(), pointer.clone()],
        vec![
            value(
                2,
                Type::Scalar(ScalarType::U32),
                OperationKind::Call {
                    callee: "helper".into(),
                    arguments: vec![ValueId(1)],
                },
            ),
            output(2),
        ],
    );
    let mut body = BasicBlock::new(BlockId(0));
    body.operations.push(read(
        1,
        0,
        Type::Scalar(ScalarType::U32),
        AddressSpace::Constant,
        4,
    ));
    body.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    graph.functions.push(Function::internal_helper(
        "helper",
        fe2o3_kernel_ir::Signature::new(vec![pointer], vec![Type::Scalar(ScalarType::U32)]),
        vec![ValueId(0)],
        vec![body],
    ));
    let owner = admit(&graph);
    for target in targets() {
        let request = request(vec![
            SimulationStorageArgumentV29::Existing(SimulationArgumentV1::Buffer(buffer(0, target))),
            SimulationStorageArgumentV29::InlineObject(image(
                0,
                4,
                91u32.to_le_bytes().to_vec(),
                vec![true; 4],
                vec![],
            )),
        ]);
        assert_eq!(
            output_bits(
                &simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()).unwrap()
            ),
            91
        );
    }
}

#[test]
fn empty_and_one_past_object_slices_are_values_not_nonzero_access_permissions() {
    let rows = vec![
        scalar(),
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: 2,
                stride: 4,
            },
        },
    ];
    for target in targets() {
        for start in [0, 2] {
            for access in [false, true] {
                let ty = if access {
                    object_pointer(0, AddressSpace::Global, AccessMode::ReadWrite)
                } else {
                    Type::slice(
                        Type::StorageObject(StorageLayoutIdV1(0)),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    )
                };
                let owner = admit(&graph(
                    rows.clone(),
                    vec![ty],
                    if access {
                        vec![read(
                            1,
                            0,
                            Type::Scalar(ScalarType::U32),
                            AddressSpace::Global,
                            4,
                        )]
                    } else {
                        vec![]
                    },
                ));
                let mut object_view = view(1, 0, vec![]);
                object_view.range = Some(SimulationObjectRangeV29 { start, elements: 0 });
                let mut request =
                    request(vec![SimulationStorageArgumentV29::ObjectView(object_view)]);
                request.shared_storage.push(SimulationSharedStorageV29 {
                    id: BufferBackingIdV1(1),
                    storage: SimulationStorageBackingV29::Object {
                        image: image(1, 4, vec![0; 8], vec![true; 8], vec![]),
                        access: AccessMode::ReadWrite,
                    },
                });
                let result =
                    simulate_canonical_storage_inputs_v29(&owner, &request, target, limits());
                if access {
                    assert!(matches!(result, Err(SimulationErrorV1::Execution(_))));
                } else {
                    let result = result.unwrap();
                    let SimulationStorageArgumentObservationV29::ObjectView { pointer, elements } =
                        &result.arguments()[0]
                    else {
                        panic!("empty view")
                    };
                    assert_eq!(*elements, Some(0));
                    assert_eq!(pointer.byte_offset, start as usize * 4);
                    assert_eq!(pointer.lower_bound, pointer.upper_bound);
                }
            }
        }
    }
}

#[test]
fn packed_field_pointer_geometry_keeps_operation_specific_alignment_checks() {
    let rows = vec![
        scalar(),
        StorageLayoutV1 {
            size: 5,
            alignment: 1,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 1,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            ),
        },
    ];
    for alignment in [1, 4] {
        let owner = admit(&graph(
            rows.clone(),
            vec![
                output_type(),
                object_pointer(0, AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![
                read(
                    2,
                    1,
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    alignment,
                ),
                output(2),
            ],
        ));
        for target in targets() {
            let mut bytes = vec![0; 5];
            bytes[1..].copy_from_slice(&19u32.to_le_bytes());
            let mut request = request(vec![
                SimulationStorageArgumentV29::Existing(SimulationArgumentV1::Buffer(buffer(
                    0, target,
                ))),
                SimulationStorageArgumentV29::ObjectView(view(
                    1,
                    0,
                    vec![SimulationObjectComponentV29::Field(0)],
                )),
            ]);
            request.shared_storage.push(SimulationSharedStorageV29 {
                id: BufferBackingIdV1(1),
                storage: SimulationStorageBackingV29::Object {
                    image: image(1, 1, bytes, vec![true; 5], vec![]),
                    access: AccessMode::ReadWrite,
                },
            });
            let result = simulate_canonical_storage_inputs_v29(&owner, &request, target, limits());
            if alignment == 1 {
                assert_eq!(output_bits(&result.unwrap()), 19);
            } else {
                assert!(matches!(
                    result,
                    Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                        kind: SimulationExecutionErrorKindV1::MisalignedAccess {
                            required: 4,
                            offset: 1
                        },
                        ..
                    }))
                ));
            }
        }
    }
}
