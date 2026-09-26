use super::storage_inputs_tests_v29::*;
use super::*;

#[test]
fn actual_pointer_read_writes_the_registered_scalar_backing() {
    let owner = admit(&graph(
        vec![scalar(), pointer(0)],
        vec![inline_type(1)],
        vec![
            read(
                1,
                0,
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
                AddressSpace::Constant,
                8,
            ),
            constant(2, 0xcafe1234),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(1),
                    value: ValueId(2),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ],
    ));
    for target in targets() {
        let mut request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
            1,
            8,
            vec![0; 8],
            vec![true; 8],
            vec![relocation(1, vec![], view(7, 0, vec![]))],
        ))]);
        request.shared_storage.push(SimulationSharedStorageV29 {
            id: BufferBackingIdV1(7),
            storage: SimulationStorageBackingV29::Scalar(buffer(0, target)),
        });
        let original = request.clone();
        let result =
            simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()).unwrap();
        let SimulationStorageBackingObservationV29::Scalar(buffer) =
            &result.shared_storage()[0].storage
        else {
            panic!("scalar backing")
        };
        assert_eq!(buffer.bytes(), &0xcafe1234u32.to_le_bytes());
        let SimulationStorageArgumentObservationV29::InlineObject(image) = &result.arguments()[0]
        else {
            panic!("pointer observation")
        };
        assert_eq!(image.relocations.len(), 1);
        assert_eq!(
            image.relocations[0].pointer.origin,
            SimulationInputOriginV29::Backing(BufferBackingIdV1(7))
        );
        assert_eq!(request, original);
    }
}

#[test]
fn forward_self_and_cyclic_object_relocations_do_not_unfold_the_referent_graph() {
    let rows = vec![
        scalar(),
        pointer(2),
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(1),
                }]
                .into_boxed_slice(),
            ),
        },
    ];
    let owner = admit(&graph(rows, vec![], vec![]));
    for target in targets() {
        for cycle in [false, true] {
            let mut request = request(vec![]);
            for id in [17, 3] {
                let to = if cycle {
                    if id == 17 { 3 } else { 17 }
                } else {
                    id
                };
                request.shared_storage.push(SimulationSharedStorageV29 {
                    id: BufferBackingIdV1(id),
                    storage: SimulationStorageBackingV29::Object {
                        image: image(
                            2,
                            8,
                            vec![0; 8],
                            vec![true; 8],
                            vec![relocation(
                                1,
                                vec![SimulationObjectComponentV29::Field(0)],
                                view(to, 2, vec![]),
                            )],
                        ),
                        access: AccessMode::ReadWrite,
                    },
                });
            }
            let result =
                simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()).unwrap();
            assert_eq!(result.shared_storage().len(), 2);
            for (index, shared) in result.shared_storage().iter().enumerate() {
                assert_eq!(shared.id, request.shared_storage[index].id);
                let SimulationStorageBackingObservationV29::Object { image, .. } = &shared.storage
                else {
                    panic!("object output")
                };
                assert_eq!(image.relocations.len(), 1);
                assert_eq!(
                    image.relocations[0].pointer.origin,
                    SimulationInputOriginV29::Backing(if cycle {
                        request.shared_storage[1 - index].id
                    } else {
                        shared.id
                    })
                );
            }
        }
    }
}

