thread_local! {
    static OBJECT_LOAN_NONCANDIDATE_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OBJECT_LOAN_NONCANDIDATE_QUERIES_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static AGGREGATE_OBJECT_PROJECT_MUTATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static AGGREGATE_OBJECT_FOREIGN_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AGGREGATE_OBJECT_FOREIGN_REPLAYED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AGGREGATE_OBJECT_INDEX_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static AGGREGATE_OBJECT_INDEX_MUTATED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AGGREGATE_OBJECT_PROJECT_READ_KEYS_V29: std::cell::RefCell<Vec<(usize, ValueId)>> = const { std::cell::RefCell::new(Vec::new()) };
    static AGGREGATE_OBJECT_READ_COUNT_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
    static AGGREGATE_OBJECT_SCALE_ROSTER_V29: std::cell::Cell<Option<(usize, usize, usize)>> = const { std::cell::Cell::new(None) };
}

fn aggregate_reborrow_original_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    flat_aggregate_object_owner_v29(false)
}

fn flat_aggregate_object_owner_v29(mutable: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = suffix_owner(SuffixCase::Root);
    let source = original.source_semantic();
    let mut types = source.types().to_vec();
    let tuple = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, U32]).unwrap()),
        None,
    );
    let kind = if mutable {
        SemanticBorrowKindV1::Mutable
    } else {
        SemanticBorrowKindV1::Shared
    };
    let borrowed = reference(
        &mut types,
        tuple,
        if mutable {
            SemanticMutabilityV1::Mutable
        } else {
            SemanticMutabilityV1::Immutable
        },
        false,
    );
    let mut functions = source.functions().to_vec();
    for index in [0usize, 3] {
        let old = &functions[index];
        let mut locals = old.locals().to_vec();
        let mut append = |ty| {
            let id = locals.len() as u32;
            locals.push(local(220 + id as u8, ty, SemanticLocalRoleV1::Temporary));
            id
        };
        let value = append(tuple);
        let parent = append(borrowed);
        let child = append(borrowed);
        let output = append(U32);
        let dereference = |local| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, tuple)
                        .unwrap(),
                ],
                tuple,
            )
            .unwrap()
        };
        let mut statements = old.blocks()[0].statements().to_vec();
        statements.push(assign(
            place(value, tuple),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![literal(17), literal(29)],
                )
                .unwrap(),
            ),
        ));
        statements.push(assign(
            place(parent, borrowed),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: place(value, tuple),
            },
        ));
        statements.push(assign(
            place(child, borrowed),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: dereference(parent),
            },
        ));
        let field = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(child),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, tuple).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), U32).unwrap(),
            ],
            U32,
        )
        .unwrap();
        if mutable {
            statements.push(assign(
                field.clone(),
                SemanticRvalueKindV1::Use(literal(31)),
            ));
        }
        for _ in 0..AGGREGATE_OBJECT_READ_COUNT_V29.get() {
            statements.push(assign(
                place(output, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field.clone())),
            ));
        }
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

fn original_aggregate_projected_reads_v29(original: &SemanticFunctionDeclV1) -> usize {
    original
        .blocks()
        .iter()
        .flat_map(|block| block.statements())
        .filter(|row| {
            let SemanticStatementKindV1::Assign(assignment) = row.kind() else {
                return false;
            };
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) =
                assignment.value().kind()
            else {
                return false;
            };
            matches!(place.projections(), [root, field]
            if root.kind() == SemanticProjectionKindV1::Dereference
                && field.kind() == SemanticProjectionKindV1::Field(1)
                && place.ty() == U32)
        })
        .count()
}

