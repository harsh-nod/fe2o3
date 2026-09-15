use super::*;

pub(super) fn identity(tag: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([tag; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("scoped_index_entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

pub(super) fn capability(source: u8, role: ExecutionCapabilityRoleV1) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: identity(source),
        provenance: provenance(),
        workgroup_brand: Some([7; 32]),
        epoch: Some([8; 32]),
        role,
    })
}

fn contract(
    operation: ExecutionCapabilityOperationV1,
    input: u8,
    output: u8,
    operand: u32,
    source: u8,
) -> ExecutionCapabilityOpV1 {
    ExecutionCapabilityOpV1 {
        signature: ExecutionCapabilitySignatureV1::new(&[identity(input)], identity(output))
            .unwrap(),
        operands: vec![ValueId(operand)],
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

pub(super) fn module() -> Module {
    use ExecutionCapabilityOperationV1 as E;
    use ExecutionCapabilityRoleV1 as R;
    let p = provenance();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(0),
            KernelContextTypeV1::new(
                "scoped_index_entry",
                p.kernel_marker,
                p.target_brand,
                p.launch_brand,
            ),
            KernelContextSourceIdentityV1::new([40; 32], [41; 32], [42; 32], [43; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), capability(11, R::Workgroup)),
            OperationKind::ExecutionCapability(contract(
                E::WorkgroupDerive {
                    context: identity(10),
                    workgroup: identity(11),
                },
                10,
                11,
                0,
                30,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), capability(14, R::WorkgroupMemoryIndex)),
            OperationKind::ExecutionCapability(contract(
                E::WorkgroupMemoryIndexV2 {
                    workgroup_reference: identity(12),
                    workgroup: identity(11),
                    option: identity(13),
                    witness: identity(14),
                },
                12,
                13,
                1,
                31,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), capability(15, R::WorkgroupMemoryIndex)),
            OperationKind::ExecutionCapability(contract(
                E::WorkgroupMemoryIndexIntoDisjoint {
                    input_witness: identity(14),
                    output_witness: identity(15),
                },
                14,
                15,
                2,
                32,
            )),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let required = block
        .operations
        .iter()
        .flat_map(Operation::required_capabilities)
        .collect();
    let mut function = Function::kernel_entry(
        "scoped_index_entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    function.required_capabilities = required;
    let mut module = Module::new("scoped_index");
    let mut kernel = Kernel::new(
        "scoped_index",
        "scoped_index_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    module.required_capabilities = function.required_capabilities.clone();
    module.kernels.push(kernel);
    module.functions.push(function);
    module
}

pub(super) fn operations(module: &Module) -> &[Operation] {
    &module.functions[0].body.as_ref().unwrap().blocks[0].operations
}
pub(super) fn operations_mut(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}
pub(super) fn operation_contract(operation: &Operation) -> &ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &operation.kind else {
        panic!("execution contract")
    };
    contract
}
pub(super) fn contract_mut(module: &mut Module, index: usize) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &mut operations_mut(module)[index].kind
    else {
        panic!("execution contract")
    };
    contract
}
