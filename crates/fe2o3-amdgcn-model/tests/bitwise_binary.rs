use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_kernel_to_gfx942_llvm_ir, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

const SUPPORTED_BITWISE_SCALARS: [ScalarType; 10] = [
    ScalarType::Bool,
    ScalarType::I8,
    ScalarType::I16,
    ScalarType::I32,
    ScalarType::I64,
    ScalarType::U8,
    ScalarType::U16,
    ScalarType::U32,
    ScalarType::U64,
    ScalarType::Index,
];

const BITWISE_OPERATORS: [(BinaryOp, &str); 3] = [
    (BinaryOp::BitAnd, "and"),
    (BinaryOp::BitOr, "or"),
    (BinaryOp::BitXor, "xor"),
];

fn bitwise_module(scalars: &[ScalarType], operators: &[BinaryOp]) -> Module {
    let parameter_count = scalars.len() * 2;
    let mut parameter_types = Vec::with_capacity(parameter_count);
    let mut parameter_ids = Vec::with_capacity(parameter_count);
    let mut operations = Vec::with_capacity(scalars.len() * operators.len());

    for (scalar_index, scalar) in scalars.iter().copied().enumerate() {
        let ty = Type::Scalar(scalar);
        let lhs = ValueId((scalar_index * 2) as u32);
        let rhs = ValueId((scalar_index * 2 + 1) as u32);
        parameter_types.extend([ty.clone(), ty.clone()]);
        parameter_ids.extend([lhs, rhs]);
        for (operator_index, operator) in operators.iter().copied().enumerate() {
            let result = parameter_count + scalar_index * operators.len() + operator_index;
            operations.push(Operation::effect_free(
                ValueDef::new(ValueId(result as u32), ty.clone()),
                OperationKind::Binary {
                    op: operator,
                    lhs,
                    rhs,
                },
            ));
        }
    }

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = operations;
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::kernel_entry(
        "bitwise_entry",
        Signature::new(parameter_types, vec![]),
        parameter_ids,
        vec![entry],
    );
    let mut kernel = Kernel::new(
        "bitwise_kernel",
        "bitwise_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::bitwise-binary");
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
        _ => panic!("unsupported bitwise test scalar {scalar:?}"),
    }
}

fn assert_complete_bitwise_llvm(llvm: &str) {
    let parameter_count = SUPPORTED_BITWISE_SCALARS.len() * 2;
    for (scalar_index, scalar) in SUPPORTED_BITWISE_SCALARS.iter().copied().enumerate() {
        for (operator_index, (_, opcode)) in BITWISE_OPERATORS.iter().copied().enumerate() {
            let result = parameter_count + scalar_index * BITWISE_OPERATORS.len() + operator_index;
            let lhs = scalar_index * 2;
            let rhs = lhs + 1;
            let expected = format!(
                "%v{result} = {opcode} {} %arg{lhs}, %arg{rhs}",
                llvm_type(scalar)
            );
            assert!(llvm.contains(&expected), "missing {expected:?} in:\n{llvm}");
        }
    }
}

#[test]
fn every_supported_boolean_and_integer_bitwise_operation_has_exact_llvm() {
    let operators = BITWISE_OPERATORS.map(|(operator, _)| operator);
    let module = bitwise_module(&SUPPORTED_BITWISE_SCALARS, &operators);

    let baseline = lower_kernel_to_llvm_ir(&module, &KernelId::new("bitwise_kernel")).unwrap();
    assert_complete_bitwise_llvm(&baseline);

    let gfx942 = lower_kernel_to_gfx942_llvm_ir(&module, &KernelId::new("bitwise_kernel")).unwrap();
    assert_complete_bitwise_llvm(&gfx942);
}

#[test]
fn unsupported_integer_widths_remain_located_and_fail_closed() {
    for scalar in [ScalarType::I128, ScalarType::U128] {
        for operator in [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor] {
            let error = lower_kernel_to_llvm_ir(
                &bitwise_module(&[scalar], &[operator]),
                &KernelId::new("bitwise_kernel"),
            )
            .unwrap_err();
            let [diagnostic] = error.diagnostics() else {
                panic!("expected one diagnostic for {operator:?} over {scalar:?}")
            };
            assert_eq!(
                diagnostic.code,
                LoweringDiagnosticCode::UnsupportedOperation
            );
            assert_eq!(diagnostic.location.block, Some(BlockId(0)));
            assert_eq!(diagnostic.location.operation, Some(0));
            assert!(diagnostic.message.contains(&format!("{operator:?}")));
            assert!(diagnostic.message.contains(&format!("{scalar:?}")));
        }
    }
}

#[test]
fn non_integral_bitwise_input_is_rejected_by_kernel_ir_verification() {
    let error = lower_kernel_to_llvm_ir(
        &bitwise_module(&[ScalarType::F32], &[BinaryOp::BitOr]),
        &KernelId::new("bitwise_kernel"),
    )
    .unwrap_err();
    assert!(error.contains(LoweringDiagnosticCode::InputVerification(
        DiagnosticCode::InvalidOperandType
    )));
}
