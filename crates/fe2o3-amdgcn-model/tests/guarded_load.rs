use fe2o3_amdgcn_model::{lower_compiler_module_to_llvm_ir, lower_kernel_to_gfx942_llvm_ir};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId,
};

fn guarded_load_module() -> Module {
    let slice = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(3),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
        ),
        OperationKind::SliceData { slice: ValueId(0) },
    ));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
        OperationKind::GuardedLoad {
            pointer: ValueId(3),
            predicate: ValueId(1),
            fallback: ValueId(2),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("guarded-load-backend");
    module.functions.push(Function::kernel_entry(
        "guarded_load",
        Signature::new(
            vec![slice, Type::BOOL, Type::Scalar(ScalarType::U32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "guarded_load",
        "guarded_load",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

fn assert_non_speculative_cfg_diamond(llvm: &str) {
    let branch = llvm
        .find("br i1 %arg1, label %guarded_load_bb0_op1_true, label %guarded_load_bb0_op1_false")
        .unwrap();
    let true_label = llvm[branch..]
        .find("guarded_load_bb0_op1_true:")
        .map(|offset| branch + offset)
        .unwrap();
    let load = llvm[true_label..]
        .find("%v4.loaded = load i32, ptr addrspace(1) %v3, align 4")
        .map(|offset| true_label + offset)
        .unwrap();
    let false_label = llvm[load..]
        .find("guarded_load_bb0_op1_false:")
        .map(|offset| load + offset)
        .unwrap();
    let merge_label = llvm[false_label..]
        .find("guarded_load_bb0_op1_merge:")
        .map(|offset| false_label + offset)
        .unwrap();
    assert!(branch < true_label && true_label < load && load < false_label);
    assert!(!llvm[false_label..merge_label].contains(" load "));
    assert!(llvm[merge_label..].contains(
        "%v4 = phi i32 [ %v4.loaded, %guarded_load_bb0_op1_true ], [ %arg2, %guarded_load_bb0_op1_false ]"
    ));
}

#[test]
fn guarded_load_lowers_to_a_non_speculative_cfg_diamond() {
    let module = guarded_load_module();
    assert_non_speculative_cfg_diamond(&lower_compiler_module_to_llvm_ir(&module).unwrap());
    assert_non_speculative_cfg_diamond(
        &lower_kernel_to_gfx942_llvm_ir(&module, &module.kernels[0].id).unwrap(),
    );
}
