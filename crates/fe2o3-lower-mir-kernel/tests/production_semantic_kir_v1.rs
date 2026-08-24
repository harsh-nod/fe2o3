use fe2o3_kernel_ir::{
    AmdGpuDiagnosticOperation, BinaryOp, BlockId, CheckedBinaryOperator, LaunchDomain,
    OperationKind, ScalarType, TargetCapability, Terminator, Type, WaveWidth, WorkgroupSize,
    decode_module_v7, gfx942_xnack_minus_target_capability, verify_module,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV3, PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1,
    ProductionFormalMemoryOwnerV1, ProductionRankedSemanticProjectionReceiptV1,
    ProductionSemanticKirErrorV1, ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
    ProductionSemanticKirResourceV1, SemanticKirSyntheticOperationRuleV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedTerminatorV1, ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
};

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn unit_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4)),
        SemanticLayoutIdentityV1::from_sha256(bytes(4)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    )
}

fn scalar_type(tag: u8, scalar: SemanticScalarTypeV1) -> SemanticTypeDeclV1 {
    let (size, primitive, maximum) = match scalar {
        SemanticScalarTypeV1::Bool => (1, SemanticBackendPrimitiveV1::integer(false, 8, 1), 1_u128),
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        } => (
            4,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            u32::MAX.into(),
        ),
        _ => panic!("unsupported test scalar"),
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            size,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, maximum),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(scalar),
    )
}

fn local_place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn scalar_constant(ty: SemanticTypeIdV1, value: u32, bytes: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), bytes).unwrap()),
    ))
}

fn scalar_loop_owner() -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                local_place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let entry = block(
        20,
        vec![assign(
            1,
            u32_ty,
            SemanticRvalueKindV1::Use(scalar_constant(u32_ty, 0, 4)),
        )],
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
    );
    let header = block(
        21,
        vec![assign(
            2,
            bool_ty,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: SemanticOperandV1::Copy(local_place(1, u32_ty)),
                right: scalar_constant(u32_ty, 3, 4),
            },
        )],
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(local_place(2, bool_ty)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    1,
                    edge(SemanticEdgeRoleV1::SwitchValue, 2),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
            )
            .unwrap(),
        },
    );
    let body = block(
        22,
        vec![assign(
            1,
            u32_ty,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Copy(local_place(1, u32_ty)),
                right: scalar_constant(u32_ty, 1, 4),
            },
        )],
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
    );
    let exit = block(23, vec![], SemanticTerminatorKindV1::Return);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(24)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [unit, u32_ty, bool_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(30 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let contract = SemanticKernelSourceContractV1::new(
        Some(SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap()),
        None,
        None,
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(24)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(24)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(24)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(24)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(24)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![entry, header, body, exit],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"semantic_scalar_loop_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(25)),
        contract,
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![
            unit_type(),
            scalar_type(
                40,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            scalar_type(41, SemanticScalarTypeV1::Bool),
        ],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn checked_arithmetic_owner() -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let checked_ty = SemanticTypeIdV1::from_index(3);
    let checked_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(42)),
        SemanticLayoutIdentityV1::from_sha256(bytes(42)),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![SemanticPaddingV1::new(5, 3).unwrap()])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![u32_ty, bool_ty]).unwrap()),
    );
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(1, checked_ty),
            SemanticRvalueV1::new(
                checked_ty,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Multiply,
                    scalar_constant(u32_ty, 7, 4),
                    scalar_constant(u32_ty, 9, 4),
                )),
            ),
        )),
    );
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(43)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [unit, checked_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(44 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(48)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(48)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(48)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(48)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(48)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block(49, vec![statement], SemanticTerminatorKindV1::Return)],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"semantic_checked_arithmetic_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(49)),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![
            unit_type(),
            scalar_type(
                40,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            scalar_type(41, SemanticScalarTypeV1::Bool),
            checked_type,
        ],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn owner_from_parts(
    types: Vec<SemanticTypeDeclV1>,
    locals: Vec<SemanticLocalDeclV1>,
    entry: u32,
    blocks: Vec<SemanticBasicBlockV1>,
    symbol: &[u8],
) -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(60)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let launch =
        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let contract = SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(60)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(60)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(60)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(60)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(60)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(entry),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(symbol.to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(61)),
        contract,
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn return_local() -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(62)),
        SemanticTypeIdV1::from_index(0),
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn terminator_emission_owner() -> ProductionSemanticMirOwnerV1 {
    let bool_ty = SemanticTypeIdV1::from_index(1);
    let target = SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::SwitchOtherwise,
        SemanticBlockIdV1::from_index(0),
    );
    let switch = SemanticTerminatorKindV1::SwitchInt {
        discriminant: scalar_constant(bool_ty, 1, 1),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(0),
                ),
            )],
            target,
        )
        .unwrap(),
    };
    owner_from_parts(
        vec![unit_type(), scalar_type(63, SemanticScalarTypeV1::Bool)],
        vec![return_local()],
        1,
        vec![
            block(64, vec![], SemanticTerminatorKindV1::Return),
            block(65, vec![], switch),
        ],
        b"semantic_terminator_emission_test",
    )
}