fn enum_rows(pointer_niche: bool) -> Vec<StorageLayoutV1> {
    if pointer_niche {
        vec![
            scalar(),
            pointer(2),
            StorageLayoutV1 {
                size: 8,
                alignment: 8,
                kind: StorageLayoutKindV1::Record(
                    vec![StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(1),
                    }]
                    .into_boxed_slice(),
                ),
            },
            StorageLayoutV1 {
                size: 8,
                alignment: 8,
                kind: StorageLayoutKindV1::Variants {
                    encoding: StorageVariantEncodingV1::Niche {
                        tag: StorageFieldV1 {
                            offset: 0,
                            layout: StorageLayoutIdV1(1),
                        },
                        untagged_variant: 0,
                        first_niche_variant: 1,
                        last_niche_variant: 1,
                        niche_start: 0,
                    },
                    variants: vec![
                        StorageVariantV1 {
                            discriminant: 0,
                            direct_tag_bits: None,
                            uninhabited: false,
                            layout: StorageLayoutIdV1(2),
                        },
                        StorageVariantV1 {
                            discriminant: 1,
                            direct_tag_bits: None,
                            uninhabited: false,
                            layout: StorageLayoutIdV1(0),
                        },
                    ]
                    .into_boxed_slice(),
                },
            },
        ]
    } else {
        vec![
            scalar(),
            StorageLayoutV1 {
                size: 8,
                alignment: 4,
                kind: StorageLayoutKindV1::Record(
                    vec![StorageFieldV1 {
                        offset: 4,
                        layout: StorageLayoutIdV1(0),
                    }]
                    .into_boxed_slice(),
                ),
            },
            StorageLayoutV1 {
                size: 8,
                alignment: 4,
                kind: StorageLayoutKindV1::Variants {
                    encoding: StorageVariantEncodingV1::Direct {
                        tag: StorageFieldV1 {
                            offset: 0,
                            layout: StorageLayoutIdV1(0),
                        },
                    },
                    variants: vec![
                        StorageVariantV1 {
                            discriminant: 0,
                            direct_tag_bits: Some(0),
                            uninhabited: false,
                            layout: StorageLayoutIdV1(1),
                        },
                        StorageVariantV1 {
                            discriminant: 1,
                            direct_tag_bits: Some(1),
                            uninhabited: false,
                            layout: StorageLayoutIdV1(1),
                        },
                    ]
                    .into_boxed_slice(),
                },
            },
        ]
    }
}

#[test]
fn self_guarded_pointer_niche_uses_prepared_nonnull_domain_without_recursive_dereference() {
    let owner = admit(&graph(
        enum_rows(true),
        vec![object_pointer(
            3,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )],
        vec![value(
            1,
            Type::Scalar(ScalarType::U128),
            OperationKind::Storage(StorageOperationV1::ReadDiscriminant {
                address: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 8),
            }),
        )],
    ));
    for target in targets() {
        let mut request = request(vec![SimulationStorageArgumentV29::ObjectView(view(
            5,
            3,
            vec![],
        ))]);
        request.shared_storage.push(SimulationSharedStorageV29 {
            id: BufferBackingIdV1(5),
            storage: SimulationStorageBackingV29::Object {
                image: image(
                    3,
                    8,
                    vec![0; 8],
                    vec![true; 8],
                    vec![relocation(
                        1,
                        vec![
                            SimulationObjectComponentV29::Variant(0),
                            SimulationObjectComponentV29::Field(0),
                        ],
                        view(5, 2, vec![SimulationObjectComponentV29::Variant(0)]),
                    )],
                ),
                access: AccessMode::ReadWrite,
            },
        });
        let result =
            simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()).unwrap();
        let SimulationStorageBackingObservationV29::Object { image, .. } =
            &result.shared_storage()[0].storage
        else {
            panic!("enum object")
        };
        assert_eq!(
            image.relocations[0].pointer.origin,
            SimulationInputOriginV29::Backing(BufferBackingIdV1(5))
        );
        assert_eq!(
            image.relocations[0].pointer.pointee,
            SimulationObservedPointeeV29::Object(StorageLayoutIdV1(2))
        );
        assert_eq!(image.bytes, vec![0; 8]);
    }
}

