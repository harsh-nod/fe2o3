use dialect_amdgcn::{
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
};
use dialect_kernel::IndexBinaryKindAttr;
use fe2o3_kernel_ir::{
    AccessMode, AmdGpuDiagnosticOperation, BinaryOp, BlockId, CastKind, CheckedBinaryOperator,
    FunctionRole, LaunchDomain, OperationKind, ScalarType, TargetCapability, Terminator, Type,
    WaveWidth, WorkgroupSize, analyze_interprocedural_effects_v1, decode_module_v8,
    gfx942_xnack_minus_target_capability, gfx950_xnack_minus_target_capability, verify_module,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV3, InertCanonicalMirToKirCorrespondenceEvidenceV5,
    PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1, ProductionCorrespondenceEvidenceErrorV5,
    ProductionFormalMemoryOwnerV1, ProductionRankedAccessSourceV1,
    ProductionRankedSemanticProjectionReceiptV1, ProductionSemanticKirErrorV1,
    ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1, ProductionSemanticKirResourceV1,
    SemanticKirParameterProjectionV1, SemanticKirSyntheticOperationRuleV1,
    validate_borrowed_ranked_semantic_projection_candidate_v1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
    ProductionRankedValueV1, ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
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
            bits: 16,
        } => (
            2,
            SemanticBackendPrimitiveV1::integer(false, 16, 2),
            u16::MAX.into(),
        ),
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        } => (
            4,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            u32::MAX.into(),
        ),
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        } => (
            8,
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            u64::MAX.into(),
        ),
        SemanticScalarTypeV1::Float { bits: 32 } => {
            (4, SemanticBackendPrimitiveV1::float(32, 4), u32::MAX.into())
        }
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

fn direct_abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn shared_reference_abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
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
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn compiler_intrinsic_callable(
    tag: u8,
    inputs: Vec<SemanticAbiValueV1>,
    output: SemanticAbiValueV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    let arguments = inputs
        .into_iter()
        .map(SemanticAbiArgumentV1::source)
        .collect::<Vec<_>>();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        arguments.len() as u32,
        arguments,
        output,
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(tag)),
    }
}

fn gfx950_reduction_admission(
    width: u32,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let unit = SemanticTypeIdV1::from_index(0);
    let context = SemanticTypeIdV1::from_index(1);
    let context_ref = SemanticTypeIdV1::from_index(2);
    let f32_ty = SemanticTypeIdV1::from_index(3);
    let source = SemanticSourceProvenanceV1::unavailable();
    let context_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(70)),
        SemanticLayoutIdentityV1::from_sha256(bytes(70)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let context_reference_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(71)),
        SemanticLayoutIdentityV1::from_sha256(bytes(71)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                context,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    0,
                    1,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let call_edge = |target| {
        SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn,
            SemanticBlockIdV1::from_index(target),
        )
    };
    let context_call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![],
            Some(SemanticCallDestinationV1::new(
                local_place(1, context),
                call_edge(1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let borrow = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(2, context_ref),
            SemanticRvalueV1::new(
                context_ref,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: local_place(1, context),
                },
            ),
        )),
    );
    let value = SemanticOperandV1::Constant(SemanticConstantV1::new(
        f32_ty,
        SemanticConstantValueV1::Scalar(
            SemanticScalarValueV1::new(f32::to_bits(1.0).into(), 4).unwrap(),
        ),
    ));
    let reduction_call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(2),
            vec![SemanticOperandV1::Copy(local_place(2, context_ref)), value],
            Some(SemanticCallDestinationV1::new(
                local_place(3, f32_ty),
                call_edge(2),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let function_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(72)),
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
    let locals = [unit, context, context_ref, f32_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(73 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
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
        SemanticFunctionIdentityV1::from_sha256(bytes(72)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(72)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(72)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(72)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(72)),
        source,
        function_abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            block(80, vec![], context_call),
            block(81, vec![borrow], reduction_call),
            block(82, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"gfx950_collective_width".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(83)),
        contract,
    ));
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        compiler_intrinsic_callable(
            84,
            vec![],
            SemanticAbiValueV1::new(context, SemanticAbiPassModeV1::Ignore),
            SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupContextCurrent { context },
        ),
        compiler_intrinsic_callable(
            85,
            vec![
                shared_reference_abi_value(context_ref),
                direct_abi_value(f32_ty),
            ],
            direct_abi_value(f32_ty),
            SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupReduceF32 {
                context,
                width,
                kind: SemanticSubgroupReductionKindV1::Sum,
            },
        ),
    ];
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![
            unit_type(),
            context_type,
            context_reference_type,
            scalar_type(86, SemanticScalarTypeV1::Float { bits: 32 }),
        ],
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
}

fn gfx950_reduction_owner(width: u32) -> ProductionSemanticMirOwnerV1 {
    ProductionSemanticMirOwnerV1::try_new(
        gfx950_reduction_admission(width).unwrap(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap()
}

fn bind_exact_gfx950_target(mut module: fe2o3_kernel_ir::Module) -> fe2o3_kernel_ir::Module {
    let target = gfx950_xnack_minus_target_capability();
    module.required_capabilities.insert(target.clone());
    module.functions[0]
        .required_capabilities
        .insert(target.clone());
    module.kernels[0].required_capabilities.insert(target);
    module
}

#[test]
fn semantic_collective_widths_lower_through_canonical_kir_to_llvm() {
    for width in [1_u32, 2, 4, 8, 16, 32, 64] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            gfx950_reduction_owner(width),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        let operation =
            &lowered.module().functions[0].body.as_ref().unwrap().blocks[1].operations[1];
        assert!(matches!(
            operation.kind,
            OperationKind::Wave(fe2o3_kernel_ir::WaveOperation {
                kind: fe2o3_kernel_ir::WaveOperationKind::ReduceF32 { tile_width, .. },
                ..
            }) if tile_width == width
        ));
        let target_bound = bind_exact_gfx950_target(lowered.module().clone());
        verify_module(&target_bound).unwrap();
        let llvm = lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(&target_bound).unwrap();
        if width == 1 {
            assert!(llvm.contains("select i1 true, float"));
        } else {
            assert_eq!(
                llvm.matches("call i32 @llvm.amdgcn.ds.bpermute").count(),
                width.trailing_zeros() as usize
            );
        }
    }
}

#[test]
fn semantic_collective_rejects_invalid_widths_before_kir_construction() {
    for width in [0_u32, 3, 65] {
        assert_eq!(
            gfx950_reduction_admission(width).unwrap_err(),
            SemanticMirErrorV1::InvalidFunctionAbi,
            "width {width} must fail closed in semantic MIR admission"
        );
    }
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

fn mutable_u32_reference_type(tag: u8, u32_ty: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    u32_reference_type(tag, u32_ty, SemanticMutabilityV1::Mutable)
}

fn immutable_u32_reference_type(tag: u8, u32_ty: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    u32_reference_type(tag, u32_ty, SemanticMutabilityV1::Immutable)
}

fn u32_reference_type(
    tag: u8,
    u32_ty: SemanticTypeIdV1,
    mutability: SemanticMutabilityV1,
) -> SemanticTypeDeclV1 {
    reference_type(tag, u32_ty, 4, 4, mutability)
}

fn reference_type(
    tag: u8,
    pointee: SemanticTypeIdV1,
    pointee_size: u64,
    pointee_alignment: u64,
    mutability: SemanticMutabilityV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    match mutability {
                        SemanticMutabilityV1::Immutable => {
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        }
                        SemanticMutabilityV1::Mutable => {
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                        }
                    },
                    pointee_size,
                    pointee_alignment,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn mutable_global_u32_pointer_type(tag: u8, u32_ty: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(1, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                u32_ty,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                1,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        ),
    )
}

fn retained_scalar_diamond_owner() -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let private_ref_ty = SemanticTypeIdV1::from_index(3);
    let source = SemanticSourceProvenanceV1::unavailable();
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                local_place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let pointer_dereference = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, u32_ty).unwrap()],
        u32_ty,
    )
    .unwrap();
    let entry = block(
        180,
        vec![
            assign(
                1,
                u32_ty,
                SemanticRvalueKindV1::Use(scalar_constant(u32_ty, 1, 4)),
            ),
            assign(
                2,
                private_ref_ty,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: local_place(1, u32_ty),
                },
            ),
            assign(
                3,
                bool_ty,
                SemanticRvalueKindV1::Use(scalar_constant(bool_ty, 1, 1)),
            ),
        ],
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(local_place(3, bool_ty)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    1,
                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        },
    );
    let left = block(
        181,
        vec![
            assign(
                6,
                u32_ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, u32_ty))),
            ),
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    pointer_dereference,
                    scalar_constant(u32_ty, 7, 4),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            ),
            assign(
                7,
                u32_ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, u32_ty))),
            ),
            assign(
                4,
                u32_ty,
                SemanticRvalueKindV1::Use(scalar_constant(u32_ty, 10, 4)),
            ),
        ],
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
    );
    let right = block(
        182,
        vec![assign(
            4,
            u32_ty,
            SemanticRvalueKindV1::Use(scalar_constant(u32_ty, 20, 4)),
        )],
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
    );
    let join = block(
        183,
        vec![assign(
            5,
            u32_ty,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Copy(local_place(1, u32_ty)),
                right: SemanticOperandV1::Copy(local_place(4, u32_ty)),
            },
        )],
        SemanticTerminatorKindV1::Return,
    );
    let locals = [
        unit,
        u32_ty,
        private_ref_ty,
        bool_ty,
        u32_ty,
        u32_ty,
        u32_ty,
        u32_ty,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(190 + index as u8)),
            ty,
            if index == 0 {
                SemanticLocalRoleV1::Return
            } else {
                SemanticLocalRoleV1::Temporary
            },
            source,
        )
    })
    .collect();
    owner_from_parts(
        vec![
            unit_type(),
            scalar_type(
                196,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            scalar_type(197, SemanticScalarTypeV1::Bool),
            mutable_u32_reference_type(198, u32_ty),
        ],
        locals,
        0,
        vec![entry, left, right, join],
        b"semantic_retained_scalar_diamond_test",
    )
}

