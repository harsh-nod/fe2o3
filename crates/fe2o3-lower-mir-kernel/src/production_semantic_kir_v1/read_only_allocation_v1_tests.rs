mod read_only_allocation_v1_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    include!("read_only_allocation_bounds_v1_tests.rs");

    fn ty(index: u32) -> SemanticTypeIdV1 {
        SemanticTypeIdV1::from_index(index)
    }
    fn place(local: u32, kind: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
    }

    pub(super) fn types(float: bool) -> Vec<SemanticTypeDeclV1> {
        let raw = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
        );
        let reference = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
        );
        let length = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
        );
        let decl = |tag, layout, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
                layout,
                shape,
            )
        };
        let pointer = |tag, pointee, kind, mutability, scalar| {
            decl(
                tag,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::Scalar(scalar),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        ty(pointee),
                        kind,
                        mutability,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
        };
        let aggregate = |tag, pointer| {
            decl(
                tag,
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    16,
                    8,
                    SemanticFieldsShapeV1::arbitrary(
                        if pointer == 3 {
                            vec![0, 8, 16]
                        } else {
                            vec![0, 8]
                        },
                        if pointer == 3 {
                            vec![0, 1, 2]
                        } else {
                            vec![0, 1]
                        },
                    )
                    .unwrap(),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::ScalarPair {
                        first: raw,
                        second: length,
                    },
                    None,
                    false,
                    None,
                    8,
                    0,
                    SemanticTypeLayoutDetailsV1::Aggregate(
                        SemanticAggregateLayoutV1::new(
                            if pointer == 3 {
                                vec![0, 8, 16]
                            } else {
                                vec![0, 8]
                            },
                            vec![],
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(if pointer == 3 {
                        vec![ty(pointer), ty(2), ty(0)]
                    } else {
                        vec![ty(pointer), ty(2)]
                    })
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                    ),
                    None,
                ),
            )
        };
        vec![
            unit_type(),
            plain_bit_scalar_type(
                141,
                if float {
                    SemanticBackendPrimitiveV1::float(32, 4)
                } else {
                    SemanticBackendPrimitiveV1::integer(false, 16, 2)
                },
                if float {
                    SemanticScalarTypeV1::Float { bits: 32 }
                } else {
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 16,
                    }
                },
            ),
            plain_bit_scalar_type(
                143,
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
            ),
            pointer(
                145,
                1,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                raw,
            ),
            aggregate(147, 3),
            pointer(
                149,
                1,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                raw,
            ),
            aggregate(151, 5),
            pointer(
                153,
                6,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                reference,
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            16,
                            8,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        ]
    }

    fn abi(inputs: &[u32], output: u32, kernel: bool) -> SemanticFunctionAbiV1 {
        let initialized = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap();
        let value = |index| {
            SemanticAbiValueV1::new(
                ty(index),
                match index {
                    0 => SemanticAbiPassModeV1::Ignore,
                    4 | 6 => SemanticAbiPassModeV1::Pair {
                        first: initialized,
                        second: initialized,
                    },
                    7 => SemanticAbiPassModeV1::Direct(
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
                            16,
                            Some(8),
                        )
                        .unwrap(),
                    ),
                    _ => SemanticAbiPassModeV1::Direct(initialized),
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
            inputs.len() as u32,
            inputs
                .iter()
                .copied()
                .map(|index| SemanticAbiArgumentV1::source(value(index)))
                .collect(),
            value(output),
        )
        .unwrap()
        .with_source_argument_ownership(
            inputs
                .iter()
                .map(|&index| match index {
                    4 => SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                    7 => SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    _ => SemanticSourceArgumentOwnershipV1::ByValue,
                })
                .collect(),
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

    #[derive(Clone, Copy)]
    enum ConstructorSourceV1 {
        ExclusiveArgument,
        MovedTemporary,
        NonexclusiveArgument,
    }

    fn request(float: bool, copy_constructor: bool) -> InertSemanticMirRequestV1 {
        request_with_source(
            float,
            copy_constructor,
            ConstructorSourceV1::ExclusiveArgument,
        )
    }

    fn request_with_source(
        float: bool,
        copy_constructor: bool,
        constructor_source: ConstructorSourceV1,
    ) -> InertSemanticMirRequestV1 {
        let temporary = matches!(constructor_source, ConstructorSourceV1::MovedTemporary);
        let source = SemanticSourceProvenanceV1::unavailable();
        let edge = |next| {
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(next),
            )
        };
        let call = |callee, arguments, destination, next| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(callee),
                    arguments,
                    Some(SemanticCallDestinationV1::new(destination, edge(next))),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
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
        let borrow = |local| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(local, 7),
                    SemanticRvalueV1::new(
                        ty(7),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(4, 6),
                        },
                    ),
                )),
            )
        };
        let fallback = |value: u16| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty(1),
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(
                        if float {
                            u128::from(f32::from(value).to_bits())
                        } else {
                            u128::from(value)
                        },
                        if float { 4 } else { 2 },
                    )
                    .unwrap(),
                ),
            ))
        };
        let blocks = vec![
            block(
                181,
                if temporary {
                    vec![SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            place(11, 4),
                            SemanticRvalueV1::new(
                                ty(4),
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, 4))),
                            ),
                        )),
                    )]
                } else {
                    vec![]
                },
                call(
                    1,
                    vec![if copy_constructor {
                        SemanticOperandV1::Copy(place(if temporary { 11 } else { 1 }, 4))
                    } else {
                        SemanticOperandV1::Move(place(if temporary { 11 } else { 1 }, 4))
                    }],
                    place(4, 6),
                    1,
                ),
            ),
            block(
                182,
                vec![borrow(5)],
                call(
                    2,
                    vec![SemanticOperandV1::Copy(place(5, 7))],
                    place(6, 2),
                    2,
                ),
            ),
            block(
                183,
                vec![borrow(7)],
                call(
                    3,
                    vec![
                        SemanticOperandV1::Copy(place(7, 7)),
                        SemanticOperandV1::Copy(place(2, 2)),
                        fallback(11),
                    ],
                    place(8, 1),
                    3,
                ),
            ),
            block(
                184,
                vec![borrow(9)],
                call(
                    3,
                    vec![
                        SemanticOperandV1::Copy(place(9, 7)),
                        SemanticOperandV1::Copy(place(3, 2)),
                        fallback(29),
                    ],
                    place(10, 1),
                    4,
                ),
            ),
            block(185, vec![], SemanticTerminatorKindV1::Return),
        ];
        let locals = [0, 4, 2, 2, 6, 7, 2, 7, 1, 7, 1, 4]
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([190 + index as u8; 32]),
                    ty(kind),
                    match index {
                        0 => SemanticLocalRoleV1::Return,
                        1..=3 => SemanticLocalRoleV1::Argument(index as u32 - 1),
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
            if matches!(
                constructor_source,
                ConstructorSourceV1::NonexclusiveArgument
            ) {
                abi(&[4, 2, 2], 0, true)
                    .with_source_argument_ownership(vec![
                        SemanticSourceArgumentOwnershipV1::ByValue;
                        3
                    ])
                    .unwrap()
            } else {
                abi(&[4, 2, 2], 0, true)
            },
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"read_only_allocation_replay".to_vec()).unwrap(),
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
        InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([231; 32])),
            types(float),
            vec![],
            vec![],
            vec![],
            vec![function],
            vec![
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
                callable(
                    211,
                    &[4],
                    6,
                    SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
                        slice: ty(4),
                        view: ty(6),
                        element: ty(1),
                    },
                ),
                callable(
                    221,
                    &[7],
                    2,
                    SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { view: ty(6) },
                ),
                callable(
                    231,
                    &[7, 2, 1],
                    1,
                    SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr {
                        view: ty(6),
                        element: ty(1),
                    },
                ),
            ],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
    }

    fn owner(float: bool) -> ProductionSemanticKirOwnerV1 {
        let semantic = request(float, false)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let source = ProductionSemanticMirOwnerV1::try_new(
            semantic,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        ProductionSemanticKirOwnerV1::try_lower(source, ProductionSemanticKirLimitsV1::default())
            .unwrap()
    }

    fn loads(owner: &ProductionSemanticKirOwnerV1) -> Vec<(usize, usize, Operation)> {
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
                    .filter(|(_, op)| matches!(op.kind, OperationKind::GuardedLoad { .. }))
                    .map(move |(index, op)| (block, index, op.clone()))
            })
            .collect()
    }

    #[test]
    fn read_only_allocation_owner_preserves_rw_root_and_exact_total_reads() {
        for float in [false, true] {
            // These are admitted semantic owner fixtures, not fabricated source admission.
            let owner = owner(float);
            owner.verify_equivalence().unwrap();
            let function = &owner.module().functions[0];
            assert_eq!(
                function.signature.parameters[0],
                Type::slice(
                    Type::Scalar(if float {
                        ScalarType::F32
                    } else {
                        ScalarType::U16
                    }),
                    AddressSpace::Global,
                    AccessMode::ReadWrite
                )
            );
            let operations: Vec<_> = function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .collect();
            assert_eq!(loads(&owner).len(), 2);
            assert_eq!(
                operations
                    .iter()
                    .filter(|op| matches!(op.kind, OperationKind::SliceLength { .. }))
                    .count(),
                1
            );
            assert_eq!(
                operations
                    .iter()
                    .filter(|op| matches!(
                        op.kind,
                        OperationKind::Cast {
                            kind: CastKind::RestrictPointerAccess,
                            ..
                        }
                    ))
                    .count(),
                1
            );
            assert!(!operations.iter().any(|op| matches!(
                op.kind,
                OperationKind::Load { .. } | OperationKind::Store { .. }
            )));
            for (_, _, operation) in loads(&owner) {
                let OperationKind::GuardedLoad {
                    pointer, predicate, ..
                } = operation.kind
                else {
                    unreachable!()
                };
                let definition = |id| {
                    operations
                        .iter()
                        .find(|op| op.results.iter().any(|result| result.id == id))
                        .unwrap()
                };
                let OperationKind::GetElementPointer { offset, .. } = definition(pointer).kind
                else {
                    panic!("exact guarded address")
                };
                let OperationKind::Select {
                    condition,
                    false_value,
                    ..
                } = definition(offset).kind
                else {
                    panic!("safe index selection")
                };
                assert_eq!(condition, predicate);
                assert!(matches!(
                    definition(false_value).kind,
                    OperationKind::Constant(Constant::Index(0))
                ));
                assert!(matches!(
                    definition(predicate).kind,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn read_only_allocation_replay_rejects_guard_extent_address_and_fallback_mutants() {
        for mutation in 0..7 {
            let mut owner = owner(false);
            let loads = loads(&owner);
            let OperationKind::GuardedLoad {
                pointer: p0,
                predicate: g0,
                fallback: f0,
                ..
            } = loads[0].2.kind
            else {
                unreachable!()
            };
            let (block, index, _) = loads[1];
            let RetainedProductionKirModuleV1::Legacy(module) = &mut owner.module else {
                panic!("legacy fixture")
            };
            let body = module.functions[0].body.as_mut().unwrap();
            if mutation < 2 {
                let OperationKind::GuardedLoad { predicate, .. } =
                    body.blocks[block].operations[index].kind
                else {
                    unreachable!()
                };
                body.blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|op| op.results.iter().any(|value| value.id == predicate))
                    .unwrap()
                    .kind = OperationKind::Constant(Constant::Bool(mutation == 0));
            } else if mutation == 5 {
                body.blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|op| matches!(op.kind, OperationKind::SliceLength { .. }))
                    .unwrap()
                    .kind = OperationKind::Constant(Constant::Index(0));
            } else if mutation == 6 {
                let OperationKind::GuardedLoad {
                    pointer, access, ..
                } = body.blocks[block].operations[index].kind
                else {
                    unreachable!()
                };
                body.blocks[block].operations[index].kind = OperationKind::Load { pointer, access };
            } else {
                let OperationKind::GuardedLoad {
                    pointer,
                    predicate,
                    fallback,
                    ..
                } = &mut body.blocks[block].operations[index].kind
                else {
                    unreachable!()
                };
                match mutation {
                    2 => *predicate = g0,
                    3 => *pointer = p0,
                    4 => *fallback = f0,
                    _ => unreachable!(),
                }
            }
            verify_module(owner.module()).expect("mutant remains well-typed KIR");
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
    fn read_only_allocation_consumes_exact_optimized_argument_copy() {
        for float in [false, true] {
            let semantic = request(float, true)
                .admit(SemanticMirLimitsV1::default())
                .unwrap();
            let call = match semantic.functions()[0].blocks()[0].terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => call,
                _ => unreachable!(),
            };
            assert!(matches!(call.arguments()[0], SemanticOperandV1::Copy(_)));
            let source = ProductionSemanticMirOwnerV1::try_new(
                semantic,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap();
            let owner = ProductionSemanticKirOwnerV1::try_lower(
                source,
                ProductionSemanticKirLimitsV1::default(),
            )
            .unwrap();
            owner.verify_equivalence().unwrap();
            assert_eq!(loads(&owner).len(), 2);
            assert_eq!(
                owner.module().functions[0].signature.parameters[0],
                Type::slice(
                    Type::Scalar(if float {
                        ScalarType::F32
                    } else {
                        ScalarType::U16
                    }),
                    AddressSpace::Global,
                    AccessMode::ReadWrite
                )
            );
        }
    }

    #[test]
    fn read_only_allocation_rejects_copy_without_original_exclusive_argument_custody() {
        for constructor in [
            ConstructorSourceV1::MovedTemporary,
            ConstructorSourceV1::NonexclusiveArgument,
        ] {
            let semantic = request_with_source(false, true, constructor)
                .admit(SemanticMirLimitsV1::default())
                .unwrap();
            let source = ProductionSemanticMirOwnerV1::try_new(
                semantic,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap();
            assert!(
                ProductionSemanticKirOwnerV1::try_lower(
                    source,
                    ProductionSemanticKirLimitsV1::default()
                )
                .is_err()
            );
        }
        // The previous consuming Move transport remains accepted.
        let semantic = request_with_source(false, false, ConstructorSourceV1::MovedTemporary)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let source = ProductionSemanticMirOwnerV1::try_new(
            semantic,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        ProductionSemanticKirOwnerV1::try_lower(source, ProductionSemanticKirLimitsV1::default())
            .unwrap()
            .verify_equivalence()
            .unwrap();
    }

    #[test]
    fn read_only_allocation_descriptor_does_not_invent_a_length_type() {
        let mut types = types(false);
        assert_eq!(
            read_only_allocation_fields_v1(&types, ty(6)),
            Some((ty(1), ty(2)))
        );
        let original = types[6].clone();
        for fields in [
            vec![ty(5), ty(1)],
            vec![ty(3), ty(2)],
            vec![ty(5), ty(2), ty(2)],
        ] {
            types[6] = SemanticTypeDeclV1::new(
                original.identity(),
                original.layout_identity(),
                original.layout().clone(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
            );
            assert!(read_only_allocation_fields_v1(&types, ty(6)).is_none());
        }
    }
}
