use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 512 * 1024 * 1024;

fn transform(types: &mut Vec<SemanticTypeDeclV1>, functions: &mut Vec<SemanticFunctionDeclV1>) {
    super::super::super::paired::aggregate_tests::checked_transform(
        types,
        functions,
        SemanticCheckedBinaryOpV1::Add,
        false,
        false,
    );
    let old = functions.last_mut().unwrap();
    let scalar = old.locals()[1].ty();
    let raw = TypeId::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([251; 32]),
        SemanticLayoutIdentityV1::from_sha256([251; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            BackendRepr::scalar(BackendScalar::initialized(
                BackendPrimitive::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                scalar,
                PointerKind::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                PointerMetadata::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = old.locals().to_vec();
    assert_eq!(locals.len(), 5);
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([251; 32]),
        raw,
        SemanticLocalRoleV1::Temporary,
        old.source(),
    ));
    let place = |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let field = Place::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), scalar).unwrap()],
        scalar,
    )
    .unwrap();
    let assign = |destination: Place, value| {
        SemanticStatementV1::new(
            old.source(),
            Statement::Assign(SemanticAssignmentV1::new(
                destination.clone(),
                SemanticRvalueV1::new(destination.ty(), value),
            )),
        )
    };
    let mut blocks = old.blocks().to_vec();
    let mut statements = blocks[2].statements().to_vec();
    statements.extend([
        assign(
            place(3, scalar),
            Rvalue::Use(Operand::Copy(place(1, scalar))),
        ),
        assign(
            place(5, raw),
            Rvalue::AddressOf {
                place: place(3, scalar),
                mutability: SemanticMutabilityV1::Mutable,
            },
        ),
        assign(
            field.clone(),
            Rvalue::Binary {
                operation: SemanticBinaryOpV1::Subtract,
                left: Operand::Copy(field.clone()),
                right: Operand::Copy(place(2, scalar)),
            },
        ),
        assign(
            place(3, scalar),
            Rvalue::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: Operand::Move(place(3, scalar)),
                right: Operand::Copy(place(2, scalar)),
            },
        ),
        assign(
            place(0, scalar),
            Rvalue::Binary {
                operation: SemanticBinaryOpV1::Multiply,
                left: Operand::Move(field),
                right: Operand::Copy(place(2, scalar)),
            },
        ),
    ]);
    blocks[2] = SemanticBasicBlockV1::new(
        blocks[2].identity(),
        blocks[2].source(),
        statements,
        blocks[2].terminator().clone(),
    )
    .unwrap();
    *old = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
}

#[test]
fn original_wrapping_projected_components_and_memory_moves_retain_ordered_events() {
    super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        transform,
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                for root in 0..2 {
                    for instance in 1..=2 {
                        assert!(slots.has_original_object(root, instance, 3, out)?);
                        let body = SourceByteBody::derive(plan, slots, root, instance, out)?;
                        let operations: Vec<_> = body
                            .events
                            .iter()
                            .filter_map(|event| match event {
                                Event::ScalarOperands(operation)
                                    if operation.operator.wrapping() =>
                                {
                                    Some(operation)
                                }
                                _ => None,
                            })
                            .collect();
                        assert_eq!(operations.len(), 3);
                        assert_eq!(operations[0].operator, Operator::WrappingSubtract);
                        assert!(matches!(
                            operations[0].left,
                            Value::Component { moved: false, .. }
                        ));
                        assert!(matches!(
                            operations[0].destination,
                            Destination::Component(_)
                        ));
                        assert_eq!(operations[1].operator, Operator::WrappingAdd);
                        assert!(matches!(
                            operations[1].left,
                            Value::Read { moved: true, .. }
                        ));
                        assert!(matches!(operations[1].destination, Destination::Memory(_)));
                        assert_eq!(operations[2].operator, Operator::WrappingMultiply);
                        assert!(matches!(
                            operations[2].left,
                            Value::Component { moved: true, .. }
                        ));
                        assert!(matches!(operations[2].destination, Destination::Local(_)));
                        assert!(operations.iter().all(|operation| matches!(
                            operation.right,
                            Some(Value::Local { moved: false, .. })
                        )));
                    }
                }
                let mut generated =
                    super::super::super::source_function::SourceByteProgram::derive(
                        plan, slots, out,
                    )?;
                generated.emit(out)?;
                assert!(out.text.contains("InvocationSourceByteValueV36::Component"));
                assert!(out.text.contains("InvocationSourceByteValueV36::Read"));
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_wrapping_dispatch_never_interprets_checked_or_unchecked_rvalues() {
    for (checked, unchecked) in [
        (
            SemanticCheckedBinaryOpV1::Add,
            SemanticUncheckedBinaryOpV1::Add,
        ),
        (
            SemanticCheckedBinaryOpV1::Subtract,
            SemanticUncheckedBinaryOpV1::Subtract,
        ),
        (
            SemanticCheckedBinaryOpV1::Multiply,
            SemanticUncheckedBinaryOpV1::Multiply,
        ),
    ] {
        super::super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| {
                super::super::super::paired::aggregate_tests::checked_transform(
                    types, functions, checked, false, false,
                )
            },
            |plan, out| {
                super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                    let semantic = slots
                        .correspondence(out)?
                        .source(out.budget)?
                        .source_semantic(out.budget)?;
                    let mut reached = 0;
                    for root in 0..2 {
                        for instance in 1..=2 {
                            let row = plan.instance(root, instance, out)?;
                            let function = &semantic.functions()[row.function.index() as usize];
                            let context = Context {
                                slots,
                                types: semantic.types(),
                                function,
                                root,
                                instance,
                                locals: row.locals.clone(),
                            };
                            for statement in function.blocks()[0].statements() {
                                let Statement::Assign(assignment) = statement.kind() else {
                                    continue;
                                };
                                let Rvalue::CheckedBinary(binary) = assignment.value().kind()
                                else {
                                    continue;
                                };
                                reached += 1;
                                assert!(matches!(
                                    Operation::derive(
                                        &context,
                                        assignment,
                                        Destination::Local(context.locals.start + 4),
                                        out,
                                    ),
                                    Err(Error::Statement(
                                        "original MIR typed byte statement is not modeled"
                                    ))
                                ));
                                let scalar = binary.left().ty();
                                let unchecked = SemanticAssignmentV1::new(
                                    Place::new(SemanticLocalIdV1::from_index(0), vec![], scalar)
                                        .unwrap(),
                                    SemanticRvalueV1::new(
                                        scalar,
                                        Rvalue::UncheckedBinary(
                                            SemanticUncheckedBinaryRvalueV1::new(
                                                unchecked,
                                                binary.left().clone(),
                                                binary.right().clone(),
                                            ),
                                        ),
                                    ),
                                );
                                assert!(matches!(
                                    Operation::derive(
                                        &context,
                                        &unchecked,
                                        Destination::Local(context.locals.start),
                                        out,
                                    ),
                                    Err(Error::Statement(
                                        "original MIR typed byte statement is not modeled"
                                    ))
                                ));
                            }
                        }
                    }
                    assert_eq!(reached, 4);
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}