#[test]
fn source_and_target_selectors_require_initialized_active_tags_before_publication() {
    for niche in [false, true] {
        let mut rows = enum_rows(false);
        if niche {
            let StorageLayoutKindV1::Variants { encoding, variants } = &mut rows[2].kind else {
                unreachable!()
            };
            *encoding = StorageVariantEncodingV1::Niche {
                tag: StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                },
                untagged_variant: 0,
                first_niche_variant: 1,
                last_niche_variant: 1,
                niche_start: 0,
            };
            for variant in variants {
                variant.direct_tag_bits = None;
            }
        }
        let owner = admit(&graph(
            rows,
            vec![object_pointer(
                0,
                AddressSpace::Global,
                AccessMode::ReadWrite,
            )],
            vec![],
        ));
        for target in targets() {
            for (active, initialized, succeeds) in
                [(0, true, true), (1, true, false), (0, false, false)]
            {
                let mut request = request(vec![SimulationStorageArgumentV29::ObjectView(view(
                    1,
                    0,
                    vec![
                        SimulationObjectComponentV29::Variant(0),
                        SimulationObjectComponentV29::Field(0),
                    ],
                ))]);
                let bits = if niche { 1 - active } else { active };
                let mut bytes = vec![0; 8];
                bytes[..4].copy_from_slice(&(bits as u32).to_le_bytes());
                let mut mask = vec![true; 8];
                mask[..4].fill(initialized);
                request.shared_storage.push(SimulationSharedStorageV29 {
                    id: BufferBackingIdV1(1),
                    storage: SimulationStorageBackingV29::Object {
                        image: image(2, 4, bytes, mask, vec![]),
                        access: AccessMode::ReadWrite,
                    },
                });
                let mut events = Events::default();
                let result = simulate_canonical_storage_inputs_with_sinks_v29(
                    &owner,
                    &request,
                    None,
                    target,
                    limits(),
                    SimulationDebugCaptureLimitsV1::disabled(),
                    &mut events,
                    &mut NoopSimulationDebugSinkV1,
                );
                if succeeds {
                    assert!(result.is_ok());
                } else {
                    assert_violation(
                        result,
                        if initialized {
                            "input projection names an inactive or uninhabited variant"
                        } else {
                            "input variant tag is not initialized"
                        },
                    );
                    assert!(events.0.is_empty());
                }
            }
        }
    }
}

#[test]
fn relocation_sources_discharge_every_enclosing_selector_before_any_publication() {
    let direct = |payload| StorageLayoutKindV1::Variants {
        encoding: StorageVariantEncodingV1::Direct {
            tag: StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(0),
            },
        },
        variants: (0..2)
            .map(|index| StorageVariantV1 {
                discriminant: index,
                direct_tag_bits: Some(index),
                uninhabited: false,
                layout: StorageLayoutIdV1(payload),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    };
    let rows = vec![
        scalar(),
        pointer(0),
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 8,
                    layout: StorageLayoutIdV1(1),
                }]
                .into_boxed_slice(),
            ),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: direct(2),
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 8,
                    layout: StorageLayoutIdV1(3),
                }]
                .into_boxed_slice(),
            ),
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: direct(4),
        },
    ];
    let owner = admit(&graph(rows, vec![], vec![]));
    for target in targets() {
        for (outer, inner, initialized, succeeds) in [
            (0u32, 0u32, true, true),
            (1, 0, true, false),
            (0, 1, true, false),
            (0, 0, false, false),
        ] {
            let mut bytes = vec![0; 24];
            bytes[..4].copy_from_slice(&outer.to_le_bytes());
            bytes[8..12].copy_from_slice(&inner.to_le_bytes());
            let mut mask = vec![true; 24];
            mask[8..12].fill(initialized);
            let mut request = request(vec![]);
            request.shared_storage.push(SimulationSharedStorageV29 {
                id: BufferBackingIdV1(1),
                storage: SimulationStorageBackingV29::Object {
                    image: image(
                        5,
                        8,
                        bytes,
                        mask,
                        vec![relocation(
                            1,
                            vec![
                                SimulationObjectComponentV29::Variant(0),
                                SimulationObjectComponentV29::Field(0),
                                SimulationObjectComponentV29::Variant(0),
                                SimulationObjectComponentV29::Field(0),
                            ],
                            view(9, 0, vec![]),
                        )],
                    ),
                    access: AccessMode::ReadWrite,
                },
            });
            request.shared_storage.push(SimulationSharedStorageV29 {
                id: BufferBackingIdV1(9),
                storage: SimulationStorageBackingV29::Scalar(buffer(5, target)),
            });
            let original = request.clone();
            let mut events = Events::default();
            let result = simulate_canonical_storage_inputs_with_sinks_v29(
                &owner,
                &request,
                None,
                target,
                limits(),
                SimulationDebugCaptureLimitsV1::disabled(),
                &mut events,
                &mut NoopSimulationDebugSinkV1,
            );
            if succeeds {
                let result = result.unwrap();
                let SimulationStorageBackingObservationV29::Object { image, .. } =
                    &result.shared_storage()[0].storage
                else {
                    panic!("outer object")
                };
                assert_eq!(image.relocations.len(), 1);
                assert_eq!(image.relocations[0].byte_offset, 16);
                assert_eq!(
                    image.relocations[0].pointer.origin,
                    SimulationInputOriginV29::Backing(BufferBackingIdV1(9))
                );
            } else {
                assert_violation(
                    result,
                    if initialized {
                        "input projection names an inactive or uninhabited variant"
                    } else {
                        "input variant tag is not initialized"
                    },
                );
                assert!(events.0.is_empty());
            }
            assert_eq!(request, original);
        }
    }
}

