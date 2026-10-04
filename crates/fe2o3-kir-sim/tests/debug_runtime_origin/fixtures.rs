//! Synthetic canonical KIR controls, not source-produced compilation evidence.

use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;

pub const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

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

fn finish(functions: Vec<Function>) -> AdmittedSimulationModuleV1 {
    finish_with_workgroup(functions, None)
}

fn finish_with_workgroup(
    mut functions: Vec<Function>,
    workgroup_size: Option<WorkgroupSize>,
) -> AdmittedSimulationModuleV1 {
    let capabilities: std::collections::BTreeSet<_> = functions
        .iter()
        .flat_map(Function::derived_capabilities)
        .collect();
    for function in &mut functions {
        function.required_capabilities = capabilities.clone();
    }
    let mut module = Module::new("synthetic-runtime-origin-controls");
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities = capabilities.clone();
    kernel.workgroup_size = workgroup_size;
    module.required_capabilities = capabilities;
    module.functions = functions;
    module.kernels.push(kernel);
    AdmittedSimulationModuleV1::admit_v9(
        VerifiedCanonicalKernelIrV9::from_module(module).expect("verified synthetic control"),
        SimulationLimitsV1::default(),
    )
    .expect("admitted synthetic control")
}

fn counted_loop(trips: u64, callees: &[&str]) -> Vec<BasicBlock> {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        one(0, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        one(1, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        one(
            2,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(trips)),
        ),
    ];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });
    let mut body = BasicBlock::new(BlockId(1));
    body.parameters = vec![ValueDef::new(ValueId(10), Type::INDEX)];
    body.operations = callees.iter().map(|name| call(name)).collect();
    body.operations.extend([
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
    ]);
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(12),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(11)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut end = BasicBlock::new(BlockId(2));
    end.terminator = Some(Terminator::Return { values: vec![] });
    vec![entry, body, end]
}

pub fn loops_and_helpers() -> AdmittedSimulationModuleV1 {
    finish(vec![
        Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            counted_loop(3, &["helper", "empty"]),
        ),
        Function::internal_helper(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            counted_loop(2, &["leaf"]),
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
    ])
}

pub fn memory_and_fence() -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let module = finish(vec![Function::kernel_entry(
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
            Operation::new(
                vec![],
                OperationKind::Fence(Fence {
                    memory_scope: SynchronizationScope::Device,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Global],
                    ),
                }),
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

pub fn yielding_helper() -> AdmittedSimulationModuleV1 {
    let helper = returned(vec![
        one(
            0,
            Type::Scalar(ScalarType::U32),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave64,
            )),
        ),
        Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ),
        one(
            1,
            Type::Scalar(ScalarType::U32),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave64,
            )),
        ),
    ]);
    finish(vec![
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
            vec![helper],
        ),
    ])
}

pub fn transpose() -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let format = Gfx950LdsTransposeFormatV1::Fp8E4M3;
    let source = Type::slice(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let storage = Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let operation =
        |kind| OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(kind));
    let mut parameters = vec![source];
    parameters.extend(vec![Type::INDEX; 6]);
    let function = Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        (0..7).map(ValueId).collect(),
        vec![returned(vec![
            one(
                10,
                storage.clone(),
                operation(Gfx950LdsTransposeOperationKindV1::Current { format }),
            ),
            one(
                11,
                storage.clone(),
                operation(Gfx950LdsTransposeOperationKindV1::Stage {
                    format,
                    storage: ValueId(10),
                    source_slice: ValueId(0),
                    offset: ValueId(1),
                    rows: ValueId(2),
                    columns: ValueId(3),
                    stride: ValueId(4),
                    token_base: ValueId(5),
                    reduction_base: ValueId(6),
                }),
            ),
            one(
                12,
                storage,
                operation(Gfx950LdsTransposeOperationKindV1::Publish {
                    format,
                    storage: ValueId(11),
                }),
            ),
            Operation::new(
                (13..21)
                    .map(|id| ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)))
                    .collect(),
                operation(Gfx950LdsTransposeOperationKindV1::Read {
                    format,
                    storage: ValueId(12),
                }),
            ),
        ])],
    );
    let input = vec![ScalarBitsV1::new(ScalarType::U8, 0, TARGET).unwrap(); 2048];
    let buffer = BufferArgumentV1::from_scalars(AccessMode::ReadOnly, 1, &input, TARGET).unwrap();
    let mut arguments = vec![SimulationArgumentV1::Buffer(buffer)];
    arguments
        .extend([0, 16, 128, 128, 0, 0].map(|value| {
            SimulationArgumentV1::Scalar(ScalarBitsV1::index(value, TARGET).unwrap())
        }));
    (
        finish_with_workgroup(vec![function], Some(WorkgroupSize::new(64, 1, 1))),
        SimulationRequestV1::new("kernel", [64, 1, 1], [64, 1, 1], arguments),
    )
}