fn abort_owner() -> ProductionSemanticMirOwnerV1 {
    owner_from_parts(
        vec![unit_type()],
        vec![return_local()],
        0,
        vec![block(66, vec![], SemanticTerminatorKindV1::Abort)],
        b"semantic_abort_test",
    )
}

fn bounds_assert_owner() -> ProductionSemanticMirOwnerV1 {
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let success = SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::AssertSuccess,
        SemanticBlockIdV1::from_index(1),
    );
    owner_from_parts(
        vec![
            unit_type(),
            scalar_type(
                67,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            scalar_type(68, SemanticScalarTypeV1::Bool),
        ],
        vec![return_local()],
        0,
        vec![
            block(
                69,
                vec![],
                SemanticTerminatorKindV1::Assert {
                    condition: scalar_constant(bool_ty, 0, 1),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: scalar_constant(u32_ty, 4, 4),
                        index: scalar_constant(u32_ty, 5, 4),
                    },
                    target: success,
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(70, vec![], SemanticTerminatorKindV1::Return),
        ],
        b"semantic_bounds_trap_test",
    )
}

fn direct_enum_constant_owner() -> ProductionSemanticMirOwnerV1 {
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let result_ty = SemanticTypeIdV1::from_index(2);
    let primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    let scalar = SemanticBackendScalarV1::initialized(
        primitive,
        SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
    );
    let variant_layout = |index| {
        SemanticEnumVariantLayoutV1::from_rustc(
            index,
            8,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
        )
        .unwrap()
    };
    let result = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(72)),
        SemanticLayoutIdentityV1::from_sha256(bytes(72)),
        SemanticTypeLayoutV1::enum_layout(
            8,
            4,
            SemanticEnumLayoutV1::new(
                vec![variant_layout(0), variant_layout(1)],
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, scalar)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Enum {
            discriminant: u32_ty,
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![u32_ty]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![u32_ty]).unwrap()),
            ]
            .into_boxed_slice(),
        },
    );
    let assignment = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(1, result_ty),
            SemanticRvalueV1::new(
                result_ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    result_ty,
                    SemanticConstantValueV1::Bytes(
                        SemanticConstantBytesV1::new(vec![1, 0, 0, 0, 9, 0, 0, 0]).unwrap(),
                    ),
                ))),
            ),
        )),
    );
    owner_from_parts(
        vec![
            unit_type(),
            scalar_type(
                71,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            result,
        ],
        vec![
            return_local(),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(73)),
                result_ty,
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ),
        ],
        0,
        vec![block(
            74,
            vec![assignment],
            SemanticTerminatorKindV1::Return,
        )],
        b"semantic_direct_enum_constant_test",
    )
}

fn admitted(effectful_statement: bool, unsupported_terminator: bool) -> AdmittedInertSemanticMirV1 {
    let type_id = SemanticTypeIdV1::from_index(0);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(type_id, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let statements = vec![SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        if effectful_statement {
            SemanticStatementKindV1::Deinitialize(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], type_id).unwrap(),
            )
        } else {
            SemanticStatementKindV1::Nop
        },
    )];
    let block0 = block(10, statements, SemanticTerminatorKindV1::Return);
    let target = SemanticBlockIdV1::from_index(0);
    let block1_terminator = if unsupported_terminator {
        SemanticTerminatorKindV1::FalseEdge {
            real_target: SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::FalseEdgeReal, target),
            imaginary_target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::FalseEdgeImaginary,
                target,
            ),
        }
    } else {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            target,
        ))
    };
    let block1 = block(11, vec![], block1_terminator);
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(2)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(2)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(2)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(3)),
            type_id,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        )],
        SemanticBlockIdV1::from_index(1),
        vec![block0, block1],
    )
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let launch =
        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let contract = SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
    let function = function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"semantic_kir_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(5)),
        contract,
    ));
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
}

