use super::super::descriptor_loans::{DescriptorEvent, Recipe};
use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceReferenceCarrierV38;
use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;

fn loans_fixture(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
) {
    fixture(types, functions, callables);
    for root in 0..2 {
        let old = &functions[root];
        let source = old.source();
        let reference = old.locals()[6].ty();
        let descriptor = old.locals()[4].ty();
        let mut locals = old.locals().to_vec();
        for identity in [246, 247] {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([identity; 32]),
                reference,
                SemanticLocalRoleV1::Temporary,
                source,
            ));
        }
        let place =
            |local| Place::new(SemanticLocalIdV1::from_index(local), vec![], reference).unwrap();
        let mut blocks = old.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        assert_eq!(statements.len(), 3);
        statements.push(SemanticStatementV1::new(
            source,
            Statement::Assign(SemanticAssignmentV1::new(
                place(9),
                SemanticRvalueV1::new(reference, Rvalue::Use(Operand::Copy(place(6)))),
            )),
        ));
        statements.push(SemanticStatementV1::new(
            source,
            Statement::Assign(SemanticAssignmentV1::new(
                place(10),
                SemanticRvalueV1::new(
                    reference,
                    Rvalue::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: Place::new(
                            SemanticLocalIdV1::from_index(9),
                            vec![
                                SemanticProjectionV1::new(Projection::Dereference, descriptor)
                                    .unwrap(),
                            ],
                            descriptor,
                        )
                        .unwrap(),
                    },
                ),
            )),
        ));
        // A Move preserves the newly issued child loan, not only its Slice bits.
        statements.push(SemanticStatementV1::new(
            source,
            Statement::Assign(SemanticAssignmentV1::new(
                place(9),
                SemanticRvalueV1::new(reference, Rvalue::Use(Operand::Move(place(10)))),
            )),
        ));
        let Terminator::Call(call) = blocks[0].terminator().kind() else {
            panic!("reachable length call");
        };
        let end = SemanticDirectCallV1::new_callable(
            call.callee(),
            vec![Operand::Copy(place(9))],
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            source,
            statements,
            SemanticTerminatorV1::new(source, Terminator::Call(end)),
        )
        .unwrap();
        functions[root] = Function::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            source,
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    }
}

fn run_loans(work: usize, storage: usize, hostile: bool) -> (Result<()>, usize, usize, usize) {
    let reached = std::cell::Cell::new(0);
    let result = super::super::super::super::invocations::tests::run_captured_callable_transform(
        work,
        storage,
        loans_fixture,
        capture,
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                for root in 0..2 {
                    let body = SourceByteBody::derive(plan, slots, root, 0, out)?;
                    let context = body.context(out)?;
                    let mut first = None;
                    for (statement, local, mutable) in
                        [(2, 6, false), (3, 9, false), (4, 10, false), (5, 9, false)]
                    {
                        let original = witness_events::original_value(
                            slots,
                            plan.instance(root, 0, out)?.function,
                            0,
                            Some(statement),
                            Role::Destination,
                            local,
                            true,
                            out,
                        )?;
                        let endpoint = slots
                            .correspondence(out)?
                            .ssa_typed_endpoint_v36(root, 0, original, out.budget)?;
                        let reference = endpoint
                            .reference(out.budget)?
                            .expect("nominal descriptor loan");
                        assert_eq!(
                            reference.carrier(out.budget)?,
                            ProductionSourceReferenceCarrierV38::DescriptorSlice
                        );
                        assert_eq!(
                            reference.borrow_kind(out.budget)?,
                            if mutable {
                                SemanticBorrowKindV1::Mutable
                            } else {
                                SemanticBorrowKindV1::Shared
                            }
                        );
                        assert!(matches!(
                            endpoint.physical_type(out.budget)?,
                            Some(fe2o3_kernel_ir::Type::Slice(_))
                        ));
                        assert_eq!(reference.origin_local(out.budget)?.index(), 4);
                        assert_eq!(
                            reference.origin_type(out.budget)?,
                            context.function.locals()[4].ty()
                        );
                        let recipe = Recipe::derive(slots, plan, root, &endpoint, out)?;
                        assert_eq!(recipe.origin, body.locals.start + 4);
                        assert_eq!(recipe.width, 4);
                        assert_eq!(recipe.metadata_bits, 64);
                        if statement == 2 {
                            first = Some(recipe);
                        }
                    }
                    let first = first.expect("direct borrow present");
                    assert!(matches!(
                        body.event_at(0, 2, out)?,
                        Event::Descriptor(DescriptorEvent::Borrow { parent: None, .. })
                    ));
                    assert!(matches!(
                        body.event_at(0, 3, out)?,
                        Event::Descriptor(DescriptorEvent::Transfer { moved: false, .. })
                    ));
                    assert!(
                        matches!(body.event_at(0, 4, out)?, Event::Descriptor(DescriptorEvent::Borrow { parent: Some((_, parent)), .. }) if parent == first)
                    );
                    assert!(matches!(
                        body.event_at(0, 5, out)?,
                        Event::Descriptor(DescriptorEvent::Transfer { moved: true, .. })
                    ));
                    if hostile {
                        let Statement::Assign(assignment) =
                            context.function.blocks()[0].statements()[2].kind()
                        else {
                            panic!();
                        };
                        // An authentic assignment must not bind another original occurrence.
                        assert!(
                            descriptor_loans::derive(
                                &context,
                                plan,
                                plan.instance(root, 0, out)?.function,
                                0,
                                3,
                                assignment,
                                out
                            )
                            .is_err()
                        );
                        let unrelated = context.function.locals()[7].ty();
                        assert!(slots.descriptor_slice_class(unrelated, out)?.is_none());
                    }
                    reached.set(reached.get() + 1);
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
                write!(out, "{SOURCE_BYTES_V36}").map_err(|_| out.error())?;
                program.emit(out)?;
                paired.emit(out)?;
                assert!(
                    out.text
                        .contains("invocation_source_descriptor_length_v51(cursor.source")
                );
                assert!(
                    out.text
                        .contains("invocation_source_descriptor_reference_current_v51(source")
                );
                Ok(())
            })
        },
    );
    if result.0.is_ok() {
        assert_eq!(reached.get(), 2);
    }
    result
}