fn retained_scalar_loop_owner() -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let reference_ty = SemanticTypeIdV1::from_index(3);
    let source = SemanticSourceProvenanceV1::unavailable();
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                local_place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let entry = block(
        200,
        vec![
            assign(
                1,
                u32_ty,
                SemanticRvalueKindV1::Use(scalar_constant(u32_ty, 0, 4)),
            ),
            assign(
                2,
                reference_ty,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: local_place(1, u32_ty),
                },
            ),
        ],
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
    );
    let header = block(
        201,
        vec![assign(
            3,
            bool_ty,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: SemanticOperandV1::Copy(local_place(1, u32_ty)),
                right: scalar_constant(u32_ty, 3, 4),
            },
        )],
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(local_place(3, bool_ty)),
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
        202,
        vec![
            assign(
                4,
                u32_ty,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: SemanticOperandV1::Copy(local_place(1, u32_ty)),
                    right: scalar_constant(u32_ty, 1, 4),
                },
            ),
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(2),
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Dereference,
                                u32_ty,
                            )
                            .unwrap(),
                        ],
                        u32_ty,
                    )
                    .unwrap(),
                    SemanticOperandV1::Copy(local_place(4, u32_ty)),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            ),
        ],
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
    );
    let exit = block(203, vec![], SemanticTerminatorKindV1::Return);
    let locals = [unit, u32_ty, reference_ty, bool_ty, u32_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(210 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    owner_from_parts(
        vec![
            unit_type(),
            scalar_type(
                215,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            scalar_type(216, SemanticScalarTypeV1::Bool),
            mutable_u32_reference_type(217, u32_ty),
        ],
        locals,
        0,
        vec![entry, header, body, exit],
        b"semantic_retained_scalar_loop_test",
    )
}

fn retained_scalar_argument_owner() -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let shared_ref_ty = SemanticTypeIdV1::from_index(2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(218)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(u32_ty))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                local_place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let helper_call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![SemanticOperandV1::Copy(local_place(7, u32_ty))],
            Some(SemanticCallDestinationV1::new(
                local_place(5, u32_ty),
                edge(SemanticEdgeRoleV1::CallReturn, 2),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(218)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(218)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(218)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(218)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(218)),
        source,
        abi,
        vec![
            return_local(),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(219)),
                u32_ty,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(220)),
                u32_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(221)),
                u32_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(234)),
                shared_ref_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(235)),
                u32_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(244)),
                u32_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(245)),
                u32_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                222,
                vec![
                    assign(
                        2,
                        u32_ty,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, u32_ty))),
                    ),
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                            local_place(1, u32_ty),
                            scalar_constant(u32_ty, 9, 4),
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                    assign(
                        3,
                        u32_ty,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, u32_ty))),
                    ),
                    assign(
                        4,
                        shared_ref_ty,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: local_place(1, u32_ty),
                        },
                    ),
                    assign(
                        6,
                        u32_ty,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(4),
                                vec![
                                    SemanticProjectionV1::new(
                                        SemanticProjectionKindV1::Dereference,
                                        u32_ty,
                                    )
                                    .unwrap(),
                                ],
                                u32_ty,
                            )
                            .unwrap(),
                        )),
                    ),
                ],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(
                237,
                vec![assign(
                    7,
                    u32_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(4),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Dereference,
                                    u32_ty,
                                )
                                .unwrap(),
                            ],
                            u32_ty,
                        )
                        .unwrap(),
                    )),
                )],
                helper_call,
            ),
            block(238, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"semantic_retained_scalar_argument_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(223)),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(239)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(u32_ty))],
        direct_abi_value(u32_ty),
    )
    .unwrap();
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(240)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(240)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(240)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(240)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(240)),
        source,
        helper_abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(241)),
                u32_ty,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(242)),
                u32_ty,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block(
            243,
            vec![assign(
                0,
                u32_ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, u32_ty))),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![
            unit_type(),
            scalar_type(
                224,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            immutable_u32_reference_type(236, u32_ty),
        ],
        vec![],
        vec![],
        vec![],
        vec![function, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn retained_pointer_atomic_rmw_owner() -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let pointer_ty = SemanticTypeIdV1::from_index(2);
    let pointer_reference_ty = SemanticTypeIdV1::from_index(3);
    let source = SemanticSourceProvenanceV1::unavailable();
    let atomic_address = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, u32_ty).unwrap()],
        u32_ty,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(225)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(pointer_ty))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(225)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(225)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(225)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(225)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(225)),
        source,
        abi,
        vec![
            return_local(),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(226)),
                pointer_ty,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(227)),
                u32_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(228)),
                pointer_reference_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block(
            229,
            vec![
                SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        local_place(3, pointer_reference_ty),
                        SemanticRvalueV1::new(
                            pointer_reference_ty,
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Mutable,
                                place: local_place(1, pointer_ty),
                            },
                        ),
                    )),
                ),
                SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
                        local_place(2, u32_ty),
                        atomic_address,
                        scalar_constant(u32_ty, 1, 4),
                        SemanticAtomicRmwOpV1::Add,
                        SemanticAtomicAccessV1::new(
                            SemanticAtomicOrderingV1::Relaxed,
                            SemanticAtomicScopeV1::Agent,
                        ),
                    )),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"semantic_retained_pointer_atomic_rmw_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(230)),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![
            unit_type(),
            scalar_type(
                231,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            mutable_global_u32_pointer_type(232, u32_ty),
            reference_type(233, pointer_ty, 8, 8, SemanticMutabilityV1::Mutable),
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

#[derive(Clone, Copy)]
enum RetainedScalarInvalidationV1 {
    Uninitialized,
    Move,
    StorageDead,
}

fn retained_scalar_invalidation_owner(
    invalidation: RetainedScalarInvalidationV1,
) -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let reference_ty = SemanticTypeIdV1::from_index(2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                local_place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let mut statements = Vec::new();
    if !matches!(invalidation, RetainedScalarInvalidationV1::Uninitialized) {
        statements.push(assign(
            1,
            u32_ty,
            SemanticRvalueKindV1::Use(scalar_constant(u32_ty, 3, 4)),
        ));
    }
    statements.push(assign(
        2,
        reference_ty,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: local_place(1, u32_ty),
        },
    ));
    match invalidation {
        RetainedScalarInvalidationV1::Uninitialized => {}
        RetainedScalarInvalidationV1::Move => statements.push(assign(
            3,
            u32_ty,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(local_place(1, u32_ty))),
        )),
        RetainedScalarInvalidationV1::StorageDead => statements.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
        )),
    }
    statements.push(assign(
        4,
        u32_ty,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, u32_ty))),
    ));
    let locals = [unit, u32_ty, reference_ty, u32_ty, u32_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(225 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    owner_from_parts(
        vec![
            unit_type(),
            scalar_type(
                230,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            mutable_u32_reference_type(231, u32_ty),
        ],
        locals,
        0,
        vec![block(232, statements, SemanticTerminatorKindV1::Return)],
        b"semantic_retained_scalar_invalidation_test",
    )
}

#[test]
fn retained_scalar_slot_coexists_with_sparse_phi_and_observes_alias_writes() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        retained_scalar_diamond_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let entry = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(0))
        .unwrap();
    let slot = entry
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                address_space: fe2o3_kernel_ir::AddressSpace::Private,
                alignment: 4,
                ..
            } => operation.results.first().map(|result| result.id),
            _ => None,
        })
        .expect("retained scalar must have one exact private slot");
    assert_eq!(
        entry
            .operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::Alloca { .. }))
            .count(),
        1
    );
    let left = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(1))
        .unwrap();
    let slot_accesses = left
        .operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::Load { pointer, .. } if pointer == slot => Some("load"),
            OperationKind::Store { pointer, .. } if pointer == slot => Some("store"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(slot_accesses, ["load", "store", "load"]);
    let join = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(3))
        .unwrap();
    assert_eq!(
        join.parameters.len(),
        1,
        "only the branch value needs a phi"
    );
    assert!(join.operations.iter().any(
        |operation| matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == slot)
    ));
}

