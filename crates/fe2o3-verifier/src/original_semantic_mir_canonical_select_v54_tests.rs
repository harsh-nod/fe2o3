use super::*;

fn select_module(ty: Type, duplicate: bool) -> Module {
    let mut module = Module::new("canonical-select-three-operands");
    for ordinal in 0..2 {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(3), ty.clone()),
            OperationKind::Select {
                condition: ValueId(0),
                true_value: ValueId(1),
                false_value: ValueId(if duplicate { 1 } else { 2 }),
            },
        ));
        entry.terminator = Some(Terminator::Return {
            values: vec![ValueId(3)],
        });
        module.functions.push(Function::internal_helper(
            format!("select{ordinal}"),
            Signature::new(vec![Type::BOOL, ty.clone(), ty.clone()], vec![ty.clone()]),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![entry],
        ));
    }
    module
}

#[test]
fn canonical_byte_select_keeps_three_exact_inputs_and_admitted_scalar_payload_bits() {
    for ty in [
        Type::BOOL,
        Type::Scalar(ScalarType::I8),
        Type::Scalar(ScalarType::U8),
        Type::Scalar(ScalarType::I16),
        Type::Scalar(ScalarType::U16),
        Type::Scalar(ScalarType::I32),
        Type::Scalar(ScalarType::U32),
        Type::Scalar(ScalarType::I64),
        Type::Scalar(ScalarType::U64),
        Type::INDEX,
    ] {
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
            for duplicate in [false, true] {
                with_module(&select_module(ty.clone(), duplicate), |inventory, floor| {
                    let text = run(inventory, floor, LIMIT, LIMIT, |out| {
                        let scalar = CanonicalByteScalarV30::derive(inventory, 1, width, out)?;
                        assert_eq!(scalar.arguments, 3);
                        assert_eq!(scalar.destination, 7);
                        assert_eq!(
                            scalar.inputs.map(|(input, _)| input),
                            [4, 5, if duplicate { 5 } else { 6 }]
                        );
                        assert_eq!(scalar.inputs[0].1, ScalarV30::Bool);
                        assert_eq!(scalar.inputs[1].1, scalar_type(&ty, width)?);
                        assert_eq!(scalar.inputs[1].1, scalar.inputs[2].1);
                        assert_eq!(
                            scalar.nodes[3].expression,
                            ExpressionV30::Select {
                                condition: 0,
                                true_value: 1,
                                false_value: 2,
                            }
                        );
                        scalar.emit_definition(4, out)?;
                        let (before, after) = names();
                        scalar.emit_step(4, before, after, out)
                    })
                    .0
                    .unwrap();
                    assert!(text.contains("if m0 == 1int { m1 } else { m2 }"));
                    assert!(text.contains("ok0 && v0.len() == 8"));
                    assert!(
                        text.contains("match v0[4] { MemoryValueV30::Scalar(v) => 0 <= v < 2int")
                    );
                    assert!(text.contains("v0.update(7int, MemoryValueV30::Scalar"));
                    assert!(text.contains("else { v0 }"));
                    assert!(text.contains("let m1 = m0;\n let g1 = g0;\n let f1 = f0;"));
                    assert!(!text.contains("assume(") && !text.contains("external_body"));
                });
            }
        }
    }
}

