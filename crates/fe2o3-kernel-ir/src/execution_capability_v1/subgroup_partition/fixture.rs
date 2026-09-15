fn identity(tag: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([tag; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("partition_entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn contract(
    operation: ExecutionCapabilityOperationV1,
    arguments: &[u8],
    output: u8,
    operands: &[u32],
    source: u8,
) -> ExecutionCapabilityOpV1 {
    ExecutionCapabilityOpV1 {
        signature: ExecutionCapabilitySignatureV1::new(
            &arguments.iter().copied().map(identity).collect::<Vec<_>>(),
            identity(output),
        )
        .unwrap(),
        operands: operands.iter().copied().map(ValueId).collect(),
        provenance: provenance(),
        workgroup_brand: Some([7; 32]),
        epoch_before: Some([8; 32]),
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [9; 32],
            operation: [source; 32],
            block: 0,
            occurrence: None,
        },
        operation,
    }
}

fn capability(source: u8, role: ExecutionCapabilityRoleV1) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: identity(source),
        provenance: provenance(),
        workgroup_brand: Some([7; 32]),
        epoch: Some([8; 32]),
        role,
    })
}

fn module() -> Module {
    use ExecutionCapabilityOperationV1 as E;
    use ExecutionCapabilityRoleV1 as R;
    use SubgroupPartitionOperationV1 as P;
    let subgroup = contract(
        E::SubgroupDerive {
            workgroup: identity(11),
            subgroup: identity(12),
            width: 64,
        },
        &[11],
        12,
        &[1],
        31,
    );
    let derive = contract(
        E::SubgroupPartition(P::Derive {
            subgroup_reference: identity(13),
            subgroup: identity(12),
            epoch: identity(14),
            partition: identity(15),
            width: 64,
            partition_width: 16,
        }),
        &[13, 14],
        15,
        &[2, 1],
        32,
    );
    let reduce = contract(
        E::SubgroupPartition(P::ReduceSumF32 {
            partition_reference: identity(16),
            partition: identity(15),
            element: identity(17),
            width: 64,
            partition_width: 16,
        }),
        &[16, 17],
        17,
        &[3, 4],
        33,
    );
    let broadcast = contract(
        E::SubgroupPartition(P::BroadcastF32 {
            partition_reference: identity(16),
            partition: identity(15),
            element: identity(17),
            source_lane: identity(18),
            width: 64,
            partition_width: 16,
        }),
        &[16, 17, 18],
        17,
        &[3, 6, 5],
        34,
    );
    let workgroup = contract(
        E::WorkgroupDerive {
            context: identity(10),
            workgroup: identity(11),
        },
        &[10],
        11,
        &[0],
        30,
    );
    let p = provenance();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(0),
            KernelContextTypeV1::new(
                "partition_entry",
                p.kernel_marker,
                p.target_brand,
                p.launch_brand,
            ),
            KernelContextSourceIdentityV1::new([40; 32], [41; 32], [42; 32], [43; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), capability(11, R::Workgroup)),
            OperationKind::ExecutionCapability(workgroup),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), capability(12, R::Subgroup { width: 64 })),
            OperationKind::ExecutionCapability(subgroup),
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(3),
                capability(
                    15,
                    R::SubgroupPartition {
                        width: 64,
                        partition_width: 16,
                    },
                ),
            ),
            OperationKind::ExecutionCapability(derive),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::F32)),
            OperationKind::Constant(Constant::F32Bits(1.0_f32.to_bits())),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(15)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::Scalar(ScalarType::F32)),
            OperationKind::ExecutionCapability(reduce),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::Scalar(ScalarType::F32)),
            OperationKind::ExecutionCapability(broadcast),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let requirements = block
        .operations
        .iter()
        .flat_map(Operation::required_capabilities)
        .collect();
    let mut function = Function::kernel_entry(
        "partition_entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    function.required_capabilities = requirements;
    let mut module = Module::new("partition");
    module.required_capabilities = function.required_capabilities.clone();
    let mut kernel = Kernel::new(
        "partition",
        "partition_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    module.kernels.push(kernel);
    module.functions.push(function);
    module
}

fn operations(module: &Module) -> &[Operation] {
    &module.functions[0].body.as_ref().unwrap().blocks[0].operations
}

fn operations_mut(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn operation_contract(operation: &Operation) -> &ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &operation.kind else {
        panic!("execution contract")
    };
    contract
}

fn contract_mut(module: &mut Module, index: usize) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &mut operations_mut(module)[index].kind
    else {
        panic!("execution contract")
    };
    contract
}
