fn exclusive_stride_argument_fixture_v1(
    parameter_ty: SemanticTypeIdV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    invocation: bool,
) -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
    u32,
) {
    let (types, callables, base) = derived_exclusive_index_fixture(64, 16, 63, invocation, false);
    let mut locals = base.locals().to_vec();
    let first = locals.len() as u32 - 4;
    let parameter = locals.len() as u32;
    let widened = parameter + 1;
    locals.push(local(243, parameter_ty, SemanticLocalRoleV1::Argument(2)));
    locals.push(local(244, U64_TYPE, SemanticLocalRoleV1::Temporary));
    let mut blocks = base.blocks().to_vec();
    let old = &blocks[7];
    let mut statements = vec![typed_assignment(
        widened,
        U64_TYPE,
        if parameter_ty == U64_TYPE {
            SemanticRvalueKindV1::Use(typed_operand(parameter, parameter_ty))
        } else {
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: typed_operand(parameter, parameter_ty),
            }
        },
    )];
    let mut replaced = 0;
    for statement in old.statements() {
        if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
            && assignment.destination().local().index() == first + 1
        {
            assert!(matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Multiply,
                    ..
                }
            ));
            replaced += 1;
            statements.push(typed_assignment(
                first + 1,
                U64_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Multiply,
                    left: typed_operand(first, U64_TYPE),
                    right: typed_operand(widened, U64_TYPE),
                },
            ));
        } else {
            statements.push(statement.clone());
        }
    }
    assert_eq!(replaced, 1);
    blocks[7] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        statements,
        old.terminator().clone(),
    )
    .unwrap();
    let mut abi_arguments = base
        .abi()
        .fixed_arguments()
        .iter()
        .map(|argument| argument.value().clone())
        .collect::<Vec<_>>();
    assert_eq!(abi_arguments.len(), 2);
    abi_arguments.push(SemanticAbiValueV1::new(
        parameter_ty,
        SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
    ));
    let mut source_ownership = base.abi().source_argument_ownership().to_vec();
    source_ownership.push(ownership);
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(246)),
        SemanticLayoutIdentityV1::from_sha256(bytes(246)),
        base.abi().canon_abi(),
        base.abi().can_unwind(),
        false,
        abi_arguments,
        base.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(source_ownership)
    .unwrap();
    let function = exclusive_argument_function_v1(&base, abi, locals, blocks);
    assert_eq!(function.abi().source_input_types()[2], parameter_ty);
    assert_eq!(
        function.locals()[parameter as usize].role(),
        SemanticLocalRoleV1::Argument(2)
    );
    (types, callables, function, parameter)
}

fn exclusive_argument_function_v1(
    base: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        base.identity(),
        base.role(),
        base.item_definition_identity(),
        base.monomorphization_identity(),
        base.generic_type_arguments_identity(),
        base.const_generic_arguments_identity(),
        base.source(),
        abi,
        locals,
        base.entry(),
        blocks,
    )
    .unwrap()
}

fn evaluate_stride_index_v1(
    operations: &[ProductionRankedOperationV1],
    output: ProductionRankedValueV1,
    raw: u64,
    stride: u64,
) -> u64 {
    let mut values = HashMap::new();
    let value = |values: &HashMap<ProductionRankedValueV1, u64>,
                 operand: &ProductionRankedValueV1| {
        match operand {
            ProductionRankedValueV1::Argument(_) => stride,
            ProductionRankedValueV1::Local(_) => values[operand],
            ProductionRankedValueV1::BlockArgument { .. } => {
                panic!("the straight-line stride fixture must not contain a loop block argument")
            }
        }
    };
    for operation in operations {
        let (result, result_value) = match operation {
            ProductionRankedOperationV1::InvocationIndex {
                result,
                dimension: 0,
                ..
            } => (*result, raw),
            ProductionRankedOperationV1::IndexConstant { result, value } => (*result, *value),
            ProductionRankedOperationV1::IndexBinary {
                result,
                kind,
                lhs,
                rhs,
            } => {
                let lhs = value(&values, lhs);
                let rhs = value(&values, rhs);
                (
                    *result,
                    match kind {
                        IndexBinaryKindAttr::Add => lhs.checked_add(rhs).unwrap(),
                        IndexBinaryKindAttr::Multiply => lhs.checked_mul(rhs).unwrap(),
                        IndexBinaryKindAttr::Divide => lhs / rhs,
                        IndexBinaryKindAttr::Remainder => lhs % rhs,
                    },
                )
            }
            _ => continue,
        };
        values.insert(ProductionRankedValueV1::Local(result), result_value);
    }
    value(&values, &output)
}

