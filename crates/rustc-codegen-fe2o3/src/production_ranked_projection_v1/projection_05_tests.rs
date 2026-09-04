    #[test]
    fn capability_alias_cycle_terminates_with_exact_edge_charge() {
        let mut edges = vec![Vec::new(); 2];
        let mut charged = 0;
        for (source, destination) in [(0, 1), (1, 0)] {
            push_capability_edge(
                &mut edges,
                &mut charged,
                source,
                CapabilityEdgeV1 {
                    destination,
                    use_block: 0,
                    kind: CapabilityEdgeKindV1::Alias,
                },
            )
            .unwrap();
        }
        let seed = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(0),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        };
        let mut values = vec![Some(seed), None];
        let grid = vec![None; 2];
        let mut worklist = VecDeque::from([0]);
        let mut processed = 0;
        while let Some(source) = worklist.pop_front() {
            for edge in &edges[source] {
                processed += 1;
                assign_index_capability(
                    edge.destination,
                    values[source].unwrap(),
                    &mut values,
                    &grid,
                    &mut worklist,
                )
                .unwrap();
            }
        }

        assert_eq!(processed, charged);
        assert_eq!(values, vec![Some(seed), Some(seed)]);
    }
    #[test]
    fn conflicting_capability_def_use_paths_fail_closed() {
        let first = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(0),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        };
        let second = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(1),
            ..first
        };
        let mut values = vec![None, Some(first)];
        let grid = vec![None; 2];
        let mut worklist = VecDeque::new();

        assert!(matches!(
            assign_index_capability(1, second, &mut values, &grid, &mut worklist),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple index capabilities reach one semantic local"
            ))
        ));
    }

    #[test]
    fn rust_pointee_kinds_define_conservative_alias_classes() {
        let shared = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
            true,
            2,
        );
        let shared_interior_mutable = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            false,
            5,
        );
        let unique = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
            true,
            3,
        );
        let unqualified = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
            false,
            4,
        );

        assert_eq!(shared.noalias_class, 1);
        assert!(!shared.writable);
        assert_eq!(shared_interior_mutable.noalias_class, 1);
        assert!(shared_interior_mutable.writable);
        assert_eq!(unique.noalias_class, 4);
        assert!(unique.writable);
        assert_eq!(unqualified.noalias_class, 0);
        assert!(unqualified.writable);
    }

    fn volatile_load_source_types_v1(
        element_shape: SemanticTypeShapeV1,
        pointer_kind: SemanticPointerKindV1,
        mutability: SemanticMutabilityV1,
    ) -> Vec<SemanticTypeDeclV1> {
        let element = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(116)),
            SemanticLayoutIdentityV1::from_sha256(bytes(116)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            element_shape,
        );
        let slice = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(117)),
            SemanticLayoutIdentityV1::from_sha256(bytes(117)),
            SemanticTypeLayoutV1::new(None, 4).unwrap(),
            SemanticTypeShapeV1::Slice {
                element: SemanticTypeIdV1::from_index(0),
            },
        );
        let source = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(118)),
            SemanticLayoutIdentityV1::from_sha256(bytes(118)),
            SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(1),
                    pointer_kind,
                    mutability,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            ),
        );
        vec![element, slice, source]
    }

    #[test]
    fn volatile_load_read_only_eligibility_is_local_and_exact() {
        let scalar = SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 });
        let shared = volatile_load_source_types_v1(
            scalar,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
        );
        let source = SemanticTypeIdV1::from_index(2);
        let element = SemanticTypeIdV1::from_index(0);
        let generic_contract = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            false,
            1,
        );
        assert!(generic_contract.writable);
        assert!(volatile_load_frozen_shared_scalar_type_v1(
            &shared,
            source,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            element,
        ));

        for (ownership, pointer_kind, mutability, pointee_kind) in [
            (
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            ),
            (
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            ),
            (
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            ),
            (
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                SemanticAbiPointeeKindV1::Raw,
            ),
        ] {
            let types = volatile_load_source_types_v1(
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
                pointer_kind,
                mutability,
            );
            assert!(!volatile_load_frozen_shared_scalar_type_v1(
                &types,
                source,
                ownership,
                pointee_kind,
                element,
            ));
        }

        for element_shape in [
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ] {
            let types = volatile_load_source_types_v1(
                element_shape,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
            );
            assert!(!volatile_load_frozen_shared_scalar_type_v1(
                &types,
                source,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticAbiPointeeKindV1::SharedReference { frozen: false },
                element,
            ));
        }
    }

    #[test]
    fn authenticated_source_ownership_distinguishes_borrows_owners_and_raw_pointers() {
        let raw = allocation_contract_from_pointee(SemanticAbiPointeeKindV1::Raw, false, 3);
        let exclusive = authenticated_source_allocation_contract_v1(
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            SemanticAbiPointeeKindV1::Raw,
            raw,
        )
        .unwrap();
        assert_eq!(exclusive.allocation_origin, 3);
        assert_eq!(exclusive.noalias_class, 4);
        assert!(exclusive.writable);

        for ownership in [
            SemanticSourceArgumentOwnershipV1::RawPointer,
            SemanticSourceArgumentOwnershipV1::ByValue,
        ] {
            assert_eq!(
                authenticated_source_allocation_contract_v1(
                    ownership,
                    SemanticAbiPointeeKindV1::Raw,
                    raw,
                )
                .unwrap()
                .noalias_class,
                0
            );
        }
    }

    #[test]
    fn authenticated_source_ownership_mismatches_fail_closed() {
        let shared = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
            true,
            2,
        );
        let raw = allocation_contract_from_pointee(SemanticAbiPointeeKindV1::Raw, false, 3);
        for (ownership, pointee, contract) in [
            (
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                shared,
            ),
            (
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticAbiPointeeKindV1::Raw,
                raw,
            ),
            (
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                SemanticAbiPointeeKindV1::Raw,
                raw,
            ),
            (
                SemanticSourceArgumentOwnershipV1::Unspecified,
                SemanticAbiPointeeKindV1::Raw,
                raw,
            ),
            (
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                shared,
            ),
            (
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                allocation_contract_from_pointee(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    true,
                    4,
                ),
            ),
            (
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticAbiPointeeKindV1::Box {
                    unpin: true,
                    global: true,
                },
                allocation_contract_from_pointee(
                    SemanticAbiPointeeKindV1::Box {
                        unpin: true,
                        global: true,
                    },
                    true,
                    5,
                ),
            ),
        ] {
            assert!(matches!(
                authenticated_source_allocation_contract_v1(ownership, pointee, contract),
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "source ownership disagrees with rustc ABI pointer provenance"
                ))
            ));
        }
    }

    #[test]
    fn source_execution_layout_derives_active_grid_axes_from_xyz_workgroup() {
        for (rank, workgroup, max_grid, global_extents) in [
            (1, [128, 1, 1], [u32::MAX, 1, 1], [0, 1, 1]),
            (2, [64, 1, 1], [u32::MAX, u32::MAX, 1], [0, 0, 1]),
            (2, [8, 8, 1], [u32::MAX, u32::MAX, 1], [0, 0, 1]),
            (
                3,
                [64, 1, 1],
                [u32::MAX, u32::from(u16::MAX), u32::from(u16::MAX)],
                [0, 0, 0],
            ),
            (
                3,
                [4, 4, 4],
                [u32::MAX, u32::from(u16::MAX), u32::from(u16::MAX)],
                [0, 0, 0],
            ),
        ] {
            let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
            let launch =
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap();
            let source_contract =
                SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
            let function =
                projection_function(vec![block(30, vec![], SemanticTerminatorKindV1::Return)])
                    .with_kernel_entry(SemanticKernelEntryV1::new(
                        SemanticLinkSymbolV1::new(b"typed_kernel".to_vec()).unwrap(),
                        SemanticKernelBindingIdentityV1::from_sha256(bytes(42)),
                        source_contract,
                    ));
            let source_launch = LaunchContract::new(
                rank,
                BlockSize::Exact(
                    fe2o3_artifacts::Dimensions::new(workgroup[0], workgroup[1], workgroup[2])
                        .unwrap(),
                ),
                fe2o3_artifacts::Dimensions::new(max_grid[0], max_grid[1], max_grid[2]).unwrap(),
                0,
                0,
            )
            .unwrap();

            assert_eq!(
                source_execution_layout_v1(
                    SemanticTargetArchitectureV1::AmdGpuGfx942,
                    &function,
                    &source_launch,
                )
                .unwrap(),
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: u64::from_le_bytes([42; 8]),
                    global_extents,
                    workgroup_extents: workgroup.map(u64::from),
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                }
            );
        }
    }

    #[test]
    fn source_execution_layout_authenticates_finite_grid_and_rejects_hostility() {
        let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
        let launch =
            SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
        let source_contract =
            SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
        let function =
            projection_function(vec![block(30, vec![], SemanticTerminatorKindV1::Return)])
                .with_kernel_entry(SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(b"typed_kernel".to_vec()).unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256(bytes(42)),
                    source_contract,
                ));
        let finite = LaunchContract::new(
            1,
            BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
            fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        assert!(matches!(
            source_execution_layout_v1(
                SemanticTargetArchitectureV1::AmdGpuGfx942,
                &function,
                &finite,
            )
            .unwrap(),
            ProductionRankedOperationV1::ExecutionLayout {
                global_extents: [64, 1, 1],
                workgroup_extents: [64, 1, 1],
                ..
            }
        ));

        let substituted_workgroup = LaunchContract::new(
            1,
            BlockSize::Exact(fe2o3_artifacts::Dimensions::new(256, 1, 1).unwrap()),
            fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        assert!(matches!(
            source_execution_layout_v1(
                SemanticTargetArchitectureV1::AmdGpuGfx942,
                &function,
                &substituted_workgroup,
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "authenticated LaunchContract workgroup disagrees with semantic source workgroup"
            ))
        ));
        let degenerate_rank_two = LaunchContract::new(
            2,
            BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
            fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        assert!(matches!(
            source_execution_layout_v1(
                SemanticTargetArchitectureV1::AmdGpuGfx942,
                &function,
                &degenerate_rank_two,
            ),
            Ok(ProductionRankedOperationV1::ExecutionLayout {
                global_extents: [64, 1, 1],
                workgroup_extents: [64, 1, 1],
                ..
            })
        ));
        assert!(matches!(
            checked_global_extent_v1(u64::MAX, 2, u64::from(u32::MAX)),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "authenticated finite grid extent overflows u64"
            ))
        ));
        assert!(
            LaunchContract::new(
                1,
                BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
                fe2o3_artifacts::Dimensions::new(1, 2, 1).unwrap(),
                0,
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn checked_reference_provenance_covers_only_the_exact_pointee() {
        let origin = CheckedReferenceOriginV1 {
            source: CheckedReferenceSourceV1::GuardedAccess(7),
            availability: None,
        };
        let origins = [None, None, None, Some(origin)];
        assert_eq!(
            checked_reference_origin_for_place(&dereferenced_place(), &origins),
            Some(origin)
        );
        let nested_index = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ARRAY_TYPE)
                    .unwrap(),
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 4,
                        from_end: false,
                    },
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        assert_eq!(
            checked_reference_origin_for_place(&nested_index, &origins),
            None
        );

        let function = projection_function(vec![block(
            31,
            vec![
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], POINTER_TYPE)
                        .unwrap(),
                    SemanticRvalueV1::new(
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(3),
                                vec![],
                                POINTER_TYPE,
                            )
                            .unwrap(),
                        )),
                    ),
                ))),
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    dereferenced_place(),
                    SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
                ))),
            ],
            SemanticTerminatorKindV1::Return,
        )]);
        assert_eq!(local_definition_counts(&function)[3], 1);
    }

    #[test]
    fn shared_indexed_borrow_is_retained_as_an_already_projected_read() {
        let function = projection_function(vec![block(
            31,
            vec![statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], POINTER_TYPE)
                        .unwrap(),
                    SemanticRvalueV1::new(
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: ranked_place(0),
                        },
                    ),
                ),
            ))],
            SemanticTerminatorKindV1::Return,
        )]);
        let option_dominance = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enum_dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let origins = checked_reference_origins(
            &function,
            &[],
            0,
            &vec![Vec::new(); function.locals().len()],
            &option_dominance,
            &enum_dominance,
        )
        .unwrap();

        assert_eq!(
            origins[3],
            Some(CheckedReferenceOriginV1 {
                source: CheckedReferenceSourceV1::ProjectedSharedBorrow,
                availability: None,
            })
        );
    }

    #[test]
    fn checked_reference_dereference_requires_the_authenticated_some_branch() {
        let (function, producers) = option_dominance_chain(1);
        let option_dominance = SemanticOptionDominanceV1::analyze(&function, &producers).unwrap();
        let availability = option_dominance
            .availability(SemanticLocalIdV1::from_index(1))
            .unwrap();
        let mut origins = vec![None; function.locals().len()];
        origins[1] = Some(CheckedReferenceOriginV1 {
            source: CheckedReferenceSourceV1::GuardedAccess(7),
            availability: Some(CapabilityAvailabilityV1::Option(availability)),
        });
        let references = CheckedReferencesV1 {
            origins,
            option_dominance,
            enum_payload_dominance: SemanticEnumPayloadDominanceV1::analyze(
                &function,
                &assertion_proof_types(),
            )
            .unwrap(),
        };
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                    .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();

        assert_eq!(
            checked_reference_origin(&place, 2, &references).unwrap(),
            Some(CheckedReferenceSourceV1::GuardedAccess(7))
        );
        assert!(matches!(
            checked_reference_origin(&place, 3, &references),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked reference is dereferenced outside its authenticated payload region"
            ))
        ));
    }

    #[test]
    fn atomic_rmw_projects_result_one_atomic_address_effect_and_value() {
        let (operations, sources, _) = audit_statements(vec![statement(
            SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
                scalar_place(),
                ranked_place(0),
                SemanticOperandV1::Copy(ranked_place(1)),
                SemanticAtomicRmwOpV1::Add,
                atomic_access(),
            )),
        )])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![AccessKindAttr::AtomicReadModifyWrite, AccessKindAttr::Read,]
        );
        assert_eq!(sources.len(), 2);
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::AtomicAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                ordering: AtomicOrderingAttr::Relaxed,
                scope: AtomicScopeAttr::Agent,
                ..
            }
        )));
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::AtomicRead
                    | AccessKindAttr::AtomicWrite
                    | AccessKindAttr::AtomicReadModifyWrite,
                ..
            }
        )));
    }

    #[test]
    fn authenticated_singleton_pointer_projects_a_rank_one_atomic_object() {
        let function = projection_function(vec![block(
            97,
            vec![statement(SemanticStatementKindV1::AtomicRmw(
                SemanticAtomicRmwV1::new(
                    scalar_place(),
                    dereferenced_place(),
                    constant(1),
                    SemanticAtomicRmwOpV1::Add,
                    atomic_access(),
                ),
            ))],
            SemanticTerminatorKindV1::Return,
        )]);
        let mut contracts = synthetic_local_contracts(&function);
        contracts.allocations[3].as_mut().unwrap().singleton_object = true;
        let (operations, sources, _) =
            audit_function_with_local_contracts(&function, &contracts).unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![AccessKindAttr::AtomicReadModifyWrite]
        );
        assert_eq!(sources.len(), 1);
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::ViewInSpace {
                shape,
                memory_space: MemorySpaceAttr::Global,
                ..
            } if shape == &[1]
        )));
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::AtomicAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                ordering: AtomicOrderingAttr::Relaxed,
                scope: AtomicScopeAttr::Agent,
                indices,
                ..
            } if indices.len() == 1
        )));

        let hostile = synthetic_local_contracts(&function);
        assert_unsupported(
            audit_function_with_local_contracts(&function, &hostile),
            "a dereferenced memory access without a ranked index projection",
        );
    }

    #[test]
    fn compatible_atomic_effect_does_not_require_an_invocation_derived_coordinate() {
        let effect_source = ProjectedEffectSourceV1 {
            access: AccessKindAttr::AtomicReadModifyWrite,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: None,
        };
        let atomic = ProjectedSemanticBlockV1 {
            items: vec![ProjectedBlockItemV1::Effect {
                operation: ProductionRankedOperationV1::AtomicAccess {
                    kind: AccessKindAttr::AtomicReadModifyWrite,
                    ordering: AtomicOrderingAttr::Relaxed,
                    scope: AtomicScopeAttr::System,
                    view: ProductionRankedValueV1::Argument(0),
                    indices: vec![ProductionRankedValueV1::Argument(1)],
                },
                source: Some(effect_source),
            }],
        };
        assert!(!atomic.requires_invocation_index());

        let ordinary = ProjectedSemanticBlockV1 {
            items: vec![ProjectedBlockItemV1::Effect {
                operation: ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: ProductionRankedValueV1::Argument(0),
                    indices: vec![ProductionRankedValueV1::Argument(1)],
                },
                source: Some(ProjectedEffectSourceV1 {
                    access: AccessKindAttr::Write,
                    ..effect_source
                }),
            }],
        };
        assert!(ordinary.requires_invocation_index());
    }

    #[test]
    fn atomic_compare_exchange_projects_both_candidates_and_address_effects() {
        let (operations, sources, _) = audit_statements(vec![statement(
            SemanticStatementKindV1::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
                scalar_place(),
                ranked_place(0),
                SemanticOperandV1::Copy(ranked_place(1)),
                SemanticOperandV1::Move(ranked_place(2)),
                atomic_access(),
                SemanticAtomicOrderingV1::Relaxed,
                false,
            )),
        )])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![
                AccessKindAttr::AtomicReadModifyWrite,
                AccessKindAttr::Read,
                AccessKindAttr::Read,
            ]
        );
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn discriminant_and_deinitialize_places_are_not_silently_skipped() {
        let discriminant_read = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            scalar_place(),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Discriminant(ranked_place(0)),
            ),
        ));
        let (operations, sources, _) = audit_statements(vec![
            statement(SemanticStatementKindV1::SetDiscriminant {
                place: ranked_place(1),
                variant_index: 0,
            }),
            statement(SemanticStatementKindV1::Deinitialize(ranked_place(2))),
            statement(discriminant_read),
        ])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![
                AccessKindAttr::Write,
                AccessKindAttr::Write,
                AccessKindAttr::Read,
            ]
        );
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn storage_markers_and_nop_are_explicit_zero_effect_statements() {
        let (operations, sources, _) = audit_statements(vec![
            statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(2),
            )),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            )),
            statement(SemanticStatementKindV1::Nop),
        ])
        .unwrap();
        assert!(operations.is_empty());
        assert!(sources.is_empty());

        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(99),
            ))]),
            "a storage statement with an out-of-range local",
        );
    }

    #[test]
    fn explicit_or_dereferenced_unranked_memory_fails_closed() {
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Store(
                SemanticMemoryStoreV1::new(
                    scalar_place(),
                    constant(7),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ),
            ))]),
            "an explicit memory operation without a ranked index projection",
        );

        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    scalar_place(),
                    SemanticRvalueV1::new(
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            dereferenced_place(),
                            SemanticVolatilityV1::Volatile,
                            None,
                        )),
                    ),
                ),
            ))]),
            "a dereferenced memory access without a ranked index projection",
        );
    }

    #[test]
    fn transparent_address_formation_is_zero_effect_and_requires_allocation_provenance() {
        let address_local = SemanticLocalIdV1::from_index(4);
        let source = dereferenced_place();
        for value in [
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: source.clone(),
            },
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: source.clone(),
            },
        ] {
            let address = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(address_local, vec![], POINTER_TYPE).unwrap(),
                SemanticRvalueV1::new(POINTER_TYPE, value),
            )));
            let function = projection_function_with_locals(
                vec![block(31, vec![address], SemanticTerminatorKindV1::Return)],
                vec![
                    local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(21, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                    local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                    local(23, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                    local(24, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                ],
            );
            let contracts = synthetic_local_contracts(&function);
            let (operations, sources, ranked_ir) =
                audit_function_with_local_contracts(&function, &contracts).unwrap();
            assert!(operations.is_empty());
            assert!(sources.is_empty());
            assert!(ranked_ir.is_empty());

            let mut hostile_contracts = synthetic_local_contracts(&function);
            hostile_contracts.allocations[3] = None;
            hostile_contracts.allocation_provenance[3] = None;
            assert!(matches!(
                audit_function_with_local_contracts(&function, &hostile_contracts),
                Err(
                    ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
                        local: 3,
                        projections: 1,
                        ty: 2,
                    }
                )
            ));
        }
    }

    #[test]
    fn unsupported_place_forms_fail_before_a_clean_result() {
        let hostile = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Subslice {
                        from: 0,
                        to: 1,
                        from_end: false,
                    },
                    ARRAY_TYPE,
                )
                .unwrap(),
            ],
            ARRAY_TYPE,
        )
        .unwrap();
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Deinitialize(
                hostile,
            ))]),
            "an indexed place containing a subslice projection",
        );

        let dynamic = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Deinitialize(
                dynamic,
            ))]),
            "a dynamic array index before exact static-extent guard projection",
        );
    }

    #[test]
    fn unreachable_blocks_are_still_audited_for_memory_effects() {
        let function = projection_function(vec![
            block(40, vec![], SemanticTerminatorKindV1::Return),
            block(
                41,
                vec![statement(SemanticStatementKindV1::Store(
                    SemanticMemoryStoreV1::new(
                        scalar_place(),
                        constant(1),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    ),
                ))],
                SemanticTerminatorKindV1::Unreachable,
            ),
        ]);
        assert_unsupported(
            audit_function(&function),
            "an explicit memory operation without a ranked index projection",
        );
    }

    #[test]
    fn unresolved_call_and_drop_effects_fail_before_a_clean_result() {
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let call_error = audit_function(&projection_function(vec![block(
            42,
            vec![],
            SemanticTerminatorKindV1::Call(call),
        )]))
        .unwrap_err();
        assert!(matches!(
            &call_error,
            ProductionRankedProjectionErrorV1::UnresolvedCallableEffect {
                block: 0,
                callee: 0,
                tail: false,
                ..
            }
        ));
        assert!(call_error.to_string().contains(
            "a call terminator before exact callable memory-effect summaries are available",
        ));

        let edge = SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::DropReturn,
            SemanticBlockIdV1::from_index(0),
        );
        let drop_error = audit_function(&projection_function(vec![block(
            43,
            vec![],
            SemanticTerminatorKindV1::Drop {
                place: scalar_place(),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: edge,
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        )]))
        .unwrap_err();
        assert!(matches!(
            &drop_error,
            ProductionRankedProjectionErrorV1::UnresolvedDropEffect {
                block: 0,
                drop_glue: 0,
                ..
            }
        ));
        assert!(drop_error.to_string().contains(
            "a drop terminator before exact drop-glue memory-effect summaries are available",
        ));
    }
