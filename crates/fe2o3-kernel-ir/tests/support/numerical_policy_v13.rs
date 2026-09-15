#![allow(dead_code)]

use fe2o3_kernel_ir::*;

pub fn identity(tag: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([tag; 32])
}

pub fn provenance() -> ExecutionCapabilityProvenanceV1 {
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

pub fn contract() -> ExecutionCapabilityOpV1 {
    let operation = ExecutionCapabilityOperationV1::NumericalPolicyIssue {
        context: identity(10),
        capability: identity(11),
        policy: identity(12),
        mode: NumericalModeV1::StrictIeee,
    };
    ExecutionCapabilityOpV1 {
        operands: vec![ValueId(0)],
        signature: ExecutionCapabilitySignatureV1::new(&[identity(10)], identity(11)).unwrap(),
        provenance: provenance(),
        workgroup_brand: None,
        epoch_before: None,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [20; 32],
            operation: [21; 32],
            block: 0,
            occurrence: None,
        },
        operation,
    }
}

pub fn authority() -> ExecutionCapabilityTypeV1 {
    ExecutionCapabilityTypeV1 {
        source_type: identity(11),
        provenance: provenance(),
        workgroup_brand: None,
        epoch: None,
        role: ExecutionCapabilityRoleV1::NumericalPolicy {
            policy: identity(12),
            mode: NumericalModeV1::StrictIeee,
        },
    }
}

pub fn module() -> Module {
    let provenance = provenance();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(0),
            KernelContextTypeV1::new(
                "entry",
                provenance.kernel_marker,
                provenance.target_brand,
                provenance.launch_brand,
            ),
            KernelContextSourceIdentityV1::new([30; 32], [31; 32], [32; 32], [33; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), Type::ExecutionCapability(authority())),
            OperationKind::ExecutionCapability(contract()),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("numerical-policy");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "entry_kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}

pub fn issuance_mut(module: &mut Module) -> &mut Operation {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[1]
}

pub fn contract_mut(module: &mut Module) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &mut issuance_mut(module).kind else {
        panic!("issuance")
    };
    contract
}

pub fn authority_mut(module: &mut Module) -> &mut ExecutionCapabilityTypeV1 {
    let Type::ExecutionCapability(capability) = &mut issuance_mut(module).results[0].ty else {
        panic!("authority")
    };
    capability
}

pub fn used_policy_module() -> Module {
    let mut module = module();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(1)],
    });
    let mut block = BasicBlock::new(BlockId(1));
    block.parameters.push(ValueDef::new(
        ValueId(2),
        Type::ExecutionCapability(authority()),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(block);
    module
}

pub fn floating_point_module(with_policy: bool, mode: NumericalModeV1) -> Module {
    let mut module = module();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    if !with_policy {
        operations.pop();
    }
    // Separate binary32 multiply/add differs from contraction for these exact inputs.
    for (id, bits) in [(2, 0x3f80_0001), (3, 0x3f7f_ffff), (4, 0xbf80_0000)] {
        operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::F32)),
            OperationKind::Constant(Constant::F32Bits(bits)),
        ));
    }
    for (id, op, lhs, rhs) in [(5, BinaryOp::Multiply, 2, 3), (6, BinaryOp::Add, 5, 4)] {
        operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::F32)),
            OperationKind::Binary {
                op,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        ));
    }
    module
        .required_capabilities
        .insert(TargetCapability::Execution(
            ExecutionCapabilityRequirementV1::Numerical {
                value_type: ScalarType::F32,
                mode,
            },
        ));
    module
}

pub fn rejected_issuance_mutations() -> Vec<(&'static str, Module)> {
    let mut cases = Vec::new();
    for mode in [
        NumericalModeV1::AllowContraction,
        NumericalModeV1::AllowApproximation,
    ] {
        let mut bad = module();
        let ExecutionCapabilityOperationV1::NumericalPolicyIssue { mode: actual, .. } =
            &mut contract_mut(&mut bad).operation
        else {
            unreachable!()
        };
        *actual = mode;
        authority_mut(&mut bad).role = ExecutionCapabilityRoleV1::NumericalPolicy {
            policy: identity(12),
            mode,
        };
        cases.push(("relaxed mode", bad));
    }
    for (name, mutate) in [
        (
            "nominal type",
            (|module: &mut Module| authority_mut(module).source_type = identity(99))
                as fn(&mut Module),
        ),
        ("role", |module| {
            authority_mut(module).role = ExecutionCapabilityRoleV1::KernelAuthority
        }),
        ("policy", |module| {
            authority_mut(module).role = ExecutionCapabilityRoleV1::NumericalPolicy {
                policy: identity(99),
                mode: NumericalModeV1::StrictIeee,
            }
        }),
        ("root", |module| {
            authority_mut(module).provenance.root = FunctionId::new("other")
        }),
        ("kernel binding", |module| {
            authority_mut(module).provenance.kernel_binding[0] ^= 1
        }),
        ("frontend unit", |module| {
            authority_mut(module).provenance.frontend_unit[0] ^= 1
        }),
        ("kernel marker", |module| {
            authority_mut(module).provenance.kernel_marker[0] ^= 1
        }),
        ("target", |module| {
            authority_mut(module).provenance.target_brand[0] ^= 1
        }),
        ("launch", |module| {
            authority_mut(module).provenance.launch_brand[0] ^= 1
        }),
        ("issuance", |module| {
            authority_mut(module).provenance.issuance[0] ^= 1
        }),
        ("source", |module| {
            contract_mut(module).source.operation = [0; 32]
        }),
        ("operand arity", |module| {
            contract_mut(module).operands.clear()
        }),
        ("result arity", |module| {
            issuance_mut(module).results.clear()
        }),
        ("obligations", |module| {
            contract_mut(module).obligations = ExecutionSafetyObligationsV1::from_bits(0)
        }),
    ] {
        let mut bad = module();
        mutate(&mut bad);
        cases.push((name, bad));
    }
    cases
}