fn semantic_owner(
    effectful_statement: bool,
    unsupported_terminator: bool,
) -> ProductionSemanticMirOwnerV1 {
    ProductionSemanticMirOwnerV1::try_new(
        admitted(effectful_statement, unsupported_terminator),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn exact_nonzero_entry_lowers_to_verified_kir_with_correspondence() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();

    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks[0].id, BlockId(1));
    assert_eq!(body.blocks[1].id, BlockId(0));
    assert_eq!(
        lowered.correspondence().blocks()[0]
            .semantic_block()
            .index(),
        1
    );
    assert_eq!(
        lowered.correspondence().blocks()[1].source_statement_count(),
        1
    );
    let statement_spans = lowered.correspondence().statement_operation_spans();
    assert_eq!(statement_spans.len(), 1);
    assert_eq!(statement_spans[0].semantic_block().index(), 0);
    assert_eq!(statement_spans[0].statement_ordinal(), 0);
    assert_eq!(statement_spans[0].kernel_ir_block(), BlockId(0));
    assert_eq!(statement_spans[0].first_operation_ordinal(), 0);
    assert_eq!(statement_spans[0].operation_count(), 0);
    assert_eq!(
        lowered.correspondence().terminator_operation_spans().len(),
        2
    );
    assert!(
        lowered
            .correspondence()
            .terminator_operation_spans()
            .iter()
            .all(|span| span.operation_count() == 0)
    );
    assert!(
        lowered
            .correspondence()
            .synthetic_operation_spans()
            .is_empty()
    );
    assert_eq!(
        lowered.module().kernels[0].domain,
        LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
        }
    );
    assert_eq!(
        lowered.module().kernels[0].workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );
    assert!(!lowered.grants_artifact_or_launch_authority());
}

#[test]
fn mutable_scalar_loop_uses_block_parameters_and_backedge_arguments() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        scalar_loop_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let blocks = &lowered.module().functions[0].body.as_ref().unwrap().blocks;
    let header = blocks.iter().find(|block| block.id == BlockId(1)).unwrap();
    let body = blocks.iter().find(|block| block.id == BlockId(2)).unwrap();
    assert_eq!(header.parameters.len(), 1);
    assert_eq!(body.parameters.len(), 1);
    assert!(matches!(
        body.terminator.as_ref().unwrap(),
        fe2o3_kernel_ir::Terminator::Branch { target, arguments }
            if *target == BlockId(1) && arguments.len() == 1
    ));
    let body_statement = lowered
        .correspondence()
        .statement_operation_spans()
        .iter()
        .find(|span| span.semantic_block().index() == 2 && span.statement_ordinal() == 0)
        .copied()
        .unwrap();
    assert_eq!(body_statement.first_operation_ordinal(), 0);
    assert_eq!(body_statement.operation_count(), 2);
}

#[test]
fn terminator_emission_has_its_own_exact_operation_span() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        terminator_emission_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let span = lowered
        .correspondence()
        .terminator_operation_spans()
        .iter()
        .find(|span| span.semantic_block().index() == 1)
        .copied()
        .unwrap();
    assert_eq!(span.kernel_ir_block(), BlockId(1));
    assert_eq!(span.first_operation_ordinal(), 0);
    assert_eq!(span.operation_count(), 1);
    lowered.verify_equivalence().unwrap();
}

#[test]
fn exact_rust_enum_bytes_lower_to_payload_and_logical_discriminant() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        direct_enum_constant_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let operations = &lowered.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert_eq!(operations.len(), 2);
    assert!(matches!(
        operations[0].kind,
        fe2o3_kernel_ir::OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(9))
    ));
    assert!(matches!(
        operations[1].kind,
        fe2o3_kernel_ir::OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(1))
    ));
}

#[test]
fn runtime_assert_failure_trap_has_typed_synthetic_coverage() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        abort_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let [span] = lowered.correspondence().synthetic_operation_spans() else {
        panic!("runtime abort must retain one synthetic trap span");
    };
    assert_eq!(
        span.rule(),
        SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
    );
    assert_eq!(span.kernel_ir_block(), BlockId(1));
    assert_eq!(span.first_operation_ordinal(), 0);
    assert_eq!(span.operation_count(), 1);
    assert_eq!(
        lowered.correspondence().terminator_operation_spans()[0].operation_count(),
        0
    );
    lowered.verify_equivalence().unwrap();
    assert!(!lowered.grants_artifact_or_launch_authority());
}

