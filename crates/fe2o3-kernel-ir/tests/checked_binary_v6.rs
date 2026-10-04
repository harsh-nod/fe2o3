use fe2o3_kernel_ir::*;

fn checked_module(
    lhs_ty: Type,
    rhs_ty: Type,
    value_ty: Type,
    overflow_ty: Type,
    operator: CheckedBinaryOperator,
) -> Module {
    let checked = Operation::checked_binary(
        ValueDef::new(ValueId(2), value_ty),
        ValueDef::new(ValueId(3), overflow_ty),
        operator,
        ValueId(0),
        ValueId(1),
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(checked);
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("checked-binary-v6");
    module.functions.push(Function::definition(
        "checked",
        Signature::new(vec![lhs_ty, rhs_ty], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

fn valid_module(ty: Type, operator: CheckedBinaryOperator) -> Module {
    checked_module(ty.clone(), ty.clone(), ty, Type::BOOL, operator)
}

fn operation(module: &Module) -> &Operation {
    &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0]
}

fn operation_mut(module: &mut Module) -> &mut Operation {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0]
}

#[test]
fn every_signed_unsigned_and_index_width_verifies_for_all_operators() {
    let scalar_types = [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::I128,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::U128,
        ScalarType::Index,
    ];
    let operators = [
        CheckedBinaryOperator::Add,
        CheckedBinaryOperator::Subtract,
        CheckedBinaryOperator::Multiply,
    ];

    for scalar in scalar_types {
        for operator in operators {
            let module = valid_module(Type::Scalar(scalar), operator);
            verify_module(&module)
                .unwrap_or_else(|error| panic!("{scalar:?} {operator:?} failed: {error}"));
        }
    }
}

#[test]
fn operand_and_result_traversal_retains_both_sides_and_both_results() {
    let module = valid_module(Type::Scalar(ScalarType::I32), CheckedBinaryOperator::Add);
    let operation = operation(&module);
    assert_eq!(operation.operands(), vec![ValueId(0), ValueId(1)]);
    assert_eq!(
        operation.result_ids().collect::<Vec<_>>(),
        vec![ValueId(2), ValueId(3)]
    );
    assert_eq!(operation.kind.operands(), vec![ValueId(0), ValueId(1)]);
}

#[test]
fn rejects_float_and_mismatched_operands() {
    let float = valid_module(Type::F32, CheckedBinaryOperator::Add);
    assert!(
        verify_module(&float)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidOperandType)
    );

    let mismatched = checked_module(
        Type::Scalar(ScalarType::I32),
        Type::Scalar(ScalarType::U32),
        Type::Scalar(ScalarType::I32),
        Type::BOOL,
        CheckedBinaryOperator::Subtract,
    );
    assert!(
        verify_module(&mismatched)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidOperandType)
    );
}

#[test]
fn rejects_value_overflow_and_arity_result_mutations() {
    let mut wrong_value = valid_module(
        Type::Scalar(ScalarType::I32),
        CheckedBinaryOperator::Multiply,
    );
    operation_mut(&mut wrong_value).results[0].ty = Type::Scalar(ScalarType::U32);
    assert!(
        verify_module(&wrong_value)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let mut wrong_overflow = valid_module(
        Type::Scalar(ScalarType::U64),
        CheckedBinaryOperator::Multiply,
    );
    operation_mut(&mut wrong_overflow).results[1].ty = Type::Scalar(ScalarType::U8);
    assert!(
        verify_module(&wrong_overflow)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let mut missing = valid_module(Type::Scalar(ScalarType::U16), CheckedBinaryOperator::Add);
    operation_mut(&mut missing).results.pop();
    assert!(
        verify_module(&missing)
            .unwrap_err()
            .contains(DiagnosticCode::ResultArity)
    );

    let mut extra = valid_module(
        Type::Scalar(ScalarType::I8),
        CheckedBinaryOperator::Subtract,
    );
    operation_mut(&mut extra)
        .results
        .push(ValueDef::new(ValueId(4), Type::BOOL));
    assert!(
        verify_module(&extra)
            .unwrap_err()
            .contains(DiagnosticCode::ResultArity)
    );
}