#[test]
fn later_retag_invalidates_the_original_imported_payload_view() {
    let rows = enum_rows(false);
    for read_after in [false, true] {
        let mut operations = vec![Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::SetDiscriminant {
                address: ValueId(0),
                variant: 1,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            }),
        )];
        if read_after {
            operations.push(read(
                2,
                1,
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                4,
            ));
        }
        let owner = admit(&graph(
            rows.clone(),
            vec![
                object_pointer(2, AddressSpace::Global, AccessMode::ReadWrite),
                object_pointer(0, AddressSpace::Global, AccessMode::ReadWrite),
            ],
            operations,
        ));
        let mut request = request(vec![
            SimulationStorageArgumentV29::ObjectView(view(1, 2, vec![])),
            SimulationStorageArgumentV29::ObjectView(view(
                1,
                0,
                vec![
                    SimulationObjectComponentV29::Variant(0),
                    SimulationObjectComponentV29::Field(0),
                ],
            )),
        ]);
        request.shared_storage.push(SimulationSharedStorageV29 {
            id: BufferBackingIdV1(1),
            storage: SimulationStorageBackingV29::Object {
                image: image(2, 4, vec![0; 8], vec![true; 8], vec![]),
                access: AccessMode::ReadWrite,
            },
        });
        assert_violation(
            simulate_canonical_storage_inputs_v29(&owner, &request, targets()[0], limits()),
            "variant view was invalidated",
        );
    }
}