#[test]
fn exclusive_index_retains_widened_source_stride_and_invocation_arithmetic() {
    let (types, callables, function, _) = exclusive_stride_argument_fixture_v1(
        SCALAR_TYPE,
        SemanticSourceArgumentOwnershipV1::ByValue,
        true,
    );
    let (projection, operations) =
        project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
            .unwrap();
    let write = projection.direct_write_effects[7].as_ref().unwrap();
    assert_eq!(write.access, AccessKindAttr::Write);
    assert_eq!(write.comparisons.len(), 1);
    for stride in [0, 1, 64, 4096, u64::from(u32::MAX)] {
        for raw in [0, 1, 63, 64, 1023] {
            assert_eq!(
                evaluate_stride_index_v1(&operations, write.indices[0], raw, stride),
                (raw / 64) * stride + (raw & 63)
            );
        }
    }
    // This is an index-expression projection, not a race-freedom/bounds claim.
}

#[test]
fn exclusive_index_source_stride_requires_independent_invocation_and_totality() {
    for (ty, ownership, invocation, launch) in [
        (
            SCALAR_TYPE,
            SemanticSourceArgumentOwnershipV1::Unspecified,
            true,
            Some(1024),
        ),
        (
            SCALAR_TYPE,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            true,
            Some(1024),
        ),
        (
            U64_TYPE,
            SemanticSourceArgumentOwnershipV1::ByValue,
            true,
            Some(1024),
        ),
        (
            SCALAR_TYPE,
            SemanticSourceArgumentOwnershipV1::ByValue,
            false,
            Some(1024),
        ),
        (
            SCALAR_TYPE,
            SemanticSourceArgumentOwnershipV1::ByValue,
            true,
            None,
        ),
    ] {
        let (types, callables, function, _) =
            exclusive_stride_argument_fixture_v1(ty, ownership, invocation);
        assert_incomplete(
            project_capability_index_fixture_with_launch(&types, &callables, &function, launch),
            "a typed global exclusive store requires an exact invocation-derived index",
        );
    }
}

#[test]
fn exclusive_index_source_stride_rejects_definition_and_identity_substitution() {
    for mutation in 0..3 {
        let (types, callables, function, parameter) = exclusive_stride_argument_fixture_v1(
            SCALAR_TYPE,
            SemanticSourceArgumentOwnershipV1::ByValue,
            true,
        );
        let mut locals = function.locals().to_vec();
        let mut blocks = function.blocks().to_vec();
        if mutation < 2 {
            let old = &locals[parameter as usize];
            locals[parameter as usize] = SemanticLocalDeclV1::new(
                old.identity(),
                old.ty(),
                if mutation == 0 {
                    SemanticLocalRoleV1::Temporary
                } else {
                    SemanticLocalRoleV1::Argument(0)
                },
                old.source(),
            );
        } else {
            let old = &blocks[7];
            let mut statements = vec![
                typed_assignment(
                    parameter,
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Use(typed_constant(SCALAR_TYPE, 16, 4)),
                ),
                typed_assignment(
                    parameter,
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Use(typed_constant(SCALAR_TYPE, 32, 4)),
                ),
            ];
            statements.extend_from_slice(old.statements());
            blocks[7] = SemanticBasicBlockV1::new(
                old.identity(),
                old.source(),
                statements,
                old.terminator().clone(),
            )
            .unwrap();
        }
        let changed =
            exclusive_argument_function_v1(&function, function.abi().clone(), locals, blocks);
        assert_incomplete(
            project_capability_index_fixture_with_launch(&types, &callables, &changed, Some(1024)),
            "a typed global exclusive store requires an exact invocation-derived index",
        );
    }
}