#[test]
fn original_descriptor_loans_preserve_slice_identity_through_copy_move_reborrow_and_length() {
    run_loans(LIMIT, LIMIT, false).0.unwrap();
}

#[test]
fn original_descriptor_loans_refuse_wrong_occurrences_and_equal_layout_nonnominal_types() {
    run_loans(LIMIT, LIMIT, true).0.unwrap();
}

#[test]
fn original_descriptor_loan_complete_route_has_exact_and_one_short_resources() {
    let measured = run_loans(LIMIT, LIMIT, false);
    measured.0.unwrap();
    let exact = run_loans(measured.1, measured.3, false);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for work in [true, false] {
        let w = measured.1 - usize::from(work);
        let s = measured.3 - usize::from(!work);
        let failure = run_loans(w, s, false);
        assert!(
            matches!((&failure.0, work),
            (Err(Error::Source(SourceError::Resource(Resource::Work(error)))), true)
                if error.actual() == measured.1 && error.limit() == w)
                || matches!((&failure.0, work),
            (Err(Error::Source(SourceError::Resource(Resource::Storage(error)))), false)
                if error.actual() == measured.3 && error.limit() == s),
            "{:?}",
            failure.0
        );
    }
}

fn mutable_fixture(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
) {
    fixture(types, functions, callables);
    let descriptor = functions[0].locals()[4].ty();
    let shared = functions[0].locals()[6].ty();
    let mutable = TypeId::from_index(types.len() as u32);
    types.push(
        Type::new(
            SemanticTypeIdentityV1::from_sha256([206; 32]),
            SemanticLayoutIdentityV1::from_sha256([206; 32]),
            types[shared.index() as usize].layout().clone(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    descriptor,
                    PointerKind::Reference,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    PointerMetadata::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                        16,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    for root in 0..2 {
        let old = &functions[root];
        let source = old.source();
        let mut locals = old.locals().to_vec();
        for identity in [246, 247, 248] {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([identity; 32]),
                mutable,
                SemanticLocalRoleV1::Temporary,
                source,
            ));
        }
        let place =
            |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
        let deref = |local| {
            Place::new(
                SemanticLocalIdV1::from_index(local),
                vec![SemanticProjectionV1::new(Projection::Dereference, descriptor).unwrap()],
                descriptor,
            )
            .unwrap()
        };
        let mut blocks = old.blocks().to_vec();
        let mut statements = blocks[0].statements()[..2].to_vec();
        for (local, ty, kind, referent) in [
            (
                9,
                mutable,
                SemanticBorrowKindV1::Mutable,
                place(4, descriptor),
            ),
            (10, mutable, SemanticBorrowKindV1::Mutable, deref(9)),
        ] {
            statements.push(SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(
                    place(local, ty),
                    SemanticRvalueV1::new(
                        ty,
                        Rvalue::Borrow {
                            kind,
                            place: referent,
                        },
                    ),
                )),
            ));
        }
        statements.push(SemanticStatementV1::new(
            source,
            Statement::Assign(SemanticAssignmentV1::new(
                place(11, mutable),
                SemanticRvalueV1::new(mutable, Rvalue::Use(Operand::Move(place(10, mutable)))),
            )),
        ));
        statements.push(SemanticStatementV1::new(
            source,
            Statement::Assign(SemanticAssignmentV1::new(
                place(6, shared),
                SemanticRvalueV1::new(
                    shared,
                    Rvalue::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: deref(11),
                    },
                ),
            )),
        ));
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            source,
            statements,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        functions[root] = Function::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            source,
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    }
}

