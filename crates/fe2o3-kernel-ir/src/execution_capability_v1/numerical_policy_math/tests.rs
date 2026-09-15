use super::*;
use crate::*;

fn identity(tag: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([tag; 32])
}

fn binding() -> NumericalPolicyMathBindingV1 {
    NumericalPolicyMathBindingV1 {
        math_reference: identity(13),
        math: identity(14),
        policy_reference: identity(15),
        capability: identity(11),
        bound: identity(16),
        bound_reference: identity(17),
        policy: identity(12),
        kernel_brand: identity(18),
        mode: NumericalModeV1::StrictIeee,
    }
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn math_contract(
    math: NumericalPolicyMathOperationV1,
    operands: Vec<ValueId>,
    tag: u8,
) -> ExecutionCapabilityOpV1 {
    let binding = math.binding();
    let (inputs, output) = match math {
        NumericalPolicyMathOperationV1::MathDerive { context, .. } => (vec![context], binding.math),
        NumericalPolicyMathOperationV1::Bind { .. } => (
            vec![binding.math_reference, binding.policy_reference],
            binding.bound,
        ),
        NumericalPolicyMathOperationV1::F32 {
            bound_reference,
            element,
            function,
            ..
        } => {
            let mut inputs = vec![bound_reference];
            inputs.extend(std::iter::repeat_n(element, function.arity()));
            (inputs, element)
        }
    };
    let operation = ExecutionCapabilityOperationV1::NumericalPolicyMath(math);
    ExecutionCapabilityOpV1 {
        operands,
        signature: ExecutionCapabilitySignatureV1::new(&inputs, output).unwrap(),
        provenance: provenance(),
        workgroup_brand: None,
        epoch_before: None,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [20; 32],
            operation: [tag; 32],
            block: 0,
            occurrence: None,
        },
        operation,
    }
}

fn authority(source_type: ExecutionTypeIdentityV1, role: ExecutionCapabilityRoleV1) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type,
        role,
        provenance: provenance(),
        workgroup_brand: None,
        epoch: None,
    })
}

fn consumer(function: F32MathFunction) -> NumericalPolicyMathOperationV1 {
    NumericalPolicyMathOperationV1::F32 {
        binding: binding(),
        bound_reference: binding().bound_reference,
        element: identity(19),
        function,
    }
}

const FUNCTIONS: [F32MathFunction; 13] = [
    F32MathFunction::Sqrt,
    F32MathFunction::FusedMultiplyAdd,
    F32MathFunction::Floor,
    F32MathFunction::Ceil,
    F32MathFunction::Truncate,
    F32MathFunction::RoundTiesEven,
    F32MathFunction::Sin,
    F32MathFunction::Cos,
    F32MathFunction::Exp,
    F32MathFunction::Exp2,
    F32MathFunction::Ln,
    F32MathFunction::Log2,
    F32MathFunction::Log10,
];