fn inspect_aggregate_reborrow_candidate_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.unwrap().plan;
    assert_ne!(plan.storage, SourceReferenceStorageV29::PromotedOnly);
    let mut aggregate_invocations = 0;
    for lowered in emitted.iter().flatten() {
        let instance = lowered.source_call_instance.unwrap();
        let original = instances.instance(instance).unwrap().declaration();
        let expected_reads = original_aggregate_projected_reads_v29(original);
        if expected_reads != 0 {
            assert_eq!(expected_reads, AGGREGATE_OBJECT_READ_COUNT_V29.get());
            aggregate_invocations += 1;
        }
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
                if place.projections().len() != 1 {
                    continue;
                }
                assert_eq!(
                    place.projections()[0].kind(),
                    SemanticProjectionKindV1::Dereference
                );
                assert!(matches!(
                    instances.owner().source_semantic().types()[place.ty().index() as usize]
                        .shape(),
                    SemanticTypeShapeV1::Tuple(_)
                ));
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
                    fn h<T>() -> usize {
                        std::mem::size_of::<T>()
                            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
                    }
                    let headers = h::<Option<SourceObjectLoanV29>>()
                        + h::<Option<&SemanticTypeDeclV1>>()
                        + h::<Option<&SemanticTypeShapeV1>>();
                    let before = (budget.work(), budget.storage());
                    assert!(
                        source_object_loan_access_v29(
                            plan,
                            site,
                            place,
                            SourceReferenceAccessV29::Read,
                            budget
                        )?
                        .is_none()
                    );
                    assert_eq!(
                        (budget.work() - before.0, budget.storage() - before.1),
                        (5 * 5 + 2, headers)
                    );
                    // This probe returns no authority and does not alias Borrow to Read.
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
                        "original aggregate role remains Borrow: {:?}",
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
                OBJECT_LOAN_NONCANDIDATE_QUERIES_V29
                    .set(OBJECT_LOAN_NONCANDIDATE_QUERIES_V29.get() + 1);
            }
        }
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        let mut projected_reads = 0;
        for row in &anchors.rows {
            if !matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                continue;
            }
            let payload = anchors.object_payload(row, budget)?;
            let ScopedObjectRoleV29::ReadValue {
                source: endpoint,
                read: ScopedObjectReadOriginV29::Original(read),
            } = payload.role
            else {
                continue;
            };
            if !matches!(endpoint.object, ScopedObjectIdentityV29::Reference { .. }) {
                continue;
            }
            let ScopedObjectOperationV29::ReadValue { address: pointer, .. } = payload.operation else {
                panic!("original Reference read must retain its actual ReadValue pointer");
            };
            AGGREGATE_OBJECT_PROJECT_READ_KEYS_V29.with_borrow_mut(|keys| {
                let key = (instance.index(), pointer);
                if !keys.contains(&key) {
                    keys.push(key);
                }
            });
            assert_eq!(endpoint.path.count, 1);
            let place = scoped_object_original_place_v29(original, read.site, read.role).unwrap();
            let (block, statement) = scoped_memory_site_key_v29(read.site);
            let site = SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(block),
                statement: statement.map(|row| row as usize),
            };
            with_canonical_call_scratch_v1(budget, |budget| {
                let checked = source_aggregate_object_access_v29(
                    plan,
                    site,
                    place,
                    SourceReferenceAccessV29::Read,
                    budget,
                )?
                .unwrap();
                assert_eq!(checked.dereference, 0);
                assert_eq!(checked.loan.original.ty, endpoint.root_type);
                assert_eq!(
                    checked.loan.original.kind,
                    SourceBackingKindV29::Object(endpoint.root_schema)
                );
                assert!(
                    source_aggregate_object_endpoint_access_v29(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Read,
                        endpoint,
                        budget
                    )?
                    .is_some()
                );
                let mut wrong_type = endpoint;
                wrong_type.root_type = SemanticTypeIdV1::from_index(u32::MAX);
                let refused = source_aggregate_object_endpoint_access_v29(
                    plan,
                    site,
                    place,
                    SourceReferenceAccessV29::Read,
                    wrong_type,
                    budget,
                );
                assert!(
                    matches!(
                        refused,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "typed object source payload differs from its actual operation",
                            ..
                        })
                    ),
                    "{:?}",
                    refused.err()
                );
                let cloned = (*place).clone();
                for (source, access) in [
                    (&cloned, SourceReferenceAccessV29::Read),
                    (place, SourceReferenceAccessV29::Write),
                ] {
                    let refused =
                        source_aggregate_object_access_v29(plan, site, source, access, budget);
                    assert!(
                        matches!(
                            refused,
                            Err(ProductionSemanticKirErrorV1::Unsupported {
                                detail: "source reference access occurrence is not retained",
                                ..
                            })
                        ),
                        "{:?}",
                        refused.err()
                    );
                }
                if AGGREGATE_OBJECT_FOREIGN_V29.get() {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                    let mut foreign = ArgumentBudgetV1::new(&mut work, 37);
                    let refused = source_aggregate_object_access_v29(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Read,
                        &mut foreign,
                    );
                    assert!(matches!(
                        refused,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                    let before = (budget.work(), budget.storage());
                    let replay = source_aggregate_object_endpoint_access_v29(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Read,
                        endpoint,
                        budget,
                    );
                    assert!(matches!(
                        replay,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!((budget.work(), budget.storage()), before);
                    AGGREGATE_OBJECT_FOREIGN_REPLAYED_V29.set(true);
                    return Ok(());
                }
                assert!(
                    source_aggregate_object_endpoint_access_v29(
                        plan,
                        site,
                        place,
                        SourceReferenceAccessV29::Read,
                        endpoint,
                        budget
                    )?
                    .is_some()
                );
                Ok(())
            })?;
            if AGGREGATE_OBJECT_FOREIGN_V29.get() {
                return Ok(());
            }
            projected_reads += 1;
        }
        assert_eq!(
            projected_reads, expected_reads,
            "exact projected aggregate read roster from the original invocation"
        );
    }
    assert_eq!(
        aggregate_invocations, 3,
        "original root and both helper invocations"
    );
    OBJECT_LOAN_NONCANDIDATE_VISITS_V29.set(OBJECT_LOAN_NONCANDIDATE_VISITS_V29.get() + 1);
    Ok(())
}

