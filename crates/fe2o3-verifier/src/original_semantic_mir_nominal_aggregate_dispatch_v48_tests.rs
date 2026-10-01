// Reuse the genuine original intrinsic/loan fixture, adding a nominal Move
// before borrowing its new local. The equal-layout type has no issuing intrinsic.
fn nominal_move_fixture_v48(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
) {
    fixture(types, functions, callables, 2);
    let helper = functions.last_mut().unwrap();
    let witness = helper.locals()[4].ty();
    let reference = helper.locals()[5].ty();
    let original = &types[witness.index() as usize];
    assert!(matches!(original.shape(), Shape::Aggregate(_)));
    let plain = TypeId::from_index(types.len() as u32);
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([181; 32]),
        SemanticLayoutIdentityV1::from_sha256([181; 32]),
        original.layout().clone(),
        original.shape().clone(),
    ));
    let mut locals = helper.locals().to_vec();
    let moved = locals.len() as u32;
    for (ordinal, ty) in [witness, plain].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([182 + ordinal as u8; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            helper.source(),
        ));
    }
    let place = |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |destination: Place, value| {
        SemanticStatementV1::new(
            helper.source(),
            Statement::Assign(SemanticAssignmentV1::new(
                destination.clone(),
                SemanticRvalueV1::new(destination.ty(), value),
            )),
        )
    };
    let mut blocks = helper.blocks().to_vec();
    let old = &blocks[1];
    let mut statements = old.statements().to_vec();
    assert!(matches!(statements[0].kind(), Statement::Assign(assignment)
        if matches!(assignment.value().kind(), Rvalue::Borrow { kind: BorrowKind::Shared, place }
            if place.local().index() == 4 && place.ty() == witness)));
    statements[0] = assign(
        place(5, reference),
        Rvalue::Borrow {
            kind: BorrowKind::Shared,
            place: place(moved, witness),
        },
    );
    statements.insert(
        0,
        assign(
            place(moved, witness),
            Rvalue::Use(Operand::Move(place(4, witness))),
        ),
    );
    blocks[1] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        statements,
        old.terminator().clone(),
    )
    .unwrap();
    *helper = Function::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        helper.source(),
        helper.abi().clone(),
        locals,
        helper.entry(),
        blocks,
    )
    .unwrap();
}

fn nominal_move_run_v48(
    work: usize,
    storage: usize,
    negatives: bool,
) -> (Result<()>, usize, usize, usize) {
    let reached = std::cell::Cell::new(false);
    let result = super::super::super::super::invocations::tests::run_callable_transform(
        work,
        storage,
        nominal_move_fixture_v48,
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
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
                for root in 0..2 {
                    for instance in 1..=2 {
                        let body = SourceByteBody::derive(plan, slots, root, instance, out)?;
                        let context = body.context(out)?;
                        let witness = context.function.locals()[4].ty();
                        let destination = context.function.locals().len() as u32 - 2;
                        let plain_local = destination + 1;
                        let plain = context.function.locals()[plain_local as usize].ty();
                        assert!(matches!(
                            context.types[witness.index() as usize].shape(),
                            Shape::Aggregate(_)
                        ));
                        assert!(slots.witness_class(witness, out)?.is_some());
                        assert!(slots.witness_class(plain, out)?.is_none());
                        assert_eq!(slots.aggregate_leaf_count(witness, out)?, None);
                        assert_eq!(slots.aggregate_leaf_count(plain, out)?, Some(2));
                        assert!(!slots.has_original_object(root, instance, 4, out)?);
                        assert!(!slots.has_original_object(root, instance, destination, out)?);
                        let event = format!(
                            "InvocationSourceByteEventV36::WitnessTransfer {{ destination: {}, input: {}, source_type: {}, reference: false, moved: true }}",
                            context.local(destination)?,
                            context.local(4)?,
                            witness.index(),
                        );
                        assert_eq!(out.text.matches(&event).count(), 1);
                        let original = context.function.blocks()[1].statements()[0].kind();
                        assert!(matches!(
                            context.statement(original, out)?,
                            Event::WitnessTransfer(_)
                        ));
                        if negatives {
                            let place = |local, ty| {
                                Place::new(SemanticLocalIdV1::from_index(local), vec![], ty)
                                    .unwrap()
                            };
                            let statement = |destination: Place, input| {
                                Statement::Assign(SemanticAssignmentV1::new(
                                    destination.clone(),
                                    SemanticRvalueV1::new(destination.ty(), Rvalue::Use(input)),
                                ))
                            };
                            // These are classifier queries against an authenticated owner,
                            // not execution of an uninitialized lookalike value.
                            let lookalike = statement(
                                place(plain_local, plain),
                                Operand::Move(place(plain_local, plain)),
                            );
                            assert!(matches!(
                                context.statement(&lookalike, out)?,
                                Event::AggregateTransfer(_)
                            ));
                            for (bad, expected) in [
                                (
                                    statement(
                                        place(destination, witness),
                                        Operand::Copy(place(4, witness)),
                                    ),
                                    "original MIR typed byte statement is not modeled",
                                ),
                                (
                                    statement(
                                        place(destination, witness),
                                        Operand::Move(place(plain_local, plain)),
                                    ),
                                    "original MIR typed byte statement identity or layout differs",
                                ),
                                (
                                    statement(
                                        place(destination, witness),
                                        Operand::Move(place(plain_local, witness)),
                                    ),
                                    "original MIR typed byte statement identity or layout differs",
                                ),
                            ] {
                                assert!(
                                    matches!(context.statement(&bad, out), Err(Error::Statement(actual)) if actual == expected)
                                );
                            }
                        }
                    }
                }
                reached.set(true);
                Ok(())
            })
        },
    );
    if result.0.is_ok() {
        assert!(reached.get());
    }
    result
}

