mod conditional_total_read_replay_v1_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    fn ty(index: u32) -> SemanticTypeIdV1 {
        SemanticTypeIdV1::from_index(index)
    }

    fn place(local: u32, index: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(index)).unwrap()
    }

    fn constant(index: u32, value: u64) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(index),
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(u128::from(value), if index == 1 { 4 } else { 8 })
                    .unwrap(),
            ),
        ))
    }

    fn types() -> Vec<SemanticTypeDeclV1> {
        let pointer_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
        );
        let length_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
        );
        let slice_layout = SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::Array {
                stride_bytes: 4,
                count: 0,
            },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap();
        let decl = |tag, layout, shape| {
            let value = SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
                layout,
                shape,
            );
            if tag == 157 || tag == 161 {
                value.with_rustc_abi_properties(
                    SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                        Some(
                            SemanticAbiPointeeInfoV1::new(
                                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                                if tag == 157 { 0 } else { 48 },
                                if tag == 157 { 4 } else { 8 },
                            )
                            .unwrap(),
                        ),
                        None,
                    ),
                )
            } else {
                value
            }
        };
        let pointer = |pointee, metadata| {
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(pointee),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    metadata,
                )
                .unwrap(),
            )
        };
        let aggregate = |fields: Vec<_>| {
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap())
        };
        let enumeration = |variants: Vec<Vec<_>>| SemanticTypeShapeV1::Enum {
            discriminant: ty(1),
            variants: variants
                .into_iter()
                .enumerate()
                .map(|(index, fields)| {
                    SemanticEnumVariantV1::new(
                        index as u128,
                        SemanticAggregateTypeV1::new(fields).unwrap(),
                    )
                })
                .collect(),
        };
        vec![
            unit_type(),
            plain_bit_scalar_type(
                151,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            plain_bit_scalar_type(
                153,
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
            ),
            decl(
                155,
                slice_layout,
                SemanticTypeShapeV1::Slice { element: ty(1) },
            ),
            decl(
                157,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(16),
                    8,
                    SemanticBackendReprV1::ScalarPair {
                        first: pointer_scalar,
                        second: length_scalar,
                    },
                    false,
                )
                .unwrap(),
                pointer(3, SemanticPointerMetadataV1::SliceLength),
            ),
            decl(
                159,
                SemanticTypeLayoutV1::aggregate(
                    Some(48),
                    8,
                    SemanticAggregateLayoutV1::new(vec![0, 16, 24, 32, 40, 48], vec![]).unwrap(),
                )
                .unwrap(),
                aggregate(vec![ty(4), ty(2), ty(2), ty(2), ty(2), ty(0)]),
            ),
            decl(
                161,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(pointer_scalar),
                    false,
                )
                .unwrap(),
                pointer(5, SemanticPointerMetadataV1::None),
            ),
            decl(
                163,
                enum_layout(24, &[vec![], vec![], vec![8, 16]]),
                enumeration(vec![vec![], vec![], vec![ty(2), ty(2)]]),
            ),
            decl(
                165,
                enum_layout(56, &[vec![8], vec![8]]),
                enumeration(vec![vec![ty(5)], vec![ty(7)]]),
            ),
        ]
    }

    fn enum_layout(size: u64, offsets: &[Vec<u64>]) -> SemanticTypeLayoutV1 {
        let variants = offsets
            .iter()
            .enumerate()
            .map(|(index, fields)| {
                SemanticEnumVariantLayoutV1::from_rustc(
                    u32::try_from(index).unwrap(),
                    size,
                    8,
                    SemanticFieldsShapeV1::arbitrary(
                        fields.clone(),
                        (0..u32::try_from(fields.len()).unwrap()).collect(),
                    )
                    .unwrap(),
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    8,
                    0,
                    SemanticAggregateLayoutV1::new(fields.clone(), vec![]).unwrap(),
                )
                .unwrap()
            })
            .collect();
        SemanticTypeLayoutV1::enum_layout(
            size,
            8,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 32, 4),
                        SemanticScalarValidityRangeV1::new(
                            0,
                            u128::try_from(offsets.len() - 1).unwrap(),
                        ),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn abi(inputs: &[u32], output: u32, kernel: bool) -> SemanticFunctionAbiV1 {
        let initialized = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap();
        let reference = |size, align| {
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    true,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                size,
                Some(align),
            )
            .unwrap()
        };
        let direct = |index| {
            SemanticAbiValueV1::new(
                ty(index),
                if index == 4 {
                    SemanticAbiPassModeV1::Pair {
                        first: reference(0, 4),
                        second: initialized,
                    }
                } else if index == 8 {
                    SemanticAbiPassModeV1::Indirect {
                        attributes: SemanticAbiValueAttributesV1::new(
                            SemanticAbiRegularAttributesV1::new(
                                true,
                                Some(SemanticAbiPointerCaptureV1::CapturesNone),
                                true,
                                false,
                                false,
                                true,
                            ),
                            SemanticAbiExtensionV1::None,
                            56,
                            Some(8),
                        )
                        .unwrap(),
                        metadata_attributes: None,
                        on_stack: false,
                    }
                } else {
                    SemanticAbiPassModeV1::Direct(if index == 6 {
                        reference(48, 8)
                    } else {
                        initialized
                    })
                },
            )
        };
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([171; 32]),
            SemanticLayoutIdentityV1::from_sha256([172; 32]),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            u32::try_from(inputs.len()).unwrap(),
            inputs
                .iter()
                .copied()
                .map(|index| SemanticAbiArgumentV1::source(direct(index)))
                .collect(),
            if output == 0 {
                SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore)
            } else {
                direct(output)
            },
        )
        .unwrap()
    }

    fn callable(
        tag: u8,
        inputs: &[u32],
        output: u32,
        operation: SemanticCompilerIntrinsicOperationV1,
    ) -> SemanticCallableDeclV1 {
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([tag + 1; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag + 2; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag + 3; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag + 4; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                abi(inputs, output, false),
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag + 5; 32]),
        }
    }

    fn semantic_owner() -> ProductionSemanticMirOwnerV1 {
        let source = SemanticSourceProvenanceV1::unavailable();
        let edge = |role, block| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
        };
        let call = |callee: u32, arguments, destination, next| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(callee + 1),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        destination,
                        edge(SemanticEdgeRoleV1::CallReturn, next),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        let assign = |destination, index, value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    SemanticRvalueV1::new(ty(index), value),
                )),
            )
        };
        let block = |tag, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        };
        let payload = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), ty(8)).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(5)).unwrap(),
            ],
            ty(5),
        )
        .unwrap();
        let blocks = vec![
            block(
                181,
                vec![],
                call(
                    0,
                    vec![
                        SemanticOperandV1::Copy(place(1, 4)),
                        constant(2, 0),
                        constant(2, 2),
                        constant(2, 4),
                        constant(2, 4),
                    ],
                    place(4, 8),
                    1,
                ),
            ),
            block(
                182,
                vec![assign(
                    place(5, 1),
                    1,
                    SemanticRvalueKindV1::Discriminant(place(4, 8)),
                )],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(5, 1)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                    )
                    .unwrap(),
                },
            ),
            block(
                183,
                vec![
                    assign(
                        place(6, 5),
                        5,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(payload)),
                    ),
                    assign(
                        place(7, 6),
                        6,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(6, 5),
                        },
                    ),
                ],
                call(
                    1,
                    vec![
                        SemanticOperandV1::Copy(place(7, 6)),
                        SemanticOperandV1::Copy(place(2, 2)),
                        constant(2, 0),
                        constant(1, 11),
                    ],
                    place(8, 1),
                    3,
                ),
            ),
            block(
                184,
                vec![assign(
                    place(10, 6),
                    6,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(6, 5),
                    },
                )],
                call(
                    1,
                    vec![
                        SemanticOperandV1::Copy(place(10, 6)),
                        SemanticOperandV1::Copy(place(2, 2)),
                        SemanticOperandV1::Copy(place(3, 2)),
                        constant(1, 29),
                    ],
                    place(9, 1),
                    4,
                ),
            ),
            block(185, vec![], SemanticTerminatorKindV1::Return),
            block(186, vec![], SemanticTerminatorKindV1::Return),
        ];
        let locals = [0, 4, 2, 2, 8, 1, 5, 6, 1, 1, 6]
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([190 + u8::try_from(index).unwrap(); 32]),
                    ty(kind),
                    match index {
                        0 => SemanticLocalRoleV1::Return,
                        1..=3 => SemanticLocalRoleV1::Argument(u32::try_from(index - 1).unwrap()),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                    source,
                )
            })
            .collect();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([201; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([202; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([203; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([204; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([205; 32]),
            source,
            abi(&[4, 2, 2], 0, true),
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"conditional_total_read_replay".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([206; 32]),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        None,
                    )
                    .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ));
        let callables = vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            callable(
                211,
                &[4, 2, 2, 2, 2],
                8,
                SemanticCompilerIntrinsicOperationV1::StridedReadView2DFromSharedSlice {
                    result: ty(8),
                    view: ty(5),
                    error: ty(7),
                    element: ty(1),
                },
            ),
            callable(
                221,
                &[6, 2, 2, 1],
                1,
                SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr {
                    view: ty(5),
                    element: ty(1),
                },
            ),
        ];
        let semantic = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([231; 32])),
            types(),
            vec![],
            vec![],
            vec![],
            vec![function],
            callables,
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap()
    }

    fn owner() -> ProductionSemanticKirOwnerV1 {
        ProductionSemanticKirOwnerV1::try_lower(
            semantic_owner(),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap()
    }

    fn guarded_loads(owner: &ProductionSemanticKirOwnerV1) -> Vec<(usize, usize, Operation)> {
        owner.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(block, body)| {
                body.operations
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| matches!(value.kind, OperationKind::GuardedLoad { .. }))
                    .map(move |(operation, value)| (block, operation, value.clone()))
            })
            .collect()
    }

    #[test]
    fn conditional_total_read_replay_preserves_two_distinct_genuine_calls() {
        // An admitted semantic fixture exercises the actual lower/replay path;
        // it does not pretend to be a compiler-authenticated source artifact.
        let owner = owner();
        owner.verify_equivalence().unwrap();
        let loads = guarded_loads(&owner);
        assert_eq!(loads.len(), 2);
        let OperationKind::GuardedLoad {
            pointer: p0,
            predicate: g0,
            fallback: f0,
            ..
        } = loads[0].2.kind
        else {
            unreachable!()
        };
        let OperationKind::GuardedLoad {
            pointer: p1,
            predicate: g1,
            fallback: f1,
            ..
        } = loads[1].2.kind
        else {
            unreachable!()
        };
        assert_ne!(p0, p1);
        assert_ne!(g0, g1);
        assert_ne!(f0, f1);
    }

    #[test]
    fn conditional_total_read_replay_rejects_guard_address_and_fallback_mutants() {
        for mutation in 0..5 {
            let mut owner = owner();
            let loads = guarded_loads(&owner);
            let OperationKind::GuardedLoad {
                pointer: first_pointer,
                predicate: first_predicate,
                fallback: first_fallback,
                ..
            } = loads[0].2.kind
            else {
                unreachable!()
            };
            let (block, operation, _) = &loads[1];
            let RetainedProductionKirModuleV1::Legacy(module) = &mut owner.module else {
                panic!("fixture must retain replayable legacy module")
            };
            let body = module.functions[0].body.as_mut().unwrap();
            if mutation < 2 {
                let OperationKind::GuardedLoad { predicate, .. } =
                    body.blocks[*block].operations[*operation].kind
                else {
                    unreachable!()
                };
                let definition = body
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|op| op.results.iter().any(|result| result.id == predicate))
                    .unwrap();
                definition.kind = OperationKind::Constant(Constant::Bool(mutation == 0));
            } else {
                let OperationKind::GuardedLoad {
                    pointer,
                    predicate,
                    fallback,
                    ..
                } = &mut body.blocks[*block].operations[*operation].kind
                else {
                    unreachable!()
                };
                match mutation {
                    2 => *predicate = first_predicate,
                    3 => *pointer = first_pointer,
                    4 => *fallback = first_fallback,
                    _ => unreachable!(),
                }
            }
            verify_module(owner.module())
                .expect("mutant remains well-typed and dominance-valid KIR");
            assert!(
                matches!(
                    owner.verify_equivalence(),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn conditional_total_read_witness_requires_source_site_and_single_consumer() {
        let owner = owner();
        let semantic = owner.semantic_ssa.source_owner().semantic();
        let body = owner.module().functions[0].body.as_ref().unwrap();
        let index = build_kir_correlation_index(
            body,
            10_000,
            &mut UnsupportedIndexCorrelationBudgetV1 { remaining: 100_000 },
        )
        .unwrap();
        assert_eq!(index.memory_consumers.len(), 2);
        let consumer = index.memory_consumers[0];
        let operation = index.operations[&consumer.location];
        let site = SemanticAccessSiteV1 {
            block: 2,
            statement: None,
            ordinal: 0,
        };
        let function = SemanticFunctionIdV1::from_index(0);
        assert!(
            authenticate_conditional_total_read_v1(
                Some(semantic),
                function,
                site,
                site,
                consumer,
                operation,
                1
            )
            .is_some()
        );
        assert!(
            authenticate_conditional_total_read_v1(
                None, function, site, site, consumer, operation, 1
            )
            .is_none()
        );
        for count in [0, 2] {
            assert!(
                authenticate_conditional_total_read_v1(
                    Some(semantic),
                    function,
                    site,
                    site,
                    consumer,
                    operation,
                    count
                )
                .is_none()
            );
        }
        let constructor_site = SemanticAccessSiteV1 { block: 0, ..site };
        assert!(
            authenticate_conditional_total_read_v1(
                Some(semantic),
                function,
                constructor_site,
                constructor_site,
                consumer,
                operation,
                1
            )
            .is_none()
        );
        let other_call_site = SemanticAccessSiteV1 { block: 3, ..site };
        assert!(
            authenticate_conditional_total_read_v1(
                Some(semantic),
                function,
                site,
                other_call_site,
                consumer,
                operation,
                1
            )
            .is_none()
        );
        let mut unguarded = operation.clone();
        let OperationKind::GuardedLoad {
            pointer, access, ..
        } = unguarded.kind
        else {
            unreachable!()
        };
        unguarded.kind = OperationKind::Load { pointer, access };
        assert!(
            authenticate_conditional_total_read_v1(
                Some(semantic),
                function,
                site,
                site,
                consumer,
                &unguarded,
                1
            )
            .is_none()
        );
        assert!(
            authenticate_conditional_total_read_v1(
                Some(semantic),
                function,
                site,
                site,
                KirMemoryConsumerV1 {
                    pointer: ValueId(u32::MAX),
                    ..consumer
                },
                operation,
                1
            )
            .is_none()
        );
    }

    #[test]
    fn conditional_total_read_counts_actual_consumers_without_coalescing_calls() {
        let owner = owner();
        let loads = guarded_loads(&owner);
        let mut module = owner.module().clone();
        let body = module.functions[0].body.as_mut().unwrap();
        let first_block = body.blocks[loads[0].0].id;
        let mut duplicate = loads[0].2.clone();
        duplicate.results[0].id = ValueId(u32::MAX - 1);
        body.blocks[loads[0].0].operations.push(duplicate);
        verify_module(&module)
            .expect("extra actual read remains valid KIR, but is not source replay");

        let body = module.functions[0].body.as_ref().unwrap();
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 100_000 };
        let index = build_kir_correlation_index(body, 10_000, &mut budget).unwrap();
        assert_eq!(index.memory_consumers.len(), 3);
        let sites = index
            .memory_consumers
            .iter()
            .map(|consumer| {
                let site = SemanticAccessSiteV1 {
                    block: if consumer.location.block == first_block {
                        2
                    } else {
                        3
                    },
                    statement: None,
                    ordinal: 0,
                };
                ((consumer.location, consumer.operation_access_ordinal), site)
            })
            .collect();
        let counts = public_effect_counts_by_call_v1(&index, &sites, &mut budget).unwrap();
        assert_eq!(counts, BTreeMap::from([((2, None), 2), ((3, None), 1)]));
        let semantic = owner.semantic_ssa.source_owner().semantic();
        for consumer in &index.memory_consumers {
            let site = sites[&(consumer.location, consumer.operation_access_ordinal)];
            let witness = authenticate_conditional_total_read_v1(
                Some(semantic),
                SemanticFunctionIdV1::from_index(0),
                site,
                site,
                *consumer,
                index.operations[&consumer.location],
                counts[&(site.block, site.statement)],
            );
            assert_eq!(witness.is_some(), site.block == 3);
        }
    }
}
