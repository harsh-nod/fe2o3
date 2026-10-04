    fn push_neutral_recipe_sum_v1(
        operations: &mut Vec<Operation>,
        next_value: &mut u32,
        scalar: &Type,
        lhs: ValueId,
        rhs: ValueId,
    ) -> ValueId {
        if matches!(scalar, Type::Scalar(ScalarType::U32 | ScalarType::I32)) {
            let value = ValueId(*next_value);
            let overflow = ValueId(*next_value + 1);
            *next_value += 2;
            operations.push(Operation::checked_binary(
                ValueDef::new(value, scalar.clone()),
                ValueDef::new(overflow, Type::BOOL),
                CheckedBinaryOperator::Add,
                lhs,
                rhs,
            ));
            value
        } else {
            assert_eq!(*scalar, Type::Scalar(ScalarType::F32));
            push_neutral_recipe_result_v1(
                operations,
                next_value,
                scalar.clone(),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs,
                    rhs,
                },
            )
        }
    }

    #[test]
    fn workgroup_sum_replay_requires_exact_arithmetic_operands_and_result_shape() {
        for scalar in [ScalarType::U32, ScalarType::I32, ScalarType::F32] {
            let scalar = Type::Scalar(scalar);
            let lhs = ValueId(1);
            let rhs = ValueId(2);
            let mut next_value = 3;
            let mut operations = Vec::new();
            let result =
                push_neutral_recipe_sum_v1(&mut operations, &mut next_value, &scalar, lhs, rhs);
            assert_eq!(operations.len(), 1);
            let operation = &operations[0];
            assert_eq!(
                exact_workgroup_sum_result_v1(operation, &scalar, lhs, rhs),
                Some(result),
            );
            let reject = |mutation: fn(&mut Operation)| {
                let mut altered = operation.clone();
                mutation(&mut altered);
                assert_eq!(
                    exact_workgroup_sum_result_v1(&altered, &scalar, lhs, rhs),
                    None,
                    "accepted altered workgroup sum: {altered:?}",
                );
            };
            reject(|operation| {
                let OperationKind::Binary { lhs, rhs, .. } = &mut operation.kind else {
                    unreachable!();
                };
                std::mem::swap(lhs, rhs);
            });
            reject(|operation| operation.results.clear());
            reject(|operation| operation.results[0].ty = Type::Scalar(ScalarType::U64));
            reject(|operation| {
                operation
                    .results
                    .push(ValueDef::new(ValueId(99), Type::BOOL));
            });
            for op in [
                BinaryOp::Subtract,
                BinaryOp::Checked(CheckedBinaryOperator::Subtract),
                BinaryOp::Checked(CheckedBinaryOperator::Multiply),
            ] {
                let mut altered = operation.clone();
                altered.kind = OperationKind::Binary { op, lhs, rhs };
                assert_eq!(
                    exact_workgroup_sum_result_v1(&altered, &scalar, lhs, rhs),
                    None,
                );
            }
            if scalar == Type::Scalar(ScalarType::F32) {
                assert_eq!(next_value, 4);
                reject(|operation| {
                    let OperationKind::Binary { op, .. } = &mut operation.kind else {
                        unreachable!();
                    };
                    *op = BinaryOp::Checked(CheckedBinaryOperator::Add);
                    operation
                        .results
                        .push(ValueDef::new(ValueId(4), Type::BOOL));
                });
            } else {
                assert_eq!(next_value, 5);
                reject(|operation| operation.results.truncate(1));
                reject(|operation| operation.results[1].ty = Type::Scalar(ScalarType::U32));
                reject(|operation| operation.results.swap(0, 1));
                reject(|operation| operation.results[1].id = operation.results[0].id);
                reject(|operation| {
                    let OperationKind::Binary { op, .. } = &mut operation.kind else {
                        unreachable!();
                    };
                    *op = BinaryOp::Add;
                    operation.results.truncate(1);
                });
            }
            assert_eq!(
                exact_workgroup_sum_result_v1(operation, &Type::INDEX, lhs, rhs),
                None,
            );
        }
    }

    #[test]
    fn neutral_recipe_replay_rejects_non_wrapping_integer_sums() {
        for scalar in [ScalarType::U32, ScalarType::I32] {
            for replacement in [
                BinaryOp::Add,
                BinaryOp::Checked(CheckedBinaryOperator::Subtract),
                BinaryOp::Checked(CheckedBinaryOperator::Multiply),
            ] {
                let mut fixture = neutral_recipe_replay_fixture_v1(2, scalar);
                let operation = neutral_consumer_operations_mut_v1(&mut fixture)
                    .iter_mut()
                    .find(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Binary {
                                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                                ..
                            }
                        ) && operation.results[0].ty == Type::Scalar(scalar)
                    })
                    .expect("integer reduction emits a checked scalar sum");
                let OperationKind::Binary { op, .. } = &mut operation.kind else {
                    unreachable!();
                };
                *op = replacement;
                if replacement == BinaryOp::Add {
                    operation.results.truncate(1);
                }
                assert_neutral_recipe_mismatch_v1(&fixture);
            }
        }
    }