#[test]
fn canonical_byte_select_exact_work_and_storage_replay_are_arity_scoped() {
    with_module(
        &select_module(Type::Scalar(ScalarType::U64), false),
        |inventory, floor| {
            for limit in [44, 43] {
                let result = run(inventory, floor, limit, LIMIT, |out| {
                    CanonicalByteScalarV30::derive(inventory, 1, FormalIndexWidth::Bits64, out)
                        .map(|_| ())
                });
                if limit == 44 {
                    result.0.unwrap();
                    assert_eq!(result.1, 44);
                } else {
                    assert!(
                        matches!(result.0, Err(Error::Resource(Resource::Work(e))) if e.actual() == 44 && e.limit() == 43)
                    );
                }
            }
            let body = |out: &mut Writer<'_, '_>| {
                let plan =
                    CanonicalByteScalarV30::derive(inventory, 1, FormalIndexWidth::Bits64, out)?;
                plan.emit_definition(4, out)?;
                let (before, after) = names();
                plan.emit_step(4, before, after, out)
            };
            let measured = run(inventory, floor, LIMIT, LIMIT, body);
            let text = measured.0.unwrap();
            let exact = run(inventory, floor, measured.1, measured.2, body);
            assert_eq!(exact.0.unwrap(), text);
            assert_eq!((exact.1, exact.2), (measured.1, measured.2));
            assert!(matches!(
                run(inventory, floor, measured.1 - 1, measured.2, body).0,
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                run(inventory, floor, measured.1, measured.2 - 1, body).0,
                Err(Error::Resource(Resource::Storage(_)))
            ));
        },
    );
}

fn select_nodes() -> [NodeV30; 4] {
    let scalar = ScalarV30::Integer {
        width: 32,
        signed: false,
    };
    [
        NodeV30 {
            scalar: ScalarV30::Bool,
            expression: ExpressionV30::Argument(0),
        },
        NodeV30 {
            scalar,
            expression: ExpressionV30::Argument(1),
        },
        NodeV30 {
            scalar,
            expression: ExpressionV30::Argument(2),
        },
        NodeV30 {
            scalar,
            expression: ExpressionV30::Select {
                condition: 0,
                true_value: 1,
                false_value: 2,
            },
        },
    ]
}

#[test]
fn canonical_byte_select_whole_program_and_block_collectors_keep_all_three_edges() {
    for duplicate in [false, true] {
        with_module(
            &select_module(Type::Scalar(ScalarType::U32), duplicate),
            |inventory, floor| {
                run(inventory, floor, LIMIT, LIMIT, |out| {
                    let program = canonical::CanonicalProgramV30::derive(inventory, 1, out)?;
                    assert_eq!(program.nodes.len(), 4);
                    assert_eq!(program.returned, Some(3));
                    assert_eq!(
                        program.nodes[3].expression,
                        ExpressionV30::Select {
                            condition: 0,
                            true_value: 1,
                            false_value: if duplicate { 1 } else { 2 },
                        }
                    );
                    let block = canonical::control::TargetBlock::derive(inventory, 1, out)?;
                    let mut trace = super::super::super::target_trace::ConcreteTrace::arguments(
                        inventory, 1, 8, out,
                    )?;
                    trace.append(&block, out)?;
                    assert_eq!(trace.nodes.len(), 4);
                    assert_eq!(trace.nodes[3].expression, program.nodes[3].expression);
                    let result = inventory.functions()[1].definitions.end - 1;
                    assert_eq!(trace.value(result, out)?, 3);
                    Ok(())
                })
                .0
                .unwrap();
            },
        );
    }
}

#[test]
fn canonical_byte_select_whole_program_and_block_collectors_have_exact_limits() {
    with_module(
        &select_module(Type::Scalar(ScalarType::I32), false),
        |inventory, floor| {
            let body = |out: &mut Writer<'_, '_>| {
                let program = canonical::CanonicalProgramV30::derive(inventory, 0, out)?;
                let block = canonical::control::TargetBlock::derive(inventory, 0, out)?;
                let mut trace = super::super::super::target_trace::ConcreteTrace::arguments(
                    inventory, 0, 8, out,
                )?;
                trace.append(&block, out)?;
                assert_eq!(trace.nodes.last(), program.nodes.last());
                Ok(())
            };
            let measured = run(inventory, floor, LIMIT, LIMIT, body);
            measured.0.unwrap();
            let exact = run(inventory, floor, measured.1, measured.2, body);
            exact.0.unwrap();
            assert_eq!((exact.1, exact.2), (measured.1, measured.2));
            assert!(matches!(
                run(inventory, floor, measured.1 - 1, measured.2, body).0,
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                run(inventory, floor, measured.1, measured.2 - 1, body).0,
                Err(Error::Resource(Resource::Storage(_)))
            ));
        },
    );
}

