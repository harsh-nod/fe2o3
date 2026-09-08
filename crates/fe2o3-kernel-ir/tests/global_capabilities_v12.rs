use fe2o3_kernel_ir::*;

fn context_type() -> KernelContextTypeV1 {
    KernelContextTypeV1::new("entry", [1; 32], [2; 32], [3; 32])
}

fn source_identity() -> KernelContextSourceIdentityV1 {
    KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32])
}

fn read_only_module() -> Module {
    let element = Type::Scalar(ScalarType::F32);
    let physical = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let capability = GlobalCapabilityTypeV1::read_only(element.clone(), context_type());
    let pointer = capability.physical_pointer_type();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(ValueId(2), context_type(), source_identity()),
        Operation::global_capability_bind(ValueId(3), capability, ValueId(2), ValueId(0)),
        Operation::global_capability_index(ValueId(4), ValueId(3), ValueId(1), None),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(3) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(4),
                rhs: ValueId(5),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Select {
                condition: ValueId(6),
                true_value: ValueId(4),
                false_value: ValueId(7),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(3) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(9),
                offset: ValueId(8),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), element.clone()),
            OperationKind::Constant(Constant::F32Bits(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), element),
            OperationKind::GuardedLoad {
                pointer: ValueId(10),
                predicate: ValueId(6),
                fallback: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("global-capability-read");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![physical, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn disjoint_write_module() -> Module {
    let element = Type::Scalar(ScalarType::F32);
    let physical = Type::slice(element.clone(), AddressSpace::Global, AccessMode::WriteOnly);
    let index_space = GlobalDisjointIndexSpaceV1::Index1d;
    let index_contract = GlobalDisjointIndexContractV1::new([8; 32], index_space);
    let capability =
        GlobalCapabilityTypeV1::disjoint_write(element.clone(), context_type(), index_contract);
    let pointer = capability.physical_pointer_type();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(ValueId(3), context_type(), source_identity()),
        Operation::global_capability_bind(ValueId(4), capability, ValueId(3), ValueId(0)),
        Operation::global_capability_index(
            ValueId(5),
            ValueId(4),
            ValueId(1),
            Some(index_contract),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(4) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(5),
                rhs: ValueId(6),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Select {
                condition: ValueId(7),
                true_value: ValueId(5),
                false_value: ValueId(8),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(4) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(10),
                offset: ValueId(9),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(11),
                predicate: ValueId(7),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("global-capability-write");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![physical, Type::INDEX, element], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn exclusive_read_write_module() -> Module {
    let mut module = disjoint_write_module();
    let element = Type::Scalar(ScalarType::F32);
    let physical = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let capability = GlobalCapabilityTypeV1::exclusive_read_write(element, context_type());
    let pointer = capability.physical_pointer_type();
    module.functions[0].signature.parameters[0] = physical;
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations[1].results[0].ty = Type::GlobalCapability(capability);
    let OperationKind::GlobalCapabilityIndex(index) = &mut operations[2].kind else {
        unreachable!()
    };
    index.index_space = None;
    operations[7].results[0].ty = pointer.clone();
    operations[8].results[0].ty = pointer;
    module
}

#[test]
fn v12_round_trips_all_global_capability_roles() {
    for module in [
        read_only_module(),
        exclusive_read_write_module(),
        disjoint_write_module(),
    ] {
        verify_module(&module).unwrap();
        let bytes = encode_module_v12(&module).unwrap();
        assert_eq!(decode_module_v12(&bytes).unwrap(), module);
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(bytes).unwrap();
    }
}

#[test]
fn guarded_store_accepts_a_component_precondition_only_when_the_exact_bound_is_conjoined() {
    let mut module = disjoint_write_module();
    {
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        operations.insert(
            5,
            Operation::effect_free(
                ValueDef::new(ValueId(12), Type::BOOL),
                OperationKind::Constant(Constant::Bool(true)),
            ),
        );
        operations.insert(
            6,
            Operation::effect_free(
                ValueDef::new(ValueId(13), Type::BOOL),
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(12),
                    rhs: ValueId(7),
                },
            ),
        );
        let OperationKind::Select { condition, .. } = &mut operations[8].kind else {
            unreachable!()
        };
        *condition = ValueId(13);
        let OperationKind::GuardedStore { predicate, .. } = &mut operations[11].kind else {
            unreachable!()
        };
        *predicate = ValueId(13);
    }
    verify_module(&module).unwrap();

    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let OperationKind::Binary { rhs, .. } = &mut operations[6].kind else {
        unreachable!()
    };
    *rhs = ValueId(12);
    assert!(
        verify_module(&module)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidGlobalCapability)
    );
}

#[test]
fn older_versions_reject_global_capability_types() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("global-capability-type-freeze");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![Type::GlobalCapability(GlobalCapabilityTypeV1::read_only(
                Type::F32,
                context_type(),
            ))],
            vec![],
        ),
        vec![ValueId(0)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let encoders: [fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>; 11] = [
        encode_module_v1,
        encode_module_v2,
        encode_module_v3,
        encode_module_v4,
        encode_module_v5,
        encode_module_v6,
        encode_module_v7,
        encode_module_v8,
        encode_module_v9,
        encode_module_v10,
        encode_module_v11,
    ];
    for encode in encoders {
        assert!(matches!(
            encode(&module),
            Err(KernelIrEncodeError::UnsupportedInVersion {
                feature: "branded global-capability type",
                ..
            })
        ));
    }
}

#[test]
fn verifier_rejects_context_access_and_index_space_substitution() {
    let mut wrong_context = read_only_module();
    let operation = &mut wrong_context.functions[0].body.as_mut().unwrap().blocks[0].operations[1];
    operation.results[0].ty = Type::GlobalCapability(GlobalCapabilityTypeV1::read_only(
        Type::F32,
        KernelContextTypeV1::new("entry", [1; 32], [9; 32], [3; 32]),
    ));
    assert!(
        verify_module(&wrong_context)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let mut wrong_access = read_only_module();
    wrong_access.functions[0].signature.parameters[0] =
        Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    assert!(
        verify_module(&wrong_access)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let mut wrong_index_space = disjoint_write_module();
    let OperationKind::GlobalCapabilityIndex(index) =
        &mut wrong_index_space.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    index.index_space = Some(GlobalDisjointIndexContractV1::new(
        [8; 32],
        GlobalDisjointIndexSpaceV1::ShiftedIndex1d { offset: 1 },
    ));
    assert!(
        verify_module(&wrong_index_space)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidGlobalCapability)
    );
}

#[test]
fn verifier_rejects_raw_slice_bypass_and_wrong_bounds_chain() {
    let mut bypass = read_only_module();
    bypass.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(13), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(0) },
        ));
    assert!(
        verify_module(&bypass)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidGlobalCapability)
    );

    let mut wrong_bound = read_only_module();
    let OperationKind::GetElementPointer { offset, .. } =
        &mut wrong_bound.functions[0].body.as_mut().unwrap().blocks[0].operations[8].kind
    else {
        unreachable!()
    };
    *offset = ValueId(4);
    assert!(
        verify_module(&wrong_bound)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidGlobalCapability)
    );
}
