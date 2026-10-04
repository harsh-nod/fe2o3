use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Constant, Function, IntrinsicOperation, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV7, WorkgroupSize,
};

fn operation(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

pub fn canonical_kir_v7() -> Vec<u8> {
    let element = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        operation(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        operation(
            2,
            pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(1),
            },
        ),
        operation(3, element, OperationKind::Constant(Constant::U32(17))),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });

    let entry = Function::kernel_entry(
        "fill_impl",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut kernel = Kernel::new(
        "fill",
        "fill_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("fe2o3-kir-sim-cli::tutorial-fill-v1");
    module.functions.push(entry);
    module.kernels.push(kernel);
    VerifiedCanonicalKernelIrV7::from_module(module)
        .expect("tutorial fill must remain verified canonical KIR V7")
        .into_canonical_bytes()
}
