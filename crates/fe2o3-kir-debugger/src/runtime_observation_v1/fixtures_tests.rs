//! Synthetic interpreter inputs only; never ordinary-source qualification.
use super::*;
use fe2o3_kernel_ir::*;

pub(super) const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

pub(super) fn assert_result_eq(
    actual: &Result<SimulationExecutionV1, SimulationErrorV1>,
    expected: &Result<SimulationExecutionV1, SimulationErrorV1>,
) {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
        (
            Err(SimulationErrorV1::Execution(actual)),
            Err(SimulationErrorV1::Execution(expected)),
        ) => {
            assert_eq!(actual, expected)
        }
        _ => panic!("unexpected preflight failure or result mismatch: {actual:?} / {expected:?}"),
    }
}

fn one(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn call(name: &str) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: name.into(),
            arguments: vec![],
        },
    )
}

fn returned(operations: Vec<Operation>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn admit(mut functions: Vec<Function>) -> AdmittedSimulationModuleV1 {
    let capabilities: std::collections::BTreeSet<_> = functions
        .iter()
        .flat_map(Function::derived_capabilities)
        .collect();
    for function in &mut functions {
        function.required_capabilities = capabilities.clone();
    }
    let mut module = Module::new("synthetic-origin-retention");
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities = capabilities.clone();
    module.required_capabilities = capabilities;
    module.functions = functions;
    module.kernels.push(kernel);
    AdmittedSimulationModuleV1::admit_v9(
        VerifiedCanonicalKernelIrV9::from_module(module).unwrap(),
        simulation_limits(),
    )
    .unwrap()
}

pub(super) fn loops() -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        one(0, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        one(1, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        one(2, Type::INDEX, OperationKind::Constant(Constant::Index(3))),
    ];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });
    let mut body = BasicBlock::new(BlockId(1));
    body.parameters = vec![ValueDef::new(ValueId(10), Type::INDEX)];
    body.operations = vec![
        call("helper"),
        call("empty"),
        one(
            11,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(10),
                rhs: ValueId(1),
            },
        ),
        one(
            12,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(11),
                rhs: ValueId(2),
            },
        ),
    ];
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(12),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(11)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut end = returned(vec![]);
    end.id = BlockId(2);
    let module = admit(vec![
        Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry, body, end],
        ),
        Function::internal_helper(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returned(vec![call("leaf"), call("leaf")])],
        ),
        Function::internal_helper(
            "leaf",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returned(vec![one(
                0,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(7)),
            )])],
        ),
        Function::internal_helper(
            "empty",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returned(vec![])],
        ),
    ]);
    (
        module,
        SimulationRequestV1::new("kernel", [2, 1, 1], [2, 1, 1], vec![]),
    )
}

pub(super) fn memory() -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let module = admit(vec![Function::kernel_entry(
        "entry",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![returned(vec![
            one(
                1,
                scalar.clone(),
                OperationKind::Constant(Constant::U32(42)),
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            one(
                2,
                scalar,
                OperationKind::Load {
                    pointer: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ])],
    )]);
    let buffer =
        BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &[ScalarBitsV1::u32(0)], TARGET)
            .unwrap();
    (
        module,
        SimulationRequestV1::new(
            "kernel",
            [1, 1, 1],
            [1, 1, 1],
            vec![SimulationArgumentV1::Buffer(buffer)],
        ),
    )
}

pub(super) fn barrier() -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let barrier = Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    );
    let module = admit(vec![
        Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returned(vec![call("helper"), call("helper")])],
        ),
        Function::internal_helper(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returned(vec![barrier])],
        ),
    ]);
    (
        module,
        SimulationRequestV1::new("kernel", [2, 1, 1], [2, 1, 1], vec![]),
    )
}

pub(super) fn simulation_limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: 64 * 1024,
        max_reachable_functions: 8,
        max_reachable_operations: 256,
        max_invocations: 2,
        max_workgroups: 1,
        max_scheduled_slots: 64,
        max_steps: 512,
        max_call_depth: 8,
        max_ssa_values: 256,
        max_allocations: 16,
        max_allocation_bytes: 4096,
        max_total_bytes: 16 * 1024,
        max_resident_bytes: 32 * 1024 * 1024,
        max_events: 8192,
        max_memory_access_records: 256,
    }
}

pub(super) fn debugger_limits(records: usize) -> DebuggerLimitsV1 {
    DebuggerLimitsV1::new(records, 65_536, 1024 * 1024).unwrap()
}

pub(super) fn private_allocations() -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let module = admit(vec![
        Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returned(vec![call("helper"), call("helper")])],
        ),
        Function::internal_helper(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returned(vec![
                one(
                    0,
                    pointer,
                    OperationKind::Alloca {
                        element: scalar.clone(),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    },
                ),
                one(1, scalar, OperationKind::Constant(Constant::U32(42))),
                Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(0),
                        value: ValueId(1),
                        access: MemoryAccess::new(AddressSpace::Private, 4),
                    },
                ),
            ])],
        ),
    ]);
    (
        module,
        SimulationRequestV1::new("kernel", [1, 1, 1], [1, 1, 1], vec![]),
    )
}
