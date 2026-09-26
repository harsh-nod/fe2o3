use super::storage_inputs_tests_v29::*;
use super::*;

#[test]
fn exact_inline_schema_alignment_extent_and_constant_role_are_required() {
    let owner = admit(&graph(vec![scalar()], vec![inline_type(0)], vec![]));
    let original = owner.canonical_bytes().to_vec();
    for target in targets() {
        let good = request(vec![SimulationStorageArgumentV29::InlineObject(image(
            0,
            4,
            vec![0; 4],
            vec![true; 4],
            vec![],
        ))]);
        assert!(simulate_canonical_storage_inputs_v29(&owner, &good, target, limits()).is_ok());
        for bad in [
            image(1, 4, vec![0; 4], vec![true; 4], vec![]),
            image(0, 8, vec![0; 4], vec![true; 4], vec![]),
            image(0, 4, vec![0; 8], vec![true; 8], vec![]),
        ] {
            let request = request(vec![SimulationStorageArgumentV29::InlineObject(bad)]);
            let mut events = Events::default();
            assert!(matches!(
                simulate_canonical_storage_inputs_with_sinks_v29(
                    &owner,
                    &request,
                    None,
                    target,
                    limits(),
                    SimulationDebugCaptureLimitsV1::disabled(),
                    &mut events,
                    &mut NoopSimulationDebugSinkV1
                ),
                Err(SimulationErrorV1::Preflight(
                    SimulationPreflightErrorV1::StorageProfileNotAdmitted
                ))
            ));
            assert!(events.0.is_empty());
        }
        for space in [AddressSpace::Private, AddressSpace::Global] {
            let other = admit(&graph(
                vec![scalar()],
                vec![object_pointer(0, space, AccessMode::ReadOnly)],
                vec![],
            ));
            assert!(matches!(
                simulate_canonical_storage_inputs_v29(&other, &good, target, limits()),
                Err(SimulationErrorV1::Preflight(
                    SimulationPreflightErrorV1::ArgumentType { argument: 0, .. }
                ))
            ));
        }
    }
    assert_eq!(owner.canonical_bytes(), original);
}

#[test]
fn explicit_profile_retains_exact_argument_count_and_legacy_scalar_target_checks() {
    let owner = admit(&graph(vec![], vec![Type::Scalar(ScalarType::U32)], vec![]));
    for target in targets() {
        assert!(matches!(
            simulate_canonical_storage_inputs_v29(&owner, &request(vec![]), target, limits()),
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::ArgumentCount {
                    expected: 1,
                    actual: 0
                }
            ))
        ));
        let wrong = request(vec![SimulationStorageArgumentV29::Existing(
            SimulationArgumentV1::Scalar(ScalarBitsV1::new(ScalarType::U64, 7, target).unwrap()),
        )]);
        assert!(matches!(
            simulate_canonical_storage_inputs_v29(&owner, &wrong, target, limits()),
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::ArgumentType { argument: 0, .. }
            ))
        ));
    }
}

#[test]
fn missing_duplicate_and_scalar_object_backing_substitutions_refuse_before_events() {
    let owner = admit(&graph(
        vec![scalar()],
        vec![object_pointer(
            0,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )],
        vec![],
    ));
    for target in targets() {
        let original = request(vec![SimulationStorageArgumentV29::ObjectView(view(
            9,
            0,
            vec![],
        ))]);
        let mut events = Events::default();
        assert_violation(
            simulate_canonical_storage_inputs_with_sinks_v29(
                &owner,
                &original,
                None,
                target,
                limits(),
                SimulationDebugCaptureLimitsV1::disabled(),
                &mut events,
                &mut NoopSimulationDebugSinkV1,
            ),
            "input referent has no original allocation",
        );
        assert!(events.0.is_empty());
        let mut duplicate = original.clone();
        let backing = SimulationSharedStorageV29 {
            id: BufferBackingIdV1(9),
            storage: SimulationStorageBackingV29::Scalar(buffer(4, target)),
        };
        duplicate.shared_storage = vec![backing.clone(), backing];
        assert!(matches!(
            simulate_canonical_storage_inputs_v29(&owner, &duplicate, target, limits()),
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::DuplicateBacking(9)
            ))
        ));
        duplicate.shared_storage.pop();
        assert!(
            simulate_canonical_storage_inputs_v29(&owner, &duplicate, target, limits()).is_ok()
        );
        let mut bad = duplicate.clone();
        let SimulationStorageArgumentV29::ObjectView(view) = &mut bad.arguments[0] else {
            unreachable!()
        };
        view.path.push(SimulationObjectComponentV29::Field(0));
        assert_violation(
            simulate_canonical_storage_inputs_v29(&owner, &bad, target, limits()),
            "input scalar referent changes exact element layout",
        );
        assert_eq!(original.shared_storage.len(), 0);
    }
}