#[test]
fn exclusive_index_source_stride_optional_read_cannot_allocate_arguments() {
    let (types, callables, function, _) = exclusive_stride_argument_fixture_v1(
        SCALAR_TYPE,
        SemanticSourceArgumentOwnershipV1::ByValue,
        true,
    );
    let (projection, _) =
        project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
            .unwrap();
    let inventory = assertion_definition_inventory(&function).unwrap();
    let constants = constant_locals(&function).unwrap();
    let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
    let mut arguments = vec![None; function.locals().len()];
    let mut next_argument = 0;
    let mut operations = Vec::new();
    let mut next_value = 0;
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[7].terminator().kind() else {
        unreachable!()
    };
    let result = TotalUnsignedIndexProjectorV1::new(
        &types,
        &function,
        &constants,
        &inventory.counts,
        &inventory.address_escaped,
        &inventory.assignments,
        &mut proof,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
    )
    .unwrap()
    .with_invocation_roots(&callables, &projection.index_values, 1024)
    .unwrap()
    .with_exclusive_source_arguments_v1()
    .unwrap()
    .optional_read_index_v1(
        &call.arguments()[1],
        7,
        function.blocks()[7].statements().len(),
    )
    .unwrap();
    assert!(result.is_none());
    assert!(arguments.iter().all(Option::is_none));
    assert!(operations.is_empty());
    assert_eq!(next_argument, 0);
    assert_eq!(next_value, 0);
}

#[test]
fn exclusive_index_parameter_preserves_source_slot_bound_and_shared_work_failure() {
    let (types, callables, function, parameter) = exclusive_stride_argument_fixture_v1(
        SCALAR_TYPE,
        SemanticSourceArgumentOwnershipV1::ByValue,
        true,
    );
    let indices = vec![None; function.locals().len()];
    let inventory = assertion_definition_inventory(&function).unwrap();
    let constants = constant_locals(&function).unwrap();
    for exhausted in [false, true] {
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        if exhausted {
            proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - 7;
        }
        let before = proof.work;
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 7;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let mut projector = TotalUnsignedIndexProjectorV1::new(
            &types,
            &function,
            &constants,
            &inventory.counts,
            &inventory.address_escaped,
            &inventory.assignments,
            &mut proof,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )
        .unwrap()
        .with_invocation_roots(&callables, &indices, 1024)
        .unwrap()
        .with_exclusive_source_arguments_v1()
        .unwrap();
        // Exact original source use: bb7:s0 widens this declared parameter.
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[7].statements()[0].kind()
        else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Cast { operand, .. } = assignment.value().kind() else {
            unreachable!()
        };
        assert_eq!(simple_operand_local(operand).unwrap().index(), parameter);
        let result = projector.resolve_operand(operand, 7, 0);
        drop(projector);
        assert_eq!(proof.work, before + 8);
        assert!(operations.is_empty());
        assert_eq!(next_value, 0);
        if exhausted {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "uniform induction CFG analysis exceeds its work limit"
                ))
            ));
            assert!(arguments.iter().all(Option::is_none));
            assert_eq!(next_argument, 7);
        } else {
            let value = result.unwrap().unwrap();
            assert_eq!(value.ranked, ProductionRankedValueV1::Argument(7));
            assert_eq!(value.maximum, u64::from(u32::MAX));
            assert_eq!(value.exact, None);
            assert!(!value.invocation_dependent);
            assert_eq!(arguments[2], Some(7));
            assert!(
                arguments
                    .iter()
                    .enumerate()
                    .all(|(index, value)| index == 2 || value.is_none())
            );
            assert_eq!(next_argument, 8);
        }
    }
}