fn run_aggregate_reborrow_candidate_v29(
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(Option<ScopedSlotCustodyObserverV29>, Vec<(usize, ValueId)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            AGGREGATE_OBJECT_PROJECT_READ_KEYS_V29.replace(std::mem::take(&mut self.1));
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_aggregate_reborrow_candidate_v29)),
        AGGREGATE_OBJECT_PROJECT_READ_KEYS_V29.replace(Vec::new()),
    );
    OBJECT_LOAN_NONCANDIDATE_VISITS_V29.set(0);
    OBJECT_LOAN_NONCANDIDATE_QUERIES_V29.set(0);
    run_original_source_fixture_v29(
        aggregate_reborrow_original_owner_v29,
        false,
        true,
        3,
        |_, _, _, _, _| Ok(()),
        work,
        storage,
    )
}

#[test]
fn original_aggregate_reborrow_is_not_an_implicit_object_read() {
    let result = run_aggregate_reborrow_candidate_v29(LIMIT, LIMIT);
    assert!(result.0.is_ok() && result.3, "{:?}", result.0);
    assert_eq!(OBJECT_LOAN_NONCANDIDATE_VISITS_V29.get(), 3);
    assert!(OBJECT_LOAN_NONCANDIDATE_QUERIES_V29.get() >= 9);
}

#[test]
fn original_aggregate_reborrow_candidate_has_exact_and_one_short_transaction_limits() {
    let (positive, work, storage, completed) = run_aggregate_reborrow_candidate_v29(LIMIT, LIMIT);
    assert!(positive.is_ok() && completed, "{positive:?}");
    let exact = run_aggregate_reborrow_candidate_v29(work, storage);
    assert!(exact.0.is_ok() && exact.3, "{:?}", exact.0);
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let short = run_aggregate_reborrow_candidate_v29(work_limit, storage_limit);
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
            other => panic!("aggregate reborrow candidate resource boundary: {other:?}"),
        }
    }
}

