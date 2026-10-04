use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EndPayload {
    OverwriteNone,
    MoveField,
    DeadEnum,
}

fn lifetime_fixture(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    mode: EndPayload,
) {
    logical_fixture(types, functions, Fixture::SharedSome);
    let helper = functions.last_mut().unwrap();
    let old = &helper.blocks()[0];
    let source = old.source();
    let enumeration = helper.locals()[4].ty();
    let word = helper.locals()[8].ty();
    let reference = helper.locals()[9].ty();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let dead = |local| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local)),
        )
    };
    let constructor = |statement: &SemanticStatementV1, variant| {
        matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if assignment.destination().local().index() == 4
                && matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(value)
                    if value.kind() == &SemanticAggregateKindV1::EnumVariant(variant)))
    };
    let some = old
        .statements()
        .iter()
        .position(|s| constructor(s, 1))
        .unwrap();
    let none = old
        .statements()
        .iter()
        .find(|s| constructor(s, 0))
        .unwrap()
        .clone();
    let mut some_statements = vec![old.statements()[some].clone()];
    if mode == EndPayload::MoveField {
        let field = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), enumeration)
                    .unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), reference).unwrap(),
            ],
            reference,
        )
        .unwrap();
        some_statements.push(assign(
            9,
            reference,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field)),
        ));
    }
    let mut joined: Vec<_> = old.statements()[some + 1..].iter().filter(|statement| {
        matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if assignment.destination().local().index() == 0 || assignment.destination().local().index() == 5)
    }).cloned().collect();
    assert_eq!(
        joined.len(),
        2,
        "retain exact original discriminant and return cast"
    );
    if mode == EndPayload::OverwriteNone {
        joined.push(none.clone());
    }
    joined.extend([dead(9), dead(10), dead(7)]);
    if mode == EndPayload::DeadEnum {
        joined.push(dead(4));
    }
    joined.extend([
        dead(8),
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(8)),
        ),
        assign(
            8,
            word,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, word))),
        ),
        dead(8),
    ]);
    if mode != EndPayload::DeadEnum {
        joined.push(dead(4));
    }
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let block = |identity, statements, terminator| {
        SemanticBasicBlockV1::new(
            identity,
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let blocks = vec![
        block(
            old.identity(),
            old.statements()[..some].to_vec(),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(2, word)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([243; 32]),
            some_statements,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([244; 32]),
            vec![none],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([245; 32]),
            joined,
            old.terminator().kind().clone(),
        ),
    ];
    assert!(
        blocks
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
    *helper = SemanticFunctionDeclV1::new(
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
}

#[test]
fn original_inactive_enum_spills_preserve_referent_storage_end_and_restart_cuts() {
    for mode in [
        EndPayload::OverwriteNone,
        EndPayload::MoveField,
        EndPayload::DeadEnum,
    ] {
        let completed = std::cell::Cell::new(false);
        let result = super::super::super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| lifetime_fixture(types, functions, mode),
            |plan, out| {
                super::super::super::super::source_function::tests::with_slots(
                    plan,
                    out,
                    |slots, out| {
                        let relation = slots.correspondence(out)?;
                        let mut lifetimes = [[0usize; 2]; 2];
                        for root in 0..2 {
                            assert_eq!(
                                relation.enum_spill_count_v48(root, out.budget)?,
                                2,
                                "each helper's dynamic pointer field needs an authenticated spill"
                            );
                            for ordinal in 0..2 {
                                let receipt = relation.enum_spill_v48(root, ordinal, out.budget)?;
                                let (definition, operation) = receipt.allocation(out.budget)?;
                                let inventory = relation.inventory(out.budget)?;
                                let allocation = &inventory.operations()[operation];
                                let fe2o3_kernel_ir::OperationKind::Alloca {
                                    element: fe2o3_kernel_ir::Type::StorageObject(schema),
                                    ..
                                } = allocation.operation.kind
                                else {
                                    panic!(
                                        "pointer spill must allocate its explicit holder layout"
                                    );
                                };
                                assert!(matches!(
                                    inventory.owner().module().storage_layouts[schema.0 as usize]
                                        .kind,
                                    fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(_)
                                ));
                                let pointer = inventory.definitions()[definition].value.unwrap();
                                assert!(inventory.operations().iter().any(|row|
                                    row.coordinate.block.function == allocation.coordinate.block.function
                                    && matches!(row.operation.kind,
                                        fe2o3_kernel_ir::OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::WriteValue { address, .. })
                                            if address == pointer)), "exact holder needs its typed constructor store");
                            }
                            for instance in 0..plan.root(root, out)?.instances.len() {
                                let row = plan.instance(root, instance, out)?;
                                if !row.active || instance == 0 {
                                    continue;
                                }
                                let body =
                                    SourceByteBody::derive(plan, slots, root, instance, out)?;
                                let context = body.context(out)?;
                                assert!(!slots.has_original_object(root, instance, 4, out)?);
                                assert!(slots.has_original_object(root, instance, 8, out)?);
                                for (block, original) in
                                    context.function.blocks().iter().enumerate()
                                {
                                    for (statement, _) in original.statements().iter().enumerate() {
                                        match body.event_at(block, statement, out)? {
                                            Event::ObjectLive { local, .. }
                                                if local == row.locals.start + 8 =>
                                            {
                                                lifetimes[root][0] += 1
                                            }
                                            Event::ObjectDead { local }
                                                if local == row.locals.start + 8 =>
                                            {
                                                lifetimes[root][1] += 1
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                        assert_eq!(
                            lifetimes,
                            [[4, 4], [4, 4]],
                            "two original lifetimes per helper"
                        );
                        let mut source =
                            super::super::super::super::source_function::SourceByteProgram::derive(
                                plan, slots, out,
                            )?;
                        let paired = super::super::super::super::paired::PairedInvocations::derive(
                            plan,
                            &source,
                            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                            out,
                        )?;
                        let bindings =
                            super::super::super::super::byte_bindings::SourceByteBindings::derive(
                                slots, out,
                            )?;
                        slots.emit(out)?;
                        source.emit(out)?;
                        bindings.emit(out)?;
                        let start = out.text.len();
                        paired.emit(out)?;
                        let generated = &out.text[start..];
                        assert!(generated.contains("value.fields.contains_key(0)"));
                        assert!(generated.contains("value.variant == 0"));
                        assert!(generated.contains("value.variant == 1"));
                        assert!(!generated.contains("assume("));
                        completed.set(true);
                        Ok(())
                    },
                )
            },
        );
        result.0.unwrap();
        assert!(
            completed.get(),
            "all genuine lifetime cuts and paired effects must be generated"
        );
    }
}

#[test]
fn original_enum_spill_cannot_keep_a_reference_current_across_referent_restart() {
    let reached = std::cell::Cell::new(false);
    let result = super::super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            lifetime_fixture(types, functions, EndPayload::DeadEnum);
            let helper = functions.last_mut().unwrap();
            let old = &helper.blocks()[3];
            let word = helper.locals()[8].ty();
            let mut statements: Vec<_> = old.statements().iter().filter(|statement| {
                !matches!(statement.kind(), SemanticStatementKindV1::StorageDead(local) if local.index() == 9)
            }).cloned().collect();
            let restart_write = statements.iter().rposition(|statement| {
                matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment) if assignment.destination().local().index() == 8)
            }).unwrap();
            statements.insert(
                restart_write + 1,
                SemanticStatementV1::new(
                    old.source(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(8), vec![], word)
                            .unwrap(),
                        SemanticRvalueV1::new(
                            word,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(9),
                                    vec![
                                        SemanticProjectionV1::new(
                                            SemanticProjectionKindV1::Dereference,
                                            word,
                                        )
                                        .unwrap(),
                                    ],
                                    word,
                                )
                                .unwrap(),
                            )),
                        ),
                    )),
                ),
            );
            let mut blocks = helper.blocks().to_vec();
            blocks[3] = SemanticBasicBlockV1::new(
                old.identity(),
                old.source(),
                statements,
                old.terminator().clone(),
            )
            .unwrap();
            *helper = SemanticFunctionDeclV1::new(
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
        |_plan, _out| {
            reached.set(true);
            Ok(())
        },
    );
    let error = result
        .0
        .expect_err("a saved original loan cannot revive with the reused source local");
    assert!(
        !reached.get(),
        "invalid original lifetime must fail before publication"
    );
    assert!(
        // This fixture deliberately preserves local 9's loan when local 8 dies.
        // The source storage cut is therefore invalid before its later use.
        format!("{error:?}").contains("source reference referent storage dies with a live loan"),
        "wrong source lifetime refusal: {error:?}"
    );
}
