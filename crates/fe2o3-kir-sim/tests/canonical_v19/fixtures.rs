//! Inert test-only canonical graphs; no source authentication or native evidence.
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, Constant, Function,
    Gfx942CompleteBodyDeclarationVNext, Gfx942CompleteBodyOriginVNext as Origin,
    Gfx942CompleteBodyStepVNext as Step, Gfx942OrderedProgramRegistersV1 as Registers,
    Gfx942ProgramDestinationV1 as Destination, Gfx942ProgramInstructionV1 as Instruction,
    Gfx942ProgramRoleV1 as Role, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId, WorkgroupSize,
};
fn u32_ty() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn pointer_ty() -> Type {
    Type::pointer(u32_ty(), AddressSpace::Global, AccessMode::ReadWrite)
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}
fn movement(
    block: u8,
    index: u8,
    id: u32,
    destination: Destination,
    source: Role,
    value: u32,
) -> Operation {
    op(
        id,
        u32_ty(),
        OperationKind::Gfx942CompleteBodyStep(Step {
            authored_block: block,
            authored_instruction: index,
            instruction: Instruction::Move {
                destination,
                source,
            },
            operands: [Some(ValueId(value)), None],
        }),
    )
}
fn declaration(blocks: u8, count: u8) -> Operation {
    let mut labels = [0; 8];
    labels[..usize::from(blocks)].copy_from_slice(if blocks == 1 {
        &[251]
    } else {
        &[250, 4, 0, 7]
    });
    Operation::new(
        vec![],
        OperationKind::Gfx942CompleteBodyDeclaration(Gfx942CompleteBodyDeclarationVNext {
            origin: Origin {
                root_axes: [[1; 32]; 5],
                mir_body: [2; 32],
                semantic_block: [3; 32],
                source_signature: [4; 32],
                rustc_fn_abi: [5; 32],
                frontend_bytes_sha256: [6; 32],
                raw_block: 9,
            },
            registers: Registers::new(32, 33, [34, 35, 63]).unwrap(),
            parameters: [ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
            labels,
            block_count: blocks,
            instruction_count: count,
        }),
    )
}
fn tail(base: u32, value: u32) -> Vec<Operation> {
    vec![
        op(
            base,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            base + 1,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            base + 2,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(base),
                rhs: ValueId(base + 1),
            },
        ),
        op(
            base + 3,
            pointer_ty(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            base + 4,
            pointer_ty(),
            OperationKind::GetElementPointer {
                base: ValueId(base + 3),
                offset: ValueId(base),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(base + 4),
                predicate: ValueId(base + 2),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("synthetic_complete_body");
    let mut function = Function::kernel_entry(
        "synthetic_entry",
        Signature::new(
            vec![
                Type::slice(u32_ty(), AddressSpace::Global, AccessMode::ReadWrite),
                u32_ty(),
                u32_ty(),
                u32_ty(),
                u32_ty(),
            ],
            vec![],
        ),
        (0..5).map(ValueId).collect(),
        blocks,
    );
    let mut kernel = Kernel::new(
        "synthetic_kernel",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    function.required_capabilities = function.derived_capabilities();
    kernel.required_capabilities = function.required_capabilities.clone();
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}
pub(crate) fn single() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        declaration(1, 1),
        movement(0, 0, 5, Destination::Output, Role::Input0, 1),
    ];
    block.operations.extend(tail(6, 5));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![block])
}
pub(crate) fn diamond() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        declaration(4, 3),
        movement(0, 0, 5, Destination::Scratch, Role::Input0, 1),
        op(6, u32_ty(), OperationKind::Constant(Constant::U32(0))),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(4),
                rhs: ValueId(6),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(5)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(5)],
    });
    let mut zero = BasicBlock::new(BlockId(1));
    zero.parameters = vec![ValueDef::new(ValueId(8), u32_ty())];
    zero.operations = vec![movement(1, 1, 9, Destination::Output, Role::Scratch, 8)];
    zero.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(8), ValueId(9)],
    });
    let mut nonzero = BasicBlock::new(BlockId(2));
    nonzero.parameters = vec![ValueDef::new(ValueId(10), u32_ty())];
    nonzero.operations = vec![movement(2, 2, 11, Destination::Output, Role::Input2, 3)];
    nonzero.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(10), ValueId(11)],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge.parameters = vec![
        ValueDef::new(ValueId(12), u32_ty()),
        ValueDef::new(ValueId(13), u32_ty()),
    ];
    merge.operations = tail(14, 13);
    merge.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![entry, zero, nonzero, merge])
}
