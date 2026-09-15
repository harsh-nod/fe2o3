use fe2o3_mir_model::semantic_mir_v1::*;

const MATH_FUNCTIONS: [SemanticF32MathFunctionV1; 13] = [
    SemanticF32MathFunctionV1::Sqrt,
    SemanticF32MathFunctionV1::FusedMultiplyAdd,
    SemanticF32MathFunctionV1::Floor,
    SemanticF32MathFunctionV1::Ceil,
    SemanticF32MathFunctionV1::Truncate,
    SemanticF32MathFunctionV1::RoundTiesEven,
    SemanticF32MathFunctionV1::Sin,
    SemanticF32MathFunctionV1::Cos,
    SemanticF32MathFunctionV1::Exp,
    SemanticF32MathFunctionV1::Exp2,
    SemanticF32MathFunctionV1::Ln,
    SemanticF32MathFunctionV1::Log2,
    SemanticF32MathFunctionV1::Log10,
];

fn fixture(
    function: SemanticF32MathFunctionV1,
) -> (
    Vec<SemanticTypeDeclV1>,
    SemanticNumericalPolicyMathContractV1,
    KernelContextTypeV1,
) {
    let id = SemanticTypeIdV1::from_index;
    let identity = |tag| SemanticTypeIdentityV1::from_sha256([tag; 32]);
    let ty = |index: u8, layout, shape| {
        SemanticTypeDeclV1::new(
            identity(10 + index),
            SemanticLayoutIdentityV1::from_sha256([10 + index; 32]),
            layout,
            shape,
        )
    };
    let reference = |index, pointee| {
        ty(
            index,
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    id(pointee),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
    };
    let zst = |index| {
        ty(
            index,
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        )
    };
    let types = vec![
        reference(0, 1),
        ty(
            1,
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![id(2), id(4), id(7)]).unwrap(),
            ),
        ),
        reference(2, 3),
        zst(3),
        reference(4, 5),
        zst(5),
        ty(
            6,
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        ),
        zst(7),
    ];
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([40; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([41; 32]),
        identity(42),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([43; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([44; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([45; 32]),
    )
    .unwrap();
    let contract = SemanticNumericalPolicyMathContractV1::new(
        SemanticNumericalPolicyMathTypesV1::new([0, 1, 2, 3, 4, 5, 6].map(id)),
        identity(30),
        identity(31),
        function,
        SemanticNumericalModeV1::StrictIeee,
        function.required_implementation(),
        provenance,
        SemanticFunctionIdentityV1::from_sha256([46; 32]),
    )
    .unwrap();
    (
        types,
        contract,
        KernelContextTypeV1::new("entry", [42; 32], [43; 32], [44; 32]),
    )
}

fn source(tag: u8) -> ExecutionCapabilitySourceV1 {
    ExecutionCapabilitySourceV1 {
        function: [50; 32],
        operation: [tag; 32],
        block: 0,
        occurrence: None,
    }
}

fn adapter(function: SemanticF32MathFunctionV1) -> NumericalPolicyMathLoweringV1 {
    let (types, contract, context) = fixture(function);
    NumericalPolicyMathLoweringV1::new(&types, contract, &context).unwrap()
}

fn bound(adapter: &NumericalPolicyMathLoweringV1) -> (ValueId, Type) {
    (
        ValueId(3),
        numerical_policy_math_result_type_v1(
            NumericalPolicyMathOperationV1::Bind {
                binding: adapter.binding,
            },
            &adapter.provenance,
        )
        .unwrap(),
    )
}

fn operands(adapter: &NumericalPolicyMathLoweringV1) -> Vec<(ValueId, Type)> {
    let mut operands = vec![bound(adapter)];
    operands.extend((0..adapter.function.arity()).map(|i| (ValueId(4 + i as u32), Type::F32)));
    operands
}

#[test]
fn mir19_math_adapter_preserves_all_seven_type_edges_and_thirteen_consumers() {
    for function in MATH_FUNCTIONS {
        let (types, mir, context) = fixture(function);
        let lowering = NumericalPolicyMathLoweringV1::new(&types, mir, &context).unwrap();
        let binding = lowering.binding;
        assert_eq!(
            binding.bound_reference.bytes(),
            *types[0].identity().as_bytes()
        );
        assert_eq!(binding.bound.bytes(), *types[1].identity().as_bytes());
        assert_eq!(
            binding.math_reference.bytes(),
            *types[2].identity().as_bytes()
        );
        assert_eq!(binding.math.bytes(), *types[3].identity().as_bytes());
        assert_eq!(
            binding.policy_reference.bytes(),
            *types[4].identity().as_bytes()
        );
        assert_eq!(binding.capability.bytes(), *types[5].identity().as_bytes());
        assert_eq!(lowering.element.bytes(), *types[6].identity().as_bytes());
        assert_eq!(binding.policy.bytes(), *mir.policy().as_bytes());
        assert_eq!(binding.kernel_brand.bytes(), *mir.kernel_brand().as_bytes());
        let (contract, result) = lowering
            .operation(lowering.consumer(), source(51), &operands(&lowering))
            .unwrap();
        assert_eq!(result, Type::F32);
        assert_eq!(contract.operands.len(), 1 + function.arity());
        assert_eq!(
            contract.signature.arguments().next(),
            Some(binding.bound_reference)
        );
        assert_eq!(contract.signature.output(), lowering.element);
        assert_eq!(
            lowering.consumer().numerical_requirements(),
            Some((
                fe2o3_kernel_ir::NumericalModeV1::StrictIeee,
                lower_f32_math_function(function).required_implementation(),
            ))
        );
        assert!(matches!(
            contract.operation,
            ExecutionCapabilityOperationV1::NumericalPolicyMath(_)
        ));
    }
}

#[test]
fn mir19_math_adapter_requires_an_actual_context_operand_for_derivation() {
    let lowering = adapter(MATH_FUNCTIONS[0]);
    let derive = NumericalPolicyMathOperationV1::MathDerive {
        context: ExecutionTypeIdentityV1::new([60; 32]),
        binding: lowering.binding,
    };
    assert!(lowering.operation(derive, source(52), &[]).is_err());
    assert!(
        lowering
            .operation(derive, source(52), &[(ValueId(0), Type::F32)])
            .is_err()
    );
    let context = Type::KernelContext(KernelContextTypeV1::new(
        "entry", [42; 32], [43; 32], [44; 32],
    ));
    let (operation, result) = lowering
        .operation(derive, source(52), &[(ValueId(0), context)])
        .unwrap();
    assert_eq!(operation.operands, vec![ValueId(0)]);
    assert_eq!(
        operation.signature.arguments().collect::<Vec<_>>(),
        vec![ExecutionTypeIdentityV1::new([60; 32])]
    );
    assert!(matches!(result, Type::ExecutionCapability(ref cap)
        if cap.source_type == lowering.binding.math
            && cap.role == ExecutionCapabilityRoleV1::NumericalPolicyMathSource(lowering.binding)));
}

#[test]
fn mir19_math_adapter_binding_retains_both_typed_values_and_rejects_swaps() {
    let lowering = adapter(MATH_FUNCTIONS[0]);
    let binding = lowering.binding;
    let math = numerical_policy_math_capability_type_v1(
        binding.math,
        ExecutionCapabilityRoleV1::NumericalPolicyMathSource(binding),
        &lowering.provenance,
    );
    let policy = numerical_policy_math_capability_type_v1(
        binding.capability,
        ExecutionCapabilityRoleV1::NumericalPolicy {
            policy: binding.policy,
            mode: binding.mode,
        },
        &lowering.provenance,
    );
    let bind = NumericalPolicyMathOperationV1::Bind { binding };
    let operands = vec![(ValueId(2), math), (ValueId(1), policy)];
    let (operation, result) = lowering.operation(bind, source(53), &operands).unwrap();
    assert_eq!(operation.operands, vec![ValueId(2), ValueId(1)]);
    assert_eq!(
        operation.signature.arguments().collect::<Vec<_>>(),
        vec![binding.math_reference, binding.policy_reference]
    );
    assert_eq!(result, bound(&lowering).1);
    assert!(
        lowering
            .operation(bind, source(53), &operands[..1])
            .is_err()
    );
    assert!(
        lowering
            .operation(
                bind,
                source(53),
                &[operands[1].clone(), operands[0].clone()]
            )
            .is_err()
    );
}

#[test]
fn mir19_math_adapter_rejects_erased_bound_scalar_and_fma_arity() {
    let lowering = adapter(SemanticF32MathFunctionV1::FusedMultiplyAdd);
    let mut operands = operands(&lowering);
    assert!(
        lowering
            .operation(lowering.consumer(), source(54), &operands)
            .is_ok()
    );
    assert!(
        lowering
            .operation(lowering.consumer(), source(54), &operands[1..])
            .is_err()
    );
    assert!(
        lowering
            .operation(lowering.consumer(), source(54), &operands[..3])
            .is_err()
    );
    operands[0].1 = Type::F32;
    assert!(
        lowering
            .operation(lowering.consumer(), source(54), &operands)
            .is_err()
    );
    operands[0] = bound(&lowering);
    operands[2].1 = Type::Scalar(ScalarType::F64);
    assert!(
        lowering
            .operation(lowering.consumer(), source(54), &operands)
            .is_err()
    );
}

#[test]
fn mir19_math_adapter_rejects_every_substituted_provenance_axis() {
    let lowering = adapter(MATH_FUNCTIONS[0]);
    for axis in 0..7 {
        let mut operands = operands(&lowering);
        let Type::ExecutionCapability(capability) = &mut operands[0].1 else {
            panic!()
        };
        match axis {
            0 => capability.provenance.root = FunctionId::new("other"),
            1 => capability.provenance.kernel_binding = [80; 32],
            2 => capability.provenance.frontend_unit = [80; 32],
            3 => capability.provenance.kernel_marker = [80; 32],
            4 => capability.provenance.target_brand = [80; 32],
            5 => capability.provenance.launch_brand = [80; 32],
            6 => capability.provenance.issuance = [80; 32],
            _ => unreachable!(),
        }
        assert!(
            lowering
                .operation(lowering.consumer(), source(54), &operands)
                .is_err()
        );
    }
}

#[test]
fn mir19_math_adapter_rejects_wrong_roles_brands_policy_and_scoped_tokens() {
    let lowering = adapter(MATH_FUNCTIONS[0]);
    for mutation in 0..6 {
        let mut operands = operands(&lowering);
        let Type::ExecutionCapability(capability) = &mut operands[0].1 else {
            panic!()
        };
        let mut binding = lowering.binding;
        match mutation {
            0 => capability.role = ExecutionCapabilityRoleV1::NumericalPolicyMathSource(binding),
            1 => {
                binding.policy = ExecutionTypeIdentityV1::new([81; 32]);
                capability.role = ExecutionCapabilityRoleV1::NumericalPolicyMathBound(binding);
            }
            2 => {
                binding.kernel_brand = ExecutionTypeIdentityV1::new([81; 32]);
                capability.role = ExecutionCapabilityRoleV1::NumericalPolicyMathBound(binding);
            }
            3 => capability.workgroup_brand = Some([81; 32]),
            4 => capability.epoch = Some([81; 32]),
            5 => capability.source_type = lowering.binding.bound_reference,
            _ => unreachable!(),
        }
        assert!(
            lowering
                .operation(lowering.consumer(), source(54), &operands)
                .is_err()
        );
    }
}

#[test]
fn mir19_math_adapter_rejects_mutable_raw_and_substituted_reference_edges() {
    for edge in [0_usize, 2, 4] {
        for mutation in 0..3 {
            let (mut types, mir, context) = fixture(MATH_FUNCTIONS[0]);
            let original = &types[edge];
            let pointer = SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(if mutation == 2 { 7 } else { edge as u32 + 1 }),
                if mutation == 0 {
                    SemanticPointerKindV1::Raw
                } else {
                    SemanticPointerKindV1::Reference
                },
                if mutation == 1 {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap();
            types[edge] = SemanticTypeDeclV1::new(
                original.identity(),
                original.layout_identity(),
                original.layout().clone(),
                SemanticTypeShapeV1::Pointer(pointer),
            );
            assert!(NumericalPolicyMathLoweringV1::new(&types, mir, &context).is_err());
        }
    }
}

#[test]
fn mir19_math_adapter_retains_open_obligations_and_source_coordinates() {
    let lowering = adapter(MATH_FUNCTIONS[0]);
    let mut site = source(54);
    site.block = 17;
    let (operation, _) = lowering
        .operation(lowering.consumer(), site.clone(), &operands(&lowering))
        .unwrap();
    assert_eq!(operation.source, site);
    assert_eq!(
        operation.obligations.bits(),
        ExecutionSafetyObligationsV1::LIFETIME_VALIDITY
            | ExecutionSafetyObligationsV1::NUMERICAL_POLICY
            | ExecutionSafetyObligationsV1::TARGET_SUPPORT
    );
    assert!(operation.workgroup_brand.is_none());
    assert!(operation.epoch_before.is_none());
    assert!(operation.epoch_after.is_none());
    let mut missing = source(54);
    missing.operation = [0; 32];
    assert!(
        lowering
            .operation(lowering.consumer(), missing, &operands(&lowering))
            .is_err()
    );
}
