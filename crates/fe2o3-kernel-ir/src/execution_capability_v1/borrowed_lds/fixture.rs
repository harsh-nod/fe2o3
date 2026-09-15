// Inert canonical KIR fixture, never a Rust source or machine-proof receipt.
pub(super) fn id(tag: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([tag; 32])
}

pub(super) fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("borrowed_lds_entry"),
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
        source_type: id(source),
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
    occurrence: bool,
) -> ExecutionCapabilityOpV1 {
    ExecutionCapabilityOpV1 {
        operands: vec![ValueId(operand)],
        signature: ExecutionCapabilitySignatureV1::new(&[id(input)], id(output)).unwrap(),
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        operation,
        provenance: provenance(),
        workgroup_brand: Some([7; 32]),
        epoch_before: Some([8; 32]),
        epoch_after: None,
        source: ExecutionCapabilitySourceV1 {
            function: [9; 32],
            operation: [source; 32],
            block: 0,
            occurrence: occurrence.then(|| {
                ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                    [100; 32],
                    [101; 32],
                    [102; 32],
                    u32::from(source),
                    0,
                )
                .unwrap()
            }),
        },
    }
}

pub(super) fn module(occurrence: bool, converted: bool) -> Module {
    use ExecutionCapabilityOperationV1 as E;
    use ExecutionCapabilityRoleV1 as R;
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let context = KernelContextTypeV1::new("borrowed_lds_entry", [3; 32], [4; 32], [5; 32]);
    let allocation = contract(
        E::LdsAllocateBorrowed {
            workgroup_reference: id(12),
            workgroup: id(11),
            lds: id(90),
            element: id(92),
            layout,
            elements: 64,
        },
        12,
        90,
        1,
        80,
        occurrence,
    );
    let mut operations = vec![
        Operation::kernel_context_issue(
            ValueId(0),
            context,
            KernelContextSourceIdentityV1::new([40; 32], [41; 32], [42; 32], [43; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), capability(11, R::Workgroup)),
            OperationKind::ExecutionCapability(contract(
                E::WorkgroupDerive {
                    context: id(10),
                    workgroup: id(11),
                },
                10,
                11,
                0,
                30,
                occurrence,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(90),
                capability(
                    90,
                    R::Lds {
                        element: id(92),
                        layout,
                        elements: 64,
                        state: ExecutionLdsStateV1::Uninitialized,
                    },
                ),
            ),
            OperationKind::ExecutionCapability(allocation),
        ),
    ];
    if converted {
        assert!(occurrence);
        let conversion = ReusableLdsConversionV1 {
            input: id(90),
            output: id(91),
            element: id(92),
            layout,
            elements: 64,
            defined_function: [81; 32],
            defined_abi: [94; 32],
            defined_body: [95; 32],
            source_binding: [96; 32],
        };
        operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(91),
                capability(
                    91,
                    R::ReusableLds {
                        element: id(92),
                        layout,
                        elements: 64,
                    },
                ),
            ),
            OperationKind::ExecutionCapability(contract(
                E::ReusableLdsConversion(conversion),
                90,
                91,
                90,
                81,
                true,
            )),
        ));
    }
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let requirements = block
        .operations
        .iter()
        .flat_map(Operation::required_capabilities)
        .collect();
    let mut function = Function::kernel_entry(
        "borrowed_lds_entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    function.required_capabilities = requirements;
    let mut kernel = Kernel::new(
        "borrowed_lds",
        "borrowed_lds_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    let mut module = Module::new("borrowed_lds");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

pub(super) fn operations(module: &Module) -> &[Operation] {
    &module.functions[0].body.as_ref().unwrap().blocks[0].operations
}
pub(super) fn operations_mut(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}
pub(super) fn contract_at(module: &Module, index: usize) -> &ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &operations(module)[index].kind else {
        panic!("execution contract");
    };
    contract
}
pub(super) fn contract_mut(module: &mut Module, index: usize) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &mut operations_mut(module)[index].kind
    else {
        panic!("execution contract");
    };
    contract
}