#[test]
fn retained_scalar_slot_analysis_obeys_the_production_work_limit() {
    let error = ProductionSemanticKirOwnerV1::try_lower(
        retained_scalar_diamond_owner(),
        ProductionSemanticKirLimitsV1::new_with_max_operations(16, 64, 64, 1),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            ..
        }
    ));
}

#[test]
fn retained_scalar_slot_is_mutated_through_a_borrow_across_a_loop_backedge() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        retained_scalar_loop_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let entry = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(0))
        .unwrap();
    let slot = entry
        .operations
        .iter()
        .find_map(|operation| {
            matches!(operation.kind, OperationKind::Alloca { .. }).then(|| operation.results[0].id)
        })
        .unwrap();
    let header = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(1))
        .unwrap();
    let loop_body = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(2))
        .unwrap();
    assert!(header.operations.iter().any(
        |operation| matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == slot)
    ));
    assert!(loop_body.operations.iter().any(
        |operation| matches!(operation.kind, OperationKind::Store { pointer, .. } if pointer == slot)
    ));
    assert!(
        header.parameters.is_empty(),
        "slot state must not leak into SSA phis"
    );
}

#[test]
fn retained_scalar_argument_is_stored_once_in_the_entry_slot() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        retained_scalar_argument_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    lowered
        .canonical_kernel_ir_v11()
        .expect("pointer narrowing requires canonical Kernel IR V11")
        .revalidate()
        .unwrap();
    assert!(lowered.canonical_kernel_ir_v8().is_none());
    assert!(lowered.canonical_kernel_ir_v8_identity().is_none());
    let function = &lowered.module().functions[0];
    let body = function.body.as_ref().unwrap();
    let [span] = lowered.correspondence().synthetic_operation_spans() else {
        panic!("retained argument must have one private-slot prologue span");
    };
    assert_eq!(
        span.rule(),
        SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage
    );
    assert_eq!(span.first_operation_ordinal(), 0);
    assert_eq!(span.operation_count(), 2);
    let parameter = body.parameters[0];
    let slot = body.blocks[0].operations[0].results[0].id;
    assert!(matches!(
        body.blocks[0].operations[0].kind,
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            address_space: fe2o3_kernel_ir::AddressSpace::Private,
            alignment: 4,
            ..
        }
    ));
    assert!(matches!(
        body.blocks[0].operations[1].kind,
        OperationKind::Store {
            pointer,
            value,
            ..
        } if pointer == slot && value == parameter
    ));
    assert!(body.blocks[0].operations.iter().skip(2).any(
        |operation| matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == slot)
    ));
    let read_only_pointer = body.blocks[0]
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value,
                to:
                    Type::Pointer(fe2o3_kernel_ir::PointerType {
                        pointee,
                        address_space: fe2o3_kernel_ir::AddressSpace::Private,
                        access: AccessMode::ReadOnly,
                    }),
            } if *value == slot && pointee.as_ref() == &Type::Scalar(ScalarType::U32) => {
                operation.results.first().map(|result| result.id)
            }
            _ => None,
        })
        .expect("shared retained borrow must explicitly restrict the slot pointer");
    assert!(body.blocks[0].operations.iter().any(
        |operation| matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == read_only_pointer)
    ));
    match &body.blocks[0].terminator {
        Some(Terminator::Branch { target, arguments }) if *target == BlockId(1) => {
            assert!(arguments.is_empty());
        }
        other => panic!("expected branch carrying the restricted pointer, found {other:?}"),
    }
    let call_block = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(1))
        .unwrap();
    let edge_load = call_block
        .operations
        .iter()
        .find_map(|operation| {
            matches!(operation.kind, OperationKind::Load { pointer, .. } if pointer == read_only_pointer)
                .then(|| operation.results[0].id)
        })
        .expect("the dominating restricted pointer must remain readable after the CFG edge");
    assert!(call_block.operations.iter().any(|operation| {
        matches!(&operation.kind, OperationKind::Call { arguments, .. } if arguments == &[edge_load])
    }));
    let helper = &lowered.module().functions[1];
    assert_eq!(
        helper.signature.parameters,
        vec![Type::Scalar(ScalarType::U32)]
    );
    let mut amdgpu_module = lowered.module().clone();
    let target = gfx942_xnack_minus_target_capability();
    amdgpu_module.required_capabilities.insert(target.clone());
    for function in &mut amdgpu_module.functions {
        if function.body.is_some() {
            function.required_capabilities.insert(target.clone());
        }
    }
    amdgpu_module.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    amdgpu_module.kernels[0]
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    amdgpu_module.kernels[0]
        .required_capabilities
        .insert(target);
    verify_module(&amdgpu_module).unwrap();
    let llvm = lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(&amdgpu_module).unwrap();
    assert!(llvm.contains("select i1 true, ptr addrspace(5)"));
}

#[test]
fn retained_pointer_atomic_rmw_loads_the_pointer_value_from_its_private_slot() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        retained_pointer_atomic_rmw_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let operations = &body.blocks[0].operations;
    let slot = operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::Alloca {
                element: Type::Pointer(pointer),
                address_space: fe2o3_kernel_ir::AddressSpace::Private,
                alignment: 8,
                ..
            } if pointer.address_space == fe2o3_kernel_ir::AddressSpace::Global => {
                Some(operation.results[0].id)
            }
            _ => None,
        })
        .expect("retained thin pointer must have one private slot");
    let loaded_pointer = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::Load { pointer, .. } if pointer == slot => Some(operation.results[0].id),
            _ => None,
        })
        .expect("atomic address must load the pointer value from the slot");
    assert!(operations.iter().any(|operation| matches!(
        operation.kind,
        OperationKind::Atomic(ref atomic)
            if atomic.pointer == loaded_pointer && atomic.pointer != slot
    )));
}

#[test]
fn retained_scalar_slot_rejects_uninitialized_move_and_storage_dead_uses() {
    for (invalidation, failing_statement) in [
        (RetainedScalarInvalidationV1::Uninitialized, 0),
        (RetainedScalarInvalidationV1::Move, 3),
        (RetainedScalarInvalidationV1::StorageDead, 3),
    ] {
        assert!(matches!(
            ProductionSemanticKirOwnerV1::try_lower(
                retained_scalar_invalidation_owner(invalidation),
                ProductionSemanticKirLimitsV1::default(),
            ),
            Err(ProductionSemanticKirErrorV1::MissingLocalDefinition {
                block: 0,
                statement: Some(statement),
                local: 1,
                ..
            }) if statement == failing_statement
        ));
    }
}