#[test]
fn original_flat_aggregate_mutable_reborrow_preserves_projected_write_and_read() {
    let run = |work, storage| {
        run_original_source_fixture_v29(
            || flat_aggregate_object_owner_v29(true),
            false,
            true,
            3,
            |_, _, _, _, _| Ok(()),
            work,
            storage,
        )
    };
    let (result, work, storage, completed) = run(LIMIT, LIMIT);
    assert!(result.is_ok() && completed, "{result:?}");
    let exact = run(work, storage);
    assert!(exact.0.is_ok() && exact.3, "{:?}", exact.0);
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let short = run(work_limit, storage_limit);
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
            other => panic!("mutable aggregate resource boundary: {other:?}"),
        }
    }
}

#[test]
fn original_aggregate_object_query_retains_foreign_ledger_failure_after_swallow() {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            AGGREGATE_OBJECT_FOREIGN_V29.set(self.0);
        }
    }
    let _restore = Restore(AGGREGATE_OBJECT_FOREIGN_V29.replace(false));
    let positive = run_aggregate_reborrow_candidate_v29(LIMIT, LIMIT);
    assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
    AGGREGATE_OBJECT_FOREIGN_V29.set(true);
    AGGREGATE_OBJECT_FOREIGN_REPLAYED_V29.set(false);
    let refused = run_aggregate_reborrow_candidate_v29(LIMIT, LIMIT);
    assert!(AGGREGATE_OBJECT_FOREIGN_REPLAYED_V29.get());
    assert!(!refused.3);
    assert!(matches!(
        original_repeated_source_resource_v29(refused.0.unwrap_err()),
        ArgumentResourceV1::Accounting
    ));
}

#[test]
fn flat_aggregate_object_domain_is_reference_free_and_independently_metered() {
    let owner = aggregate_reborrow_original_owner_v29();
    let mut types = owner.source_semantic().types().to_vec();
    let flat = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(12),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, U32, U32]).unwrap()),
        None,
    );
    let ordinary = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(12),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U32, U32, U32]).unwrap()),
        None,
    );
    let nested = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 12], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![flat, U32]).unwrap()),
        None,
    );
    let reference = reference(&mut types, U32, SemanticMutabilityV1::Immutable, false);
    let with_reference = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![reference, U32]).unwrap()),
        None,
    );
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let headers = h::<bool>()
        + h::<Option<&SemanticTypeDeclV1>>()
        + h::<Option<&SemanticTypeShapeV1>>()
        + h::<&SemanticTypeDeclV1>()
        + h::<&SemanticTypeShapeV1>()
        + h::<&[SemanticTypeIdV1]>()
        + h::<&SemanticAggregateTypeV1>()
        + h::<Option<&SemanticTypeIdV1>>()
        + h::<&SemanticTypeIdV1>()
        + h::<std::slice::Iter<'_, SemanticTypeIdV1>>();
    let run = |ty, work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(11).unwrap();
        let result = source_flat_aggregate_object_v29(&types, ty, &mut budget);
        (result, budget.work(), budget.storage())
    };
    for ty in [flat, ordinary] {
        let positive = run(ty, LIMIT, LIMIT);
        assert_eq!(positive.0.unwrap(), true);
        assert_eq!((positive.1, positive.2), (4 + 3 * 3, 11 + headers));
        assert!(run(ty, 13, 11 + headers).0.unwrap());
        assert!(matches!(run(ty, 12, 11 + headers).0,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
                if error.limit() == 12 && error.actual() == 13));
        assert!(matches!(run(ty, 13, 10 + headers).0,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error)))
                if error.limit() == 10 + headers && error.actual() == 11 + headers));
    }
    for ty in [
        nested,
        with_reference,
        U32,
        SemanticTypeIdV1::from_index(u32::MAX),
    ] {
        assert!(
            !run(ty, LIMIT, LIMIT).0.unwrap(),
            "closed aggregate domain: {ty:?}"
        );
    }
}

