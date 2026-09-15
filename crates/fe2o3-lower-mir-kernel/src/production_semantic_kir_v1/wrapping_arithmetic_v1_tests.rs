mod wrapping_arithmetic_v1_tests {
    use super::*;
    use fe2o3_kir_sim::{
        AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
        SimulationErrorV1, SimulationExecutionErrorKindV1, SimulationLimitsV1, SimulationRequestV1,
        SimulationTargetV1,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiValueAttributesV1, SemanticCheckedBinaryRvalueV1,
    };

    const SCALARS: [ScalarType; 11] = [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::I128,
        ScalarType::U128,
        ScalarType::Index,
    ];
    const OPERATIONS: [(
        SemanticBinaryOpV1,
        SemanticCheckedBinaryOpV1,
        CheckedBinaryOperator,
        BinaryOp,
    ); 3] = [
        (
            SemanticBinaryOpV1::Add,
            SemanticCheckedBinaryOpV1::Add,
            CheckedBinaryOperator::Add,
            BinaryOp::Add,
        ),
        (
            SemanticBinaryOpV1::Subtract,
            SemanticCheckedBinaryOpV1::Subtract,
            CheckedBinaryOperator::Subtract,
            BinaryOp::Subtract,
        ),
        (
            SemanticBinaryOpV1::Multiply,
            SemanticCheckedBinaryOpV1::Multiply,
            CheckedBinaryOperator::Multiply,
            BinaryOp::Multiply,
        ),
    ];
    const BLOCK: SemanticBlockIdV1 = SemanticBlockIdV1::from_index(0);

    fn width(scalar: ScalarType) -> u32 {
        scalar.bit_width().map_or(64, u32::from)
    }

    fn bit_mask(scalar: ScalarType) -> u128 {
        let width = width(scalar);
        if width == 128 {
            u128::MAX
        } else {
            (1_u128 << width) - 1
        }
    }

    fn pairs(scalar: ScalarType) -> [(u128, u128); 10] {
        let mask = bit_mask(scalar);
        let sign = 1_u128 << (width(scalar) - 1);
        [
            (0, 0),
            (5, 7),
            (mask, 1),
            (0, 1),
            (mask, 2),
            (sign, mask),
            (sign, 1),
            (sign, 2),
            (sign - 1, 1),
            (sign - 1, 2),
        ]
    }

    fn oracle(
        scalar: ScalarType,
        operation: SemanticBinaryOpV1,
        lhs: u128,
        rhs: u128,
    ) -> (u128, bool) {
        macro_rules! native {
            ($ty:ty) => {{
                let lhs = lhs as $ty;
                let rhs = rhs as $ty;
                let (value, overflow) = match operation {
                    SemanticBinaryOpV1::Add => lhs.overflowing_add(rhs),
                    SemanticBinaryOpV1::Subtract => lhs.overflowing_sub(rhs),
                    SemanticBinaryOpV1::Multiply => lhs.overflowing_mul(rhs),
                    _ => unreachable!("closed arithmetic test operation"),
                };
                (value as u128 & bit_mask(scalar), overflow)
            }};
        }
        match scalar {
            ScalarType::I8 => native!(i8),
            ScalarType::U8 => native!(u8),
            ScalarType::I16 => native!(i16),
            ScalarType::U16 => native!(u16),
            ScalarType::I32 => native!(i32),
            ScalarType::U32 => native!(u32),
            ScalarType::I64 => native!(i64),
            ScalarType::U64 | ScalarType::Index => native!(u64),
            ScalarType::I128 => native!(i128),
            ScalarType::U128 => native!(u128),
            _ => unreachable!("closed arithmetic test scalar"),
        }
    }

    struct Fixture {
        scalar: ScalarType,
        types: Vec<SemanticTypeDeclV1>,
        function: SemanticFunctionDeclV1,
    }

    impl Fixture {
        fn new(scalar: ScalarType) -> Self {
            let scalar_type = SemanticTypeIdV1::from_index(1);
            let boolean = SemanticTypeIdV1::from_index(2);
            let bytes = u64::from(width(scalar) / 8);
            let types = vec![
                unit_type(),
                integer_type(71, scalar.is_signed_integer(), width(scalar) as u16),
                bool_type(),
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([73; 32]),
                    SemanticLayoutIdentityV1::from_sha256([74; 32]),
                    SemanticTypeLayoutV1::aggregate(
                        Some(2 * bytes),
                        bytes,
                        SemanticAggregateLayoutV1::new(vec![0, bytes], vec![]).unwrap(),
                    )
                    .unwrap(),
                    SemanticTypeShapeV1::Tuple(
                        SemanticAggregateTypeV1::new(vec![scalar_type, boolean]).unwrap(),
                    ),
                ),
            ];
            let source = SemanticSourceProvenanceV1::unavailable();
            let argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                scalar_type,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            ));
            let abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([75; 32]),
                SemanticLayoutIdentityV1::from_sha256([76; 32]),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                2,
                vec![argument.clone(), argument],
                SemanticAbiValueV1::new(
                    SemanticTypeIdV1::from_index(0),
                    SemanticAbiPassModeV1::Ignore,
                ),
            )
            .unwrap();
            let local = |tag, ty, role| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag; 32]),
                    ty,
                    role,
                    source,
                )
            };
            let function = SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([77; 32]),
                SemanticFunctionRoleV1::InternalHelper,
                SemanticItemDefinitionIdentityV1::from_sha256([78; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([79; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([80; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([81; 32]),
                source,
                abi,
                vec![
                    local(
                        82,
                        SemanticTypeIdV1::from_index(0),
                        SemanticLocalRoleV1::Return,
                    ),
                    local(83, scalar_type, SemanticLocalRoleV1::Argument(0)),
                    local(84, scalar_type, SemanticLocalRoleV1::Argument(1)),
                ],
                BLOCK,
                vec![
                    SemanticBasicBlockV1::new(
                        SemanticBlockIdentityV1::from_sha256([85; 32]),
                        source,
                        vec![],
                        SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
            Self {
                scalar,
                types,
                function,
            }
        }

        fn lowering(&self) -> SemanticFunctionLoweringV1<'_> {
            self.lowering_with_callables(&[])
        }

        fn lowering_with_callables<'a>(
            &'a self,
            callables: &'a [SemanticCallableDeclV1],
        ) -> SemanticFunctionLoweringV1<'a> {
            let scalar_type = SemanticTypeIdV1::from_index(1);
            let mut lowering = SemanticFunctionLoweringV1::new(
                &self.types,
                callables,
                &self.function,
                SemanticParameterBindingsV1 {
                    declarations: &[(0, 1, scalar_type), (1, 2, scalar_type)],
                    values: &[ValueId(0), ValueId(1)],
                    types: &[Type::Scalar(self.scalar), Type::Scalar(self.scalar)],
                    local_bindings: None,
                },
                None,
                None,
                BTreeSet::new(),
                1,
                false,
                64,
            )
            .unwrap();
            lowering.next_value = 4;
            lowering
        }

        fn launch_call<T>(
            &self,
            kind: IndexKind,
            axis: SemanticAxisV1,
            rank: u8,
            extra_argument: bool,
            run: impl FnOnce(&mut SemanticFunctionLoweringV1<'_>, &SemanticDirectCallV1) -> T,
        ) -> T {
            let scalar = SemanticTypeIdV1::from_index(1);
            let source = SemanticSourceProvenanceV1::unavailable();
            let abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([91; 32]),
                SemanticLayoutIdentityV1::from_sha256([92; 32]),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                0,
                vec![],
                SemanticAbiValueV1::new(
                    scalar,
                    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
                ),
            )
            .unwrap();
            let operation = match kind {
                IndexKind::Local => SemanticCompilerIntrinsicOperationV1::ThreadIndex(axis),
                IndexKind::Workgroup => SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(axis),
                IndexKind::WorkgroupSize => {
                    SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(axis)
                }
                IndexKind::WorkgroupCount => {
                    SemanticCompilerIntrinsicOperationV1::GridDimension(axis)
                }
                IndexKind::Global => unreachable!("global index has no source launch terminal"),
            };
            let callables = [SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256([93; 32]),
                    SemanticItemDefinitionIdentityV1::from_sha256([94; 32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([95; 32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([96; 32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([97; 32]),
                    source,
                    abi,
                ),
                operation,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([98; 32]),
            }];
            let place = |local| {
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], scalar).unwrap()
            };
            let arguments = if extra_argument {
                vec![SemanticOperandV1::Copy(place(2))]
            } else {
                vec![]
            };
            let call = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(1),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            let function = SemanticFunctionDeclV1::new(
                self.function.identity(),
                self.function.role(),
                self.function.item_definition_identity(),
                self.function.monomorphization_identity(),
                self.function.generic_type_arguments_identity(),
                self.function.const_generic_arguments_identity(),
                source,
                self.function.abi().clone(),
                self.function.locals().to_vec(),
                BLOCK,
                vec![
                    SemanticBasicBlockV1::new(
                        SemanticBlockIdentityV1::from_sha256([99; 32]),
                        source,
                        vec![],
                        SemanticTerminatorV1::new(
                            source,
                            SemanticTerminatorKindV1::Call(call.clone()),
                        ),
                    )
                    .unwrap(),
                    self.function.blocks()[0].clone(),
                ],
            )
            .unwrap();
            let fixture = Self {
                scalar: self.scalar,
                types: self.types.clone(),
                function,
            };
            let mut lowering = fixture.lowering_with_callables(&callables);
            lowering.launch_rank = rank;
            run(&mut lowering, &call)
        }

        fn lower(
            &self,
            lowering: &mut SemanticFunctionLoweringV1<'_>,
            operation: SemanticBinaryOpV1,
            checked: bool,
            operations: &mut Vec<Operation>,
        ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
            let operand = |local| {
                SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(local),
                        vec![],
                        SemanticTypeIdV1::from_index(1),
                    )
                    .unwrap(),
                )
            };
            let value = if checked {
                let checked_operation = OPERATIONS.iter().find(|row| row.0 == operation).unwrap().1;
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    checked_operation,
                    operand(1),
                    operand(2),
                ))
            } else {
                SemanticRvalueKindV1::Binary {
                    operation,
                    left: operand(1),
                    right: operand(2),
                }
            };
            lowering.lower_rvalue(
                BLOCK,
                Some(0),
                SemanticTypeIdV1::from_index(if checked { 3 } else { 1 }),
                &value,
                operations,
            )
        }

        fn module(&self, operation: SemanticBinaryOpV1, checked: bool) -> Module {
            let mut lowering = self.lowering();
            let mut operations = Vec::new();
            let result = self
                .lower(&mut lowering, operation, checked, &mut operations)
                .unwrap();
            assert_eq!(
                operations.len(),
                1,
                "dynamic arithmetic needs one operation"
            );
            let expected_operation = OPERATIONS.iter().find(|row| row.0 == operation).unwrap().2;
            assert!(matches!(
                operations[0].kind,
                OperationKind::Binary { op: BinaryOp::Checked(actual), lhs: ValueId(0), rhs: ValueId(1) }
                if actual == expected_operation
            ));
            assert_eq!(operations[0].results.len(), 2);
            assert_eq!(operations[0].results[0].ty, Type::Scalar(self.scalar));
            assert_eq!(operations[0].results[1].ty, Type::BOOL);
            let values = if checked {
                assert!(matches!(result, SemanticValueBindingV1::Aggregate(_)));
                let values = result.values().unwrap();
                assert_eq!(values.len(), 2);
                values
            } else {
                vec![
                    result
                        .value()
                        .expect("ordinary arithmetic returns only its modular value"),
                ]
            };
            assert_eq!(values[0].0, operations[0].results[0].id);
            for (index, (value, ty)) in values.into_iter().enumerate() {
                let alignment = if ty == Type::BOOL {
                    1
                } else {
                    width(self.scalar) / 8
                };
                operations.push(Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(2 + index as u32),
                        value,
                        access: MemoryAccess::new(AddressSpace::Global, alignment),
                    },
                ));
            }
            let mut block = BasicBlock::new(BlockId(0));
            block.operations = operations;
            block.terminator = Some(Terminator::Return { values: vec![] });
            let function = Function::kernel_entry(
                "arithmetic_impl",
                Signature::new(
                    vec![
                        Type::Scalar(self.scalar),
                        Type::Scalar(self.scalar),
                        Type::pointer(
                            Type::Scalar(self.scalar),
                            AddressSpace::Global,
                            AccessMode::ReadWrite,
                        ),
                        Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::ReadWrite),
                    ],
                    vec![],
                ),
                vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                vec![block],
            );
            let mut module = Module::new("ordinary-mir-wrapping-test");
            module.functions.push(function);
            module.kernels.push(Kernel::new(
                "arithmetic",
                "arithmetic_impl",
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            ));
            verify_module(&module).unwrap();
            module
        }
    }

    fn admit(module: Module) -> AdmittedSimulationModuleV1 {
        let canonical = VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
        AdmittedSimulationModuleV1::admit_v11(canonical, SimulationLimitsV1::default()).unwrap()
    }

    fn request(scalar: ScalarType, lhs: u128, rhs: u128) -> SimulationRequestV1 {
        let target = SimulationTargetV1::amdgpu_64();
        let bits = |scalar, value| ScalarBitsV1::new(scalar, value, target).unwrap();
        SimulationRequestV1::new(
            "arithmetic",
            [1, 1, 1],
            [1, 1, 1],
            vec![
                SimulationArgumentV1::Scalar(bits(scalar, lhs)),
                SimulationArgumentV1::Scalar(bits(scalar, rhs)),
                SimulationArgumentV1::Buffer(
                    BufferArgumentV1::from_scalars(
                        AccessMode::ReadWrite,
                        width(scalar) / 8,
                        &[bits(scalar, 0)],
                        target,
                    )
                    .unwrap(),
                ),
                SimulationArgumentV1::Buffer(
                    BufferArgumentV1::from_scalars(
                        AccessMode::ReadWrite,
                        1,
                        &[bits(ScalarType::Bool, 1)],
                        target,
                    )
                    .unwrap(),
                ),
            ],
        )
    }

    fn read_bits(bytes: &[u8]) -> u128 {
        let mut padded = [0_u8; 16];
        padded[..bytes.len()].copy_from_slice(bytes);
        u128::from_le_bytes(padded)
    }

    #[test]
    fn ordinary_mir_add_sub_mul_match_native_modular_results_at_every_integer_width() {
        for scalar in SCALARS {
            let fixture = Fixture::new(scalar);
            for (operation, _, _, _) in OPERATIONS {
                let admitted = admit(fixture.module(operation, false));
                for (lhs, rhs) in pairs(scalar) {
                    let result = admitted
                        .simulate(
                            &request(scalar, lhs, rhs),
                            SimulationTargetV1::amdgpu_64(),
                            SimulationLimitsV1::default(),
                        )
                        .unwrap();
                    assert_eq!(
                        read_bits(result.buffer(2).unwrap().bytes()),
                        oracle(scalar, operation, lhs, rhs).0,
                        "{scalar:?} {operation:?}: {lhs:#x}, {rhs:#x}",
                    );
                    assert_eq!(
                        result.buffer(3).unwrap().bytes(),
                        &[1],
                        "ordinary result must not expose an overflow flag"
                    );
                }
            }
        }
    }

    #[test]
    fn explicit_checked_mir_retains_the_result_and_exact_overflow_flag() {
        for scalar in SCALARS
            .into_iter()
            .filter(|&scalar| scalar != ScalarType::Index)
        {
            let fixture = Fixture::new(scalar);
            for (operation, _, _, _) in OPERATIONS {
                let admitted = admit(fixture.module(operation, true));
                for (lhs, rhs) in pairs(scalar) {
                    let result = admitted
                        .simulate(
                            &request(scalar, lhs, rhs),
                            SimulationTargetV1::amdgpu_64(),
                            SimulationLimitsV1::default(),
                        )
                        .unwrap();
                    let expected = oracle(scalar, operation, lhs, rhs);
                    assert_eq!(read_bits(result.buffer(2).unwrap().bytes()), expected.0);
                    assert_eq!(result.buffer(3).unwrap().bytes(), &[u8::from(expected.1)]);
                }
            }
        }
    }

    #[test]
    fn unchanged_plain_kir_arithmetic_still_refuses_overflow() {
        for scalar in SCALARS {
            let fixture = Fixture::new(scalar);
            for (operation, _, _, plain) in OPERATIONS {
                let mut module = fixture.module(operation, false);
                let arithmetic =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
                let OperationKind::Binary { op, .. } = &mut arithmetic.kind else {
                    unreachable!()
                };
                *op = plain;
                arithmetic.results.truncate(1);
                verify_module(&module).unwrap();
                let admitted = admit(module);
                let (lhs, rhs) = pairs(scalar)
                    .into_iter()
                    .find(|&(lhs, rhs)| oracle(scalar, operation, lhs, rhs).1)
                    .expect("boundary corpus contains overflow for each integer operation");
                let error = admitted
                    .simulate(
                        &request(scalar, lhs, rhs),
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                    )
                    .unwrap_err();
                assert!(
                    matches!(
                        error, SimulationErrorV1::Execution(error)
                        if matches!(error.kind, SimulationExecutionErrorKindV1::UndefinedIntegerOperation(_))
                    ),
                    "{scalar:?} {operation:?} must preserve plain-KIR refusal"
                );
            }
        }
    }

    #[test]
    fn arithmetic_emission_obeys_exact_and_cumulative_operation_budgets() {
        for scalar in SCALARS {
            let fixture = Fixture::new(scalar);
            for (operation, _, _, _) in OPERATIONS {
                let mut exact = fixture.lowering();
                exact.max_operations = 1;
                let mut operations = Vec::new();
                fixture
                    .lower(&mut exact, operation, false, &mut operations)
                    .unwrap();
                assert_eq!(exact.emitted_operations, 1);
                assert_eq!(operations.len(), 1);

                let mut empty = fixture.lowering();
                empty.max_operations = 0;
                assert!(matches!(
                    fixture.lower(&mut empty, operation, false, &mut Vec::new()),
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::Operations,
                        actual: 1,
                        limit: 0,
                    })
                ));
                assert_eq!(empty.emitted_operations, 0);

                let mut cumulative = fixture.lowering();
                let mut prefix = Vec::new();
                cumulative
                    .emit_id(
                        &mut prefix,
                        Type::BOOL,
                        OperationKind::Constant(Constant::Bool(false)),
                    )
                    .unwrap();
                cumulative.max_operations = 1;
                assert!(matches!(
                    fixture.lower(&mut cumulative, operation, false, &mut prefix),
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::Operations,
                        actual: 2,
                        limit: 1,
                    })
                ));
                assert_eq!(prefix.len(), 1);
                assert_eq!(cumulative.emitted_operations, 1);
            }
        }
    }

    #[test]
    fn ordinary_arithmetic_rejects_mismatched_dynamic_operand_types_before_emission() {
        let fixture = Fixture::new(ScalarType::U32);
        for (operation, _, _, _) in OPERATIONS {
            let mut lowering = fixture.lowering();
            lowering.locals[2] = Some(SemanticValueBindingV1::Value {
                id: ValueId(1),
                ty: Type::Scalar(ScalarType::I32),
            });
            let mut operations = Vec::new();
            assert!(matches!(
                fixture.lower(&mut lowering, operation, false, &mut operations),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "semantic binary operand types differ",
                    ..
                })
            ));
            assert!(operations.is_empty());
            assert_eq!(lowering.emitted_operations, 0);
        }
    }

    mod source_launch_u32_tests {
        use super::*;

        const KINDS: [IndexKind; 4] = [
            IndexKind::Local,
            IndexKind::Workgroup,
            IndexKind::WorkgroupSize,
            IndexKind::WorkgroupCount,
        ];
        const AXES: [(SemanticAxisV1, Axis); 3] = [
            (SemanticAxisV1::X, Axis::X),
            (SemanticAxisV1::Y, Axis::Y),
            (SemanticAxisV1::Z, Axis::Z),
        ];

        #[test]
        fn every_source_launch_call_returns_u32_without_narrowing_internal_indices() {
            let fixture = Fixture::new(ScalarType::U32);
            for rank in 1..=3 {
                for kind in KINDS {
                    for (ordinal, (source_axis, axis)) in AXES.into_iter().enumerate() {
                        fixture.launch_call(kind, source_axis, rank, false, |lowering, call| {
                            let mut operations = Vec::new();
                            let terminator = lowering.lower_call(BLOCK, call, &mut operations).unwrap();
                            assert!(matches!(terminator, Terminator::Branch { target: BlockId(1), .. }));
                            assert_eq!(operations.len(), 3);
                            assert_eq!(operations[0].results.len(), 1);
                            assert_eq!(operations[0].results[0].ty, Type::INDEX);
                            if ordinal < usize::from(rank) {
                                assert!(matches!(
                                    &operations[0].kind,
                                    OperationKind::Intrinsic(intrinsic)
                                        if intrinsic.kind == (IntrinsicKind::InvocationIndex { kind, axis })
                                ));
                            } else {
                                let expected = match kind {
                                    IndexKind::Local | IndexKind::Workgroup => 0,
                                    _ => 1,
                                };
                                assert_eq!(operations[0].kind, OperationKind::Constant(Constant::Index(expected)));
                            }
                            assert_eq!(operations[1].results.len(), 1);
                            assert_eq!(operations[1].results[0].ty, Type::Scalar(ScalarType::U64));
                            assert_eq!(operations[1].kind, OperationKind::Cast {
                                kind: CastKind::Bitcast,
                                value: operations[0].results[0].id,
                                to: Type::Scalar(ScalarType::U64),
                            });
                            assert_eq!(operations[2].results.len(), 1);
                            assert_eq!(operations[2].results[0].ty, Type::Scalar(ScalarType::U32));
                            assert_eq!(operations[2].kind, OperationKind::Cast {
                                kind: CastKind::Truncate,
                                value: operations[1].results[0].id,
                                to: Type::Scalar(ScalarType::U32),
                            });
                            assert_eq!(lowering.locals[1].as_ref().unwrap().value().unwrap(),
                                (operations[2].results[0].id, Type::Scalar(ScalarType::U32)));

                            let mut internal = fixture.lowering();
                            internal.launch_rank = rank;
                            let mut internal_operations = Vec::new();
                            assert_eq!(internal.emit_launch_index_v1(&mut internal_operations, kind, axis)
                                .unwrap().value().unwrap().1, Type::INDEX);
                            assert_eq!(internal_operations.len(), 1);
                            assert_eq!(internal_operations[0].kind, operations[0].kind);
                        });
                    }
                }
            }
        }

        #[test]
        fn source_launch_calls_reject_arguments_and_non_u32_destinations_before_emission() {
            let mut char_fixture = Fixture::new(ScalarType::U32);
            char_fixture.types[1] = SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([101; 32]),
                SemanticLayoutIdentityV1::from_sha256([102; 32]),
                SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char),
            );
            let mut cases = SCALARS
                .into_iter()
                .filter(|scalar| *scalar != ScalarType::U32)
                .map(|scalar| (Fixture::new(scalar), false))
                .collect::<Vec<_>>();
            cases.push((char_fixture, false));
            cases.push((Fixture::new(ScalarType::U32), true));
            for (fixture, extra_argument) in cases {
                for rank in 1..=3 {
                    for kind in KINDS {
                        for (axis, _) in AXES {
                            fixture.launch_call(
                                kind,
                                axis,
                                rank,
                                extra_argument,
                                |lowering, call| {
                                    let mut operations = Vec::new();
                                    assert!(matches!(
                                        lowering.lower_call(BLOCK, call, &mut operations),
                                        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                                    ));
                                    assert!(operations.is_empty());
                                    assert_eq!(lowering.emitted_operations, 0);
                                },
                            );
                        }
                    }
                }
            }
        }

        #[test]
        fn source_launch_cast_obeys_exact_zero_and_one_short_operation_budgets() {
            let fixture = Fixture::new(ScalarType::U32);
            for rank in 1..=3 {
                for kind in KINDS {
                    for (axis, _) in AXES {
                        for limit in 0..=3 {
                            fixture.launch_call(kind, axis, rank, false, |lowering, call| {
                                lowering.max_operations = limit;
                                let mut operations = Vec::new();
                                let result = lowering.lower_call(BLOCK, call, &mut operations);
                                if limit == 3 {
                                    result.unwrap();
                                } else {
                                    assert!(matches!(result,
                                        Err(ProductionSemanticKirErrorV1::ResourceLimit {
                                            resource: ProductionSemanticKirResourceV1::Operations,
                                            actual, limit: actual_limit,
                                        }) if actual == limit + 1 && actual_limit == limit));
                                }
                                assert_eq!(operations.len(), limit);
                                assert_eq!(lowering.emitted_operations, limit);
                            });
                        }
                    }
                }
            }
        }

        fn observed_launch_module(
            fixture: &Fixture,
            kind: IndexKind,
            axis: SemanticAxisV1,
            rank: u8,
            grid: [u64; 3],
        ) -> Module {
            observed_launch_module_with(fixture, kind, axis, rank, grid, |_, _, value| {
                (value, Type::Scalar(ScalarType::U32), None)
            })
        }

        fn observed_launch_module_with(
            fixture: &Fixture,
            kind: IndexKind,
            axis: SemanticAxisV1,
            rank: u8,
            grid: [u64; 3],
            transform: impl FnOnce(
                &mut SemanticFunctionLoweringV1<'_>,
                &mut Vec<Operation>,
                ValueId,
            ) -> (ValueId, Type, Option<ValueId>),
        ) -> Module {
            fixture.launch_call(kind, axis, rank, false, |lowering, call| {
                let mut call_operations = Vec::new();
                let terminator = lowering
                    .lower_call(BLOCK, call, &mut call_operations)
                    .unwrap();
                let value = lowering.locals[1].as_ref().unwrap().value().unwrap().0;
                let mut operations = Vec::new();
                let (value, value_type, flag) = transform(lowering, &mut operations, value);
                // Generic output instrumentation keeps each invocation disjoint.
                let mut offset = lowering
                    .emit_id(
                        &mut operations,
                        Type::INDEX,
                        OperationKind::Constant(Constant::Index(0)),
                    )
                    .unwrap();
                for (ordinal, (_, axis)) in AXES.into_iter().enumerate().rev() {
                    let extent = lowering
                        .emit_id(
                            &mut operations,
                            Type::INDEX,
                            OperationKind::Constant(Constant::Index(grid[ordinal])),
                        )
                        .unwrap();
                    let coordinate = lowering
                        .emit_launch_index_v1(&mut operations, IndexKind::Global, axis)
                        .unwrap()
                        .value()
                        .unwrap()
                        .0;
                    offset = lowering
                        .emit_id(
                            &mut operations,
                            Type::INDEX,
                            OperationKind::Binary {
                                op: BinaryOp::Multiply,
                                lhs: offset,
                                rhs: extent,
                            },
                        )
                        .unwrap();
                    offset = lowering
                        .emit_id(
                            &mut operations,
                            Type::INDEX,
                            OperationKind::Binary {
                                op: BinaryOp::Add,
                                lhs: offset,
                                rhs: coordinate,
                            },
                        )
                        .unwrap();
                }
                let one = lowering
                    .emit_id(
                        &mut operations,
                        Type::INDEX,
                        OperationKind::Constant(Constant::Index(1)),
                    )
                    .unwrap();
                offset = lowering
                    .emit_id(
                        &mut operations,
                        Type::INDEX,
                        OperationKind::Binary {
                            op: BinaryOp::Add,
                            lhs: offset,
                            rhs: one,
                        },
                    )
                    .unwrap();
                let output_type = Type::pointer(
                    value_type.clone(),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                );
                let pointer = lowering
                    .emit_id(
                        &mut operations,
                        output_type.clone(),
                        OperationKind::GetElementPointer {
                            base: ValueId(2),
                            offset,
                        },
                    )
                    .unwrap();
                operations.push(Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer,
                        value,
                        access: MemoryAccess::new(
                            AddressSpace::Global,
                            width(value_type.as_scalar().unwrap()) / 8,
                        ),
                    },
                ));
                if let Some(flag) = flag {
                    let flag_pointer = lowering
                        .emit_id(
                            &mut operations,
                            Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::ReadWrite),
                            OperationKind::GetElementPointer {
                                base: ValueId(3),
                                offset,
                            },
                        )
                        .unwrap();
                    operations.push(Operation::new(
                        vec![],
                        OperationKind::Store {
                            pointer: flag_pointer,
                            value: flag,
                            access: MemoryAccess::new(AddressSpace::Global, 1),
                        },
                    ));
                }
                let mut entry = BasicBlock::new(BlockId(0));
                entry.operations = call_operations;
                entry.terminator = Some(terminator);
                let mut observer = BasicBlock::new(BlockId(1));
                observer.operations = operations;
                observer.terminator = Some(Terminator::Return { values: vec![] });
                let function = Function::kernel_entry(
                    "source_launch_impl",
                    Signature::new(
                        vec![
                            Type::Scalar(ScalarType::U32),
                            Type::Scalar(ScalarType::U32),
                            output_type,
                            Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::ReadWrite),
                        ],
                        vec![],
                    ),
                    vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                    vec![entry, observer],
                );
                let domain = match rank {
                    1 => LaunchDomain::D1 {
                        x: LaunchExtent::Dynamic,
                    },
                    2 => LaunchDomain::D2 {
                        x: LaunchExtent::Dynamic,
                        y: LaunchExtent::Dynamic,
                    },
                    3 => LaunchDomain::D3 {
                        x: LaunchExtent::Dynamic,
                        y: LaunchExtent::Dynamic,
                        z: LaunchExtent::Dynamic,
                    },
                    _ => unreachable!(),
                };
                let mut module = Module::new("source-launch-u32-test");
                module.functions.push(function);
                module
                    .kernels
                    .push(Kernel::new("source_launch", "source_launch_impl", domain));
                verify_module(&module).unwrap();
                module
            })
        }

        #[test]
        fn source_launch_checked_add_shift_and_widen_compose_with_exact_flags() {
            let mut fixture = Fixture::new(ScalarType::U32);
            fixture.types.push(integer_type(103, false, 64));
            let module = observed_launch_module_with(
                &fixture,
                IndexKind::Local,
                SemanticAxisV1::X,
                1,
                [3, 1, 1],
                |lowering, operations, launch_value| {
                    let u32_type = SemanticTypeIdV1::from_index(1);
                    let operand = |local| {
                        SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(local),
                                vec![],
                                u32_type,
                            )
                            .unwrap(),
                        )
                    };
                    let checked = lowering
                        .lower_rvalue(
                            BLOCK,
                            None,
                            SemanticTypeIdV1::from_index(3),
                            &SemanticRvalueKindV1::CheckedBinary(
                                SemanticCheckedBinaryRvalueV1::new(
                                    SemanticCheckedBinaryOpV1::Add,
                                    operand(1),
                                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                                        u32_type,
                                        SemanticConstantValueV1::Scalar(
                                            SemanticScalarValueV1::new(u128::from(u32::MAX), 4)
                                                .unwrap(),
                                        ),
                                    )),
                                ),
                            ),
                            operations,
                        )
                        .unwrap()
                        .values()
                        .unwrap();
                    assert_eq!(checked.len(), 2);
                    assert_eq!(checked[0].1, Type::Scalar(ScalarType::U32));
                    assert_eq!(checked[1].1, Type::BOOL);
                    assert!(matches!(operations.last().unwrap().kind,
                        OperationKind::Binary { op: BinaryOp::Checked(CheckedBinaryOperator::Add), lhs, .. }
                            if lhs == launch_value));
                    lowering.locals[1] = Some(SemanticValueBindingV1::Value {
                        id: checked[0].0,
                        ty: checked[0].1.clone(),
                    });
                    // This is normalized semantic MIR with a dynamic count, not source `<< 32` admission.
                    let shifted = lowering
                        .lower_rvalue(
                            BLOCK,
                            None,
                            u32_type,
                            &SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::ShiftLeft,
                                left: operand(1),
                                right: operand(2),
                            },
                            operations,
                        )
                        .unwrap();
                    assert_eq!(
                        operations[2].kind,
                        OperationKind::Constant(Constant::U32(31))
                    );
                    assert_eq!(
                        operations[3].kind,
                        OperationKind::Binary {
                            op: BinaryOp::BitAnd,
                            lhs: ValueId(1),
                            rhs: operations[2].results[0].id,
                        }
                    );
                    assert_eq!(
                        operations[4].kind,
                        OperationKind::Binary {
                            op: BinaryOp::ShiftLeft,
                            lhs: checked[0].0,
                            rhs: operations[3].results[0].id,
                        }
                    );
                    let shifted_value = shifted.value().unwrap().0;
                    lowering.locals[1] = Some(shifted);
                    let widened = lowering
                        .lower_rvalue(
                            BLOCK,
                            None,
                            SemanticTypeIdV1::from_index(4),
                            &SemanticRvalueKindV1::Cast {
                                kind: SemanticCastKindV1::Integer,
                                operand: operand(1),
                            },
                            operations,
                        )
                        .unwrap()
                        .value()
                        .unwrap();
                    assert_eq!(operations.len(), 6);
                    assert_eq!(
                        operations[5].kind,
                        OperationKind::Cast {
                            kind: CastKind::ZeroExtend,
                            value: shifted_value,
                            to: Type::Scalar(ScalarType::U64),
                        }
                    );
                    (widened.0, widened.1, Some(checked[1].0))
                },
            );
            let target = SimulationTargetV1::amdgpu_64();
            let canary = u128::from(u64::MAX - 7);
            let mut request = request(ScalarType::U32, 0, 32);
            request.kernel = "source_launch".into();
            request.grid = fe2o3_kir_sim::GridShapeV1([3, 1, 1]);
            request.workgroup = fe2o3_kir_sim::WorkgroupShapeV1([3, 1, 1]);
            for (argument, scalar, initial, alignment) in
                [(2, ScalarType::U64, canary, 8), (3, ScalarType::Bool, 1, 1)]
            {
                request.arguments[argument] = SimulationArgumentV1::Buffer(
                    BufferArgumentV1::from_scalars(
                        AccessMode::ReadWrite,
                        alignment,
                        &[ScalarBitsV1::new(scalar, initial, target).unwrap(); 5],
                        target,
                    )
                    .unwrap(),
                );
            }
            let result = admit(module)
                .simulate(&request, target, SimulationLimitsV1::default())
                .unwrap();
            assert_eq!(
                result
                    .buffer(2)
                    .unwrap()
                    .bytes()
                    .chunks_exact(8)
                    .map(read_bits)
                    .collect::<Vec<_>>(),
                [canary, u128::from(u32::MAX), 0, 1, canary]
            );
            assert_eq!(result.buffer(3).unwrap().bytes(), &[1, 0, 1, 1, 1]);
        }

        #[test]
        fn source_launch_calls_simulate_exact_coordinates_with_partial_workgroups_and_canaries() {
            let fixture = Fixture::new(ScalarType::U32);
            let target = SimulationTargetV1::amdgpu_64();
            let canary = 0xa5a5_5a5a_u128;
            for rank in 1..=3 {
                let grid = [
                    5,
                    if rank >= 2 { 3 } else { 1 },
                    if rank == 3 { 3 } else { 1 },
                ];
                let workgroup = [
                    3,
                    if rank >= 2 { 2 } else { 1 },
                    if rank == 3 { 2 } else { 1 },
                ];
                let count = grid.into_iter().product::<u64>() as usize;
                for kind in KINDS {
                    for (ordinal, (axis, _)) in AXES.into_iter().enumerate() {
                        let admitted =
                            admit(observed_launch_module(&fixture, kind, axis, rank, grid));
                        let mut request = request(ScalarType::U32, 0, 0);
                        request.kernel = "source_launch".into();
                        request.grid = fe2o3_kir_sim::GridShapeV1(grid);
                        request.workgroup = fe2o3_kir_sim::WorkgroupShapeV1(workgroup);
                        request.arguments[2] = SimulationArgumentV1::Buffer(
                            BufferArgumentV1::from_scalars(
                                AccessMode::ReadWrite,
                                4,
                                &vec![
                                    ScalarBitsV1::new(ScalarType::U32, canary, target).unwrap();
                                    count + 2
                                ],
                                target,
                            )
                            .unwrap(),
                        );
                        let result = admitted
                            .simulate(&request, target, SimulationLimitsV1::default())
                            .unwrap();
                        let values = result
                            .buffer(2)
                            .unwrap()
                            .bytes()
                            .chunks_exact(4)
                            .map(read_bits)
                            .collect::<Vec<_>>();
                        assert_eq!(values.len(), count + 2);
                        assert_eq!(values[0], canary);
                        assert_eq!(values[count + 1], canary);
                        assert_eq!(result.buffer(3).unwrap().bytes(), &[1]);
                        for z in 0..grid[2] {
                            for y in 0..grid[1] {
                                for x in 0..grid[0] {
                                    let coordinate = [x, y, z][ordinal];
                                    let extent = u64::from(workgroup[ordinal]);
                                    let expected = match kind {
                                        IndexKind::Local => coordinate % extent,
                                        IndexKind::Workgroup => coordinate / extent,
                                        IndexKind::WorkgroupSize => extent,
                                        IndexKind::WorkgroupCount => grid[ordinal].div_ceil(extent),
                                        IndexKind::Global => unreachable!(),
                                    };
                                    let index = ((z * grid[1] + y) * grid[0] + x) as usize + 1;
                                    assert_eq!(
                                        values[index],
                                        u128::from(expected),
                                        "rank {rank}, {kind:?}, {axis:?}, [{x}, {y}, {z}]"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