fn return_local() -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(62)),
        SemanticTypeIdV1::from_index(0),
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn indexed_slice_borrow_owner() -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let slice_ty = SemanticTypeIdV1::from_index(2);
    let slice_ref_ty = SemanticTypeIdV1::from_index(3);
    let element_ref_ty = SemanticTypeIdV1::from_index(4);
    let usize_ty = SemanticTypeIdV1::from_index(5);
    let data_pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let slice_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(65)),
        SemanticLayoutIdentityV1::from_sha256(bytes(65)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element: u32_ty },
    );
    let slice_ref_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(66)),
        SemanticLayoutIdentityV1::from_sha256(bytes(66)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(
                data_pointer,
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                ),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                slice_ty,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    0,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let element_ref_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(67)),
        SemanticLayoutIdentityV1::from_sha256(bytes(67)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(data_pointer),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                u32_ty,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    let source = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, slice_ty).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                u32_ty,
            )
            .unwrap(),
        ],
        u32_ty,
    )
    .unwrap();
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(3, element_ref_ty),
            SemanticRvalueV1::new(
                element_ref_ty,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: source,
                },
            ),
        )),
    );
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(69)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                slice_ref_ty,
                SemanticAbiPassModeV1::Pair {
                    first: SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(
                            true,
                            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                            true,
                            true,
                            false,
                            true,
                        ),
                        SemanticAbiExtensionV1::None,
                        0,
                        Some(4),
                    )
                    .unwrap(),
                    second: SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                },
            )),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                usize_ty,
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                ),
            )),
        ],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = vec![
        return_local(),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(70)),
            slice_ref_ty,
            SemanticLocalRoleV1::Argument(0),
            SemanticSourceProvenanceV1::unavailable(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(71)),
            usize_ty,
            SemanticLocalRoleV1::Argument(1),
            SemanticSourceProvenanceV1::unavailable(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(72)),
            element_ref_ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ),
    ];
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let contract = SemanticKernelSourceContractV1::new(
        Some(SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap()),
        None,
        None,
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(69)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(69)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(69)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(69)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(69)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block(73, vec![statement], SemanticTerminatorKindV1::Return)],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"semantic_indexed_slice_borrow_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(74)),
        contract,
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![
            unit_type(),
            scalar_type(
                64,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            slice_type,
            slice_ref_type,
            element_ref_type,
            scalar_type(
                68,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
            ),
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

fn promoted_enum_payload_owner(
    extract_before_variant_switch: bool,
    replace_after_discriminant: bool,
    second_branch_projection: u32,
) -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let bool_ty = SemanticTypeIdV1::from_index(2);
    let enum_ty = SemanticTypeIdV1::from_index(3);
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
    let enum_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(94)),
        SemanticLayoutIdentityV1::from_sha256(bytes(94)),
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
    let assign = |destination: SemanticPlaceV1, ty, value| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let variant = |index, value| {
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::EnumVariant(index),
                vec![scalar_constant(u32_ty, value, 4)],
            )
            .unwrap(),
        )
    };
    let field_place = |variant| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(variant), enum_ty)
                    .unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), u32_ty).unwrap(),
            ],
            u32_ty,
        )
        .unwrap()
    };
    let mut join_statements = Vec::new();
    if extract_before_variant_switch {
        join_statements.push(assign(
            local_place(3, u32_ty),
            u32_ty,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field_place(0))),
        ));
    }
    join_statements.push(assign(
        local_place(2, u32_ty),
        u32_ty,
        SemanticRvalueKindV1::Discriminant(local_place(1, enum_ty)),
    ));
    if replace_after_discriminant {
        join_statements.push(assign(local_place(1, enum_ty), enum_ty, variant(1, 11)));
    }
    let blocks = vec![
        block(
            95,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: scalar_constant(bool_ty, 1, 1),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            96,
            vec![assign(local_place(1, enum_ty), enum_ty, variant(0, 7))],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            97,
            vec![assign(local_place(1, enum_ty), enum_ty, variant(1, 9))],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            98,
            join_statements,
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(local_place(2, u32_ty)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![
                        SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue, 4)),
                        SemanticSwitchTargetV1::new(1, edge(SemanticEdgeRoleV1::SwitchValue, 5)),
                    ],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 6),
                )
                .unwrap(),
            },
        ),
        block(
            99,
            vec![assign(
                local_place(3, u32_ty),
                u32_ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field_place(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
        block(
            100,
            vec![assign(
                local_place(4, u32_ty),
                u32_ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field_place(
                    second_branch_projection,
                ))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
        block(101, vec![], SemanticTerminatorKindV1::Unreachable),
    ];
    let locals = [unit, enum_ty, u32_ty, u32_ty, u32_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(102 + index as u8)),
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
    owner_from_parts(
        vec![
            unit_type(),
            scalar_type(
                92,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            scalar_type(93, SemanticScalarTypeV1::Bool),
            enum_type,
        ],
        locals,
        0,
        blocks,
        b"semantic_promoted_enum_payload_test",
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

fn checked_noop_ranked_input(
    function_name: &str,
) -> fe2o3_pliron::ProductionRankedKernelLoweringInputV1 {
    let kernel = ProductionRankedKernelV1::new(
        function_name,
        0,
        vec![ProductionRankedBlockV1::new(
            vec![],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("borrowed_projection_validation", kernel).unwrap();
    compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
        .unwrap()
}

#[test]
fn borrowed_projection_validation_rejects_root_ir_identity_and_access_hostility() {
    let owner = semantic_owner(false, false);
    let exact = checked_noop_ranked_input("semantic_kir_test");
    validate_borrowed_ranked_semantic_projection_candidate_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &exact,
        "func @semantic_kir_test { kernel.return }",
        &[],
    )
    .unwrap();

    let missing_root = validate_borrowed_ranked_semantic_projection_candidate_v1(
        &owner,
        SemanticFunctionIdV1::from_index(1),
        &exact,
        "func @semantic_kir_test { kernel.return }",
        &[],
    )
    .unwrap_err();
    assert!(
        missing_root
            .to_string()
            .contains("ranked projection receipt has no exact kernel root")
    );

    let empty_ir = validate_borrowed_ranked_semantic_projection_candidate_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &exact,
        "",
        &[],
    )
    .unwrap_err();
    assert!(
        empty_ir
            .to_string()
            .contains("ranked projection receipt has empty diagnostic IR")
    );

    let invalid_access = [ProductionRankedAccessSourceV1::new(0, None, 0, 0, 0)];
    let access_error = validate_borrowed_ranked_semantic_projection_candidate_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &exact,
        "func @semantic_kir_test { kernel.return }",
        &invalid_access,
    )
    .unwrap_err();
    assert!(
        access_error
            .to_string()
            .contains("ranked projection receipt has invalid access correspondence")
    );

    let substituted = checked_noop_ranked_input("substituted_kernel");
    let identity_error = validate_borrowed_ranked_semantic_projection_candidate_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &substituted,
        "func @substituted_kernel { kernel.return }",
        &[],
    )
    .unwrap_err();
    assert!(
        identity_error
            .to_string()
            .contains("ranked projection receipt function identity changed")
    );
}

#[test]
fn consuming_projection_receipt_uses_the_borrowed_structural_validation() {
    let borrowed_error = validate_borrowed_ranked_semantic_projection_candidate_v1(
        &semantic_owner(false, false),
        SemanticFunctionIdV1::from_index(0),
        &checked_noop_ranked_input("substituted_kernel"),
        "func @substituted_kernel { kernel.return }",
        &[],
    )
    .unwrap_err()
    .to_string();
    let consuming_error =
        ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate(
            semantic_owner(false, false),
            checked_noop_ranked_input("substituted_kernel"),
            "func @substituted_kernel { kernel.return }".to_owned(),
            vec![],
        )
        .unwrap_err()
        .to_string();

    assert_eq!(borrowed_error, consuming_error);
    assert!(consuming_error.contains("ranked projection receipt function identity changed"));
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
    let entry = blocks.iter().find(|block| block.id == BlockId(0)).unwrap();
    let header = blocks.iter().find(|block| block.id == BlockId(1)).unwrap();
    let body = blocks.iter().find(|block| block.id == BlockId(2)).unwrap();
    let exit = blocks.iter().find(|block| block.id == BlockId(3)).unwrap();
    assert!(entry.parameters.is_empty());
    assert_eq!(header.parameters.len(), 1);
    assert_eq!(header.parameters[0].ty, Type::Scalar(ScalarType::U32));
    assert!(body.parameters.is_empty());
    assert!(exit.parameters.is_empty());

    let entry_value = entry.operations[0].results[0].id;
    let Terminator::Branch { target, arguments } = entry.terminator.as_ref().unwrap() else {
        panic!("loop entry must branch to the header");
    };
    assert_eq!(*target, BlockId(1));
    assert_eq!(arguments, &[entry_value]);

    let Terminator::ConditionalBranch {
        condition,
        then_arguments,
        else_arguments,
        ..
    } = header.terminator.as_ref().unwrap()
    else {
        panic!("loop header must branch on its computed condition");
    };
    assert_eq!(*condition, header.operations[1].results[0].id);
    assert!(then_arguments.is_empty());
    assert!(else_arguments.is_empty());

    let body_constant = body.operations[0].results[0].id;
    let body_value = body.operations[1].results[0].id;
    assert!(matches!(
        body.operations[1].kind,
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs,
            rhs,
        } if lhs == header.parameters[0].id && rhs == body_constant
    ));
    let Terminator::Branch { target, arguments } = body.terminator.as_ref().unwrap() else {
        panic!("loop body must branch back to the header");
    };
    assert_eq!(*target, BlockId(1));
    assert_eq!(arguments, &[body_value]);

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
fn optimized_indexed_slice_borrow_lowers_to_an_element_address() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        indexed_slice_borrow_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();

    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let operations = &lowered.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::SliceData { .. }))
    );
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::GetElementPointer { .. }))
    );
    assert!(
        !operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Load { .. }))
    );
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
fn promoted_enum_payloads_use_private_storage_and_variant_dominance() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        promoted_enum_payload_owner(false, false, 1),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();

    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let entry = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(0))
        .unwrap();
    assert_eq!(
        entry
            .operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::Alloca { .. }))
            .count(),
        2
    );
    for block in [1, 2] {
        let block = body
            .blocks
            .iter()
            .find(|candidate| candidate.id == BlockId(block))
            .unwrap();
        assert!(
            block
                .operations
                .iter()
                .any(|operation| matches!(operation.kind, OperationKind::Store { .. }))
        );
    }
    for block in [4, 5] {
        let block = body
            .blocks
            .iter()
            .find(|candidate| candidate.id == BlockId(block))
            .unwrap();
        assert!(
            block.parameters.is_empty(),
            "sparse SSA must reuse the dominating merge value without redundant transport",
        );
        assert!(
            block
                .operations
                .iter()
                .any(|operation| matches!(operation.kind, OperationKind::Load { .. }))
        );
    }
    assert!(
        lowered
            .correspondence()
            .synthetic_operation_spans()
            .iter()
            .all(|span| span.rule() == SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage)
    );
}

