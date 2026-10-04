use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_kernel_to_gfx942_llvm_ir, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

const SUPPORTED_UNARY_CASES: [(UnaryOp, ScalarType); 15] = [
    (UnaryOp::Not, ScalarType::Bool),
    (UnaryOp::Not, ScalarType::I8),
    (UnaryOp::Not, ScalarType::I16),
    (UnaryOp::Not, ScalarType::I32),
    (UnaryOp::Not, ScalarType::I64),
    (UnaryOp::Not, ScalarType::U8),
    (UnaryOp::Not, ScalarType::U16),
    (UnaryOp::Not, ScalarType::U32),
    (UnaryOp::Not, ScalarType::U64),
    (UnaryOp::Not, ScalarType::Index),
    (UnaryOp::Negate, ScalarType::I8),
    (UnaryOp::Negate, ScalarType::I16),
    (UnaryOp::Negate, ScalarType::I32),
    (UnaryOp::Negate, ScalarType::I64),
    (UnaryOp::Negate, ScalarType::F32),
];

fn unary_module(cases: &[(UnaryOp, ScalarType)]) -> Module {
    let parameter_types = cases
        .iter()
        .map(|(_, scalar)| Type::Scalar(*scalar))
        .collect::<Vec<_>>();
    let parameter_ids = (0..cases.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let operations = cases
        .iter()
        .copied()
        .enumerate()
        .map(|(index, (op, scalar))| {
            Operation::effect_free(
                ValueDef::new(ValueId((cases.len() + index) as u32), Type::Scalar(scalar)),
                OperationKind::Unary {
                    op,
                    operand: ValueId(index as u32),
                },
            )
        })
        .collect();

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = operations;
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::kernel_entry(
        "unary_entry",
        Signature::new(parameter_types, vec![]),
        parameter_ids,
        vec![entry],
    );
    let mut kernel = Kernel::new(
        "unary_kernel",
        "unary_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::unary");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn llvm_type(scalar: ScalarType) -> &'static str {
    match scalar {
        ScalarType::Bool => "i1",
        ScalarType::I8 | ScalarType::U8 => "i8",
        ScalarType::I16 | ScalarType::U16 => "i16",
        ScalarType::I32 | ScalarType::U32 => "i32",
        ScalarType::I64 | ScalarType::U64 | ScalarType::Index => "i64",
        ScalarType::F32 => "float",
        _ => panic!("unsupported unary test scalar {scalar:?}"),
    }
}

fn assert_complete_unary_llvm(llvm: &str) {
    for (index, (op, scalar)) in SUPPORTED_UNARY_CASES.iter().copied().enumerate() {
        let result = SUPPORTED_UNARY_CASES.len() + index;
        let ty = llvm_type(scalar);
        let expected = match (op, scalar) {
            (UnaryOp::Not, ScalarType::Bool) => {
                format!("%v{result} = xor i1 %arg{index}, true")
            }
            (UnaryOp::Not, _) => format!("%v{result} = xor {ty} %arg{index}, -1"),
            (UnaryOp::Negate, ScalarType::F32) => {
                format!("%v{result} = fneg float %arg{index}")
            }
            (UnaryOp::Negate, _) => format!("%v{result} = sub {ty} 0, %arg{index}"),
        };
        assert!(llvm.contains(&expected), "missing {expected:?} in:\n{llvm}");
    }
}

#[test]
fn every_supported_unary_operation_has_exact_llvm() {
    let module = unary_module(&SUPPORTED_UNARY_CASES);
    let baseline = lower_kernel_to_llvm_ir(&module, &KernelId::new("unary_kernel")).unwrap();
    assert_complete_unary_llvm(&baseline);

    let gfx942 = lower_kernel_to_gfx942_llvm_ir(&module, &KernelId::new("unary_kernel")).unwrap();
    assert_complete_unary_llvm(&gfx942);
}

#[test]
fn unsupported_integer_widths_remain_located_and_fail_closed() {
    for case in [
        (UnaryOp::Not, ScalarType::I128),
        (UnaryOp::Not, ScalarType::U128),
        (UnaryOp::Negate, ScalarType::I128),
    ] {
        let error = lower_kernel_to_llvm_ir(&unary_module(&[case]), &KernelId::new("unary_kernel"))
            .unwrap_err();
        let [diagnostic] = error.diagnostics() else {
            panic!("expected one diagnostic for {case:?}")
        };
        assert_eq!(
            diagnostic.code,
            LoweringDiagnosticCode::UnsupportedOperation
        );
        assert_eq!(diagnostic.location.block, Some(BlockId(0)));
        assert_eq!(diagnostic.location.operation, Some(0));
        assert!(diagnostic.message.contains(&format!("{:?}", case.0)));
        assert!(diagnostic.message.contains(&format!("{:?}", case.1)));
    }
}

#[test]
fn semantically_invalid_unary_inputs_fail_kernel_ir_verification() {
    for case in [
        (UnaryOp::Not, ScalarType::F32),
        (UnaryOp::Negate, ScalarType::U32),
    ] {
        let error = lower_kernel_to_llvm_ir(&unary_module(&[case]), &KernelId::new("unary_kernel"))
            .unwrap_err();
        assert!(error.contains(LoweringDiagnosticCode::InputVerification(
            DiagnosticCode::InvalidOperandType
        )));
    }
}