#[test]
fn empty_scalar_descriptor_pointer_roundtrips_without_granting_a_load() {
    let rows = vec![
        scalar(),
        pointer(0),
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
                value_space: AddressSpace::Global,
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
    ];
    for target in targets() {
        for element in [
            Type::Scalar(ScalarType::U32),
            Type::StorageObject(StorageLayoutIdV1(0)),
        ] {
            let slice = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
            let mut operations = vec![
                read(1, 0, slice.clone(), AddressSpace::Constant, 8),
                value(
                    2,
                    object_pointer(3, AddressSpace::Private, AccessMode::ReadWrite),
                    OperationKind::Alloca {
                        element: Type::StorageObject(StorageLayoutIdV1(3)),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 8,
                    },
                ),
                Operation::new(
                    vec![],
                    OperationKind::Storage(StorageOperationV1::WriteValue {
                        address: ValueId(2),
                        value: ValueId(1),
                        access: MemoryAccess::new(AddressSpace::Private, 8),
                    }),
                ),
                read(3, 2, slice, AddressSpace::Private, 8),
            ];
            let owner = admit(&graph(
                rows.clone(),
                vec![inline_type(3)],
                operations.clone(),
            ));
            let mut target_view = view(2, 0, vec![]);
            target_view.range = Some(SimulationObjectRangeV29 {
                start: 0,
                elements: 0,
            });
            let mut request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
                3,
                8,
                vec![0; 16],
                vec![true; 16],
                vec![relocation(
                    1,
                    vec![SimulationObjectComponentV29::Field(0)],
                    target_view,
                )],
            ))]);
            request.shared_storage.push(SimulationSharedStorageV29 {
                id: BufferBackingIdV1(2),
                storage: SimulationStorageBackingV29::Scalar(
                    BufferArgumentV1::new(
                        ScalarType::U32,
                        AccessMode::ReadWrite,
                        4,
                        vec![],
                        vec![],
                        target,
                    )
                    .unwrap(),
                ),
            });
            let result =
                simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()).unwrap();
            let SimulationStorageArgumentObservationV29::InlineObject(image) =
                &result.arguments()[0]
            else {
                panic!("descriptor")
            };
            assert_eq!(image.relocations[0].pointer.byte_offset, 0);
            assert_eq!(image.relocations[0].pointer.upper_bound, 0);

            operations.push(value(
                4,
                Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                OperationKind::SliceData { slice: ValueId(3) },
            ));
            operations.push(if matches!(element, Type::Scalar(_)) {
                value(
                    5,
                    Type::Scalar(ScalarType::U32),
                    OperationKind::Load {
                        pointer: ValueId(4),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                )
            } else {
                read(5, 4, Type::Scalar(ScalarType::U32), AddressSpace::Global, 4)
            });
            let access_owner = admit(&graph(rows.clone(), vec![inline_type(3)], operations));
            assert!(matches!(
                simulate_canonical_storage_inputs_v29(&access_owner, &request, target, limits()),
                Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::OutOfBounds { .. },
                    ..
                }))
            ));

            let SimulationStorageArgumentV29::InlineObject(descriptor) = &mut request.arguments[0]
            else {
                panic!("descriptor input")
            };
            let mut bytes = descriptor.bytes().to_vec();
            bytes[8..16].copy_from_slice(&1u64.to_le_bytes());
            *descriptor = SimulationObjectImageV29::new(
                descriptor.layout(),
                descriptor.alignment(),
                bytes,
                descriptor.initialized().to_vec(),
                descriptor.relocations().to_vec(),
            ).unwrap();
            assert_violation(
                simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()),
                "slice extent exceeds its bounded referent",
            );
        }
    }
}

#[test]
fn one_past_pointer_survives_real_copy_and_read_but_not_a_nonzero_load() {
    for target in targets() {
        for load in [false, true] {
            let mut operations = vec![
                value(
                    1,
                    object_pointer(1, AddressSpace::Private, AccessMode::ReadWrite),
                    OperationKind::Alloca {
                        element: Type::StorageObject(StorageLayoutIdV1(1)),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 8,
                    },
                ),
                Operation::new(
                    vec![],
                    OperationKind::Storage(StorageOperationV1::CopyObject {
                        source: ValueId(0),
                        destination: ValueId(1),
                        source_access: MemoryAccess::new(AddressSpace::Constant, 8),
                        destination_access: MemoryAccess::new(AddressSpace::Private, 8),
                        overlap: StorageCopyOverlapV1::NonOverlapping,
                    }),
                ),
                read(
                    2,
                    1,
                    Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    ),
                    AddressSpace::Private,
                    8,
                ),
            ];
            if load {
                operations.push(value(
                    3,
                    Type::Scalar(ScalarType::U32),
                    OperationKind::Load {
                        pointer: ValueId(2),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                ));
            }
            let owner = admit(&graph(
                vec![scalar(), pointer(0)],
                vec![inline_type(1)],
                operations,
            ));
            let mut target_view = view(7, 0, vec![]);
            target_view.range = Some(SimulationObjectRangeV29 {
                start: 1,
                elements: 0,
            });
            let mut request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
                1,
                8,
                vec![0; 8],
                vec![true; 8],
                vec![relocation(1, vec![], target_view)],
            ))]);
            request.shared_storage.push(SimulationSharedStorageV29 {
                id: BufferBackingIdV1(7),
                storage: SimulationStorageBackingV29::Scalar(buffer(5, target)),
            });
            let result = simulate_canonical_storage_inputs_v29(&owner, &request, target, limits());
            if load {
                assert!(matches!(
                    result,
                    Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                        kind: SimulationExecutionErrorKindV1::OutOfBounds { .. },
                        ..
                    }))
                ));
            } else {
                let result = result.unwrap();
                let SimulationStorageArgumentObservationV29::InlineObject(image) =
                    &result.arguments()[0]
                else {
                    panic!("pointer image")
                };
                assert_eq!(image.relocations[0].pointer.byte_offset, 4);
                assert_eq!(image.relocations[0].pointer.lower_bound, 4);
                assert_eq!(image.relocations[0].pointer.upper_bound, 4);
            }
        }
    }
}