#[test]
fn promoted_enum_payload_extraction_before_the_variant_edge_fails_closed() {
    let result = ProductionSemanticKirOwnerV1::try_lower(
        promoted_enum_payload_owner(true, false, 1),
        ProductionSemanticKirLimitsV1::default(),
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::EnumPayloadUnavailable {
            block: 3,
            local: 1,
            variant: 0,
            field: 0,
            ..
        })
    ));
}

#[test]
fn promoted_enum_reassignment_invalidates_a_stale_discriminant_edge() {
    let result = ProductionSemanticKirOwnerV1::try_lower(
        promoted_enum_payload_owner(false, true, 1),
        ProductionSemanticKirLimitsV1::default(),
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            block: Some(4),
            detail: "enum downcast does not match its known variant",
            ..
        })
    ));
}

#[test]
fn promoted_enum_payload_rejects_a_sibling_variants_projection() {
    let result = ProductionSemanticKirOwnerV1::try_lower(
        promoted_enum_payload_owner(false, false, 0),
        ProductionSemanticKirLimitsV1::default(),
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            block: Some(5),
            detail: "enum downcast does not match its known variant",
            ..
        })
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
            ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate(
                semantic_owner(false, false),
                ranked,
                "func @semantic_kir_test { kernel.return }".to_owned(),
                vec![],
            )
            .unwrap();
        let lowered = ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
            receipt,
            ProductionSemanticKirLimitsV1::default(),
            rank,
        )
        .unwrap();

        assert!(lowered.retains_mandatory_generic_checks());
        let translation = lowered
            .mir_pliron_translation_validation()
            .expect("production ranked custody requires independent translation validation");
        assert!(!translation.reconciled_projection_remains_trusted());
        assert!(!translation.claims_indexed_address_equivalence());
        assert!(!translation.claims_complete_operational_equivalence());
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
            Some(match rank {
                1 => [2, 1, 1],
                2 => [2, 2, 1],
                3 => [2, 2, 2],
                _ => unreachable!(),
            })
        );
        let evidence =
            InertCanonicalFormalMemoryAdmissionEvidenceV3::from_live_owner(&formal).unwrap();
        assert_eq!(evidence.witness_extent(), 1_u64 << rank);
        evidence.revalidate().unwrap();
    }
}

#[test]
fn normalized_ranked_recipe_remains_in_custody_through_kir_lowering() {
    let local = |identity| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity));
    let kernel = ProductionRankedKernelV1::new(
        "semantic_kir_test",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(0),
                    value: 6,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(1),
                    value: 7,
                },
                ProductionRankedOperationV1::IndexBinary {
                    result: ProductionRankedValueIdV1::new(2),
                    kind: IndexBinaryKindAttr::Multiply,
                    lhs: local(0),
                    rhs: local(1),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("ranked recipe");
    assert!(matches!(
        kernel.blocks()[0].operations()[2],
        ProductionRankedOperationV1::IndexConstant {
            result,
            value: 42,
        } if result == ProductionRankedValueIdV1::new(2)
    ));

    let construction = ProductionConstructionV1::ranked_kernel("semantic_kir_test_module", kernel)
        .expect("construction");
    let ranked =
        compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
            .expect("ranked checks");
    assert!(matches!(
        ranked.kernel().blocks()[0].operations()[2],
        ProductionRankedOperationV1::IndexConstant { value: 42, .. }
    ));

    let receipt =
        ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate(
            semantic_owner(false, false),
            ranked,
            concat!(
                "func @semantic_kir_test {\n",
                "  %0 = kernel.index_constant 6\n",
                "  %1 = kernel.index_constant 7\n",
                "  %2 = kernel.index_constant 42\n",
                "  kernel.return\n",
                "}\n",
            )
            .to_owned(),
            vec![],
        )
        .expect("projection receipt");
    assert!(matches!(
        receipt.lowering().kernel().blocks()[0].operations()[2],
        ProductionRankedOperationV1::IndexConstant { value: 42, .. }
    ));

    let lowered = ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
        1,
    )
    .expect("KIR lowering");
    assert!(lowered.retains_mandatory_generic_checks());
    assert!(lowered.mir_pliron_translation_validation().is_some());
    lowered.verify_equivalence().expect("KIR equivalence");
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
    let receipt =
        ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate(
            bounds_assert_owner(),
            ranked,
            "func @semantic_bounds_trap_test { kernel.trap }".to_owned(),
            vec![],
        )
        .unwrap();
    let lowered = ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
        1,
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
        first
            .canonical_kernel_ir_v8()
            .expect("the fixture lowers to Kernel IR V8")
            .canonical_bytes(),
        second
            .canonical_kernel_ir_v8()
            .expect("the fixture lowers to Kernel IR V8")
            .canonical_bytes(),
    );
    assert_eq!(
        first.canonical_kernel_ir_v8_identity(),
        second.canonical_kernel_ir_v8_identity(),
    );
    assert_eq!(
        first
            .canonical_kernel_ir_v8_identity()
            .expect("the fixture lowers to Kernel IR V8")
            .canonical_length(),
        first
            .canonical_kernel_ir_v8()
            .expect("the fixture lowers to Kernel IR V8")
            .canonical_bytes()
            .len() as u64,
    );
    assert_eq!(
        decode_module_v8(
            first
                .canonical_kernel_ir_v8()
                .expect("the fixture lowers to Kernel IR V8")
                .canonical_bytes(),
        )
        .unwrap(),
        *first.module(),
    );
    first
        .canonical_kernel_ir_v8()
        .expect("the fixture lowers to Kernel IR V8")
        .revalidate()
        .unwrap();
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
        decode_module_v8(
            admitted
                .semantic_kir()
                .canonical_kernel_ir_v8()
                .expect("the fixture lowers to Kernel IR V8")
                .canonical_bytes(),
        )
        .unwrap(),
        *admitted.semantic_kir().module(),
    );
    assert_eq!(
        admitted.witness_extent(),
        PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1,
    );
    let obligations = admitted
        .obligations()
        .expect("single-root admission retains the singleton obligations view");
    assert!(obligations.accesses().is_empty());
    assert!(obligations.bounds_requirements().is_empty());
    assert!(obligations.inter_invocation_conflicts().is_empty());
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
fn lowering_analysis_is_charged_before_operation_emission() {
    let result = ProductionSemanticKirOwnerV1::try_lower(
        scalar_loop_owner(),
        ProductionSemanticKirLimitsV1::new_with_max_operations(1, 4, 16, 1),
    );
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 2,
                limit: 1,
            })
        ),
        "unexpected lowering resource result: {result:?}"
    );
}