#[test]
fn ranked_checks_remain_in_custody_through_kir_and_formal_memory() {
    for (rank, expected_domain) in [
        (
            1,
            LaunchDomain::D1 {
                x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
            },
        ),
        (
            2,
            LaunchDomain::D2 {
                x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
                y: fe2o3_kernel_ir::LaunchExtent::Dynamic,
            },
        ),
        (
            3,
            LaunchDomain::D3 {
                x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
                y: fe2o3_kernel_ir::LaunchExtent::Dynamic,
                z: fe2o3_kernel_ir::LaunchExtent::Dynamic,
            },
        ),
    ] {
        let kernel = ProductionRankedKernelV1::new(
            "semantic_kir_test",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("semantic_kir_test_module", kernel).unwrap();
        let ranked = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        let receipt =
            ProductionRankedSemanticProjectionReceiptV1::assert_compiler_internal_projection(
                semantic_owner(false, false),
                ranked,
                "func @semantic_kir_test { kernel.return }".to_owned(),
            )
            .unwrap();
        let lowered = ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
            receipt,
            ProductionSemanticKirLimitsV1::default(),
            rank,
        )
        .unwrap();

        assert!(lowered.retains_mandatory_generic_checks());
        assert_eq!(lowered.module().kernels[0].domain, expected_domain);
        assert_eq!(
            lowered.module().kernels[0].workgroup_size,
            Some(WorkgroupSize::new(64, 1, 1))
        );
        lowered.verify_equivalence().unwrap();
        let formal = ProductionFormalMemoryOwnerV1::try_admit(lowered).unwrap();
        assert!(formal.semantic_kir().retains_mandatory_generic_checks());
        assert_eq!(
            formal.witness_extents(),
            match rank {
                1 => [2, 1, 1],
                2 => [2, 2, 1],
                3 => [2, 2, 2],
                _ => unreachable!(),
            }
        );
        let evidence =
            InertCanonicalFormalMemoryAdmissionEvidenceV3::from_live_owner(&formal).unwrap();
        assert_eq!(evidence.witness_extent(), 1_u64 << rank);
        evidence.revalidate().unwrap();
    }
}

#[test]
fn ranked_trap_reaches_gfx942_llvm_as_trap_then_unreachable() {
    let kernel = ProductionRankedKernelV1::new(
        "semantic_bounds_trap_test",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![],
            ProductionRankedTerminatorV1::Trap,
        )],
    )
    .unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("semantic_bounds_trap_module", kernel).unwrap();
    let ranked =
        compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
            .unwrap();
    let receipt = ProductionRankedSemanticProjectionReceiptV1::assert_compiler_internal_projection(
        bounds_assert_owner(),
        ranked,
        "func @semantic_bounds_trap_test { kernel.trap }".to_owned(),
    )
    .unwrap();
    let lowered = ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();

    let trap = AmdGpuDiagnosticOperation::Trap;
    let expected_capabilities = trap.required_capabilities();
    assert!(
        expected_capabilities
            .iter()
            .all(|capability| lowered.module().required_capabilities.contains(capability))
    );
    let function = &lowered.module().functions[0];
    assert!(
        expected_capabilities
            .iter()
            .all(|capability| function.required_capabilities.contains(capability))
    );
    let trap_block = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| {
            block.operations.last().is_some_and(|operation| {
                matches!(
                    &operation.kind,
                    OperationKind::Call { callee, arguments }
                        if matches!(
                            AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                            Some(AmdGpuDiagnosticOperation::Trap)
                        )
                )
            })
        })
        .expect("lowered ranked trap block");
    assert_eq!(trap_block.operations.len(), 1);
    assert!(trap_block.operations[0].memory_effects().is_empty());
    assert!(trap_block.operations[0].has_complete_effect_summary());
    assert!(matches!(
        trap_block.terminator,
        Some(Terminator::Unreachable)
    ));
    let [synthetic] = lowered.correspondence().synthetic_operation_spans() else {
        panic!("bounds failure must retain one synthetic trap span");
    };
    assert_eq!(
        synthetic.rule(),
        SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
    );

    let admitted = ProductionFormalMemoryOwnerV1::try_admit(lowered).unwrap();
    let mut target_module = admitted.semantic_kir().module().clone();
    let target = gfx942_xnack_minus_target_capability();
    let wave = TargetCapability::WaveWidth(WaveWidth::Wave64);
    target_module.required_capabilities.insert(target.clone());
    target_module.required_capabilities.insert(wave.clone());
    let kernel = &mut target_module.kernels[0];
    kernel.required_capabilities.insert(target.clone());
    kernel.required_capabilities.insert(wave.clone());
    let kernel_id = kernel.id.clone();
    let entry_id = kernel.entry.clone();
    let entry = target_module
        .functions
        .iter_mut()
        .find(|function| function.id == entry_id)
        .unwrap();
    entry.required_capabilities.insert(target);
    entry.required_capabilities.insert(wave);
    verify_module(&target_module).unwrap();
    let llvm =
        dialect_amdgcn::lower_kernel_to_gfx942_xnack_minus_llvm_ir(&target_module, &kernel_id)
            .unwrap();
    assert_eq!(
        llvm.matches("declare void @llvm.trap()").count(),
        1,
        "{llvm}"
    );
    assert_eq!(llvm.matches("call void @llvm.trap()").count(), 1, "{llvm}");
    assert!(llvm.contains("call void @llvm.trap()\n  unreachable"));
    assert!(!llvm.contains("call void @llvm.trap()\n  ret"));
    assert!(!llvm.contains("call void @llvm.trap()\n  br"));

    assert!(
        dialect_amdgcn::lower_kernel_to_llvm_ir(&target_module, &kernel_id)
            .unwrap_err()
            .contains(dialect_amdgcn::LoweringDiagnosticCode::UnsupportedCapability)
    );
}