#[test]
fn typed_output_cannot_export_a_pointer_into_a_dead_private_frame() {
    let private_pointer = StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
            pointee: StorageLayoutIdV1(0),
            value_space: AddressSpace::Private,
            encoded_space: AddressSpace::Private,
            access: AccessMode::ReadWrite,
            stored_bits: 32,
        }),
    };
    let owner = admit(&graph(
        vec![scalar(), private_pointer],
        vec![object_pointer(
            1,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )],
        vec![
            value(
                1,
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U32),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Storage(StorageOperationV1::WriteValue {
                    address: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                }),
            ),
        ],
    ));
    for target in targets() {
        let mut request = request(vec![SimulationStorageArgumentV29::ObjectView(view(
            4,
            1,
            vec![],
        ))]);
        request.shared_storage.push(SimulationSharedStorageV29 {
            id: BufferBackingIdV1(4),
            storage: SimulationStorageBackingV29::Object {
                image: image(1, 4, vec![0; 4], vec![false; 4], vec![]),
                access: AccessMode::ReadWrite,
            },
        });
        assert!(matches!(
            simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()),
            Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::DanglingPointer { .. },
                ..
            }))
        ));
        assert_eq!(
            match &request.shared_storage[0].storage {
                SimulationStorageBackingV29::Object { image, .. } => image.initialized(),
                _ => unreachable!(),
            },
            &[false; 4]
        );
    }
}

#[test]
fn missing_encoding_profile_and_ambiguous_prepared_pointer_domains_refuse() {
    let plain = admit(&graph(
        vec![scalar(), pointer(0)],
        vec![inline_type(1)],
        vec![],
    ));
    let mut request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
        1,
        8,
        vec![0; 8],
        vec![true; 8],
        vec![relocation(1, vec![], view(1, 0, vec![]))],
    ))]);
    request.shared_storage.push(SimulationSharedStorageV29 {
        id: BufferBackingIdV1(1),
        storage: SimulationStorageBackingV29::Scalar(buffer(5, targets()[0])),
    });
    assert_violation(
        simulate_canonical_storage_inputs_v29(
            &plain,
            &request,
            SimulationTargetV1::little_endian(IndexWidthV1::Bits64),
            limits(),
        ),
        "input pointer requires an exact target encoding profile",
    );
    let mut rows = enum_rows(true);
    let StorageLayoutKindV1::Variants { encoding, variants } = &mut rows[3].kind else {
        unreachable!()
    };
    let StorageVariantEncodingV1::Niche {
        last_niche_variant, ..
    } = encoding
    else {
        unreachable!()
    };
    *last_niche_variant = 2;
    let mut values = variants.to_vec();
    values.push(StorageVariantV1 {
        discriminant: 2,
        direct_tag_bits: None,
        uninhabited: false,
        layout: StorageLayoutIdV1(0),
    });
    *variants = values.into_boxed_slice();
    let owner = admit(&graph(
        rows,
        vec![object_pointer(
            2,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )],
        vec![],
    ));
    let mut request =
        super::storage_inputs_tests_v29::request(vec![SimulationStorageArgumentV29::ObjectView(
            view(5, 2, vec![SimulationObjectComponentV29::Variant(0)]),
        )]);
    request.shared_storage.push(SimulationSharedStorageV29 {
        id: BufferBackingIdV1(5),
        storage: SimulationStorageBackingV29::Object {
            image: image(
                3,
                8,
                vec![0; 8],
                vec![true; 8],
                vec![relocation(
                    1,
                    vec![SimulationObjectComponentV29::Tag],
                    view(5, 2, vec![SimulationObjectComponentV29::Variant(0)]),
                )],
            ),
            access: AccessMode::ReadWrite,
        },
    });
    for target in targets() {
        assert_violation(
            simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()),
            "symbolic pointer domain crosses possible logical variants",
        );
    }
}
