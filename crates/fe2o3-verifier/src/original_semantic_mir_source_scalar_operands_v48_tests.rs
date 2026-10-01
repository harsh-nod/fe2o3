use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 256 * 1024 * 1024;

fn retained_scalar_transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
) {
    let old = functions.last_mut().unwrap();
    assert_eq!(old.locals().len(), 4);
    let scalar = old.locals()[1].ty();
    let raw = TypeId::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([241; 32]),
        SemanticLayoutIdentityV1::from_sha256([241; 32]),
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
    let boolean = TypeId::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([242; 32]),
        SemanticLayoutIdentityV1::from_sha256([242; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            BackendRepr::scalar(BackendScalar::initialized(
                BackendPrimitive::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        Shape::Scalar(SemanticScalarTypeV1::Bool),
    ));
    let mut locals = old.locals().to_vec();
    for (index, ty) in [scalar, raw, boolean].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([241 + index as u8; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            old.source(),
        ));
    }
    let place = |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
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
    let mut statements = blocks[0].statements().to_vec();
    statements.extend([
        assign(
            place(4, scalar),
            Rvalue::Use(Operand::Copy(place(1, scalar))),
        ),
        assign(
            place(5, raw),
            Rvalue::AddressOf {
                place: place(4, scalar),
                mutability: SemanticMutabilityV1::Mutable,
            },
        ),
        assign(
            place(4, scalar),
            Rvalue::Binary {
                operation: SemanticBinaryOpV1::BitXor,
                left: Operand::Copy(place(4, scalar)),
                right: Operand::Copy(place(2, scalar)),
            },
        ),
        assign(
            place(3, scalar),
            Rvalue::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand: Operand::Copy(place(4, scalar)),
            },
        ),
        assign(
            place(6, boolean),
            Rvalue::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: Operand::Copy(place(4, scalar)),
                right: Operand::Copy(place(2, scalar)),
            },
        ),
    ]);
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements,
        blocks[0].terminator().clone(),
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

fn retained_program(work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        retained_scalar_transform,
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                for root in 0..2 {
                    for instance in 1..=2 {
                        assert!(slots.has_original_object(root, instance, 4, out)?);
                        let body = SourceByteBody::derive(plan, slots, root, instance, out)?;
                        let operations: Vec<_> = body
                            .events
                            .iter()
                            .filter_map(|event| {
                                if let Event::ScalarOperands(operation) = event {
                                    Some(operation)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        assert_eq!(operations.len(), 3);
                        assert!(
                            operations
                                .iter()
                                .all(|op| matches!(op.left, Value::Read { moved: false, .. }))
                        );
                        assert!(matches!(operations[0].destination, Destination::Memory(_)));
                        assert_eq!(operations[0].operator.code(), 3);
                        assert_eq!(operations[1].operator, Operator::Not);
                        assert_eq!(operations[2].output, ScalarV30::Bool);
                    }
                }
                let mut program = super::super::super::source_function::SourceByteProgram::derive(
                    plan, slots, out,
                )?;
                let paired = super::super::super::paired::PairedInvocations::derive(
                    plan,
                    &program,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    out,
                )?;
                slots.emit(out)?;
                program.emit(out)?;
                paired.emit(out)?;
                assert_eq!(
                    out.text
                        .matches("InvocationSourceByteEventV36::ScalarOperands(")
                        .count(),
                    12
                );
                assert!(
                    out.text
                        .contains("invocation_source_byte_step_v36(cursor.source, event")
                );
                assert!(!out.text.contains("assume("));
                Ok(())
            })
        },
    )
}

#[test]
fn original_memory_scalar_operands_reach_all_original_call_instances() {
    retained_program(LIMIT, LIMIT).0.unwrap();
}

#[test]
fn original_memory_scalar_operands_complete_program_has_exact_and_one_short_resources() {
    let measured = retained_program(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = retained_program(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (measured.1, measured.3));
    for (work, storage, is_work) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let error = retained_program(work, storage).0.unwrap_err();
        let mut chain: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = chain.downcast_ref::<Resource>() {
                break resource;
            }
            chain = chain
                .source()
                .unwrap_or_else(|| panic!("missing resource: {error:?}"));
        };
        match resource {
            Resource::Work(limit) if is_work => {
                assert_eq!(limit.limit(), work);
                assert_eq!(limit.actual(), measured.1);
            }
            Resource::Storage(limit) if !is_work => {
                assert_eq!(limit.limit(), storage);
                assert_eq!(limit.actual(), measured.3);
            }
            other => panic!("wrong resource: {other:?}"),
        }
    }
}