#[test]
fn original_aggregate_shaped_witness_moves_reach_nominal_source_program_on_both_roots() {
    nominal_move_run_v48(LIMIT, LIMIT, false).0.unwrap();
}

#[test]
fn nominal_move_dispatch_rejects_owned_copy_wrong_type_and_equal_layout_authority() {
    nominal_move_run_v48(LIMIT, LIMIT, true).0.unwrap();
}

#[test]
fn nominal_aggregate_dispatch_has_exact_and_one_short_complete_resources() {
    let generous = nominal_move_run_v48(LIMIT, LIMIT, false);
    generous.0.unwrap();
    let exact = nominal_move_run_v48(generous.1, generous.3, false);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (generous.1, generous.2, generous.3)
    );
    for (work, storage, is_work) in [
        (generous.1 - 1, generous.3, true),
        (generous.1, generous.3 - 1, false),
    ] {
        let error = nominal_move_run_v48(work, storage, false).0.unwrap_err();
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
                assert_eq!(limit.actual(), generous.1);
            }
            Resource::Storage(limit) if !is_work => {
                assert_eq!(limit.limit(), storage);
                assert_eq!(limit.actual(), generous.3);
            }
            other => panic!("wrong resource: {other:?}"),
        }
    }
}

#[test]
fn nominal_dispatch_preserves_genuine_structural_aggregate_copy_and_move_fallback() {
    for moved in [false, true] {
        super::super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| {
                super::super::super::paired::aggregate_tests::checked_transform(
                    types,
                    functions,
                    SemanticCheckedBinaryOpV1::Add,
                    false,
                    false,
                );
                let helper = functions.last_mut().unwrap();
                let mut blocks = helper.blocks().to_vec();
                let block = &blocks[0];
                let mut statements = block.statements().to_vec();
                let ty = helper.locals()[4].ty();
                let place = Place::new(SemanticLocalIdV1::from_index(4), vec![], ty).unwrap();
                statements.push(SemanticStatementV1::new(
                    helper.source(),
                    Statement::Assign(SemanticAssignmentV1::new(
                        place.clone(),
                        SemanticRvalueV1::new(
                            ty,
                            Rvalue::Use(if moved {
                                Operand::Move(place)
                            } else {
                                Operand::Copy(place)
                            }),
                        ),
                    )),
                ));
                blocks[0] = SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    statements,
                    block.terminator().clone(),
                )
                .unwrap();
                *helper = Function::new(
                    helper.identity(),
                    helper.role(),
                    helper.item_definition_identity(),
                    helper.monomorphization_identity(),
                    helper.generic_type_arguments_identity(),
                    helper.const_generic_arguments_identity(),
                    helper.source(),
                    helper.abi().clone(),
                    helper.locals().to_vec(),
                    helper.entry(),
                    blocks,
                )
                .unwrap();
            },
            |plan, out| {
                super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                    let mut program =
                        super::super::super::source_function::SourceByteProgram::derive(
                            plan, slots, out,
                        )?;
                    let paired = super::super::super::paired::PairedInvocations::derive(
                        plan,
                        &program,
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                        out,
                    )?;
                    program.emit(out)?;
                    paired.emit(out)?;
                    assert_eq!(
                        out.text
                            .matches("InvocationSourceByteEventV36::AggregateTransfer {")
                            .count(),
                        4
                    );
                    assert!(
                        !out.text
                            .contains("InvocationSourceByteEventV36::WitnessTransfer {")
                    );
                    for root in 0..2 {
                        for instance in 1..=2 {
                            let body = SourceByteBody::derive(plan, slots, root, instance, out)?;
                            let context = body.context(out)?;
                            let original = context.function.blocks()[0]
                                .statements()
                                .last()
                                .unwrap()
                                .kind();
                            assert!(matches!(
                                context.statement(original, out)?,
                                Event::AggregateTransfer(_)
                            ));
                        }
                    }
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}