#[test]
fn original_mutable_descriptor_loans_move_and_reborrow_without_shared_copy_authority() {
    let reached = std::cell::Cell::new(0);
    let result = super::super::super::super::invocations::tests::run_captured_callable_transform(
        LIMIT,
        LIMIT,
        mutable_fixture,
        capture,
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                for root in 0..2 {
                    let body = SourceByteBody::derive(plan, slots, root, 0, out)?;
                    assert!(matches!(
                        body.event_at(0, 2, out)?,
                        Event::Descriptor(DescriptorEvent::Borrow {
                            recipe: Recipe { mutable: true, .. },
                            parent: None,
                            ..
                        })
                    ));
                    assert!(matches!(
                        body.event_at(0, 3, out)?,
                        Event::Descriptor(DescriptorEvent::Borrow {
                            recipe: Recipe { mutable: true, .. },
                            parent: Some((_, Recipe { mutable: true, .. })),
                            ..
                        })
                    ));
                    assert!(matches!(
                        body.event_at(0, 4, out)?,
                        Event::Descriptor(DescriptorEvent::Transfer {
                            recipe: Recipe { mutable: true, .. },
                            moved: true,
                            ..
                        })
                    ));
                    assert!(matches!(
                        body.event_at(0, 5, out)?,
                        Event::Descriptor(DescriptorEvent::Borrow {
                            recipe: Recipe { mutable: false, .. },
                            parent: Some((_, Recipe { mutable: true, .. })),
                            ..
                        })
                    ));
                    let context = body.context(out)?;
                    let Statement::Assign(original) =
                        context.function.blocks()[0].statements()[4].kind()
                    else {
                        panic!();
                    };
                    let Rvalue::Use(Operand::Move(input)) = original.value().kind() else {
                        panic!();
                    };
                    let copy = SemanticAssignmentV1::new(
                        original.destination().clone(),
                        SemanticRvalueV1::new(
                            original.value().result_type(),
                            Rvalue::Use(Operand::Copy(input.clone())),
                        ),
                    );
                    assert!(matches!(
                        descriptor_loans::derive(
                            &context,
                            plan,
                            plan.instance(root, 0, out)?.function,
                            0,
                            4,
                            &copy,
                            out
                        ),
                        Err(Error::Statement(
                            "original MIR typed byte statement identity or layout differs"
                        ))
                    ));
                    reached.set(reached.get() + 1);
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
                write!(out, "{SOURCE_BYTES_V36}").map_err(|_| out.error())?;
                program.emit(out)?;
                paired.emit(out)
            })
        },
    );
    result.0.unwrap();
    assert_eq!(reached.get(), 2);
}