#[test]
fn original_memory_scalar_operator_types_retain_the_existing_primitive_contract() {
    let operations = [
        OperatorV30::And,
        OperatorV30::Or,
        OperatorV30::Xor,
        OperatorV30::Equal,
        OperatorV30::NotEqual,
        OperatorV30::Less,
        OperatorV30::LessEqual,
        OperatorV30::Greater,
        OperatorV30::GreaterEqual,
    ];
    for input in [ScalarV30::Bool].into_iter().chain(
        [8, 16, 32, 64]
            .into_iter()
            .flat_map(|width| [false, true].map(|signed| ScalarV30::Integer { width, signed })),
    ) {
        assert_eq!(Operator::Not.result(input).unwrap(), input);
        for (index, operation) in operations.into_iter().enumerate() {
            let operator = Operator::Binary(operation);
            assert_eq!(operator.code(), (index + 1) as u8);
            assert_eq!(
                operator.result(input).unwrap(),
                if index >= 3 { ScalarV30::Bool } else { input }
            );
        }
    }
    assert!(Operator::Not.result(ScalarV30::Unit).is_err());
    for operation in operations {
        assert!(Operator::Binary(operation).result(ScalarV30::Unit).is_err());
    }
    for operation in [
        SemanticBinaryOpV1::Add,
        SemanticBinaryOpV1::Divide,
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::Offset,
    ] {
        assert!(OperatorV30::from_source(operation).is_err());
    }
}

#[test]
fn original_memory_scalar_effects_use_each_operand_snapshot_before_the_write() {
    let text = include_str!("original_semantic_mir_source_scalar_operands_v48.vrs");
    let step = text
        .split_once("open spec fn invocation_source_scalar_operands_v48(")
        .unwrap()
        .1;
    let left = step
        .find("invocation_source_byte_evaluate_v36(source, event.left")
        .unwrap();
    let right = step
        .find("invocation_source_byte_evaluate_v36(left.source, operand")
        .unwrap();
    let write = step
        .find("invocation_source_scalar_operands_write_v48(right.source")
        .unwrap();
    assert!(left < right && right < write);
    assert!(step.contains("source, left.source"));
    assert!(step.contains("left.source, right.source"));
    assert!(text.contains("event.output_bits == 1 && access.width == 1"));
    assert!(text.contains("invocation_source_byte_value_typed_v36(left, event.input_bits)"));
    assert!(text.contains("invocation_source_byte_value_typed_v36(right, event.input_bits)"));
    for effects in [
        include_str!("original_semantic_mir_invocation_effects_v36.rs"),
        include_str!("original_semantic_mir_observed_effects_v39.vrs"),
    ] {
        assert!(effects.contains("Some(InvocationSourceByteEventV36::ScalarOperands(event))"));
        assert!(effects.contains("result.source == observation.after"));
    }
    assert!(!text.contains("assume("));
}