#[derive(Clone, Copy)]
enum DefinedHelperFixtureV1 {
    Valid,
    BranchReturn,
    Impure,
    Recursive,
    TailCall,
    MissingCallable,
    MissingFunction,
    MismatchedArgument,
}

#[derive(Clone, Copy)]
enum TransparentHelperCarrierFixtureV1 {
    Exact,
    LooseStorageOnly,
    WrongOwnership,
}

const TRANSPARENT_HELPER_U16_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const TRANSPARENT_HELPER_BOOL_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const TRANSPARENT_HELPER_CARRIER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn transparent_helper_u16_backend_v1(valid_end: u128) -> SemanticBackendReprV1 {
    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 16, 2),
        SemanticScalarValidityRangeV1::new(0, valid_end),
    ))
}

fn transparent_helper_bool_abi_value_v1() -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        TRANSPARENT_HELPER_BOOL_TYPE,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::ZeroExtend,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn transparent_helper_carrier_types_v1(
    mode: TransparentHelperCarrierFixtureV1,
) -> Vec<SemanticTypeDeclV1> {
    let properties =
        SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true);
    let field = scalar_type(
        230,
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 16,
        },
    )
    .with_rustc_abi_properties(properties);
    let carrier_valid_end = if matches!(mode, TransparentHelperCarrierFixtureV1::LooseStorageOnly) {
        u128::from(u16::MAX) - 1
    } else {
        u16::MAX.into()
    };
    let carrier = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(233)),
        SemanticLayoutIdentityV1::from_sha256(bytes(233)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            2,
            2,
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            transparent_helper_u16_backend_v1(carrier_valid_end),
            None,
            false,
            None,
            2,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![TRANSPARENT_HELPER_U16_TYPE]).unwrap(),
        ),
    )
    .with_rustc_abi_properties(properties);
    vec![
        unit_type(),
        field,
        scalar_type(232, SemanticScalarTypeV1::Bool),
        carrier,
    ]
}