fn module(function: F32MathFunction) -> Module {
    let binding = binding();
    let mut policy = math_contract(
        NumericalPolicyMathOperationV1::MathDerive {
            context: identity(10),
            binding,
        },
        vec![ValueId(0)],
        21,
    );
    policy.operation = ExecutionCapabilityOperationV1::NumericalPolicyIssue {
        context: identity(10),
        capability: binding.capability,
        policy: binding.policy,
        mode: binding.mode,
    };
    policy.signature =
        ExecutionCapabilitySignatureV1::new(&[identity(10)], binding.capability).unwrap();
    policy.obligations = ExecutionSafetyObligationsV1::from_bits(
        required_execution_obligations_v1(&policy.operation),
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(0),
            KernelContextTypeV1::new("entry", [3; 32], [4; 32], [5; 32]),
            KernelContextSourceIdentityV1::new([30; 32], [31; 32], [32; 32], [33; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(1),
                authority(
                    binding.capability,
                    ExecutionCapabilityRoleV1::NumericalPolicy {
                        policy: binding.policy,
                        mode: binding.mode,
                    },
                ),
            ),
            OperationKind::ExecutionCapability(policy),
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(2),
                authority(
                    binding.math,
                    ExecutionCapabilityRoleV1::NumericalPolicyMathSource(binding),
                ),
            ),
            OperationKind::ExecutionCapability(math_contract(
                NumericalPolicyMathOperationV1::MathDerive {
                    context: identity(10),
                    binding,
                },
                vec![ValueId(0)],
                22,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(3),
                authority(
                    binding.bound,
                    ExecutionCapabilityRoleV1::NumericalPolicyMathBound(binding),
                ),
            ),
            OperationKind::ExecutionCapability(math_contract(
                NumericalPolicyMathOperationV1::Bind { binding },
                vec![ValueId(2), ValueId(1)],
                23,
            )),
        ),
    ];
    for id in 4..7 {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::F32),
            OperationKind::Constant(Constant::F32Bits(0x3f80_0000)),
        ));
    }
    let mut operands = vec![ValueId(3)];
    operands.extend((4..4 + function.arity()).map(|id| ValueId(id as u32)));
    let contract = math_contract(consumer(function), operands, 24);
    let requirements = contract.operation.required_capabilities();
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(7), Type::F32),
        OperationKind::ExecutionCapability(contract),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("policy-math");
    let mut function =
        Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
    function.required_capabilities = requirements.clone();
    module.required_capabilities = requirements;
    module.functions.push(function);
    module.kernels.push(Kernel::new(
        "entry_kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}

fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn contract(operation: &mut Operation) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &mut operation.kind else {
        panic!("execution operation");
    };
    contract
}

#[test]
fn all_strict_fp32_consumers_retain_typed_chain_and_canonical_v13_transport() {
    for function in FUNCTIONS {
        let module = module(function);
        verify_module(&module).unwrap();
        let bytes = encode_module_v13(&module).unwrap();
        assert_eq!(decode_module_v13(&bytes).unwrap(), module);
        assert!(decode_module_v12(&bytes).is_err());
        let math = consumer(function);
        assert_eq!(
            math.numerical_requirements(),
            Some((
                NumericalModeV1::StrictIeee,
                function.required_implementation()
            ))
        );
        assert_eq!(
            math.obligations(),
            ExecutionSafetyObligationsV1::LIFETIME_VALIDITY
                | ExecutionSafetyObligationsV1::NUMERICAL_POLICY
                | ExecutionSafetyObligationsV1::TARGET_SUPPORT
        );
        assert_eq!(math.operand_count(), function.arity() + 1);
    }
}

#[test]
fn payload_four_is_closed_and_cannot_relabel_an_old_operation() {
    let mut module = module(F32MathFunction::Sqrt);
    for index in [2, 3, 7] {
        let contract = contract(&mut operations(&mut module)[index]);
        let bytes = encode_execution_capability_contract_v1(contract).unwrap();
        assert_eq!(&bytes[..2], &[4, 26]);
        assert_eq!(
            decode_execution_capability_contract_v1(&bytes, contract.operands.clone()),
            Some(contract.clone())
        );
        for revision in [0, 1, 2, 3, 5, 255] {
            let mut bad = bytes.clone();
            bad[0] = revision;
            assert!(
                decode_execution_capability_contract_v1(&bad, contract.operands.clone()).is_none()
            );
        }
        for length in 0..bytes.len() {
            assert!(
                decode_execution_capability_contract_v1(
                    &bytes[..length],
                    contract.operands.clone()
                )
                .is_none()
            );
        }
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(decode_execution_capability_contract_v1(&bad, contract.operands.clone()).is_none());
        for (offset, tag) in [(2, 3), (3 + 8 * 32, 1)] {
            let mut bad = bytes.clone();
            bad[offset] = tag;
            assert!(
                decode_execution_capability_contract_v1(&bad, contract.operands.clone()).is_none()
            );
        }
    }
    let policy = contract(&mut operations(&mut module)[1]);
    let mut bytes = encode_execution_capability_contract_v1(policy).unwrap();
    assert_eq!(&bytes[..2], &[2, 23]);
    bytes[0] = 4;
    assert!(decode_execution_capability_contract_v1(&bytes, policy.operands.clone()).is_none());
}

#[test]
fn payload_four_types_preserve_exact_binding_and_reject_relabelling() {
    let mut module = module(F32MathFunction::Sqrt);
    for index in [2, 3] {
        let Type::ExecutionCapability(ty) = &operations(&mut module)[index].results[0].ty else {
            unreachable!();
        };
        let bytes = encode_execution_capability_type_v1(ty).unwrap();
        assert_eq!(bytes[0], 4);
        assert_eq!(
            decode_execution_capability_type_v1(&bytes).as_ref(),
            Some(ty)
        );
        for revision in [0, 1, 2, 3, 5, 255] {
            let mut bad = bytes.clone();
            bad[0] = revision;
            assert!(decode_execution_capability_type_v1(&bad).is_none());
        }
        for length in 0..bytes.len() {
            assert!(decode_execution_capability_type_v1(&bytes[..length]).is_none());
        }
        let mut bad = bytes;
        bad.push(0);
        assert!(decode_execution_capability_type_v1(&bad).is_none());
    }
}

#[test]
fn binding_and_consumers_reject_swapped_missing_and_scalar_authority_operands() {
    for (index, operands) in [
        (3, vec![]),
        (3, vec![ValueId(1)]),
        (3, vec![ValueId(1), ValueId(2)]),
        (7, vec![ValueId(1), ValueId(4)]),
        (7, vec![ValueId(4), ValueId(4)]),
        (7, vec![ValueId(3)]),
        (7, vec![ValueId(3), ValueId(4), ValueId(5)]),
    ] {
        let mut bad = module(F32MathFunction::Sqrt);
        contract(&mut operations(&mut bad)[index]).operands = operands;
        assert!(verify_module(&bad).is_err());
    }
}

#[test]
fn all_reference_edges_policy_and_kernel_brand_must_match_the_constructor() {
    let mutations: &[fn(&mut NumericalPolicyMathBindingV1)] = &[
        |b| b.math_reference = identity(90),
        |b| b.math = identity(90),
        |b| b.policy_reference = identity(90),
        |b| b.capability = identity(90),
        |b| b.bound = identity(90),
        |b| b.bound_reference = identity(90),
        |b| b.policy = identity(90),
        |b| b.kernel_brand = identity(90),
        |b| b.mode = NumericalModeV1::AllowContraction,
        |b| b.mode = NumericalModeV1::AllowApproximation,
    ];
    for index in [3, 7] {
        for mutate in mutations {
            let mut bad = module(F32MathFunction::Sqrt);
            let ExecutionCapabilityOperationV1::NumericalPolicyMath(math) =
                &mut contract(&mut operations(&mut bad)[index]).operation
            else {
                unreachable!();
            };
            match math {
                NumericalPolicyMathOperationV1::Bind { binding }
                | NumericalPolicyMathOperationV1::F32 { binding, .. } => mutate(binding),
                _ => unreachable!(),
            }
            assert!(verify_module(&bad).is_err());
        }
    }
}

#[test]
fn unused_issuance_or_nondominating_constructor_cannot_authorize_a_consumer() {
    for pair in [(1, 7), (2, 3), (3, 7)] {
        let mut bad = module(F32MathFunction::Sqrt);
        operations(&mut bad).swap(pair.0, pair.1);
        assert!(verify_module(&bad).is_err());
    }
    for index in [0, 1, 2, 3] {
        let mut bad = module(F32MathFunction::Sqrt);
        operations(&mut bad)[index].kind = OperationKind::Constant(Constant::U32(0));
        assert!(verify_module(&bad).is_err());
    }
}

#[test]
fn select_or_block_parameter_cannot_forge_constructor_custody() {
    let mut bad = module(F32MathFunction::Sqrt);
    operations(&mut bad)[3].kind = OperationKind::Select {
        condition: ValueId(4),
        true_value: ValueId(2),
        false_value: ValueId(2),
    };
    assert!(verify_module(&bad).is_err());
    let mut bad = module(F32MathFunction::Sqrt);
    let constructor = operations(&mut bad).remove(3);
    bad.functions[0].body.as_mut().unwrap().blocks[0]
        .parameters
        .push(constructor.results[0].clone());
    assert!(verify_module(&bad).is_err());
}

#[test]
fn dominating_shared_origin_survives_a_block_edge_but_a_phi_does_not_reissue_it() {
    let mut module = module(F32MathFunction::Sqrt);
    let tail = operations(&mut module).split_off(4);
    let mut block = BasicBlock::new(BlockId(1));
    block.operations = tail;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    body.blocks.push(block);
    verify_module(&module).unwrap();

    let body = module.functions[0].body.as_mut().unwrap();
    let bound_type = body.blocks[0].operations[3].results[0].ty.clone();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(3)],
    });
    body.blocks[1]
        .parameters
        .push(ValueDef::new(ValueId(8), bound_type));
    contract(&mut body.blocks[1].operations[3]).operands[0] = ValueId(8);
    assert!(verify_module(&module).is_err());
}

