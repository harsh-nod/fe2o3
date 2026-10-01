//! Synthetic ownership tests only; no source authentication or runtime evidence.
use super::*;

fn counter() -> Counter {
    Counter::new(LogicalStorageLimitsV1 {
        max_bytes: None,
        max_items: 10_000,
    })
}
fn text(value: &str, spare: usize) -> String {
    let mut result = String::with_capacity(value.len() + spare);
    result.push_str(value);
    result
}
fn vector<T>(values: impl IntoIterator<Item = T>, capacity: usize) -> Vec<T> {
    let mut result = Vec::with_capacity(capacity);
    result.extend(values);
    result
}
fn ty() -> Type {
    Type::pointer(
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    )
}

#[test]
fn empty_module_and_nested_owner_headers_are_counted_once() {
    let mut module = Module::new(text("module", 97));
    module.storage_layouts.reserve(3);
    module.functions.reserve(7);
    module.kernels.reserve(5);
    let parameters = vector([ty()], 4);
    let results = vector([Type::BOOL], 3);
    let mut function =
        Function::external_import(text("function", 41), Signature::new(parameters, results));
    let namespace = text("namespace", 13);
    let name = text("feature", 23);
    let extension_bytes = namespace.capacity() + name.capacity();
    function
        .required_capabilities
        .insert(TargetCapability::Extension { namespace, name });
    let function_heap_bytes = function.id.retained_capacity_bytes()
        + function.signature.parameters.capacity() * size_of::<Type>()
        + function.signature.results.capacity() * size_of::<Type>()
        + 2 * size_of::<Type>()
        + size_of::<TargetCapability>()
        + extension_bytes;
    module.functions.push(function);
    let kernel = Kernel::new(
        text("kernel", 37),
        text("entry", 29),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    let kernel_heap_bytes =
        kernel.id.retained_capacity_bytes() + kernel.entry.retained_capacity_bytes();
    module.kernels.push(kernel);
    let expected = module.id.retained_capacity_bytes()
        + module.storage_layouts.capacity() * size_of::<StorageLayoutV1>()
        + module.functions.capacity() * size_of::<Function>()
        + module.kernels.capacity() * size_of::<Kernel>()
        + function_heap_bytes
        + kernel_heap_bytes;
    let mut c = counter();
    module.charge_retained_heap_v11(&mut c).unwrap();
    assert_eq!(c.bytes(), expected);
    let mut exact = Counter::new(LogicalStorageLimitsV1 {
        max_bytes: Some(expected),
        max_items: c.items(),
    });
    module.charge_retained_heap_v11(&mut exact).unwrap();
    let mut short = Counter::new(LogicalStorageLimitsV1 {
        max_bytes: Some(expected - 1),
        max_items: c.items(),
    });
    assert_eq!(
        module.charge_retained_heap_v11(&mut short),
        Err(LogicalStorageErrorV1::ByteLimit)
    );
    let mut work_short = Counter::new(LogicalStorageLimitsV1 {
        max_bytes: None,
        max_items: c.items() - 1,
    });
    assert_eq!(
        module.charge_retained_heap_v11(&mut work_short),
        Err(LogicalStorageErrorV1::ItemLimit)
    );
}

#[test]
fn types_are_iterative_bounded_and_newer_ownership_is_refused() {
    let mut c = counter();
    type_heap(&ty(), &mut c).unwrap();
    assert_eq!(c.bytes(), 2 * size_of::<Type>());
    let mut short = Counter::new(LogicalStorageLimitsV1 {
        max_bytes: None,
        max_items: 1,
    });
    assert_eq!(
        type_heap(&ty(), &mut short),
        Err(LogicalStorageErrorV1::ItemLimit)
    );
    assert_eq!(
        type_heap(&Type::StorageObject(StorageLayoutIdV1(0)), &mut counter()),
        Err(LogicalStorageErrorV1::UnsupportedV11Owner)
    );
}

#[test]
fn body_block_results_and_nested_types_keep_actual_capacities() {
    let results = vector([ValueDef::new(ValueId(2), ty())], 5);
    let mut block = BasicBlock::new(BlockId(0));
    block.parameters = vector([ValueDef::new(ValueId(1), ty())], 7);
    block.operations = vector(
        [Operation::new(
            results,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(1),
                to: ty(),
            },
        )],
        11,
    );
    block.terminator = Some(Terminator::Return {
        values: vector([ValueId(2)], 13),
    });
    let body_parameters = vector([ValueId(0)], 17);
    let blocks = vector([block], 19);
    let function = Function::internal_helper(
        text("f", 23),
        Signature::new(Vec::new(), Vec::new()),
        body_parameters,
        blocks,
    );
    let body = function.body.as_ref().unwrap();
    let block = &body.blocks[0];
    let Terminator::Return { values } = block.terminator.as_ref().unwrap() else {
        panic!()
    };
    let expected = function.id.retained_capacity_bytes()
        + body.parameters.capacity() * size_of::<ValueId>()
        + body.blocks.capacity() * size_of::<BasicBlock>()
        + block.parameters.capacity() * size_of::<ValueDef>()
        + block.operations.capacity() * size_of::<Operation>()
        + block.operations[0].results.capacity() * size_of::<ValueDef>()
        + 6 * size_of::<Type>()
        + values.capacity() * size_of::<ValueId>();
    let mut c = counter();
    function_heap(&function, &mut c).unwrap();
    assert_eq!(c.bytes(), expected);
}

