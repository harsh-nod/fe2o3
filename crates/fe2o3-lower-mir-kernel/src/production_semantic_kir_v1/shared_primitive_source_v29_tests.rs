mod shared_primitive_source_v29_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBackendPrimitiveV1, SemanticBackendReprV1, SemanticBackendScalarV1,
        SemanticLayoutNicheV1, SemanticMemoryLoadV1, SemanticScalarValidityRangeV1,
    };

    #[derive(Clone, Copy, Debug)]
    enum Read {
        Operand,
        TupleOperand,
        ExplicitLoad,
    }

    fn original_owner(read: Read) -> ProductionSemanticMirOwnerV1 {
        let source = SemanticSourceProvenanceV1::unavailable();
        let unit = SemanticTypeIdV1::from_index(0);
        let scalar = SemanticTypeIdV1::from_index(1);
        let reference = SemanticTypeIdV1::from_index(2);
        let tuple = SemanticTypeIdV1::from_index(3);
        let integer_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let pointer_primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
        let pointer_range = SemanticScalarValidityRangeV1::new(1, u64::MAX.into());
        let pointer_scalar = SemanticBackendScalarV1::initialized(pointer_primitive, pointer_range);
        let mut types = vec![
            unit_type(),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([31; 32]),
                SemanticLayoutIdentityV1::from_sha256([32; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(integer_scalar),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                }),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([111; 32]),
                SemanticLayoutIdentityV1::from_sha256([112; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(pointer_scalar),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1::new_with_kind(
                        scalar,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
        ];
        let mut local_types = vec![unit, scalar, reference, scalar];
        if matches!(read, Read::TupleOperand) {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([113; 32]),
                SemanticLayoutIdentityV1::from_sha256([114; 32]),
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    16,
                    8,
                    SemanticFieldsShapeV1::arbitrary(vec![0, 8], vec![0, 1]).unwrap(),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::ScalarPair {
                        first: integer_scalar,
                        second: pointer_scalar,
                    },
                    Some(SemanticLayoutNicheV1::new(8, pointer_primitive, pointer_range).unwrap()),
                    false,
                    None,
                    8,
                    0,
                    SemanticTypeLayoutDetailsV1::Aggregate(
                        SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
                    ),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![scalar, reference]).unwrap(),
                ),
            ));
            local_types.push(tuple);
        }
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let assign = |local, ty, value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(local, ty),
                    SemanticRvalueV1::new(ty, value),
                )),
            )
        };
        let constant = |value| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                scalar,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 8).unwrap()),
            ))
        };
        let mut statements = vec![
            assign(1, scalar, SemanticRvalueKindV1::Use(constant(17))),
            assign(
                2,
                reference,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(1, scalar),
                },
            ),
        ];
        let (holder, mut projections) = if matches!(read, Read::TupleOperand) {
            statements.push(assign(
                4,
                tuple,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Tuple,
                    vec![constant(23), SemanticOperandV1::Move(place(2, reference))],
                )
                .unwrap(),
            ));
            (
                4,
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), reference)
                        .unwrap(),
                ],
            )
        } else {
            (2, vec![])
        };
        projections.push(
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar).unwrap(),
        );
        let access =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(holder), projections, scalar)
                .unwrap();
        let value = match read {
            Read::ExplicitLoad => SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                access,
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
            Read::Operand | Read::TupleOperand => {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(access))
            }
        };
        statements.push(assign(3, scalar, value));
        statements.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(holder)),
        ));
        statements.push(assign(
            3,
            scalar,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Copy(place(3, scalar)),
                right: constant(5),
            },
        ));
        statements.push(assign(
            0,
            unit,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                unit,
                SemanticConstantValueV1::ZeroSized,
            ))),
        ));
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([115; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let block = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([116; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([117; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([118; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([119; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([120; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([121; 32]),
            source,
            abi,
            local_types
                .into_iter()
                .enumerate()
                .map(|(index, ty)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([130 + index as u8; 32]),
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
            SemanticBlockIdV1::from_index(0),
            vec![block],
        )
        .unwrap();
        let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
        let launch =
            SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
        let function = function.with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"shared_primitive_source".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([122; 32]),
            SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap(),
        ));
        let admitted = InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            types,
            vec![],
            vec![],
            vec![],
            vec![function],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap()
    }

    fn complete_lowering(read: Read, promoted: bool) {
        // Do not inject a plan or manually mark a local promotable. Both the SSA
        // owner and the actual lowerer consume this admitted original program.
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            original_owner(read),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap_or_else(|error| panic!("{read:?}: {error:?}"));
        lowered.semantic_ssa.verify_replay().unwrap();
        lowered.verify_equivalence().unwrap();
        let plan = lowered
            .semantic_ssa
            .plan_for_function(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        assert_eq!(
            plan.plan()
                .promoted_variables()
                .iter()
                .any(|local| local.get() == 1),
            promoted
        );
        let mut counts = (0, 0, 0);
        let mut operations = 0;
        let mut original_value = None;
        let mut increment = None;
        let mut loaded_value = None;
        let mut sum_operands = None;
        for function in &lowered.module().functions {
            let Some(body) = &function.body else {
                continue;
            };
            for block in &body.blocks {
                for operation in &block.operations {
                    operations += 1;
                    match &operation.kind {
                        OperationKind::Alloca { address_space, .. } => {
                            assert_eq!(*address_space, AddressSpace::Private);
                            counts.0 += 1;
                        }
                        OperationKind::Load { access, .. } => {
                            assert_eq!(access.address_space, AddressSpace::Private);
                            assert!(!access.volatile);
                            counts.1 += 1;
                            assert!(loaded_value.replace(operation.results[0].id).is_none());
                        }
                        OperationKind::Store { access, .. } => {
                            assert_eq!(access.address_space, AddressSpace::Private);
                            assert!(!access.volatile);
                            counts.2 += 1;
                        }
                        OperationKind::Constant(Constant::U64(17)) => {
                            assert!(original_value.replace(operation.results[0].id).is_none());
                        }
                        OperationKind::Constant(Constant::U64(5)) => {
                            assert!(increment.replace(operation.results[0].id).is_none());
                        }
                        OperationKind::Binary {
                            op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                            lhs,
                            rhs,
                        } => {
                            assert!(sum_operands.replace((*lhs, *rhs)).is_none());
                            assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U64));
                            assert_eq!(operation.results[1].ty, Type::BOOL);
                        }
                        _ => {}
                    }
                }
            }
        }
        assert!(
            operations > 0,
            "the original scalar definition must be emitted"
        );
        assert_eq!(counts, if promoted { (0, 0, 0) } else { (1, 1, 1) });
        assert!(original_value.is_some());
        assert_eq!(
            sum_operands,
            Some((
                if promoted {
                    original_value.unwrap()
                } else {
                    loaded_value.unwrap()
                },
                increment.unwrap(),
            ))
        );
    }

    #[test]
    fn shared_primitive_current_main_direct_operand_reaches_complete_lowering_without_backing() {
        complete_lowering(Read::Operand, true);
    }

    #[test]
    fn shared_primitive_current_main_tuple_operand_reaches_complete_lowering_without_backing() {
        complete_lowering(Read::TupleOperand, true);
    }

    #[test]
    fn shared_primitive_current_main_explicit_load_retains_its_pointer_and_complete_lowering() {
        complete_lowering(Read::ExplicitLoad, false);
    }
}
