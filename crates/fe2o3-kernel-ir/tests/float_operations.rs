use fe2o3_kernel_ir::*;

fn module_with_operation(parameters: Vec<Type>, result: Type, float: FloatOperation) -> Module {
    let parameter_ids = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let result_id = ValueId(parameters.len() as u32);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(result_id, result.clone()),
        OperationKind::Float(float),
    ));
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
fn v3_wire_round_trips_and_older_writers_reject() {
    let module = f32_math(F32MathFunction::Sin);
    assert!(matches!(
        encode_module_v1(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion { version: 1, .. })
    ));
    assert!(matches!(
        encode_module_v2(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion { version: 2, .. })
    ));

    let bytes = encode_module_v3(&module).unwrap();
    assert_eq!(&bytes[8..10], &KERNEL_IR_VERSION_V3.to_le_bytes());
    assert_eq!(decode_module_v3(&bytes).unwrap(), module);
    assert_eq!(
        encode_module_v3(&decode_module_v3(&bytes).unwrap()).unwrap(),
        bytes
    );
    assert!(matches!(
        decode_module_v2(&bytes),
        Err(KernelIrDecodeError::UnknownVersion(3))
    ));
    for length in 0..bytes.len() {
        assert!(decode_module_v3(&bytes[..length]).is_err());
    }
}

#[test]
fn v3_wire_rejects_mutated_float_tags() {
    let module = module_with_operation(
        vec![Type::Scalar(ScalarType::F16)],
        Type::F32,
        FloatOperation::Convert {
            kind: FloatConversionKind::F16ToF32,
            value: ValueId(0),
        },
    );
    let bytes = encode_module_v3(&module).unwrap();
    let needle = [21, 1, 1, 0, 0, 0, 0];
    let matches = bytes
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    let [offset] = matches.as_slice() else {
        panic!("expected one canonical float operation, found {matches:?}")
    };

    let mut operation = bytes.clone();
    operation[*offset + 1] = 0xff;
    assert!(matches!(
        decode_module_v3(&operation),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "floating-point operation",
            tag: 0xff
        })
    ));

    let mut conversion = bytes;
    conversion[*offset + 2] = 0xff;
    assert!(matches!(
        decode_module_v3(&conversion),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "float conversion",
            tag: 0xff
        })
    ));
}