#[test]
fn image_paths_cannot_name_padding_bad_fields_or_out_of_range_array_elements() {
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
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(1),
                length: 2,
                stride: 8,
            },
        },
    ];
    for target in targets() {
        for (layout, path, reason) in [
            (
                2,
                vec![SimulationObjectComponentV29::Field(1)],
                "input field ordinal is absent",
            ),
            (
                2,
                vec![SimulationObjectComponentV29::Index(0)],
                "input path does not match its physical component",
            ),
            (
                3,
                vec![SimulationObjectComponentV29::Index(2)],
                "input array index exceeds its exact length",
            ),
        ] {
            let owner = admit(&graph(rows.clone(), vec![inline_type(layout)], vec![]));
            let mut request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
                layout,
                8,
                vec![0; 16],
                vec![true; 16],
                vec![relocation(1, path, view(1, 0, vec![]))],
            ))]);
            request.shared_storage.push(SimulationSharedStorageV29 {
                id: BufferBackingIdV1(1),
                storage: SimulationStorageBackingV29::Scalar(buffer(1, target)),
            });
            assert_violation(
                simulate_canonical_storage_inputs_v29(&owner, &request, target, limits()),
                reason,
            );
        }
    }
}

#[test]
fn pointer_relocation_order_initialization_and_exact_permissions_are_not_inferred() {
    let rows = vec![
        scalar(),
        pointer(0),
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(1),
                length: 2,
                stride: 8,
            },
        },
    ];
    let owner = admit(&graph(rows, vec![inline_type(2)], vec![]));
    for target in targets() {
        let good = vec![
            relocation(
                1,
                vec![SimulationObjectComponentV29::Index(0)],
                view(1, 0, vec![]),
            ),
            relocation(
                1,
                vec![SimulationObjectComponentV29::Index(1)],
                view(1, 0, vec![]),
            ),
        ];
        for case in 0..5 {
            let mut relocations = good.clone();
            let mut initialized = vec![true; 16];
            let reason = match case {
                0 => {
                    relocations.reverse();
                    "input relocations are partial, overlapping, unordered or misrepresented"
                }
                1 => {
                    relocations[1] = relocations[0].clone();
                    "input relocations are partial, overlapping, unordered or misrepresented"
                }
                2 => {
                    initialized[7] = false;
                    "input pointer representation is not initialized"
                }
                3 => {
                    relocations[0].pointer_layout = StorageLayoutIdV1(0);
                    "input relocations are partial, overlapping, unordered or misrepresented"
                }
                _ => {
                    relocations[0].referent.access = AccessMode::ReadOnly;
                    "input relocation changes its actual referent representation"
                }
            };
            let mut request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
                2,
                8,
                vec![0; 16],
                initialized,
                relocations,
            ))]);
            request.shared_storage.push(SimulationSharedStorageV29 {
                id: BufferBackingIdV1(1),
                storage: SimulationStorageBackingV29::Scalar(buffer(9, target)),
            });
            let mut events = Events::default();
            assert_violation(
                simulate_canonical_storage_inputs_with_sinks_v29(
                    &owner,
                    &request,
                    None,
                    target,
                    limits(),
                    SimulationDebugCaptureLimitsV1::disabled(),
                    &mut events,
                    &mut NoopSimulationDebugSinkV1,
                ),
                reason,
            );
            assert!(events.0.is_empty());
        }
    }
}