fn mutate_aggregate_object_project_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for lowered in emitted.iter_mut().flatten() {
        let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
        for row in &anchors.rows {
            let ScopedMemoryAnchorKindV29::Object(index) = row.kind else {
                continue;
            };
            let payload = &mut anchors.objects[index];
            let ScopedObjectRoleV29::Project { projected, .. } = payload.role else {
                continue;
            };
            if !matches!(projected.object, ScopedObjectIdentityV29::Reference { .. }) {
                continue;
            }
            assert_eq!(projected.path.count, 1);
            let ScopedObjectOperationV29::Project { step, .. } = &mut payload.operation else {
                unreachable!()
            };
            assert_eq!(*step, ScopedObjectProjectionV29::Field(1));
            *step = ScopedObjectProjectionV29::Field(0);
            let operation = &mut lowered
                .function
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .find(|block| block.id == row.block)
                .unwrap()
                .operations[row.position];
            let OperationKind::Storage(ScopedObjectOperationV29::Project { step, .. }) =
                &mut operation.kind
            else {
                unreachable!()
            };
            assert_eq!(*step, ScopedObjectProjectionV29::Field(1));
            *step = ScopedObjectProjectionV29::Field(0);
            AGGREGATE_OBJECT_PROJECT_MUTATIONS_V29
                .set(AGGREGATE_OBJECT_PROJECT_MUTATIONS_V29.get() + 1);
        }
    }
    Ok(())
}

#[test]
fn original_flat_aggregate_loans_reject_coherent_actual_project_field_changes() {
    for mutable in [false, true] {
        let positive = run_original_source_fixture_v29(
            || flat_aggregate_object_owner_v29(mutable),
            false,
            true,
            3,
            |_, _, _, _, _| Ok(()),
            LIMIT,
            LIMIT,
        );
        assert!(
            positive.0.is_ok() && positive.3,
            "mutable={mutable}: {:?}",
            positive.0
        );
        AGGREGATE_OBJECT_PROJECT_MUTATIONS_V29.set(0);
        let refused = run_original_source_fixture_v29(
            || flat_aggregate_object_owner_v29(mutable),
            false,
            true,
            3,
            mutate_aggregate_object_project_v29,
            LIMIT,
            LIMIT,
        );
        assert!(!refused.3);
        assert!(AGGREGATE_OBJECT_PROJECT_MUTATIONS_V29.get() >= 3);
        assert!(
            matches!(refused.0, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == "typed object source payload differs from its actual operation"),
            "{:?}",
            refused.0
        );
    }
}

fn mutate_aggregate_object_project_index_v29(
    index: &mut SourceObjectPayloadIndexV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(
        index.projects.len() >= 3,
        "root and repeated helper Project roster"
    );
    let (queried, unused) = AGGREGATE_OBJECT_PROJECT_READ_KEYS_V29.with_borrow(|keys| {
        assert_eq!(keys.len(), 3, "one exact original Reference read per genuine invocation");
        let queried = index.projects.iter().position(|row| row.0 == keys[0])
            .expect("the original Reference ReadValue pointer must be indexed");
        let unused = index.projects.iter().rposition(|row| !keys.contains(&row.0))
            .expect("the fixture retains an unused aggregate-initialization Project");
        (queried, unused)
    });
    match AGGREGATE_OBJECT_INDEX_FAULT_V29.get() {
        1 => index.projects[1] = index.projects[0],
        2 => {
            index.projects.remove(unused);
        }
        3 => index.projects[queried].1 = usize::MAX,
        4 => {
            let mut row = index.projects.remove(queried);
            row.0.0 = usize::MAX;
            index.projects.push(row);
            index.projects.sort_unstable_by_key(|row| row.0);
        }
        _ => unreachable!(),
    }
    AGGREGATE_OBJECT_INDEX_MUTATED_V29.set(true);
    Ok(())
}

