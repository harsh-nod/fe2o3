use fe2o3_kernel_ir::*;

const UPPER_BOUND: u64 = 257;

fn identity(byte: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([byte; 32])
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

fn dynamic_extent() -> ExecutionDynamicExtentV1 {
    ExecutionDynamicExtentV1 {
        operand: 2,
        source_argument: 2,
        source_type: identity(9),
        value_type: ScalarType::Index,
        upper_bound: UPPER_BOUND,
        bound_check_operand: 3,
        nonnegative_check_operand: None,
    }
}

fn context_type() -> KernelContextTypeV1 {
    let provenance = provenance();
    KernelContextTypeV1::new(
        "entry",
        provenance.kernel_marker,
        provenance.target_brand,
        provenance.launch_brand,
    )
}

fn raw_bind_operation(extent: ExecutionDynamicExtentV1) -> ExecutionCapabilityOperationV1 {
    ExecutionCapabilityOperationV1::RawMemoryBind {
        authority: identity(7),
        pointer: identity(8),
        length: identity(9),
        extent,
        view: identity(10),
        element: identity(11),
        layout: ExecutionElementLayoutV1 {
            byte_size: 4,
            byte_alignment: 4,
        },
        space: ExecutionMemoryAddressSpaceV1::Private,
        access: ExecutionMemoryAccessV1::ReadOnly,
        index_space: None,
        atomic_scope: None,
        unsafe_obligation: identity(12),
    }
}

fn raw_bind_result(extent: ExecutionDynamicExtentV1) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: identity(10),
        provenance: provenance(),
        workgroup_brand: None,
        epoch: None,
        role: ExecutionCapabilityRoleV1::MemoryView {
            element: identity(11),
            layout: ExecutionElementLayoutV1 {
                byte_size: 4,
                byte_alignment: 4,
            },
            space: ExecutionMemoryAddressSpaceV1::Private,
            access: ExecutionMemoryAccessV1::ReadOnly,
            extent: ExecutionMemoryExtentV1::Dynamic(extent),
            initialization: ExecutionMemoryInitializationV1::FullyInitialized,
            index_space: None,
            atomic_scope: None,
        },
    })
}

fn raw_bind_module() -> Module {
    let extent = dynamic_extent();
    let semantic_operation = raw_bind_operation(extent);
    let requirements = semantic_operation.required_capabilities();
    let contract = ExecutionCapabilityOpV1 {
        operands: vec![ValueId(2), ValueId(0), ValueId(1), ValueId(4)],
        operation: semantic_operation,
        signature: ExecutionCapabilitySignatureV1::new(
            &[identity(7), identity(8), identity(9), identity(12)],
            identity(10),
        )
        .unwrap(),
        provenance: provenance(),
        workgroup_brand: None,
        epoch_before: None,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &raw_bind_operation(extent),
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [13; 32],
            operation: [14; 32],
            block: 0,
        },
    };

    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context_type(),
            KernelContextSourceIdentityV1::new([21; 32], [22; 32], [23; 32], [24; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Constant(Constant::Index(UPPER_BOUND)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs: ValueId(1),
                rhs: ValueId(3),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), raw_bind_result(extent)),
            OperationKind::ExecutionCapability(contract),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut function = Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    function.required_capabilities = requirements.clone();

    let mut module = Module::new("execution-capability-v13-dynamic-extent");
    module.functions.push(function);
    module.required_capabilities = requirements;
    module.kernels.push(Kernel::new(
        "entry-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn extent_mut(module: &mut Module) -> &mut ExecutionDynamicExtentV1 {
    let OperationKind::ExecutionCapability(contract) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    let ExecutionCapabilityOperationV1::RawMemoryBind { extent, .. } = &mut contract.operation
    else {
        unreachable!()
    };
    extent
}

fn assert_invalid_extent(module: &Module) {
    assert!(
        verify_module(module)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidExecutionCapability)
    );
}

#[test]
fn dynamic_extent_round_trips_all_runtime_contract_fields() {
    let module = raw_bind_module();
    verify_module(&module).unwrap();

    let encoded = encode_module_v13(&module).unwrap();
    assert_eq!(&encoded[8..10], &KERNEL_IR_VERSION_V13.to_le_bytes());
    assert_eq!(decode_module_v13(&encoded).unwrap(), module);
    let owner = VerifiedCanonicalKernelIrV13::from_canonical_bytes(encoded).unwrap();
    owner.revalidate().unwrap();
}

#[test]
fn verifier_rejects_dynamic_extent_operand_and_proof_ordinals() {
    let mut bad_value_ordinal = raw_bind_module();
    extent_mut(&mut bad_value_ordinal).operand = 1;
    assert_invalid_extent(&bad_value_ordinal);

    let mut bad_proof_ordinal = raw_bind_module();
    extent_mut(&mut bad_proof_ordinal).bound_check_operand = 2;
    assert_invalid_extent(&bad_proof_ordinal);
}

#[test]
fn verifier_rejects_dynamic_extent_semantic_and_ssa_type_substitution() {
    let mut bad_semantic_type = raw_bind_module();
    extent_mut(&mut bad_semantic_type).source_type = identity(99);
    assert_invalid_extent(&bad_semantic_type);

    let mut bad_ssa_type = raw_bind_module();
    bad_ssa_type.functions[0].signature.parameters[1] = Type::Scalar(ScalarType::U64);
    assert_invalid_extent(&bad_ssa_type);
}

#[test]
fn verifier_rejects_dynamic_extent_bound_mutation() {
    let mut wrong_constant = raw_bind_module();
    wrong_constant.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
        OperationKind::Constant(Constant::Index(UPPER_BOUND + 1));
    assert_invalid_extent(&wrong_constant);

    let mut wrong_comparison = raw_bind_module();
    let OperationKind::Compare { predicate, .. } =
        &mut wrong_comparison.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    *predicate = ComparePredicate::LessThan;
    assert_invalid_extent(&wrong_comparison);
}

#[test]
fn dynamic_extent_codec_changes_when_each_runtime_field_changes() {
    let baseline = encode_module_v13(&raw_bind_module()).unwrap();
    let mutations: [fn(&mut ExecutionDynamicExtentV1); 6] = [
        |extent| extent.operand = 1,
        |extent| extent.source_argument = 1,
        |extent| extent.source_type = identity(98),
        |extent| extent.value_type = ScalarType::U64,
        |extent| extent.upper_bound += 1,
        |extent| extent.bound_check_operand = 2,
    ];

    for mutate in mutations {
        let mut module = raw_bind_module();
        mutate(extent_mut(&mut module));
        let changed = *extent_mut(&mut module);
        let Type::ExecutionCapability(result) =
            &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[3].results[0].ty
        else {
            unreachable!()
        };
        let ExecutionCapabilityRoleV1::MemoryView { extent, .. } = &mut result.role else {
            unreachable!()
        };
        *extent = ExecutionMemoryExtentV1::Dynamic(changed);
        match encode_module_v13(&module) {
            Ok(encoded) => assert_ne!(encoded, baseline),
            Err(KernelIrEncodeError::NonCanonical { .. }) => {}
            Err(error) => panic!("unexpected V13 mutation result: {error}"),
        }
    }
}
