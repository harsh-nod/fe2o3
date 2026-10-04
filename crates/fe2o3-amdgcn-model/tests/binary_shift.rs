use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_kernel_to_gfx942_llvm_ir, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

fn mixed_width_shift_module(op: BinaryOp) -> Module {
    let lhs_ty = Type::Scalar(ScalarType::U64);
    let rhs_ty = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), lhs_ty.clone()),
        OperationKind::Binary {
            op,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });

    let function = Function::kernel_entry(
        "shift_entry",
        Signature::new(vec![lhs_ty, rhs_ty], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut kernel = Kernel::new(
        "shift_kernel",
        "shift_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::binary-shift");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn mixed_width_shifts_remain_target_neutral_but_fail_closed_for_llvm_lowering() {
    for op in [BinaryOp::ShiftLeft, BinaryOp::ShiftRight] {
        let module = mixed_width_shift_module(op);
        verify_module(&module).expect("mixed-width integer shifts are valid target-neutral KIR");

        for error in [
            lower_kernel_to_llvm_ir(&module, &KernelId::new("shift_kernel")).unwrap_err(),
            lower_kernel_to_gfx942_llvm_ir(&module, &KernelId::new("shift_kernel")).unwrap_err(),
        ] {
            let [diagnostic] = error.diagnostics() else {
                panic!("expected one diagnostic for {op:?}")
            };
            assert_eq!(
                diagnostic.code,
                LoweringDiagnosticCode::UnsupportedOperation
            );
            assert_eq!(diagnostic.location.block, Some(BlockId(0)));
            assert_eq!(diagnostic.location.operation, Some(0));
            assert!(diagnostic.message.contains(&format!("{op:?}")));
            assert!(diagnostic.message.contains("U64"));
            assert!(diagnostic.message.contains("U32"));
        }
    }
}