#[test]
fn assembly_call_type_and_barrier_payloads_are_not_omitted() {
    let assembly = InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        mnemonic: text("v_mov_b32", 79),
        operands: vector(
            [AssemblyOperand::input(
                ValueId(1),
                AssemblyConstraint::Vgpr32,
            )],
            9,
        ),
        options: BTreeSet::from([AssemblyOption::NoMemory, AssemblyOption::NoStack]),
        declared_effects: BTreeSet::from([AssemblyEffect::ReadGlobal]),
    };
    let expected = assembly.mnemonic.capacity()
        + assembly.operands.capacity() * size_of::<AssemblyOperand>()
        + assembly.options.len() * size_of::<AssemblyOption>()
        + assembly.declared_effects.len() * size_of::<AssemblyEffect>();
    let mut c = counter();
    operation_heap(&OperationKind::InlineAssembly(assembly), &mut c).unwrap();
    assert_eq!(c.bytes(), expected);
    let callee = FunctionId::new(text("callee", 71));
    let arguments = vector([ValueId(2)], 5);
    let expected = callee.retained_capacity_bytes() + arguments.capacity() * size_of::<ValueId>();
    let mut c = counter();
    operation_heap(&OperationKind::Call { callee, arguments }, &mut c).unwrap();
    assert_eq!(c.bytes(), expected);
    let operations = [
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::LaunchExtent { axis: Axis::X },
            ty(),
        )),
        OperationKind::Alloca {
            element: ty(),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: ty(),
            extent: WorkgroupMemoryExtent::Static(4),
            alignment: 8,
        }),
    ];
    for operation in operations {
        let mut c = counter();
        operation_heap(&operation, &mut c).unwrap();
        assert_eq!(c.bytes(), 2 * size_of::<Type>());
    }
    let semantics = || {
        BarrierSemantics::new(
            MemoryOrdering::AcquireRelease,
            [AddressSpace::Global, AddressSpace::Workgroup],
        )
    };
    let operations = [
        OperationKind::Barrier(Barrier {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            semantics: semantics(),
        }),
        OperationKind::Fence(Fence {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: semantics(),
        }),
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: semantics(),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    ];
    for operation in operations {
        let mut c = counter();
        operation_heap(&operation, &mut c).unwrap();
        assert_eq!(c.bytes(), 2 * size_of::<AddressSpace>());
    }
}

#[test]
fn matrix_frontend_records_retain_three_dynamic_payloads() {
    let provider = MatrixProviderIdentityV2 {
        crate_name: text("fixture", 67),
        stable_crate_id: 1,
        crate_hash: [1; 16],
        cargo_metadata_build_observation: [2; 32],
        source_identity: [3; 32],
        definition_identities: vector([[4; 16]], 9),
    };
    let canonical_record = vector([5_u8, 6], 109);
    let expected = provider.crate_name.capacity()
        + provider.definition_identities.capacity() * size_of::<[u8; 16]>()
        + canonical_record.capacity();
    let binding = MatrixFrontendBindingV2 {
        observed_source: MatrixSourceAbiObservationV2 {
            provider,
            canonical_record,
            digest: [7; 32],
        },
        projected_kernarg: MatrixProjectedKernargPolicyV1::canonical(),
    };
    let operation =
        MatrixOperation::multiply_accumulate([ValueId(0); 4], [ValueId(1); 4], [ValueId(2); 4])
            .with_frontend_binding(binding);
    let mut c = counter();
    matrix_heap(&operation, &mut c).unwrap();
    assert_eq!(c.bytes(), expected);
}

#[test]
fn every_terminator_dynamic_container_is_counted() {
    let branch = Terminator::Branch {
        target: BlockId(0),
        arguments: vector([ValueId(1)], 3),
    };
    let conditional = Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(0),
        then_arguments: vector([ValueId(1)], 5),
        else_target: BlockId(1),
        else_arguments: vector([], 7),
    };
    let switch = Terminator::Switch {
        selector: ValueId(0),
        cases: vector(
            [SwitchCase {
                value: 1,
                target: BlockId(0),
                arguments: vector([ValueId(1)], 11),
            }],
            3,
        ),
        default_target: BlockId(1),
        default_arguments: vector([], 13),
    };
    let integer_switch = Terminator::IntegerSwitch {
        selector: ValueId(0),
        cases: vector(
            [IntegerSwitchCase {
                value: Constant::U32(1),
                target: BlockId(0),
                arguments: vector([], 17),
            }],
            5,
        ),
        default_target: BlockId(1),
        default_arguments: vector([], 19),
    };
    let ret = Terminator::Return {
        values: vector([], 23),
    };
    for value in [
        branch,
        conditional,
        switch,
        integer_switch,
        ret,
        Terminator::Unreachable,
    ] {
        let expected = match &value {
            Terminator::Branch { arguments, .. } => arguments.capacity() * size_of::<ValueId>(),
            Terminator::ConditionalBranch {
                then_arguments,
                else_arguments,
                ..
            } => (then_arguments.capacity() + else_arguments.capacity()) * size_of::<ValueId>(),
            Terminator::Switch {
                cases,
                default_arguments,
                ..
            } => {
                cases.capacity() * size_of::<SwitchCase>()
                    + (cases[0].arguments.capacity() + default_arguments.capacity())
                        * size_of::<ValueId>()
            }
            Terminator::IntegerSwitch {
                cases,
                default_arguments,
                ..
            } => {
                cases.capacity() * size_of::<IntegerSwitchCase>()
                    + (cases[0].arguments.capacity() + default_arguments.capacity())
                        * size_of::<ValueId>()
            }
            Terminator::Return { values } => values.capacity() * size_of::<ValueId>(),
            Terminator::Unreachable => 0,
        };
        let mut c = counter();
        terminator_heap(&value, &mut c).unwrap();
        assert_eq!(c.bytes(), expected);
    }
}