#[test]
fn original_flat_aggregate_project_index_rejects_duplicate_missing_stale_and_foreign_rows() {
    struct Restore(Option<SourceObjectPayloadIndexObserverV29>, u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.set(self.0);
            AGGREGATE_OBJECT_INDEX_FAULT_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.replace(None),
        AGGREGATE_OBJECT_INDEX_FAULT_V29.get(),
    );
    for fault in 1..=4 {
        SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.set(None);
        let positive = run_aggregate_reborrow_candidate_v29(LIMIT, LIMIT);
        assert!(
            positive.0.is_ok() && positive.3,
            "fault={fault}: {:?}",
            positive.0
        );
        AGGREGATE_OBJECT_INDEX_FAULT_V29.set(fault);
        AGGREGATE_OBJECT_INDEX_MUTATED_V29.set(false);
        SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29
            .set(Some(mutate_aggregate_object_project_index_v29));
        let refused = run_aggregate_reborrow_candidate_v29(LIMIT, LIMIT);
        assert!(AGGREGATE_OBJECT_INDEX_MUTATED_V29.get());
        assert!(!refused.3, "fault={fault}: the whole source transaction must refuse: {:?}", refused.0);
        assert!(
            matches!(refused.0, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == "typed object source payload differs from its actual operation"),
            "fault={fault}: {:?}",
            refused.0
        );
    }
}

fn observe_aggregate_project_scale_roster_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    _: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut roster = (0, 0, 0);
    let mut aggregate_invocations = 0;
    let mut unrelated_invocations = 0;
    for lowered in emitted.iter().flatten() {
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        let instance = lowered.source_call_instance.unwrap();
        let original = instances.instance(instance).unwrap().declaration();
        let expected_reads = original_aggregate_projected_reads_v29(original);
        if expected_reads != 0 {
            assert_eq!(expected_reads, AGGREGATE_OBJECT_READ_COUNT_V29.get());
            assert!(!anchors.objects.is_empty());
            aggregate_invocations += 1;
        } else {
            unrelated_invocations += 1;
        }
        if anchors.objects.is_empty() {
            assert_eq!(
                expected_reads, 0,
                "an original aggregate invocation cannot disappear"
            );
            // These unrelated fixture functions have no static Scalar holder either,
            // so neither their anchor rows nor their events enter the index scans.
            for row in &anchors.rows {
                if !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. }) {
                    continue;
                }
                let Some(frame) = row.source else {
                    continue;
                };
                let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                    continue;
                };
                let Some(place) = scoped_source_place_v29(original, frame.site, role) else {
                    continue;
                };
                let projections = place.projections();
                assert!(
                    !(projections.len() >= 2
                        && projections.last().unwrap().kind()
                            == SemanticProjectionKindV1::Dereference
                        && projections[..projections.len() - 1]
                            .iter()
                            .all(|row| matches!(row.kind(), SemanticProjectionKindV1::Field(_)))),
                    "fixture added a Scalar static-holder index participant"
                );
            }
            continue;
        }
        roster.0 += anchors.rows.len();
        roster.1 += instances.occurrences(instance).unwrap().events().len();
        roster.2 += anchors
            .objects
            .iter()
            .filter(|payload| matches!(payload.operation, ScopedObjectOperationV29::Project { .. }))
            .count();
    }
    assert_eq!(
        aggregate_invocations, 3,
        "original root and both helper invocations"
    );
    assert!(
        unrelated_invocations > 0,
        "full fixture retains unrelated functions"
    );
    assert!(roster.2 >= 3 * AGGREGATE_OBJECT_READ_COUNT_V29.get());
    if let Some(previous) = AGGREGATE_OBJECT_SCALE_ROSTER_V29.get() {
        assert_eq!(
            previous, roster,
            "same candidate on every source reconstruction"
        );
    }
    AGGREGATE_OBJECT_SCALE_ROSTER_V29.set(Some(roster));
    Ok(())
}

fn observe_aggregate_project_scale_index_v29(
    index: &mut SourceObjectPayloadIndexV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let (_, _, projects) = AGGREGATE_OBJECT_SCALE_ROSTER_V29.get().unwrap();
    assert_eq!(
        index.projects.len(),
        projects,
        "complete emitted Project roster"
    );
    Ok(())
}