#[test]
fn policy_math_retains_every_provenance_axis_and_open_obligations() {
    let mutations: &[fn(&mut ExecutionCapabilityOpV1)] = &[
        |c| c.provenance.root = FunctionId::new("other"),
        |c| c.provenance.kernel_binding = [90; 32],
        |c| c.provenance.frontend_unit = [90; 32],
        |c| c.provenance.kernel_marker = [90; 32],
        |c| c.provenance.target_brand = [90; 32],
        |c| c.provenance.launch_brand = [90; 32],
        |c| c.provenance.issuance = [90; 32],
        |c| c.obligations = ExecutionSafetyObligationsV1::from_bits(0),
        |c| c.workgroup_brand = Some([90; 32]),
        |c| c.epoch_before = Some([90; 32]),
    ];
    for index in [2, 3, 7] {
        for mutate in mutations {
            let mut bad = module(F32MathFunction::Sqrt);
            mutate(contract(&mut operations(&mut bad)[index]));
            assert!(verify_module(&bad).is_err());
        }
    }
    let mut bad = module(F32MathFunction::Sqrt);
    bad.functions[0].required_capabilities.clear();
    assert!(verify_module(&bad).is_err());
    let mut bad = module(F32MathFunction::Sqrt);
    bad.required_capabilities.clear();
    assert!(verify_module(&bad).is_err());
}

#[test]
fn incomplete_aliasing_abs_and_unbound_reference_contracts_reject() {
    for tag in [0, 11, 12, 13, 14, 15, 16, 17] {
        let mut binding = binding();
        binding.kernel_brand = identity(tag);
        assert!(!binding.is_complete());
    }
    assert!(!consumer(F32MathFunction::Abs).is_well_formed());
    let mut operation = consumer(F32MathFunction::Sqrt);
    let NumericalPolicyMathOperationV1::F32 {
        bound_reference, ..
    } = &mut operation
    else {
        unreachable!();
    };
    *bound_reference = identity(90);
    assert!(!operation.is_well_formed());
    assert!(operation.numerical_requirements().is_none());
}
