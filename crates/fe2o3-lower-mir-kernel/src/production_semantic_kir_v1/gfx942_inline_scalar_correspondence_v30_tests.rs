// These inert shape fixtures exercise the normal owner/check/replay pipeline;
// actual rustc authentication is covered by the separate source smoke workflow.
use fe2o3_kernel_ir::{AssemblyConstraint, AssemblyEffect, AssemblyOperandKind, AssemblyOption};
use fe2o3_mir_model::semantic_mir_v1::*;

const KINDS: [SemanticGfx942InlineInstructionV30; 6] = [
    SemanticGfx942InlineInstructionV30::VMovB32,
    SemanticGfx942InlineInstructionV30::VAddU32,
    SemanticGfx942InlineInstructionV30::VSubU32,
    SemanticGfx942InlineInstructionV30::VAndB32,
    SemanticGfx942InlineInstructionV30::VOrB32,
    SemanticGfx942InlineInstructionV30::VXorB32,
];
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const SCALAR: ProductionSemanticScalarTypeV2 = ProductionSemanticScalarTypeV2::Integer {
    signed: false,
    bits: 32,
};

fn abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
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

fn semantic(kind: SemanticGfx942InlineInstructionV30) -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let pointer = SemanticTypeIdV1::from_index(2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let function_id = SemanticFunctionIdentityV1::from_sha256([10; 32]);
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let constant = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(u32::MAX.into(), 4).unwrap(),
            ),
        ))
    };
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        (0..kind.input_count()).map(|_| constant()).collect(),
        Some(SemanticCallDestinationV1::new(
            place(2, U32),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
    .with_inline_assembly_source_v30(
        SemanticInlineAssemblySourceV30::new([30; 32], function_id, [31; 32], [32; 32]).unwrap(),
    );
    let output = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let store = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            output,
            SemanticOperandV1::Copy(place(2, U32)),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    );
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([40; 32]),
        SemanticLayoutIdentityV1::from_sha256([41; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(abi_value(pointer))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        function_id,
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([11; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([12; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([13; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([14; 32]),
        source,
        abi,
        [
            (unit, SemanticLocalRoleV1::Return),
            (pointer, SemanticLocalRoleV1::Argument(0)),
            (U32, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([50 + i as u8; 32]),
                ty,
                role,
                source,
            )
        })
        .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(60, vec![], SemanticTerminatorKindV1::Call(call)),
            block(61, vec![store], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"isa_scalar_relation".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([62; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let intrinsic_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([21; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![abi_value(U32); kind.input_count()],
        abi_value(U32),
    )
    .unwrap();
    let callable = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([22; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([23; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([24; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([25; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([26; 32]),
            source,
            intrinsic_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
            SemanticGfx942InlineU32V30::new(kind, 1).unwrap(),
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([27; 32]),
    };
    let pointer_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([70; 32]),
        SemanticLayoutIdentityV1::from_sha256([71; 32]),
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
                U32,
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
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        vec![
            unit_type(),
            plain_bit_scalar_type(
                68,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            pointer_type,
        ],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            callable,
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn expression(
    kind: SemanticGfx942InlineInstructionV30,
    overflow: ProductionOverflowContractV2,
) -> ProductionSemanticExpressionV2 {
    let lhs = ProductionSemanticExpressionV2::Constant {
        scalar: SCALAR,
        bits: u32::MAX.into(),
    };
    let operation = match kind {
        SemanticGfx942InlineInstructionV30::VMovB32 => return lhs,
        SemanticGfx942InlineInstructionV30::VAddU32 => ProductionSemanticBinaryOpV2::Add,
        SemanticGfx942InlineInstructionV30::VSubU32 => ProductionSemanticBinaryOpV2::Subtract,
        SemanticGfx942InlineInstructionV30::VAndB32 => ProductionSemanticBinaryOpV2::BitAnd,
        SemanticGfx942InlineInstructionV30::VOrB32 => ProductionSemanticBinaryOpV2::BitOr,
        SemanticGfx942InlineInstructionV30::VXorB32 => ProductionSemanticBinaryOpV2::BitXor,
    };
    ProductionSemanticExpressionV2::Binary {
        operation,
        scalar: SCALAR,
        overflow,
        lhs: Box::new(lhs.clone()),
        rhs: Box::new(lhs),
    }
}

fn ranked(expr: ProductionSemanticExpressionV2) -> ProductionRankedKernelLoweringInputV1 {
    let view = ProductionRankedValueIdV1::new(0);
    let index = ProductionRankedValueIdV1::new(1);
    let value = ProductionRankedValueIdV1::new(2);
    let contract = ProductionNumericalContractV2::exact_for_expression(&expr);
    let ranked = ProductionRankedKernelV1::new(
        "isa_scalar_relation",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: u64::from_le_bytes([62; 8]),
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::ViewInSpace {
                    result: view,
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    memory_space: dialect_kernel::MemorySpaceAttr::Global,
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::InvocationIndex {
                    result: index,
                    dimension: 0,
                    launch_extent: 1,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: value,
                    expression: expr,
                    numerical_contract: contract,
                },
                ProductionRankedOperationV1::ValueAccess {
                    kind: AccessKindAttr::Write,
                    view: ProductionRankedValueV1::Local(view),
                    indices: vec![ProductionRankedValueV1::Local(index)],
                    value: ProductionRankedValueV1::Local(value),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("isa_scalar_module", ranked).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap()
}

fn checked(
    kind: SemanticGfx942InlineInstructionV30,
    expr: ProductionSemanticExpressionV2,
) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
    let receipt =
        ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate(
            semantic(kind),
            ranked(expr),
            "inert checked-owner test projection".into(),
            vec![ProductionRankedAccessSourceV1::new(1, Some(0), 0, 0, 4)],
        )?;
    ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
        1,
    )
}

#[test]
fn all_six_values_attach_to_the_original_immutable_pre_ranked_owner() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    };
    for kind in KINDS {
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            semantic(kind),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let launch = crate::ProductionSourceLaunchRosterV1::try_new(
            ssa.source_semantic(),
            &[crate::ProductionSourceLaunchRootInputV1::new(
                "isa_scalar_relation",
                [62; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
            )],
        )
        .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
        let materialized = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let original = materialized.executable().module().clone();
        let functions_allocation = materialized.executable().module().functions.as_ptr();
        let root = ProductionRankedSemanticProjectionRootV1::new(
            SemanticFunctionIdV1::from_index(0),
            1,
            ranked(expression(kind, ProductionOverflowContractV2::Wrapping)),
            "inert connected test projection".into(),
            vec![ProductionRankedAccessSourceV1::new(1, Some(0), 0, 0, 4)],
            vec![],
        );
        let receipt = ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(materialized,vec![root]).unwrap();
        let owner =
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap();
        assert_eq!(owner.pre_ranked_executable().unwrap().module(), &original);
        assert_eq!(owner.module().functions.as_ptr(), functions_allocation);
        assert_eq!(
            owner
                .mir_pliron_translation_validation()
                .unwrap()
                .value_expressions(),
            1
        );
        owner.verify_equivalence().unwrap();
    }
}

#[test]
fn all_six_values_pass_normal_checked_owner_and_exact_replay_without_rewriting_isa() {
    for kind in KINDS {
        let owner = checked(
            kind,
            expression(kind, ProductionOverflowContractV2::Wrapping),
        )
        .unwrap();
        assert_eq!(
            owner
                .mir_pliron_translation_validation()
                .unwrap()
                .value_expressions(),
            1
        );
        assert_eq!(
            owner
                .mir_pliron_translation_validation()
                .unwrap()
                .memory_effects(),
            1
        );
        let operations = owner.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|b| &b.operations)
            .collect::<Vec<_>>();
        let assembly = operations
            .iter()
            .filter_map(|op| {
                if let OperationKind::InlineAssembly(a) = &op.kind {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(assembly.len(), 1);
        assert_eq!(assembly[0].mnemonic, kind.mnemonic());
        assert!(
            !operations
                .iter()
                .any(|op| matches!(op.kind, OperationKind::Binary { .. }))
        );
        owner.verify_equivalence().unwrap();
    }
}

#[test]
fn checked_or_wrong_value_contract_cannot_replace_wrapping_isa_values() {
    let kind = SemanticGfx942InlineInstructionV30::VSubU32;
    for wrong in [
        expression(kind, ProductionOverflowContractV2::Checked),
        expression(
            SemanticGfx942InlineInstructionV30::VAddU32,
            ProductionOverflowContractV2::Wrapping,
        ),
    ] {
        assert!(matches!(
            checked(kind, wrong),
            Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch { .. }
            ))
        ));
    }
}

#[test]
fn exact_owner_replay_rejects_distinct_equal_value_operand_substitution() {
    let kind = SemanticGfx942InlineInstructionV30::VAddU32;
    let mut owner = checked(
        kind,
        expression(kind, ProductionOverflowContractV2::Wrapping),
    )
    .unwrap();
    let RetainedProductionKirModuleV1::Legacy(module) = &mut owner.module else {
        panic!("fixture owner changed")
    };
    let assembly = module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.operations)
        .find_map(|op| {
            if let OperationKind::InlineAssembly(a) = &mut op.kind {
                Some(a)
            } else {
                None
            }
        })
        .unwrap();
    assert_ne!(assembly.operands[1].kind, assembly.operands[2].kind);
    assembly.operands[1].kind = assembly.operands[2].kind.clone();
    verify_module(module).unwrap();
    // Repair canonical custody of the mutated, still-valid graph. Rejection
    // must come from exact retained-source replay, not a stale byte digest.
    owner.canonical_kernel_ir = ProductionCanonicalKernelIrV1::from_module(module.clone()).unwrap();
    assert!(matches!(
        owner.verify_equivalence(),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

fn joined(
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    semantic: Option<&AdmittedInertSemanticMirV1>,
    budget: usize,
) -> Result<(), ProductionMirPlironTranslationErrorV1> {
    let function = &module.functions[0];
    let mut work = UnsupportedIndexCorrelationBudgetV1 { remaining: budget };
    let kir = build_kir_correlation_index(function.body.as_ref().unwrap(), 1000, &mut work)
        .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    Gfx942InlineScalarCorrespondenceV30::build(
        semantic,
        correspondence,
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(0),
        function,
        &kir,
        &mut work,
    )
    .map(|_| ())
}

#[test]
fn joined_contract_rejects_missing_context_extra_isa_wrong_source_register_options_and_span() {
    let kind = SemanticGfx942InlineInstructionV30::VOrB32;
    let owner = checked(
        kind,
        expression(kind, ProductionOverflowContractV2::Wrapping),
    )
    .unwrap();
    let source = owner.semantic_ssa.source_owner().semantic();
    assert!(joined(owner.module(), &owner.correspondence, None, 100_000).is_err());
    assert_eq!(
        joined(owner.module(), &owner.correspondence, Some(source), 0),
        Err(ProductionMirPlironTranslationErrorV1::ResourceLimit)
    );
    for mutation in 0..12 {
        let mut module = owner.module().clone();
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        let index = operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::InlineAssembly(_)))
            .unwrap();
        if mutation == 5 {
            let mut extra = operations[index].clone();
            extra.results[0].id = ValueId(100);
            let OperationKind::InlineAssembly(assembly) = &mut extra.kind else {
                unreachable!()
            };
            assembly.source.statement = [98; 32];
            operations.push(extra);
        } else {
            let OperationKind::InlineAssembly(assembly) = &mut operations[index].kind else {
                unreachable!()
            };
            match mutation {
                0 => assembly.source.function = [99; 32],
                1 => assembly.source.statement = [0; 32],
                2 => assembly.operands[1].constraint = AssemblyConstraint::Sgpr32,
                3 => {
                    assembly.options.insert(AssemblyOption::Pure);
                }
                4 => assembly.operands[1].kind = AssemblyOperandKind::Input(ValueId(u32::MAX)),
                6 => assembly.source.statement = [99; 32],
                7 => {
                    assembly.declared_effects.insert(AssemblyEffect::ReadGlobal);
                }
                8 => assembly.mnemonic = "v_xor_b32".into(),
                9 => assembly.source.frontend_unit = [97; 32],
                10 => assembly.source.contract = [96; 32],
                11 => assembly.options.clear(),
                _ => unreachable!(),
            }
        }
        if mutation == 5 {
            verify_module(&module).unwrap();
        }
        assert!(
            joined(&module, &owner.correspondence, Some(source), 100_000).is_err(),
            "mutation {mutation}"
        );
    }
    let mut correspondence = owner.correspondence.clone();
    correspondence.terminator_operation_spans[0].operation_count = 0;
    assert!(joined(owner.module(), &correspondence, Some(source), 100_000).is_err());
}

fn normalized(
    owner: &ProductionSemanticKirOwnerV1,
    module: &Module,
    remaining: usize,
    depth: usize,
) -> (Option<NormalizedScalarExpressionV1>, usize) {
    let function = &module.functions[0];
    let body = function.body.as_ref().unwrap();
    let mut work = UnsupportedIndexCorrelationBudgetV1 { remaining: 100_000 };
    let mut kir = build_kir_correlation_index(body, 1000, &mut work).unwrap();
    kir.inline_scalar = Gfx942InlineScalarCorrespondenceV30::build(
        Some(owner.semantic_ssa.source_owner().semantic()),
        &owner.correspondence,
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(0),
        function,
        &kir,
        &mut work,
    )
    .unwrap();
    let result = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::InlineAssembly(_)))
        .unwrap()
        .results[0]
        .id;
    work.remaining = remaining;
    let mut visiting = BTreeSet::new();
    let normalized = normalize_kir_expression_v1(
        function,
        &kir,
        &BTreeMap::new(),
        result,
        depth,
        &mut visiting,
        &mut work,
    );
    assert!(
        visiting.is_empty(),
        "failed recursion must restore the active stack"
    );
    (normalized, work.remaining)
}

#[test]
fn partial_arithmetic_operands_are_not_relabelled_as_wrapping_by_the_isa_parent() {
    let kind = SemanticGfx942InlineInstructionV30::VAddU32;
    let owner = checked(
        kind,
        expression(kind, ProductionOverflowContractV2::Wrapping),
    )
    .unwrap();
    for op in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
        let mut module = owner.module().clone();
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        let first = operations[0].results[0].id;
        operations[1].kind = OperationKind::Binary {
            op,
            lhs: first,
            rhs: first,
        };
        verify_module(&module).unwrap();
        assert!(normalized(&owner, &module, 100_000, 0).0.is_none());
    }
    let mut module = owner.module().clone();
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let first = operations[0].results[0].id;
    operations[1].kind = OperationKind::Binary {
        op: BinaryOp::Checked(CheckedBinaryOperator::Subtract),
        lhs: first,
        rhs: first,
    };
    operations[1]
        .results
        .push(ValueDef::new(ValueId(100), Type::Scalar(ScalarType::Bool)));
    verify_module(&module).unwrap();
    let (Some(NormalizedScalarExpressionV1::Binary { overflow, rhs, .. }), _) =
        normalized(&owner, &module, 100_000, 0)
    else {
        panic!("checked input value should keep its own contract")
    };
    assert_eq!(overflow, ProductionOverflowContractV2::Wrapping);
    assert!(matches!(
        *rhs,
        NormalizedScalarExpressionV1::Binary {
            overflow: ProductionOverflowContractV2::Checked,
            ..
        }
    ));
    assert_eq!(
        normalize_kir_binary_v1(
            BinaryOp::Checked(CheckedBinaryOperator::Subtract),
            &module.functions[0].body.as_ref().unwrap().blocks[0].operations[1],
            ValueId(100)
        ),
        None
    );
}

#[test]
fn value_erasing_move_still_consumes_depth_and_exact_cumulative_work() {
    let kind = SemanticGfx942InlineInstructionV30::VMovB32;
    let owner = checked(
        kind,
        expression(kind, ProductionOverflowContractV2::Wrapping),
    )
    .unwrap();
    let (expected, remaining) = normalized(&owner, owner.module(), 100_000, 0);
    assert!(expected.is_some());
    let exact = 100_000 - remaining;
    assert_eq!(normalized(&owner, owner.module(), exact, 0).0, expected);
    assert!(normalized(&owner, owner.module(), exact - 1, 0).0.is_none());
    assert!(
        normalized(
            &owner,
            owner.module(),
            100_000,
            MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2
        )
        .0
        .is_none()
    );
}