#[test]
fn original_flat_aggregate_project_index_has_two_scans_and_logarithmic_queries() {
    struct Restore(
        usize,
        Option<ScopedSlotCustodyObserverV29>,
        Option<SourceObjectPayloadIndexObserverV29>,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            AGGREGATE_OBJECT_READ_COUNT_V29.set(self.0);
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.1);
            SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.set(self.2);
        }
    }
    let _restore = Restore(
        AGGREGATE_OBJECT_READ_COUNT_V29.get(),
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe_aggregate_project_scale_roster_v29)),
        SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29
            .replace(Some(observe_aggregate_project_scale_index_v29)),
    );
    let mut samples = Vec::new();
    for count in [8, 16, 32] {
        AGGREGATE_OBJECT_READ_COUNT_V29.set(count);
        AGGREGATE_OBJECT_SCALE_ROSTER_V29.set(None);
        SOURCE_OBJECT_PAYLOAD_CONSTRUCTION_WORK_V29.set((0, 0));
        SOURCE_OBJECT_PROJECT_CENSUS_WORK_V29.set((0, 0));
        SOURCE_OBJECT_PROJECT_INDEX_WORK_V29.set((0, 0));
        SOURCE_OBJECT_PROJECT_INDEX_STORAGE_V29.set(0);
        SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.set((0, 0, 0));
        let result = run_original_source_fixture_v29(
            aggregate_reborrow_original_owner_v29,
            false,
            true,
            3,
            |_, _, _, _, _| Ok(()),
            LIMIT,
            LIMIT,
        );
        assert!(
            result.0.is_ok() && result.3,
            "count={count}: {:?}",
            result.0
        );
        let roster = AGGREGATE_OBJECT_SCALE_ROSTER_V29.get().unwrap();
        let constructors = SOURCE_OBJECT_PAYLOAD_CONSTRUCTION_WORK_V29.get();
        let queries = SOURCE_OBJECT_PROJECT_INDEX_WORK_V29.get();
        let scans = SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.get();
        assert!(constructors.0 >= 3 && queries.0 >= 3 * count);
        assert_eq!(
            SOURCE_OBJECT_PROJECT_CENSUS_WORK_V29.get(),
            (constructors.0, constructors.0),
            "one independently counted work unit per complete Project census"
        );
        assert_eq!(
            scans,
            (
                constructors.0 * roster.0,
                constructors.0 * roster.0,
                constructors.0 * roster.1
            ),
            "one count scan, one fill scan, one original event scan per constructor"
        );
        let mut levels = 1;
        let mut remaining = roster.2;
        while remaining != 0 {
            levels += 1;
            remaining >>= 1;
        }
        assert_eq!(
            queries.1,
            queries.0 * (11 + 3 * levels),
            "owner + logarithmic key lookup + exact payload rejoin, never an anchor scan"
        );
        fn h<T>() -> usize {
            std::mem::size_of::<T>()
                + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
        }
        let headers = h::<usize>()
            + h::<Result<usize, usize>>()
            + h::<(usize, ValueId)>()
            + h::<Option<ValueId>>()
            + h::<Option<&ScopedMemoryAnchorV29>>()
            + h::<&ScopedMemoryAnchorV29>()
            + h::<&ScopedObjectPayloadV29>();
        assert_eq!(
            SOURCE_OBJECT_PROJECT_INDEX_STORAGE_V29.get(),
            queries.0 * headers,
            "query headers are a fixed independent seven-bundle equation"
        );
        samples.push((constructors, queries, roster));
    }
    assert!(samples.windows(2).all(|pair| pair[0].0.0 == pair[1].0.0));
    for dimension in 0..3 {
        let value =
            |sample: &((usize, usize), (usize, usize), (usize, usize, usize))| match dimension {
                0 => sample.0.1,
                1 => sample.1.1,
                _ => sample.2.0,
            };
        let first = value(&samples[1]).checked_sub(value(&samples[0])).unwrap();
        let second = value(&samples[2]).checked_sub(value(&samples[1])).unwrap();
        assert!(
            first > 0 && second < 3 * first,
            "constructor/query/anchor growth must remain subquadratic: dimension={dimension}, samples={samples:?}"
        );
    }
}