#[test]
fn original_descriptor_loan_runtime_keeps_nominal_version_frame_and_move_boundaries() {
    let text = include_str!("original_semantic_mir_source_descriptor_loans_v51.vrs");
    for required in [
        "source.logical.versions[recipe.origin] == reference.loan.version",
        "source.machine.frames.active[i] == reference.loan.frame",
        "reference.reference_type == recipe.reference_type",
        "recipe.mutable && !moved",
        "recipe.mutable && !expected.mutable",
        "destination == recipe.origin",
        "..source.logical.descriptor_references[local].loan",
    ] {
        assert!(text.contains(required), "{required}");
    }
    let transfer = text
        .split(
            "InvocationSourceDescriptorEventV51::Transfer { destination, input, recipe, moved } =>",
        )
        .nth(1)
        .unwrap();
    assert!(
        transfer
            .find("let reference = source.logical.descriptor_references[input]")
            .unwrap()
            < transfer.find("let consumed = if moved").unwrap()
    );
    assert!(SOURCE_BYTES_V36.contains(text));
    assert!(!text.contains("byte_allocate_v30"));
    assert!(!text.contains("source.logical.witnesses.insert"));
}

#[test]
fn original_descriptor_class_and_loan_queries_retain_exact_one_short_failures() {
    for class_query in [false, true] {
        let ready = std::cell::Cell::new(false);
        let attacked = std::cell::Cell::new(false);
        let result =
            super::super::super::super::invocations::tests::run_captured_callable_transform(
                LIMIT,
                LIMIT,
                loans_fixture,
                capture,
                |plan, out| {
                    super::super::super::source_function::tests::with_slots(
                        plan,
                        out,
                        |slots, out| {
                            let body = SourceByteBody::derive(plan, slots, 0, 0, out)?;
                            let context = body.context(out)?;
                            let ty = context.function.locals()[4].ty();
                            let original = witness_events::original_value(
                                slots,
                                plan.instance(0, 0, out)?.function,
                                0,
                                Some(2),
                                Role::Destination,
                                6,
                                true,
                                out,
                            )?;
                            let endpoint = slots
                                .correspondence(out)?
                                .ssa_typed_endpoint_v36(0, 0, original, out.budget)?;
                            let loan = endpoint
                                .reference(out.budget)?
                                .expect("positive descriptor loan");
                            let query = |out: &mut Writer<'_, '_>| -> Result<()> {
                                if class_query {
                                    assert_eq!(
                                        slots
                                            .descriptor_slice_class(ty, out)?
                                            .expect("nominal root class")
                                            .metadata_bits,
                                        64
                                    );
                                } else {
                                    assert_eq!(
                                        loan.borrow_kind(out.budget)?,
                                        SemanticBorrowKindV1::Shared
                                    );
                                }
                                Ok(())
                            };
                            // Owner query costs one, class additionally checks three fixed fields;
                            // the loan getter reads its exact retained kind with one further debit.
                            let cost = if class_query { 1 + 2 + 1 } else { 1 + 1 };
                            let before = (
                                out.budget.work(),
                                out.budget.storage(),
                                out.budget.peak_storage(),
                            );
                            query(out)?;
                            assert_eq!(out.budget.work() - before.0, cost);
                            assert_eq!(
                                (out.budget.storage(), out.budget.peak_storage()),
                                (before.1, before.2)
                            );
                            ready.set(true);
                            out.budget
                                .charge_work(LIMIT - out.budget.work() - (cost - 1))?;
                            let failure = query(out).expect_err("one-short exact query");
                            let Error::Source(SourceError::Resource(resource)) = failure else {
                                panic!("exact retained resource: {failure:?}");
                            };
                            assert!(
                                matches!(resource, Resource::Work(error) if error.actual() == LIMIT + 1 && error.limit() == LIMIT)
                            );
                            let after = (
                                out.budget.work(),
                                out.budget.storage(),
                                out.budget.peak_storage(),
                            );
                            assert!(
                                matches!(query(out), Err(Error::Source(SourceError::Resource(repeated))) if repeated == resource)
                            );
                            assert_eq!(
                                (
                                    out.budget.work(),
                                    out.budget.storage(),
                                    out.budget.peak_storage()
                                ),
                                after
                            );
                            attacked.set(true);
                            Ok(())
                        },
                    )
                },
            );
        assert!(ready.get() && attacked.get());
        assert!(
            matches!(result.0, Err(Error::Source(SourceError::Resource(Resource::Work(error)))) if error.actual() == LIMIT + 1 && error.limit() == LIMIT)
        );
    }
}
