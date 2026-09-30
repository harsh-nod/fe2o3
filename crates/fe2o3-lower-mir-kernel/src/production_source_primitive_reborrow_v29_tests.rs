thread_local! {
    static PRIMITIVE_REBORROW_WRAPPED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PRIMITIVE_REBORROW_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PRIMITIVE_REBORROW_QUERIES_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn primitive_reborrow_original_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let original = suffix_owner(SuffixCase::Root);
    let source = original.source_semantic();
    let mut types = source.types().to_vec();
    let borrowed = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let wrapped = PRIMITIVE_REBORROW_WRAPPED_V29.get();
    let holder_type = if wrapped {
        declaration(
            &mut types,
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, borrowed]).unwrap()),
            None,
        )
    } else {
        borrowed
    };
    let mut functions = source.functions().to_vec();
    for index in [0usize, 3] {
        let old = &functions[index];
        let mut locals = old.locals().to_vec();
        let mut append = |ty| {
            let id = locals.len() as u32;
            locals.push(local(220 + id as u8, ty, SemanticLocalRoleV1::Temporary));
            id
        };
        let value = append(U32);
        let parent = append(borrowed);
        let holder = if wrapped { append(holder_type) } else { parent };
        let child = append(borrowed);
        let output = append(U32);
        let mut statements = old.blocks()[0].statements().to_vec();
        statements.push(assign(
            place(value, U32),
            SemanticRvalueKindV1::Use(literal(17)),
        ));
        statements.push(assign(
            place(parent, borrowed),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(value, U32),
            },
        ));
        if wrapped {
            statements.push(assign(
                place(holder, holder_type),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![
                            literal(29),
                            SemanticOperandV1::Copy(place(parent, borrowed)),
                        ],
                    )
                    .unwrap(),
                ),
            ));
        }
        let mut projections = Vec::new();
        if wrapped {
            projections.push(
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), borrowed).unwrap(),
            );
        }
        projections
            .push(SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap());
        statements.push(assign(
            place(child, borrowed),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(holder),
                    projections,
                    U32,
                )
                .unwrap(),
            },
        ));
        statements.push(assign(
            place(output, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(child),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32)
                            .unwrap(),
                    ],
                    U32,
                )
                .unwrap(),
            )),
        ));
        let mut blocks = old.blocks().to_vec();
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            statements,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        let mut rebuilt = SemanticFunctionDeclV1::new(
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
        if let Some(kernel) = old.kernel_entry() {
            rebuilt = rebuilt.with_kernel_entry(kernel.clone());
        }
        functions[index] = rebuilt;
    }
    scoped_root_tests::fixtures::build(types, functions, source.callables().to_vec())
}

fn inspect_primitive_reborrow_role_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.unwrap().plan;
    for lowered in emitted.iter().flatten() {
        let instance = lowered.source_call_instance.unwrap();
        let original = instances.instance(instance).unwrap().declaration();
        for (block, body) in original.blocks().iter().enumerate() {
            for (statement, row) in body.statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) = row.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } = assignment.value().kind()
                else {
                    continue;
                };
                if place.ty() != U32
                    || !matches!(
                        place
                            .projections()
                            .last()
                            .map(|projection| projection.kind()),
                        Some(SemanticProjectionKindV1::Dereference)
                    )
                {
                    continue;
                }
                assert_eq!(
                    place.projections().len(),
                    if PRIMITIVE_REBORROW_WRAPPED_V29.get() {
                        2
                    } else {
                        1
                    }
                );
                let site = SourceReferenceSiteV29 {
                    instance,
                    block: SemanticBlockIdV1::from_index(block as u32),
                    statement: Some(statement),
                };
                let floor = budget.storage();
                with_canonical_call_scratch_v1(budget, |budget| {
                    source_reference_access_at_v29(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Shared),
                        budget,
                    )?;
                    // A reborrow keeps its original role; no implicit Read is admitted.
                    let read = source_reference_access_at_v29(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Read,
                        budget,
                    );
                    assert!(
                        matches!(
                            read,
                            Err(ProductionSemanticKirErrorV1::Unsupported {
                                detail: "source reference access occurrence is not retained",
                                ..
                            })
                        ),
                        "original scalar role remains Borrow: {:?}",
                        read.as_ref().err()
                    );
                    source_reference_access_at_v29(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Shared),
                        budget,
                    )?;
                    Ok(())
                })?;
                assert_eq!(budget.storage(), floor);
                PRIMITIVE_REBORROW_QUERIES_V29.set(PRIMITIVE_REBORROW_QUERIES_V29.get() + 1);
            }
        }
    }
    PRIMITIVE_REBORROW_VISITS_V29.set(PRIMITIVE_REBORROW_VISITS_V29.get() + 1);
    Ok(())
}

fn run_primitive_reborrow_role_v29(
    wrapped: bool,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(Option<ScopedSlotCustodyObserverV29>, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            PRIMITIVE_REBORROW_WRAPPED_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_primitive_reborrow_role_v29)),
        PRIMITIVE_REBORROW_WRAPPED_V29.replace(wrapped),
    );
    PRIMITIVE_REBORROW_VISITS_V29.set(0);
    PRIMITIVE_REBORROW_QUERIES_V29.set(0);
    run_original_source_fixture_v29(
        primitive_reborrow_original_owner_v29,
        false,
        true,
        3,
        |_, _, _, _, _| Ok(()),
        work,
        storage,
    )
}

#[test]
fn original_primitive_reborrow_preserves_borrow_role_without_admitting_read() {
    for wrapped in [false, true] {
        let result = run_primitive_reborrow_role_v29(wrapped, LIMIT, LIMIT);
        assert!(
            result.0.is_ok() && result.3,
            "wrapped={wrapped}: {:?}",
            result.0
        );
        assert_eq!(PRIMITIVE_REBORROW_VISITS_V29.get(), 3);
        assert!(PRIMITIVE_REBORROW_QUERIES_V29.get() >= 9);
    }
}

#[test]
fn original_primitive_reborrow_has_exact_and_one_short_transaction_limits() {
    for wrapped in [false, true] {
        let (positive, work, storage, completed) =
            run_primitive_reborrow_role_v29(wrapped, LIMIT, LIMIT);
        assert!(
            positive.is_ok() && completed,
            "wrapped={wrapped}: {positive:?}"
        );
        let exact = run_primitive_reborrow_role_v29(wrapped, work, storage);
        assert!(
            exact.0.is_ok() && exact.3,
            "wrapped={wrapped}: {:?}",
            exact.0
        );
        assert_eq!((exact.1, exact.2), (work, storage));
        for (work_limit, storage_limit, is_work) in
            [(work - 1, storage, true), (work, storage - 1, false)]
        {
            let short = run_primitive_reborrow_role_v29(wrapped, work_limit, storage_limit);
            assert!(!short.3);
            match (
                is_work,
                original_repeated_source_resource_v29(short.0.unwrap_err()),
            ) {
                (true, ArgumentResourceV1::Work(error)) => {
                    assert_eq!(error.limit(), work_limit);
                    assert!(error.actual() > work_limit);
                }
                (false, ArgumentResourceV1::Storage(error)) => {
                    assert_eq!(error.limit(), storage_limit);
                    assert!(error.actual() > storage_limit);
                }
                other => panic!("primitive reborrow resource boundary: {other:?}"),
            }
        }
    }
}
