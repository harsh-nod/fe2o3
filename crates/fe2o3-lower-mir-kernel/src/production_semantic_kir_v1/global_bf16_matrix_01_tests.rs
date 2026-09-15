mod global_bf16_matrix_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    include!("global_bf16_matrix_01/source_inputs_tests.rs");

    fn id(index: u32) -> SemanticTypeIdV1 {
        SemanticTypeIdV1::from_index(index)
    }

    struct Fixture {
        types: Vec<SemanticTypeDeclV1>,
        callables: Vec<SemanticCallableDeclV1>,
        function: SemanticFunctionDeclV1,
        context: RootKernelContextLoweringV1,
        contract: SemanticGlobalBf16MatrixLoadV1,
        matrix: SemanticValueBindingV1,
        lane: SemanticValueBindingV1,
        call: SemanticDirectCallV1,
    }

    fn fixture(role: SemanticMfmaOperandRoleV1) -> Fixture {
        let source = SemanticSourceProvenanceV1::unavailable();
        let provenance = SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(0),
            SemanticKernelBindingIdentityV1::from_sha256([6; 32]),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([4; 32]),
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([2; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([3; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([7; 32]),
        )
        .unwrap();
        let context = RootKernelContextLoweringV1 {
            entry_transfer: None,
            selected_root: SemanticFunctionIdV1::from_index(0),
            semantic_type: id(0),
            context_type: KernelContextTypeV1::new("global_bf16", [1; 32], [2; 32], [3; 32]),
            source: KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
        };
        let decl = |tag, layout, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                layout,
                shape,
            )
        };
        let scalar = |tag, bits, bytes| {
            plain_bit_scalar_type(
                tag,
                SemanticBackendPrimitiveV1::integer(false, bits, bytes),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits,
                },
            )
        };
        let aggregate = |tag, fields, offsets, bytes, align| {
            decl(
                tag,
                SemanticTypeLayoutV1::aggregate(
                    Some(bytes),
                    align,
                    SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
            )
        };
        let reference = |tag, pointee, metadata, bytes| {
            decl(
                tag,
                SemanticTypeLayoutV1::new(Some(bytes), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        metadata,
                    )
                    .unwrap(),
                ),
            )
        };
        let types = vec![
            unit_type(),
            scalar(21, 16, 2),
            scalar(22, 64, 8),
            scalar(23, 32, 4),
            decl(
                24,
                SemanticTypeLayoutV1::new(None, 2).unwrap(),
                SemanticTypeShapeV1::Slice { element: id(1) },
            ),
            reference(25, id(4), SemanticPointerMetadataV1::SliceLength, 16),
            aggregate(26, vec![id(5), id(0)], vec![0, 16], 16, 8),
            reference(27, id(6), SemanticPointerMetadataV1::None, 8),
            aggregate(
                28,
                vec![id(7), id(2), id(2), id(2), id(2), id(0), id(0), id(0)],
                vec![0, 8, 16, 24, 32, 40, 40, 40],
                40,
                8,
            ),
            reference(29, id(8), SemanticPointerMetadataV1::None, 8),
            aggregate(30, vec![id(3), id(0), id(0), id(0)], vec![0, 4, 4, 4], 4, 4),
            reference(31, id(10), SemanticPointerMetadataV1::None, 8),
            aggregate(32, vec![id(14), id(0), id(0)], vec![0, 8, 8], 8, 2),
            aggregate(33, vec![id(1)], vec![0], 2, 2),
            decl(
                34,
                SemanticTypeLayoutV1::new(Some(8), 2).unwrap(),
                SemanticTypeShapeV1::Array {
                    element: id(13),
                    length: 4,
                },
            ),
        ];
        let contract = SemanticGlobalBf16MatrixLoadV1::new(
            SemanticGlobalBf16MatrixTypesV1 {
                matrix: id(8),
                global: id(6),
                lane: id(10),
                fragment: id(12),
                element: id(1),
                index: id(2),
            },
            operand_contract(role),
            SemanticTypeIdentityV1::from_sha256([60; 32]),
            SemanticTypeIdentityV1::from_sha256([61; 32]),
            provenance,
            SemanticFunctionIdentityV1::from_sha256([62; 32]),
        )
        .unwrap();
        let original_owner = noop_semantic_owner(&["global_bf16"]);
        let original = &original_owner.semantic().functions()[0];
        let function = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            source,
            original.abi().clone(),
            [id(0), id(9), id(11), id(2), id(2), id(12)]
                .into_iter()
                .enumerate()
                .map(|(index, ty)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([100 + index as u8; 32]),
                        ty,
                        if index == 0 {
                            SemanticLocalRoleV1::Return
                        } else {
                            SemanticLocalRoleV1::Temporary
                        },
                        source,
                    )
                })
                .collect(),
            original.entry(),
            original.blocks().to_vec(),
        )
        .unwrap();
        let callable = |tag, operation| SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                source,
                if matches!(
                    operation,
                    SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { .. }
                ) {
                    SemanticFunctionAbiV1::new(
                        SemanticAbiIdentityV1::from_sha256([80; 32]),
                        SemanticLayoutIdentityV1::from_sha256([81; 32]),
                        SemanticCanonAbiV1::Rust,
                        false,
                        false,
                        [id(9), id(11), id(2), id(2)]
                            .into_iter()
                            .map(|ty| SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore))
                            .collect(),
                        SemanticAbiValueV1::new(id(12), SemanticAbiPassModeV1::Ignore),
                    )
                    .unwrap()
                    .with_source_argument_ownership(vec![
                        SemanticSourceArgumentOwnershipV1::SharedBorrow,
                        SemanticSourceArgumentOwnershipV1::SharedBorrow,
                        SemanticSourceArgumentOwnershipV1::ByValue,
                        SemanticSourceArgumentOwnershipV1::ByValue,
                    ])
                    .unwrap()
                } else {
                    original.abi().clone()
                },
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
        };
        let callables = vec![
            callable(
                61,
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                    context: id(0),
                    physical: id(5),
                    view: id(6),
                    element: id(1),
                    contract: contract.memory(),
                    provenance,
                    source_identity: SemanticFunctionIdentityV1::from_sha256([61; 32]),
                },
            ),
            callable(
                62,
                SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract },
            ),
        ];
        let matrix = SemanticValueBindingV1::Aggregate(vec![
            SemanticValueBindingV1::GlobalCapability {
                value: ValueId(40),
                semantic_view: id(6),
                element: id(1),
                contract: contract.memory(),
                provenance,
                capability: GlobalCapabilityTypeV1::read_only(
                    Type::Scalar(ScalarType::U16),
                    context.context_type.clone(),
                ),
            },
            value(2, Type::Scalar(ScalarType::U64)),
            value(3, Type::Scalar(ScalarType::U64)),
            value(4, Type::Scalar(ScalarType::U64)),
            value(5, Type::Scalar(ScalarType::U64)),
            SemanticValueBindingV1::Unit,
            SemanticValueBindingV1::Unit,
            SemanticValueBindingV1::Unit,
        ]);
        let lane = SemanticValueBindingV1::WaveLane {
            value: ValueId(1),
            wave: SemanticCurrentWaveV1::new(64),
        };
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![
                SemanticOperandV1::Copy(place(1, id(9))),
                SemanticOperandV1::Copy(place(2, id(11))),
                SemanticOperandV1::Copy(place(3, id(2))),
                SemanticOperandV1::Copy(place(4, id(2))),
            ],
            Some(SemanticCallDestinationV1::new(
                place(5, id(12)),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(0),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        Fixture {
            types,
            callables,
            function,
            context,
            contract,
            matrix,
            lane,
            call,
        }
    }

    fn value(index: u32, ty: Type) -> SemanticValueBindingV1 {
        SemanticValueBindingV1::Value {
            id: ValueId(index),
            ty,
        }
    }

    #[test]
    fn global_bf16_live_capture_requires_actual_bind_and_live_shared_owner() {
        // Graph component fixture only; it does not issue a production owner.
        for mutation in 0..3 {
            let f = fixture(SemanticMfmaOperandRoleV1::A);
            let source = SemanticSourceProvenanceV1::unavailable();
            let place = |local, ty| {
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], id(ty)).unwrap()
            };
            let statement = |destination: SemanticPlaceV1, kind| {
                SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        destination.clone(),
                        SemanticRvalueV1::new(destination.ty(), kind),
                    )),
                )
            };
            let edge = |to| {
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(to),
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
            let bind = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(place(6, 6), edge(1))),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            let read = SemanticDirectCallV1::new_callable(
                f.call.callee(),
                f.call.arguments().to_vec(),
                Some(SemanticCallDestinationV1::new(place(5, 12), edge(2))),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            let mut fields = vec![SemanticOperandV1::Copy(place(7, 7))];
            fields.extend((0..4).map(|_| {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    id(2),
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(16, 8).unwrap()),
                ))
            }));
            fields.extend((0..3).map(|_| {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    id(0),
                    SemanticConstantValueV1::ZeroSized,
                ))
            }));
            let mut statements = vec![
                statement(
                    place(7, 7),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(6, 6),
                    },
                ),
                statement(
                    place(8, 8),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, fields)
                            .unwrap(),
                    ),
                ),
                statement(
                    place(1, 9),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(8, 8),
                    },
                ),
            ];
            if mutation == 1 {
                statements.push(SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(6)),
                ));
            }
            let function = SemanticFunctionDeclV1::new(
                f.function.identity(),
                f.function.role(),
                f.function.item_definition_identity(),
                f.function.monomorphization_identity(),
                f.function.generic_type_arguments_identity(),
                f.function.const_generic_arguments_identity(),
                source,
                f.function.abi().clone(),
                [0, 9, 11, 2, 2, 12, 6, 7, 8]
                    .into_iter()
                    .enumerate()
                    .map(|(i, ty)| {
                        SemanticLocalDeclV1::new(
                            SemanticLocalIdentityV1::from_sha256([120 + i as u8; 32]),
                            id(ty),
                            match i {
                                0 => SemanticLocalRoleV1::Return,
                                2..=4 => SemanticLocalRoleV1::Argument((i - 2) as u32),
                                _ => SemanticLocalRoleV1::Temporary,
                            },
                            source,
                        )
                    })
                    .collect(),
                SemanticBlockIdV1::from_index(0),
                vec![
                    block(90, vec![], SemanticTerminatorKindV1::Call(bind)),
                    block(91, statements, SemanticTerminatorKindV1::Call(read)),
                    block(92, vec![], SemanticTerminatorKindV1::Return),
                ],
            )
            .unwrap();
            let mut callables = f.callables.clone();
            if mutation == 2 {
                let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callables[0]
                else {
                    unreachable!()
                };
                *operation =
                    SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: id(6) };
            }
            let plan = plan_semantic_function_ssa_with_module_v1(
                SemanticFunctionIdV1::from_index(0),
                &function,
                &f.types,
                &callables,
                ProductionSemanticSsaLimitsV1::default(),
            )
            .unwrap();
            let result = global_bf16_live_v1::resolve_component(
                &f.types,
                &callables,
                &function,
                plan.plan(),
                1,
                &f.call.arguments()[0],
                f.contract,
            );
            if mutation == 0 {
                result.unwrap();
            } else {
                assert!(result.is_err(), "accepted mutation {mutation}");
            }
        }
    }

    fn lower(
        f: &Fixture,
    ) -> Result<(SemanticValueBindingV1, Vec<Operation>), ProductionSemanticKirErrorV1> {
        let mut lower = SemanticFunctionLoweringV1::new(
            &f.types,
            &f.callables,
            &f.function,
            SemanticParameterBindingsV1 {
                declarations: &[],
                values: &[],
                types: &[],
                local_bindings: None,
            },
            None,
            None,
            BTreeSet::new(),
            1,
            false,
            2048,
        )?;
        lower.kernel_context = Some(&f.context);
        lower.locals[1] = Some(f.matrix.clone());
        lower.locals[2] = Some(f.lane.clone());
        lower.locals[3] = Some(value(6, Type::INDEX));
        lower.locals[4] = Some(value(7, Type::INDEX));
        lower.next_value = 100;
        let mut operations = vec![];
        let result = lower.lower_global_bf16_matrix_load_v1(
            SemanticBlockIdV1::from_index(0),
            &f.call,
            &mut operations,
            f.contract,
            SemanticFunctionIdentityV1::from_sha256([62; 32]),
        )?;
        Ok((result, operations))
    }

    // Emitter fixture, not a production source/target authentication claim.
    fn module(f: &Fixture, operations: Vec<Operation>) -> Module {
        let capability = GlobalCapabilityTypeV1::read_only(
            Type::Scalar(ScalarType::U16),
            f.context.context_type.clone(),
        );
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::kernel_context_issue(
                ValueId(20),
                f.context.context_type.clone(),
                f.context.source.clone(),
            ),
            Operation::global_capability_bind(
                ValueId(40),
                capability.clone(),
                ValueId(20),
                ValueId(0),
            ),
            Operation::global_capability_bind(
                ValueId(41),
                capability.clone(),
                ValueId(20),
                ValueId(8),
            ),
        ];
        block.operations.extend(operations);
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("global-bf16-emitter");
        module.functions.push(Function::kernel_entry(
            "global_bf16",
            Signature::new(
                vec![
                    capability.physical_slice_type(),
                    Type::Scalar(ScalarType::U32),
                    Type::Scalar(ScalarType::U64),
                    Type::Scalar(ScalarType::U64),
                    Type::Scalar(ScalarType::U64),
                    Type::Scalar(ScalarType::U64),
                    Type::INDEX,
                    Type::INDEX,
                    capability.physical_slice_type(),
                ],
                vec![],
            ),
            (0..9).map(ValueId).collect(),
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "global_bf16",
            "global_bf16",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        module
    }

    #[test]
    fn global_bf16_a_and_b_emit_four_bound_volatile_zero_filled_bit_loads() {
        for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
            let f = fixture(role);
            let (result, operations) = lower(&f).unwrap();
            assert!(
                matches!(result, SemanticValueBindingV1::MatrixFragment { values, contract, wave, .. }
                if values.len() == 4 && contract == f.contract.operand() && wave == SemanticCurrentWaveV1::new(64))
            );
            let definitions = operations
                .iter()
                .flat_map(|op| op.results.iter().map(move |result| (result.id, &op.kind)))
                .collect::<BTreeMap<_, _>>();
            let mut loads = 0;
            let mut indices = 0;
            let mut bitcasts = 0;
            for op in &operations {
                match &op.kind {
                    OperationKind::GuardedLoad {
                        predicate,
                        fallback,
                        access,
                        ..
                    } => {
                        loads += 1;
                        assert!(access.volatile);
                        assert_eq!(access.address_space, AddressSpace::Global);
                        assert!(matches!(
                            definitions[fallback],
                            OperationKind::Constant(Constant::U16(0))
                        ));
                        assert!(matches!(
                            definitions[predicate],
                            OperationKind::Binary {
                                op: BinaryOp::BitAnd,
                                ..
                            }
                        ));
                    }
                    OperationKind::GlobalCapabilityIndex(index) => {
                        indices += 1;
                        assert_eq!(index.capability, ValueId(40));
                        assert!(index.index_space.is_none());
                    }
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        to,
                        ..
                    } if *to == Type::Scalar(ScalarType::Bf16) => bitcasts += 1,
                    OperationKind::Load { .. } => panic!("matrix load lost its guard"),
                    _ => (),
                }
            }
            assert_eq!((loads, indices, bitcasts), (4, 4, 4));
            verify_module(&module(&f, operations)).unwrap();
        }
    }

    #[test]
    fn global_bf16_emitted_guards_reject_allocation_and_bound_substitution() {
        let f = fixture(SemanticMfmaOperandRoleV1::A);
        for changed_allocation in [false, true] {
            let (_, mut operations) = lower(&f).unwrap();
            if changed_allocation {
                let index = operations
                    .iter_mut()
                    .find_map(|op| match &mut op.kind {
                        OperationKind::GlobalCapabilityIndex(index) => Some(index),
                        _ => None,
                    })
                    .unwrap();
                index.capability = ValueId(41);
            } else {
                let length = operations
                    .iter()
                    .find_map(|op| {
                        matches!(op.kind, OperationKind::SliceLength { .. })
                            .then_some(op.results[0].id)
                    })
                    .unwrap();
                let wrong_bound = operations
                    .iter()
                    .find_map(|op| {
                        matches!(op.kind, OperationKind::Constant(Constant::Index(0)))
                            .then_some(op.results[0].id)
                    })
                    .unwrap();
                for op in &mut operations {
                    if let OperationKind::Compare { rhs, .. } = &mut op.kind {
                        if *rhs == length {
                            *rhs = wrong_bound;
                        }
                    }
                }
            }
            assert!(verify_module(&module(&f, operations)).is_err());
        }
    }

    #[test]
    fn global_bf16_rejects_plain_storage_foreign_bindings_and_noncurrent_lane() {
        for change in 0..10 {
            let mut f = fixture(SemanticMfmaOperandRoleV1::B);
            match change {
                0 => {
                    let SemanticValueBindingV1::Aggregate(fields) = &mut f.matrix else {
                        unreachable!()
                    };
                    fields[0] = value(
                        40,
                        Type::slice(
                            Type::Scalar(ScalarType::U16),
                            AddressSpace::Global,
                            AccessMode::ReadOnly,
                        ),
                    );
                }
                1 | 2 | 3 => {
                    let SemanticValueBindingV1::Aggregate(fields) = &mut f.matrix else {
                        unreachable!()
                    };
                    let SemanticValueBindingV1::GlobalCapability {
                        semantic_view,
                        contract,
                        capability,
                        ..
                    } = &mut fields[0]
                    else {
                        unreachable!()
                    };
                    match change {
                        1 => *semantic_view = id(8),
                        2 => {
                            *contract =
                                SemanticCapabilityMemoryContractV1::global_exclusive_read_write()
                        }
                        3 => {
                            *capability = GlobalCapabilityTypeV1::read_only(
                                Type::Scalar(ScalarType::U32),
                                f.context.context_type.clone(),
                            )
                        }
                        _ => unreachable!(),
                    }
                }
                4 => f.context.selected_root = SemanticFunctionIdV1::from_index(1),
                5 => {
                    f.callables.remove(0);
                }
                6 => f.lane = value(1, Type::Scalar(ScalarType::U32)),
                7 => {
                    f.lane = SemanticValueBindingV1::WaveLane {
                        value: ValueId(1),
                        wave: SemanticCurrentWaveV1::new(32),
                    }
                }
                8 => {
                    let SemanticValueBindingV1::Aggregate(fields) = &mut f.matrix else {
                        unreachable!()
                    };
                    let SemanticValueBindingV1::GlobalCapability { capability, .. } =
                        &mut fields[0]
                    else {
                        unreachable!()
                    };
                    *capability = GlobalCapabilityTypeV1::read_only(
                        Type::Scalar(ScalarType::U16),
                        KernelContextTypeV1::new(
                            "foreign_root_same_axes",
                            [1; 32],
                            [2; 32],
                            [3; 32],
                        ),
                    );
                }
                9 => {
                    let contract = f.contract;
                    f.contract = SemanticGlobalBf16MatrixLoadV1::new(
                        contract.types(),
                        contract.operand(),
                        contract.matrix_brand(),
                        contract.global_brand(),
                        contract.provenance(),
                        SemanticFunctionIdentityV1::from_sha256([63; 32]),
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }
            assert!(lower(&f).is_err(), "accepted mutation {change}");
        }
    }

    #[test]
    fn global_bf16_registers_existing_fragment_transport_not_scalar_symbols() {
        let f = fixture(SemanticMfmaOperandRoleV1::A);
        let bindings = compiler_issued_ssa_bindings_v1(
            &f.types,
            &f.callables,
            &f.function,
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        assert!(
            matches!(bindings.get(&f.contract.types().fragment), Some(SemanticPromotedBindingV1::MatrixFragment { contract, storage_layout })
            if *contract == f.contract.operand() && *storage_layout == SemanticMfmaStorageLayoutV1::RowMajor)
        );
        assert!(matches!(bindings.get(&f.contract.types().matrix),
            Some(SemanticPromotedBindingV1::GlobalBf16MatrixView { contract }) if *contract == f.contract));
    }

    #[test]
    fn global_bf16_view_transport_retains_global_edge_and_exact_coordinate_types() {
        let f = fixture(SemanticMfmaOperandRoleV1::A);
        let transport = SemanticPromotedBindingV1::GlobalBf16MatrixView {
            contract: f.contract,
        };
        let expected = transport
            .transport_types(
                &f.types,
                f.contract.types().matrix,
                Some(&f.context.context_type),
            )
            .unwrap();
        let values = transport.transport_values(&f.matrix).unwrap();
        assert_eq!(
            values.iter().map(|(_, ty)| ty.clone()).collect::<Vec<_>>(),
            expected
        );
        let definitions = values
            .iter()
            .map(|(id, ty)| ValueDef::new(*id, ty.clone()))
            .collect::<Vec<_>>();
        let restored = transport
            .binding_from_transport(&f.types, f.contract.types().matrix, &definitions)
            .unwrap();
        assert_eq!(transport.transport_values(&restored).unwrap(), values);
        for mutation in 0..4 {
            let mut definitions = definitions.clone();
            match mutation {
                0 => {
                    definitions[0].ty = Type::slice(
                        Type::Scalar(ScalarType::U16),
                        AddressSpace::Global,
                        AccessMode::ReadOnly,
                    );
                }
                1 => {
                    definitions[1].ty = Type::Scalar(ScalarType::U32);
                }
                2 => {
                    definitions.pop();
                }
                3 => {
                    definitions[0].ty = Type::GlobalCapability(GlobalCapabilityTypeV1::read_only(
                        Type::Scalar(ScalarType::U16),
                        KernelContextTypeV1::new("foreign", [9; 32], [2; 32], [3; 32]),
                    ));
                }
                _ => unreachable!(),
            }
            assert!(
                transport
                    .binding_from_transport(&f.types, f.contract.types().matrix, &definitions)
                    .is_err()
            );
        }
        assert!(
            transport
                .transport_types(
                    &f.types,
                    f.contract.types().global,
                    Some(&f.context.context_type)
                )
                .is_err()
        );
        assert!(
            transport
                .transport_types(&f.types, f.contract.types().matrix, None)
                .is_err()
        );
    }
}