fn transparent_helper_carrier_owner_v1(
    mode: TransparentHelperCarrierFixtureV1,
) -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let edge = |target| {
        SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn,
            SemanticBlockIdV1::from_index(target),
        )
    };
    let aggregate = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(1, TRANSPARENT_HELPER_CARRIER_TYPE),
            SemanticRvalueV1::new(
                TRANSPARENT_HELPER_CARRIER_TYPE,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Aggregate,
                        vec![scalar_constant(TRANSPARENT_HELPER_U16_TYPE, 0x7f80, 2)],
                    )
                    .unwrap(),
                ),
            ),
        )),
    );
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![SemanticOperandV1::Copy(local_place(
            1,
            TRANSPARENT_HELPER_CARRIER_TYPE,
        ))],
        Some(SemanticCallDestinationV1::new(
            local_place(2, TRANSPARENT_HELPER_BOOL_TYPE),
            edge(1),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(233)),
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
    let root = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(234)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(235)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(236)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(237)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(238)),
        source,
        root_abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(239)),
                unit,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(240)),
                TRANSPARENT_HELPER_CARRIER_TYPE,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(241)),
                TRANSPARENT_HELPER_BOOL_TYPE,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(242, vec![aggregate], SemanticTerminatorKindV1::Call(call)),
            block(243, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"transparent_helper_carrier_v1".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(244)),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));

    let helper_ownership = if matches!(mode, TransparentHelperCarrierFixtureV1::WrongOwnership) {
        SemanticSourceArgumentOwnershipV1::SharedBorrow
    } else {
        SemanticSourceArgumentOwnershipV1::ByValue
    };
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(245)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(
            TRANSPARENT_HELPER_CARRIER_TYPE,
        ))],
        transparent_helper_bool_abi_value_v1(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![helper_ownership])
    .unwrap();
    let carrier_field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(0),
                TRANSPARENT_HELPER_U16_TYPE,
            )
            .unwrap(),
        ],
        TRANSPARENT_HELPER_U16_TYPE,
    )
    .unwrap();
    let result = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(0, TRANSPARENT_HELPER_BOOL_TYPE),
            SemanticRvalueV1::new(
                TRANSPARENT_HELPER_BOOL_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::NotEqual,
                    left: SemanticOperandV1::Copy(carrier_field),
                    right: scalar_constant(TRANSPARENT_HELPER_U16_TYPE, 0x7f80, 2),
                },
            ),
        )),
    );
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(246)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(247)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(248)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(249)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(250)),
        source,
        helper_abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(251)),
                TRANSPARENT_HELPER_BOOL_TYPE,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(252)),
                TRANSPARENT_HELPER_CARRIER_TYPE,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block(253, vec![result], SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        transparent_helper_carrier_types_v1(mode),
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn defined_helper_request_v1(mode: DefinedHelperFixtureV1) -> InertSemanticMirRequestV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u64_ty = SemanticTypeIdV1::from_index(1);
    let u32_ty = SemanticTypeIdV1::from_index(2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let edge = |target| {
        SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn,
            SemanticBlockIdV1::from_index(target),
        )
    };
    let root_argument_ty = if matches!(mode, DefinedHelperFixtureV1::MismatchedArgument) {
        u32_ty
    } else {
        u64_ty
    };
    let callable = if matches!(mode, DefinedHelperFixtureV1::MissingCallable) {
        SemanticCallableIdV1::from_index(7)
    } else {
        SemanticCallableIdV1::from_index(1)
    };
    let root_call = SemanticDirectCallV1::new_callable(
        callable,
        vec![SemanticOperandV1::Copy(local_place(1, root_argument_ty))],
        Some(SemanticCallDestinationV1::new(
            local_place(2, u64_ty),
            edge(1),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(200)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(
            root_argument_ty,
        ))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let contract = SemanticKernelSourceContractV1::new(
        Some(SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap()),
        None,
        None,
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(201)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(202)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(203)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(204)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(205)),
        source,
        root_abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(206)),
                unit,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(207)),
                root_argument_ty,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(208)),
                u64_ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(209, vec![], SemanticTerminatorKindV1::Call(root_call)),
            block(210, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"defined_helper_kernel_v1".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(211)),
        contract,
    ));

    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(212)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(u64_ty))],
        direct_abi_value(u64_ty),
    )
    .unwrap();
    let return_assignment = || {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                local_place(0, u64_ty),
                SemanticRvalueV1::new(
                    u64_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(1, u64_ty))),
                ),
            )),
        )
    };
    let helper_blocks = if matches!(mode, DefinedHelperFixtureV1::BranchReturn) {
        let branch = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let otherwise = branch(SemanticEdgeRoleV1::SwitchOtherwise, 2);
        let switch = SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                branch(SemanticEdgeRoleV1::SwitchValue, 1),
            )],
            otherwise,
        )
        .unwrap();
        let assign_constant = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                local_place(0, u64_ty),
                SemanticRvalueV1::new(
                    u64_ty,
                    SemanticRvalueKindV1::Use(scalar_constant(u64_ty, 7, 8)),
                ),
            )),
        );
        vec![
            block(
                212,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(local_place(1, u64_ty)),
                    targets: switch,
                },
            ),
            block(
                213,
                vec![return_assignment()],
                SemanticTerminatorKindV1::Goto(branch(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                214,
                vec![assign_constant],
                SemanticTerminatorKindV1::Goto(branch(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(215, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else if matches!(mode, DefinedHelperFixtureV1::Impure) {
        let barrier = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(2),
            vec![],
            Some(SemanticCallDestinationV1::new(
                local_place(2, unit),
                edge(1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        vec![
            block(212, vec![], SemanticTerminatorKindV1::Call(barrier)),
            block(
                213,
                vec![return_assignment()],
                SemanticTerminatorKindV1::Return,
            ),
        ]
    } else if matches!(mode, DefinedHelperFixtureV1::Recursive) {
        let recursive = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![SemanticOperandV1::Copy(local_place(1, u64_ty))],
            Some(SemanticCallDestinationV1::new(
                local_place(0, u64_ty),
                edge(1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        vec![
            block(214, vec![], SemanticTerminatorKindV1::Call(recursive)),
            block(215, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else if matches!(mode, DefinedHelperFixtureV1::TailCall) {
        let tail = SemanticDirectTailCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![SemanticOperandV1::Copy(local_place(1, u64_ty))],
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        vec![block(216, vec![], SemanticTerminatorKindV1::TailCall(tail))]
    } else {
        vec![block(
            217,
            vec![return_assignment()],
            SemanticTerminatorKindV1::Return,
        )]
    };
    let mut helper_locals = vec![
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(221)),
            u64_ty,
            SemanticLocalRoleV1::Return,
            source,
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(222)),
            u64_ty,
            SemanticLocalRoleV1::Argument(0),
            source,
        ),
    ];
    if matches!(mode, DefinedHelperFixtureV1::Impure) {
        helper_locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(225)),
            unit,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(216)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(217)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(218)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(219)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(220)),
        source,
        helper_abi,
        helper_locals,
        SemanticBlockIdV1::from_index(0),
        helper_blocks,
    )
    .unwrap();

    let functions = if matches!(mode, DefinedHelperFixtureV1::MissingFunction) {
        vec![root]
    } else {
        vec![root, helper]
    };
    let mut callables = if matches!(mode, DefinedHelperFixtureV1::MissingFunction) {
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(9)),
        ]
    } else {
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ]
    };
    if matches!(mode, DefinedHelperFixtureV1::Impure) {
        callables.push(compiler_intrinsic_callable(
            226,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
            SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier,
        ));
    }
    let mut types = vec![
        unit_type(),
        scalar_type(
            223,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
    ];
    if matches!(mode, DefinedHelperFixtureV1::MismatchedArgument) {
        types.push(scalar_type(
            224,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ));
    }
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn defined_helper_owner_v1(mode: DefinedHelperFixtureV1) -> ProductionSemanticMirOwnerV1 {
    let admitted = defined_helper_request_v1(mode)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn reachable_defined_scalar_helper_survives_kir_effects_and_exact_llvm() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        defined_helper_owner_v1(DefinedHelperFixtureV1::Valid),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    assert_eq!(lowered.correspondence().lowered_functions().len(), 2);
    let helper_mapping = &lowered.correspondence().lowered_functions()[1];
    let helper_id = helper_mapping.kernel_ir_function().clone();
    let entry = &lowered.module().functions[0];
    let helper = &lowered.module().functions[1];
    assert_eq!(entry.role, FunctionRole::KernelEntry);
    assert_eq!(helper.role, FunctionRole::InternalHelper);
    assert_eq!(helper.signature.parameters, [Type::Scalar(ScalarType::U64)]);
    assert_eq!(helper.signature.results, [Type::Scalar(ScalarType::U64)]);
    let entry_body = entry.body.as_ref().unwrap();
    let call = entry_body.blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .unwrap();
    assert!(matches!(
        &call.kind,
        OperationKind::Call { callee, arguments }
            if callee == &helper_id && arguments == entry_body.parameters.as_slice()
    ));
    assert_eq!(call.results.len(), 1);
    assert!(!call.has_complete_effect_summary());
    assert!(matches!(
        helper.body.as_ref().unwrap().blocks[0].terminator,
        Some(Terminator::Return { ref values })
            if values == helper.body.as_ref().unwrap().parameters.as_slice()
    ));
    let effects = analyze_interprocedural_effects_v1(lowered.module()).unwrap();
    assert!(effects.function(&helper_id).unwrap().is_complete_and_pure());
    assert!(effects.function(&entry.id).unwrap().is_complete_and_pure());

    let mut module = lowered.module().clone();
    let target = gfx942_xnack_minus_target_capability();
    module.required_capabilities.insert(target.clone());
    for function in &mut module.functions {
        if function.body.is_some() {
            function.required_capabilities.insert(target.clone());
        }
    }
    module.kernels[0]
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    module.kernels[0].required_capabilities.insert(target);
    verify_module(&module).unwrap();
    let llvm = lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(&module).unwrap();
    assert!(llvm.contains(&format!("define internal i64 @{helper_id}(i64 %arg0)")));
    assert!(llvm.contains(&format!("call i64 @{helper_id}(i64 %arg0)")));
}

#[test]
fn exact_transparent_scalar_carrier_helper_uses_one_physical_u16_parameter() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        transparent_helper_carrier_owner_v1(TransparentHelperCarrierFixtureV1::Exact),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();

    let helper_function = SemanticFunctionIdV1::from_index(1);
    let helper_mapping = lowered
        .correspondence()
        .lowered_functions()
        .iter()
        .find(|mapping| mapping.semantic_function() == helper_function)
        .unwrap();
    let helper_id = helper_mapping.kernel_ir_function();
    let helper = lowered.module().function(helper_id).unwrap();
    assert_eq!(helper.role, FunctionRole::InternalHelper);
    assert_eq!(helper.signature.parameters, [Type::Scalar(ScalarType::U16)]);
    assert_eq!(helper.signature.results, [Type::Scalar(ScalarType::Bool)]);

    let entry = &lowered.module().functions[0];
    let call = entry.body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .unwrap();
    assert!(matches!(
        &call.kind,
        OperationKind::Call { callee, arguments }
            if callee == helper_id && arguments.len() == 1
    ));
    assert_eq!(call.results.len(), 1);
    assert_eq!(call.results[0].ty, Type::Scalar(ScalarType::Bool));

    let component = lowered
        .correspondence()
        .parameter_component_bindings()
        .iter()
        .find(|binding| binding.semantic_function() == helper_function)
        .unwrap();
    assert_eq!(component.semantic_local(), SemanticLocalIdV1::from_index(1));
    assert_eq!(
        component.semantic_component_type(),
        TRANSPARENT_HELPER_U16_TYPE
    );
    assert_eq!(
        component.projection(),
        [SemanticKirParameterProjectionV1::Field(0)]
    );
    assert_eq!(
        component.kernel_ir_value(),
        helper.body.as_ref().unwrap().parameters[0]
    );
    assert!(
        lowered
            .correspondence()
            .parameter_bindings()
            .iter()
            .all(|binding| binding.semantic_function() != helper_function)
    );
}

#[test]
fn transparent_helper_carrier_rejects_loose_layout_and_wrong_ownership() {
    let loose_types =
        transparent_helper_carrier_types_v1(TransparentHelperCarrierFixtureV1::LooseStorageOnly);
    assert_eq!(
        exact_transparent_scalar_carrier_field_v1(&loose_types, TRANSPARENT_HELPER_CARRIER_TYPE,),
        None
    );
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            transparent_helper_carrier_owner_v1(
                TransparentHelperCarrierFixtureV1::LooseStorageOnly,
            ),
            ProductionSemanticKirLimitsV1::default(),
        ),
        Err(ProductionSemanticKirErrorV1::ScalarTypeUnavailable {
            semantic_type: 3,
            ..
        })
    ));

    let exact_types =
        transparent_helper_carrier_types_v1(TransparentHelperCarrierFixtureV1::WrongOwnership);
    assert_eq!(
        exact_transparent_scalar_carrier_field_v1(&exact_types, TRANSPARENT_HELPER_CARRIER_TYPE,),
        Some(TRANSPARENT_HELPER_U16_TYPE)
    );
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            transparent_helper_carrier_owner_v1(TransparentHelperCarrierFixtureV1::WrongOwnership,),
            ProductionSemanticKirLimitsV1::default(),
        ),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 1,
            detail: "helper scalar carrier lacks an exact by-value source ABI",
            ..
        })
    ));
}

