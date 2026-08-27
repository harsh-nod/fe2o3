use fe2o3_kernel_ir::*;

fn module_with_operation(parameters: Vec<Type>, result: Type, float: FloatOperation) -> Module {
    let parameter_ids = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let result_id = ValueId(parameters.len() as u32);
    let mut block = BasicBlock::new(BlockId(0));
    let declaration = float.declaration();
    let mut operation = float.operation(result_id);
    operation.results[0].ty = result.clone();
    block.operations.push(operation);
    block.terminator = Some(Terminator::Return {
        values: vec![result_id],
    });
    let function = Function::internal_helper(
        "float_helper",
        Signature::new(parameters, vec![result]),
        parameter_ids,
        vec![block],
    );
    let mut module = Module::new("tests::float");
    module.functions.push(function);
    module.functions.push(declaration);
    module
}

fn f32_math(function: F32MathFunction) -> Module {
    let arguments = (0..function.arity())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    module_with_operation(
        vec![Type::F32; function.arity()],
        Type::F32,
        FloatOperation::F32Math {
            function,
            implementation: function.required_implementation(),
            arguments,
        },
    )
}

#[test]
fn exact_float_contracts_verify() {
    let conversions = [
        (
            FloatConversionKind::F16ToF32,
            Type::Scalar(ScalarType::F16),
            Type::F32,
        ),
        (
            FloatConversionKind::F32ToF16RoundTiesEven,
            Type::F32,
            Type::Scalar(ScalarType::F16),
        ),
        (
            FloatConversionKind::Bf16ToF32,
            Type::Scalar(ScalarType::Bf16),
            Type::F32,
        ),
        (
            FloatConversionKind::F32ToBf16RoundTiesEven,
            Type::F32,
            Type::Scalar(ScalarType::Bf16),
        ),
    ];
    for (kind, operand, result) in conversions {
        verify_module(&module_with_operation(
            vec![operand],
            result,
            FloatOperation::Convert {
                kind,
                value: ValueId(0),
            },
        ))
        .unwrap();
    }

    for format in [NarrowFloatFormat::F16, NarrowFloatFormat::Bf16] {
        for op in [
            WidenedFloatBinaryOp::Add,
            WidenedFloatBinaryOp::Subtract,
            WidenedFloatBinaryOp::Multiply,
            WidenedFloatBinaryOp::Divide,
        ] {
            let ty = format.ty();
            verify_module(&module_with_operation(
                vec![ty.clone(), ty.clone()],
                ty,
                FloatOperation::WidenedBinary {
                    format,
                    op,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ))
            .unwrap();
        }
    }

    for function in [
        F32MathFunction::Sqrt,
        F32MathFunction::FusedMultiplyAdd,
        F32MathFunction::Floor,
        F32MathFunction::Ceil,
        F32MathFunction::Truncate,
        F32MathFunction::RoundTiesEven,
        F32MathFunction::Sin,
        F32MathFunction::Cos,
        F32MathFunction::Exp,
        F32MathFunction::Exp2,
        F32MathFunction::Ln,
        F32MathFunction::Log2,
        F32MathFunction::Log10,
    ] {
        verify_module(&f32_math(function)).unwrap();
    }

    verify_module(&module_with_operation(
        vec![Type::Scalar(ScalarType::U32); 3],
        Type::Scalar(ScalarType::U32),
        FloatOperation::Bf16x2FusedMultiplyAdd {
            value: ValueId(0),
            multiplier: ValueId(1),
            addend: ValueId(2),
        },
    ))
    .unwrap();
}

#[test]
fn invalid_types_arity_and_implementation_fail_closed() {
    let wrong_type = module_with_operation(
        vec![Type::Scalar(ScalarType::U16)],
        Type::F32,
        FloatOperation::Convert {
            kind: FloatConversionKind::F16ToF32,
            value: ValueId(0),
        },
    );
    assert!(
        verify_module(&wrong_type)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let wrong_implementation = module_with_operation(
        vec![Type::F32],
        Type::F32,
        FloatOperation::F32Math {
            function: F32MathFunction::Sin,
            implementation: F32MathImplementation::ConstrainedLlvm,
            arguments: vec![ValueId(0)],
        },
    );
    assert!(
        verify_module(&wrong_implementation)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidFloatOperation)
    );

    let falsely_constrained_sqrt = module_with_operation(
        vec![Type::F32],
        Type::F32,
        FloatOperation::F32Math {
            function: F32MathFunction::Sqrt,
            implementation: F32MathImplementation::ConstrainedLlvm,
            arguments: vec![ValueId(0)],
        },
    );
    assert!(
        verify_module(&falsely_constrained_sqrt)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidFloatOperation)
    );

    let wrong_arity = module_with_operation(
        vec![Type::F32],
        Type::F32,
        FloatOperation::F32Math {
            function: F32MathFunction::FusedMultiplyAdd,
            implementation: F32MathImplementation::ConstrainedLlvm,
            arguments: vec![ValueId(0)],
        },
    );
    assert!(
        verify_module(&wrong_arity)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidFloatOperation)
    );
}

#[test]
fn sqrt_call_identity_reconstructs_the_ieee_semantic_contract() {
    let sqrt = FloatOperation::F32Math {
        function: F32MathFunction::Sqrt,
        implementation: F32MathImplementation::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1,
        arguments: vec![ValueId(9)],
    };
    assert_eq!(
        F32MathFunction::Sqrt.required_implementation(),
        F32MathImplementation::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1
    );
    assert_eq!(
        FloatOperation::from_intrinsic_call(&sqrt.intrinsic_function_id(), &[ValueId(9)]),
        Some(sqrt)
    );
}

#[test]
fn float_capabilities_are_derived() {
    let f16 = FloatOperation::Convert {
        kind: FloatConversionKind::F16ToF32,
        value: ValueId(0),
    };
    assert_eq!(
        f16.required_capabilities(),
        [TargetCapability::Float16].into_iter().collect()
    );
    let packed = FloatOperation::Bf16x2FusedMultiplyAdd {
        value: ValueId(0),
        multiplier: ValueId(1),
        addend: ValueId(2),
    };
    assert_eq!(
        packed.required_capabilities(),
        [TargetCapability::BFloat16].into_iter().collect()
    );
}

#[test]
fn frozen_wire_round_trips_canonical_float_intrinsic_calls() {
    let module = f32_math(F32MathFunction::Sin);
    let bytes = encode_module_v2(&module).unwrap();
    assert_eq!(&bytes[8..10], &KERNEL_IR_VERSION_V2.to_le_bytes());
    assert_eq!(decode_module_v2(&bytes).unwrap(), module);
    assert_eq!(
        encode_module_v2(&decode_module_v2(&bytes).unwrap()).unwrap(),
        bytes
    );
    for length in 0..bytes.len() {
        assert!(decode_module_v2(&bytes[..length]).is_err());
    }
}

#[test]
fn reserved_float_declarations_reject_mutation() {
    let mut module = module_with_operation(
        vec![Type::Scalar(ScalarType::F16)],
        Type::F32,
        FloatOperation::Convert {
            kind: FloatConversionKind::F16ToF32,
            value: ValueId(0),
        },
    );
    let declaration = module
        .functions
        .iter_mut()
        .find(|function| FloatOperation::from_intrinsic_id(&function.id).is_some())
        .unwrap();
    declaration.signature.results[0] = Type::F64;
    assert!(
        verify_module(&module)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidFloatOperation)
    );
}