#[test]
fn legacy_v18_requests_do_not_acquire_inline_object_or_private_parameter_admission() {
    let owner = admit(&graph(vec![scalar()], vec![inline_type(0)], vec![]));
    let old = super::storage_tests_v1::request();
    assert!(matches!(
        simulate_canonical_storage_v18(&owner, &old, targets()[0], limits()),
        Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::ArgumentType { argument: 0, .. }
        ))
    ));
}

#[test]
fn views_preserve_original_origin_permissions_and_operation_specific_bounds() {
    let owner = admit(&graph(
        vec![scalar()],
        vec![object_pointer(
            0,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )],
        vec![],
    ));
    for target in targets() {
        let mut base = request(vec![SimulationStorageArgumentV29::ObjectView(view(
            7,
            0,
            vec![],
        ))]);
        base.shared_storage.push(SimulationSharedStorageV29 {
            id: BufferBackingIdV1(7),
            storage: SimulationStorageBackingV29::Scalar(buffer(2, target)),
        });
        assert!(simulate_canonical_storage_inputs_v29(&owner, &base, target, limits()).is_ok());
        for case in 0..4 {
            let mut bad = base.clone();
            let SimulationStorageArgumentV29::ObjectView(view) = &mut bad.arguments[0] else {
                unreachable!()
            };
            let reason = match case {
                0 => {
                    view.origin = SimulationInputOriginV29::Argument(0);
                    "input referent has no original allocation"
                }
                1 => {
                    view.range = Some(SimulationObjectRangeV29 {
                        start: 2,
                        elements: 0,
                    });
                    "input range changes element stride or exceeds bounds"
                }
                2 => {
                    view.range = Some(SimulationObjectRangeV29 {
                        start: 1,
                        elements: 1,
                    });
                    "input range changes element stride or exceeds bounds"
                }
                _ => {
                    bad.shared_storage[0].storage = SimulationStorageBackingV29::Scalar(
                        BufferArgumentV1::new(
                            ScalarType::U32,
                            AccessMode::ReadOnly,
                            4,
                            vec![0; 4],
                            vec![true; 4],
                            target,
                        )
                        .unwrap(),
                    );
                    "input referent strengthens backing permissions"
                }
            };
            let mut events = Events::default();
            assert_violation(
                simulate_canonical_storage_inputs_with_sinks_v29(
                    &owner,
                    &bad,
                    None,
                    target,
                    limits(),
                    SimulationDebugCaptureLimitsV1::disabled(),
                    &mut events,
                    &mut NoopSimulationDebugSinkV1,
                ),
                reason,
            );
            assert!(events.0.is_empty());
        }
    }
}

#[test]
fn physically_partial_pointer_overlaps_cannot_be_published_through_different_union_paths() {
    let rows = vec![
        scalar(),
        pointer(0),
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 4,
                    layout: StorageLayoutIdV1(1),
                }]
                .into_boxed_slice(),
            ),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Union(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(1),
                    },
                    StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(2),
                    },
                ]
                .into_boxed_slice(),
            ),
        },
    ];
    let owner = admit(&graph(rows, vec![inline_type(3)], vec![]));
    for target in targets() {
        let mut request = request(vec![SimulationStorageArgumentV29::InlineObject(image(
            3,
            8,
            vec![0; 16],
            vec![true; 16],
            vec![
                relocation(
                    1,
                    vec![SimulationObjectComponentV29::Field(0)],
                    view(1, 0, vec![]),
                ),
                relocation(
                    1,
                    vec![
                        SimulationObjectComponentV29::Field(1),
                        SimulationObjectComponentV29::Field(0),
                    ],
                    view(1, 0, vec![]),
                ),
            ],
        ))]);
        request.shared_storage.push(SimulationSharedStorageV29 {
            id: BufferBackingIdV1(1),
            storage: SimulationStorageBackingV29::Scalar(buffer(3, target)),
        });
        let mut events = Events::default();
        assert_violation(
            simulate_canonical_storage_inputs_with_sinks_v29(
                &owner,
                &request,
                None,
                target,
                limits(),
                SimulationDebugCaptureLimitsV1::disabled(),
                &mut events,
                &mut NoopSimulationDebugSinkV1,
            ),
            "input relocations are partial, overlapping, unordered or misrepresented",
        );
        assert!(events.0.is_empty());
    }
}