#[test]
fn original_memory_scalar_emission_has_independent_exact_resource_oracle() {
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let emit = |operation: Operation, work, storage| {
        let mut work = Work::new(work);
        let mut budget = Budget::new(&mut work, storage);
        let result = (|| {
            budget.reserve_storage(SOURCE_LIMIT + headers())?;
            let mut out = Writer::new(&mut budget)?;
            operation.emit(&mut out)?;
            out.finish()
        })();
        (result, budget.work(), budget.peak_storage())
    };
    for moved in [false, true] {
        for binary in [false, true] {
            let operation = Operation {
                destination: Destination::Local(19),
                left: Value::Read {
                    access: Access {
                        address: Address::Object {
                            local: 7,
                            offset: 4,
                        },
                        ty: TypeId::from_index(3),
                        bytes: 4,
                        alignment: 4,
                    },
                    moved,
                },
                right: binary.then_some(Value::Local {
                    local: 11,
                    moved: false,
                }),
                operator: if binary {
                    Operator::Binary(OperatorV30::Less)
                } else {
                    Operator::Not
                },
                input: ScalarV30::Integer {
                    width: 32,
                    signed: true,
                },
                output: if binary {
                    ScalarV30::Bool
                } else {
                    ScalarV30::Integer {
                        width: 32,
                        signed: true,
                    }
                },
            };
            let right = if binary {
                "Some(InvocationSourceByteValueV36::Local { local: 11int, moved: false })"
            } else {
                "None"
            };
            let code = if binary { 6 } else { 0 };
            let bits = if binary { 1 } else { 32 };
            let expected = format!(
                "InvocationSourceByteEventV36::ScalarOperands(InvocationSourceScalarOperandsV48 {{ destination: InvocationSourceByteDestinationV36::Local(19int), left: InvocationSourceByteValueV36::Read {{ access: InvocationSourceByteAccessV36 {{ base: InvocationSourceByteBaseV36::ObjectLocal(7int), offset: 4int, width: 4int, alignment: 4int }}, moved: {moved} }}, right: {right}, operation: {code}int, input_bits: 32int, input_signed: true, output_bits: {bits}int }})"
            );
            let work = expected.len() + 3 + usize::from(binary);
            let storage = SOURCE_LIMIT + headers();
            let (result, used, peak) = emit(operation, work, storage);
            assert_eq!(result.unwrap(), expected);
            assert_eq!((used, peak), (work, storage));
            assert!(matches!(emit(operation, work - 1, storage).0,
                Err(Error::Resource(Resource::Work(error)))
                    if error.limit() == work - 1 && error.actual() == work));
            assert!(matches!(emit(operation, work, storage - 1).0,
                Err(Error::Resource(Resource::Storage(error)))
                    if error.limit() == storage - 1 && error.actual() == storage));
        }
    }
}

#[test]
fn original_memory_scalar_headers_cover_complete_operation_and_query_frames() {
    type Fields = (
        Destination,
        Value,
        Option<Value>,
        Operator,
        ScalarV30,
        ScalarV30,
    );
    assert_eq!(size_of::<Operation>(), size_of::<Fields>());
    let expected = size_of::<Fields>()
        + 2 * size_of::<Result<Option<Fields>>>()
        + size_of::<Operator>()
        + size_of::<Value>()
        + size_of::<Option<Value>>()
        + 2 * size_of::<ScalarV30>()
        + size_of::<Destination>()
        + 6 * size_of::<&()>();
    assert_eq!(headers(), expected);
}

#[test]
fn original_memory_scalar_queries_preserve_moves_and_reject_mismatched_types() {
    super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        retained_scalar_transform,
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let body = SourceByteBody::derive(plan, slots, 0, 1, out)?;
                let context = body.context(out)?;
                let scalar = context.function.locals()[4].ty();
                let boolean = context.function.locals()[6].ty();
                let place = |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
                let query = |output, result, left_type, right_type, moved| {
                    Statement::Assign(SemanticAssignmentV1::new(
                        place(4, output),
                        SemanticRvalueV1::new(result, Rvalue::Binary {
                            operation: SemanticBinaryOpV1::BitXor,
                            left: if moved { Operand::Move(place(4, left_type)) }
                                else { Operand::Copy(place(4, left_type)) },
                            right: Operand::Copy(place(2, right_type)),
                        }),
                    ))
                };
                for moved in [false, true] {
                    let Event::ScalarOperands(operation) = context.statement(
                        &query(scalar, scalar, scalar, scalar, moved), out,
                    )? else { panic!("missing ordered scalar event") };
                    assert!(matches!(operation.left, Value::Read { moved: actual, .. } if actual == moved));
                    assert!(matches!(operation.destination, Destination::Memory(_)));
                }
                // Classifier queries against a genuine owner, not a mutated source program.
                for bad in [
                    query(scalar, boolean, scalar, scalar, false),
                    query(scalar, scalar, boolean, scalar, false),
                    query(scalar, scalar, scalar, boolean, false),
                    query(boolean, boolean, scalar, scalar, false),
                ] {
                    assert!(matches!(context.statement(&bad, out),
                        Err(Error::Statement("original MIR typed byte statement identity or layout differs"))));
                }
                Ok(())
            })
        },
    ).0.unwrap();
}