#[test]
fn independent_lowerings_are_deterministic() {
    let first = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let second = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(first.module(), second.module());
    assert_eq!(first.correspondence(), second.correspondence());
    assert_eq!(
        first.canonical_kernel_ir_v7().canonical_bytes(),
        second.canonical_kernel_ir_v7().canonical_bytes(),
    );
    assert_eq!(
        first.canonical_kernel_ir_v7_identity(),
        second.canonical_kernel_ir_v7_identity(),
    );
    assert_eq!(
        first.canonical_kernel_ir_v7_identity().canonical_length(),
        first.canonical_kernel_ir_v7().canonical_bytes().len() as u64,
    );
    assert_eq!(
        decode_module_v7(first.canonical_kernel_ir_v7().canonical_bytes()).unwrap(),
        *first.module(),
    );
    first.canonical_kernel_ir_v7().revalidate().unwrap();
}

#[test]
fn formal_memory_admission_retains_exact_kir_without_authority() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let admitted = ProductionFormalMemoryOwnerV1::try_admit(lowered).unwrap();

    admitted.verify_equivalence().unwrap();
    assert_eq!(
        decode_module_v7(
            admitted
                .semantic_kir()
                .canonical_kernel_ir_v7()
                .canonical_bytes(),
        )
        .unwrap(),
        *admitted.semantic_kir().module(),
    );
    assert_eq!(
        admitted.witness_extent(),
        PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1,
    );
    assert!(admitted.obligations().accesses().is_empty());
    assert!(admitted.obligations().bounds_requirements().is_empty());
    assert!(
        admitted
            .obligations()
            .inter_invocation_conflicts()
            .is_empty()
    );
    assert!(!admitted.grants_artifact_or_launch_authority());
}

#[test]
fn unsupported_statement_and_terminator_fail_closed() {
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            semantic_owner(true, false),
            ProductionSemanticKirLimitsV1::default(),
        ),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            block: Some(0),
            statement: Some(0),
            ..
        })
    ));
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            semantic_owner(false, true),
            ProductionSemanticKirLimitsV1::default(),
        ),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            block: Some(1),
            statement: None,
            ..
        })
    ));
}

#[test]
fn checked_arithmetic_lowers_to_ordered_v6_results_with_exact_span() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        checked_arithmetic_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();

    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let operations = &body.blocks[0].operations;
    assert_eq!(operations.len(), 3);
    let checked = &operations[2];
    assert!(matches!(
        checked.kind,
        OperationKind::Binary {
            op: BinaryOp::Checked(CheckedBinaryOperator::Multiply),
            lhs,
            rhs,
        } if lhs == operations[0].results[0].id && rhs == operations[1].results[0].id
    ));
    assert_eq!(checked.results.len(), 2);
    assert_eq!(checked.results[0].ty, Type::Scalar(ScalarType::U32));
    assert_eq!(checked.results[1].ty, Type::BOOL);
    assert_ne!(checked.results[0].id, checked.results[1].id);

    let [span] = lowered.correspondence().statement_operation_spans() else {
        panic!("checked assignment must have one statement span");
    };
    assert_eq!(span.first_operation_ordinal(), 0);
    assert_eq!(span.operation_count(), 3);
}

#[test]
fn lowering_limits_are_enforced_before_materialization() {
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            semantic_owner(false, false),
            ProductionSemanticKirLimitsV1::new(1, 1, 1),
        ),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Blocks,
            actual: 2,
            limit: 1,
        })
    ));
}

#[test]
fn emitted_operations_are_charged_before_storage() {
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            scalar_loop_owner(),
            ProductionSemanticKirLimitsV1::new_with_max_operations(1, 4, 16, 1),
        ),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations,
            actual: 2,
            limit: 1,
        })
    ));
}