#[test]
fn canonical_byte_select_parser_refuses_wrong_arity_condition_payload_and_absent_edges() {
    let nodes = select_nodes();
    let kind = OperationKind::Select {
        condition: ValueId(0),
        true_value: ValueId(1),
        false_value: ValueId(2),
    };
    assert_eq!(
        canonical::operation_expression(&kind, &[0, 1, 2], nodes[3].scalar, &nodes).unwrap(),
        nodes[3].expression
    );
    for inputs in [
        &[0, 1][..],
        &[0, 1, 2, 3][..],
        &[1, 0, 2][..],
        &[0, 1, 99][..],
    ] {
        assert!(canonical::operation_expression(&kind, inputs, nodes[3].scalar, &nodes).is_err());
    }
    for index in 0..3 {
        let mut wrong = nodes;
        wrong[index].scalar = ScalarV30::Integer {
            width: 32,
            signed: true,
        };
        assert!(
            canonical::operation_expression(&kind, &[0, 1, 2], nodes[3].scalar, &wrong).is_err()
        );
    }
    assert!(canonical::operation_expression(&kind, &[0, 1, 2], ScalarV30::Unit, &nodes).is_err());
}

#[test]
fn canonical_byte_select_graph_emission_refuses_forward_or_ill_typed_edges() {
    with_module(
        &select_module(Type::Scalar(ScalarType::U32), false),
        |inventory, floor| {
            for mode in 0..5 {
                run(inventory, floor, LIMIT, LIMIT, |out| {
                    let mut nodes = select_nodes();
                    match mode {
                        0 => {
                            nodes[3].expression = ExpressionV30::Select {
                                condition: 3,
                                true_value: 1,
                                false_value: 2,
                            }
                        }
                        1 => {
                            nodes[3].expression = ExpressionV30::Select {
                                condition: 0,
                                true_value: 4,
                                false_value: 2,
                            }
                        }
                        2 => {
                            nodes[3].expression = ExpressionV30::Select {
                                condition: 0,
                                true_value: 1,
                                false_value: 3,
                            }
                        }
                        3 => nodes[0].scalar = nodes[1].scalar,
                        _ => nodes[2].scalar = ScalarV30::Bool,
                    }
                    let result = emit_graph_v30(
                        &nodes,
                        3,
                        0,
                        "bad_select",
                        std::iter::once(Ok(Some(3))),
                        None,
                        out,
                    );
                    assert!(matches!(result, Err(Error::Statement(_))));
                    Ok(())
                })
                .0
                .unwrap();
            }
        },
    );
}

#[test]
fn canonical_byte_select_congruence_keeps_condition_and_arm_order_distinct() {
    with_module(
        &select_module(Type::Scalar(ScalarType::U32), false),
        |inventory, floor| {
            run(inventory, floor, LIMIT, LIMIT, |out| {
                let original = select_nodes();
                for mode in 0..4 {
                    let mut other = original;
                    match mode {
                        0 => (),
                        1 => {
                            other[3].expression = ExpressionV30::Select {
                                condition: 0,
                                true_value: 2,
                                false_value: 1,
                            }
                        }
                        2 => other[0].expression = ExpressionV30::Argument(3),
                        _ => {
                            other[3].expression = ExpressionV30::Select {
                                condition: 0,
                                true_value: 1,
                                false_value: 1,
                            }
                        }
                    }
                    let classes = super::super::super::relation::classes([&original, &other], out)?;
                    assert_eq!(classes[0][3] == classes[1][3], mode == 0);
                }
                let mut forward = original;
                forward[3].expression = ExpressionV30::Select {
                    condition: 0,
                    true_value: 1,
                    false_value: 4,
                };
                assert!(
                    super::super::super::relation::classes([&original, &forward], out).is_err()
                );
                Ok(())
            })
            .0
            .unwrap();
        },
    );
}