#[test]
fn exact_function_owner_correspondence_v5_round_trips_and_rejects_hostile_rosters() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        defined_helper_owner_v1(DefinedHelperFixtureV1::Valid),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let induction = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
        lowered.semantic().semantic(),
        SemanticFunctionIdV1::from_index(0),
    )
    .unwrap();
    let evidence =
        InertCanonicalMirToKirCorrespondenceEvidenceV5::from_live_owner(&lowered, &induction)
            .unwrap();
    evidence.revalidate().unwrap();
    evidence.validate_against_module(lowered.module()).unwrap();
    assert_eq!(evidence.functions().len(), 2);
    assert_eq!(evidence.functions()[0].kernel_ir_function_ordinal(), 0);
    assert_eq!(evidence.functions()[1].kernel_ir_function_ordinal(), 1);
    assert_eq!(
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(evidence.canonical_bytes()).unwrap(),
        evidence
    );

    let canonical = evidence.canonical_bytes();
    let read_u32 = |offset: usize| {
        u32::from_le_bytes(canonical[offset..offset + 4].try_into().unwrap()) as usize
    };
    let first = 28 + read_u32(20);
    let second = first + 20 + read_u32(first + 16);

    let mut duplicate_ordinal = canonical.to_vec();
    duplicate_ordinal[second + 8..second + 12].copy_from_slice(&canonical[first + 8..first + 12]);
    assert!(matches!(
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(&duplicate_ordinal),
        Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster)
    ));

    let mut duplicate_function = canonical.to_vec();
    duplicate_function[second + 4..second + 8].copy_from_slice(&canonical[first + 4..first + 8]);
    assert!(matches!(
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(&duplicate_function),
        Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster)
            | Err(ProductionCorrespondenceEvidenceErrorV5::NonCanonical)
    ));

    let mut reordered = canonical.to_vec();
    let first_semantic = canonical[first + 4..first + 8].to_vec();
    reordered[first + 4..first + 8].copy_from_slice(&canonical[second + 4..second + 8]);
    reordered[second + 4..second + 8].copy_from_slice(&first_semantic);
    assert!(matches!(
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(&reordered),
        Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster)
            | Err(ProductionCorrespondenceEvidenceErrorV5::NonCanonical)
    ));

    let mut sparse_substitution = canonical.to_vec();
    sparse_substitution[second + 8..second + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    let sparse =
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(&sparse_substitution).unwrap();
    assert!(matches!(
        sparse.validate_against_module(lowered.module()),
        Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster)
    ));

    let mut identity_substitution = canonical.to_vec();
    identity_substitution[second + 20] ^= 1;
    let substituted =
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(&identity_substitution).unwrap();
    assert!(matches!(
        substituted.validate_against_module(lowered.module()),
        Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster)
    ));

    assert!(matches!(
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(&canonical[..canonical.len() - 1]),
        Err(ProductionCorrespondenceEvidenceErrorV5::InvalidLength)
            | Err(ProductionCorrespondenceEvidenceErrorV5::Truncated)
    ));
    let mut trailing = canonical.to_vec();
    trailing.push(0);
    assert!(matches!(
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(&trailing),
        Err(ProductionCorrespondenceEvidenceErrorV5::InvalidLength)
    ));
}

#[test]
fn recursive_defined_helper_closure_fails_closed() {
    let error = ProductionSemanticKirOwnerV1::try_lower(
        defined_helper_owner_v1(DefinedHelperFixtureV1::Recursive),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::Unsupported {
            detail: "recursive deterministic helper call graph is unsupported",
            ..
        }
    ));
}

#[test]
fn branched_helper_return_is_a_live_block_parameter() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        defined_helper_owner_v1(DefinedHelperFixtureV1::BranchReturn),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let return_block = lowered.module().functions[1]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| block.id == BlockId(3))
        .unwrap();
    let [parameter] = return_block.parameters.as_slice() else {
        panic!("branched scalar return must have one block parameter");
    };
    assert!(matches!(
        return_block.terminator,
        Some(Terminator::Return { ref values }) if values == &[parameter.id]
    ));
}

#[test]
fn impure_and_tail_called_helpers_fail_closed() {
    let impure = ProductionSemanticKirOwnerV1::try_lower(
        defined_helper_owner_v1(DefinedHelperFixtureV1::Impure),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        impure,
        ProductionSemanticKirErrorV1::Unsupported {
            detail: "reachable deterministic scalar helper is not interprocedurally complete and pure",
            ..
        }
    ));

    let tail = ProductionSemanticKirOwnerV1::try_lower(
        defined_helper_owner_v1(DefinedHelperFixtureV1::TailCall),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        tail,
        ProductionSemanticKirErrorV1::Unsupported {
            detail: "semantic tail calls remain closed in deterministic helper lowering",
            ..
        }
    ));
}

#[test]
fn malformed_defined_helper_roster_and_types_fail_before_lowering() {
    assert!(matches!(
        defined_helper_request_v1(DefinedHelperFixtureV1::MissingCallable)
            .admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidReference {
            reference: SemanticMirReferenceV1::Callable,
            index: 7,
            ..
        })
    ));
    assert!(matches!(
        defined_helper_request_v1(DefinedHelperFixtureV1::MissingFunction)
            .admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        defined_helper_request_v1(DefinedHelperFixtureV1::MismatchedArgument)
            .admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::TypeMismatch { .. })
    ));
}

fn indexed_identity_v1(index: u32, domain: u8) -> [u8; 32] {
    let mut identity = [domain; 32];
    identity[..4].copy_from_slice(&index.to_be_bytes());
    identity
}

#[test]
fn explicit_function_limit_above_the_default_revalidates_exactly() {
    const FUNCTION_COUNT: usize = 1_025;
    let unit = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut functions = Vec::with_capacity(FUNCTION_COUNT);
    for index in 0..FUNCTION_COUNT {
        let index_u32 = u32::try_from(index).unwrap();
        let role = if index == 0 {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(indexed_identity_v1(index_u32, 230)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            if index == 0 {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if index == 0 {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let blocks = if index + 1 == FUNCTION_COUNT {
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(indexed_identity_v1(index_u32, 231)),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ]
        } else {
            let call = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(index_u32 + 1),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    local_place(0, unit),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(indexed_identity_v1(index_u32, 232)),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
                )
                .unwrap(),
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(indexed_identity_v1(index_u32, 233)),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ]
        };
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(indexed_identity_v1(index_u32, 234)),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256(indexed_identity_v1(index_u32, 235)),
            SemanticMonomorphizationIdentityV1::from_sha256(indexed_identity_v1(index_u32, 236)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(indexed_identity_v1(
                index_u32, 237,
            )),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(indexed_identity_v1(
                index_u32, 238,
            )),
            source,
            abi,
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(indexed_identity_v1(index_u32, 239)),
                unit,
                SemanticLocalRoleV1::Return,
                source,
            )],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        functions.push(if index == 0 {
            function.with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"large_defined_helper_closure_v1".to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(240)),
                SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
            ))
        } else {
            function
        });
    }
    let callables = (0..FUNCTION_COUNT)
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(
                u32::try_from(index).unwrap(),
            ))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let owner =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        owner,
        ProductionSemanticKirLimitsV1::new(FUNCTION_COUNT, 2 * FUNCTION_COUNT, 0),
    )
    .unwrap();
    assert_eq!(
        lowered.correspondence().lowered_functions().len(),
        FUNCTION_COUNT
    );
    lowered.verify_equivalence().unwrap();
}
