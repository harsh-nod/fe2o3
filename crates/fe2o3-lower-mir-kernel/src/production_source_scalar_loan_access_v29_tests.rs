thread_local! {
    static SCALAR_STATIC_LOAN_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SCALAR_STATIC_LOAN_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SCALAR_STATIC_LOAN_MUTATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SCALAR_STATIC_LOAN_QUERY_COUNTS_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

fn inspect_scalar_static_loans_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mutable = OBJECT_LOAN_CASE_V29.get().mutable;
    let fault = SCALAR_STATIC_LOAN_FAULT_V29.get();
    let mut helpers = 0;
    for owner in &slots.instances {
        let source = instances.instance(owner.instance).unwrap();
        if source.function().index() != 3 {
            continue;
        }
        helpers += 1;
        let local_pointer = |local| {
            slots.slots[owner.slots.clone()].iter().find_map(|slot| {
            matches!(slot.origin.identity, ScopedAllocationIdentityV29::LegacyLocal(id) if id == local)
                .then_some(slot.origin.pointer)
        }).unwrap()
        };
        let original_pointer = local_pointer(1);
        let other_pointer = local_pointer(2);
        assert_ne!(original_pointer, other_pointer);
        let lowered = emitted[owner.instance.index()].as_mut().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
        assert!(
            anchors.objects.is_empty(),
            "this original Scalar helper must not rely on an Object-enabled payload index"
        );
        let mut reads = 0;
        let mut writes = 0;
        for row in &mut anchors.rows {
            let ScopedMemoryAnchorKindV29::Access {
                pointer,
                payload: Some(payload),
            } = &mut row.kind
            else {
                continue;
            };
            let Some(frame) = row.source else {
                assert!(matches!(
                    payload,
                    ScopedMemoryPayloadV29::Store {
                        source: ScopedMemoryStoreSourceV29::EntryArgument { .. },
                        ..
                    }
                ));
                continue;
            };
            let (place, read) = match payload {
                ScopedMemoryPayloadV29::Load { read, .. } if read.prefix == 2 => (
                    scoped_payload_place_v29(source.declaration(), read.site, read.role).unwrap(),
                    true,
                ),
                ScopedMemoryPayloadV29::Store { .. } => {
                    let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                        continue;
                    };
                    let place =
                        scoped_source_place_v29(source.declaration(), frame.site, role).unwrap();
                    if place.projections().len() != 2 {
                        continue;
                    }
                    (place, false)
                }
                _ => continue,
            };
            assert_eq!(
                place.projections()[0].kind(),
                SemanticProjectionKindV1::Field(1)
            );
            assert_eq!(
                place.projections()[1].kind(),
                SemanticProjectionKindV1::Dereference
            );
            assert_eq!(place.ty(), U32);
            if read {
                reads += 1;
            } else {
                writes += 1;
            }
            if fault == 1 && read {
                let definition = lowered
                    .function
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.results.iter().any(|value| value.id == *pointer))
                    .unwrap();
                assert!(matches!(&definition.kind, OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess, value, to: Type::Pointer(ty),
                } if *value == original_pointer && ty.access == AccessMode::ReadOnly));
            }
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
            let actual_pointer = match &mut operation.kind {
                OperationKind::Load { pointer, .. } if read => pointer,
                OperationKind::Store { pointer, .. } if !read => pointer,
                _ => panic!("authentic scalar safe-loan memory operation"),
            };
            assert_eq!(*actual_pointer, *pointer);
            if fault == 1 && read {
                // Same physical slot, different original pointer definition:
                // bypass only the original shared RestrictPointerAccess value.
                assert!(!mutable);
                assert_ne!(*pointer, original_pointer);
                *actual_pointer = original_pointer;
                *pointer = original_pointer;
                SCALAR_STATIC_LOAN_MUTATIONS_V29.set(SCALAR_STATIC_LOAN_MUTATIONS_V29.get() + 1);
            } else if fault == 2 && !read {
                assert!(mutable);
                *actual_pointer = other_pointer;
                *pointer = other_pointer;
                SCALAR_STATIC_LOAN_MUTATIONS_V29.set(SCALAR_STATIC_LOAN_MUTATIONS_V29.get() + 1);
            }
        }
        assert_eq!(reads, if mutable { 2 } else { 1 });
        assert_eq!(writes, usize::from(mutable));
    }
    assert!(helpers >= 2, "original repeated helper instances");
    SCALAR_STATIC_LOAN_VISITS_V29.set(SCALAR_STATIC_LOAN_VISITS_V29.get() + 1);
    Ok(())
}

fn run_scalar_static_loans_v29(
    mutable: bool,
    fault: u8,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(ObjectLoanCaseV29, u8, bool, bool, u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            OBJECT_LOAN_CASE_V29.set(self.0);
            OBJECT_LOAN_FAULT_V29.set(self.1);
            OBJECT_LOAN_FORCE_BACKING_V29.set(self.2);
            OBJECT_LOAN_LEGACY_CELLS_V29.set(self.3);
            SCALAR_STATIC_LOAN_FAULT_V29.set(self.4);
        }
    }
    let _restore = Restore(
        OBJECT_LOAN_CASE_V29.replace(ObjectLoanCaseV29 {
            mutable,
            wrapped: true,
            temporary: false,
            validity: false,
        }),
        OBJECT_LOAN_FAULT_V29.replace(0),
        OBJECT_LOAN_FORCE_BACKING_V29.replace(false),
        OBJECT_LOAN_LEGACY_CELLS_V29.replace(true),
        SCALAR_STATIC_LOAN_FAULT_V29.replace(fault),
    );
    SCALAR_STATIC_LOAN_VISITS_V29.set(0);
    SCALAR_STATIC_LOAN_MUTATIONS_V29.set(0);
    run_original_source_fixture_v29(
        || object_loan_original_owner_v29(SuffixCase::Cells),
        false,
        true,
        3,
        inspect_scalar_static_loans_v29,
        work,
        storage,
    )
}

#[test]
fn original_scalar_static_holders_complete_shared_reads_and_mutable_reads_writes() {
    for mutable in [false, true] {
        let result = run_scalar_static_loans_v29(mutable, 0, LIMIT, LIMIT);
        assert!(
            result.0.is_ok() && result.3,
            "mutable={mutable}: {:?}",
            result.0
        );
        assert_eq!(SCALAR_STATIC_LOAN_VISITS_V29.get(), 3);
    }
}

#[test]
fn original_scalar_static_holders_reject_changed_pointer_identity_and_referent() {
    for (mutable, fault) in [(false, 1), (true, 2)] {
        let positive = run_scalar_static_loans_v29(mutable, 0, LIMIT, LIMIT);
        assert!(positive.0.is_ok() && positive.3, "{:?}", positive.0);
        let refused = run_scalar_static_loans_v29(mutable, fault, LIMIT, LIMIT);
        assert!(!refused.3);
        assert!(SCALAR_STATIC_LOAN_VISITS_V29.get() >= 1);
        assert!(SCALAR_STATIC_LOAN_MUTATIONS_V29.get() >= 2);
        // Both coherent operation/payload mutations fail storage transport
        // before archive replay. The separate archived queries below test the
        // pointer identity and referent joins directly.
        let expected = "execution call parameters differ from their source instance";
        assert!(
            matches!(refused.0, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == expected),
            "fault={fault}: {:?}",
            refused.0
        );
    }
}

#[test]
fn original_scalar_static_holder_transactions_have_exact_and_one_short_limits() {
    for mutable in [false, true] {
        let (positive, work, storage, completed) =
            run_scalar_static_loans_v29(mutable, 0, LIMIT, LIMIT);
        assert!(positive.is_ok() && completed, "{positive:?}");
        let exact = run_scalar_static_loans_v29(mutable, 0, work, storage);
        assert!(exact.0.is_ok() && exact.3, "{:?}", exact.0);
        assert_eq!((exact.1, exact.2), (work, storage));
        for (work_limit, storage_limit, is_work) in
            [(work - 1, storage, true), (work, storage - 1, false)]
        {
            let short = run_scalar_static_loans_v29(mutable, 0, work_limit, storage_limit);
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
                other => panic!("scalar static-holder resource boundary: {other:?}"),
            }
        }
    }
}

fn inspect_scalar_static_loan_queries_v29(
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
        let original = instances.instance(instance).unwrap();
        if original.function().index() != 3 {
            continue;
        }
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        assert!(anchors.objects.is_empty());
        // This pure predicate only chooses an index allocation. Fresh budgets
        // below grant no source/loan/physical authority.
        fn h<T>() -> usize {
            std::mem::size_of::<T>()
                + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
        }
        let expected_storage = h::<bool>()
            + h::<Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>>()
            + h::<&production_call_instances_v1::ProductionCallInstanceV1<'_>>()
            + h::<Option<ScopedMemoryFrameV29>>()
            + h::<ScopedMemoryFrameV29>()
            + h::<Option<&SemanticPlaceV1>>()
            + h::<&SemanticPlaceV1>()
            + h::<std::slice::Iter<'_, ScopedMemoryAnchorV29>>()
            + h::<std::slice::Iter<'_, SemanticProjectionV1>>();
        let first = anchors
            .rows
            .iter()
            .position(|row| {
                if !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. }) {
                    return false;
                }
                let Some(frame) = row.source else {
                    return false;
                };
                let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                    return false;
                };
                let Some(place) = scoped_source_place_v29(original.declaration(), frame.site, role)
                else {
                    return false;
                };
                place.projections().len() == 2
                    && place.projections()[0].kind() == SemanticProjectionKindV1::Field(1)
                    && place.projections()[1].kind() == SemanticProjectionKindV1::Dereference
            })
            .unwrap();
        let expected_work = 2 + 4 * (first + 1) + 1;
        for (work_limit, storage_limit, cut) in [
            (expected_work, expected_storage, 0),
            (expected_work - 1, expected_storage, 1),
            (expected_work, expected_storage - 1, 2),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut local_budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            let result = source_scalar_static_holder_index_needed_v29(
                instances,
                instance,
                anchors,
                &mut local_budget,
            );
            match (cut, result) {
                (0, Ok(true)) => assert_eq!(
                    (local_budget.work(), local_budget.storage()),
                    (expected_work, expected_storage)
                ),
                (
                    1,
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error),
                    )),
                ) => {
                    assert_eq!((error.actual(), error.limit()), (expected_work, work_limit));
                }
                (
                    2,
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(error),
                    )),
                ) => {
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (expected_storage, storage_limit)
                    );
                }
                other => panic!("static holder index allocation boundary: {other:?}"),
            }
            let retained = local_budget.storage();
            local_budget.release_storage(retained).unwrap();
        }
        let archive = lowered.execution_observation.as_ref().unwrap();
        let occurrences = instances.occurrences(instance).unwrap();
        for anchor in &anchors.rows {
            let ScopedMemoryAnchorKindV29::Access {
                pointer,
                payload: Some(payload),
            } = anchor.kind
            else {
                continue;
            };
            let Some(frame) = anchor.source else {
                continue;
            };
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                continue;
            };
            let Some(place) = scoped_source_place_v29(original.declaration(), frame.site, role)
            else {
                continue;
            };
            if place.projections().len() != 2 {
                continue;
            }
            let access = match payload {
                ScopedMemoryPayloadV29::Load { read, .. } if read.prefix == 2 => {
                    SourceReferenceAccessV29::Read
                }
                ScopedMemoryPayloadV29::Store { .. } => SourceReferenceAccessV29::Write,
                _ => continue,
            };
            let (block, statement) = scoped_memory_site_key_v29(frame.site);
            let site = SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(block),
                statement: statement.map(|value| value as usize),
            };
            let mut event = None;
            for (index, row) in occurrences.events().iter().enumerate() {
                if row.site() == frame.site
                    && row.operand() == role
                    && row.role() == ExecutionEventV29::BaseUse
                    && row.event().variable().get() == place.local().index()
                {
                    assert!(event.replace(index).is_none());
                }
            }
            let event = event.unwrap();
            let floor = budget.storage();
            with_canonical_call_scratch_v1(budget, |budget| {
                let checked =
                    source_scalar_loan_access_v29(plan, site, place, access, budget)?.unwrap();
                let capture = source_object_base_occurrence_v29(
                    &occurrences,
                    frame.site,
                    role,
                    place,
                    event,
                    budget,
                )?;
                let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = capture else {
                    panic!("original static holder is promoted");
                };
                let binding =
                    archive.lookup_original_v29(instances, instance, definition, budget)?;
                assert!(source_object_private_holder_family_v29(
                    plan, binding, place, budget
                )?);
                check_source_scalar_loan_binding_v29(
                    plan, &checked, binding, place, pointer, budget,
                )?;
                let copied_place = place.clone();
                let changed_locator =
                    source_scalar_loan_access_v29(plan, site, &copied_place, access, budget);
                assert!(
                    matches!(
                        changed_locator,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "source reference access occurrence is not retained",
                            ..
                        })
                    ),
                    "{changed_locator:?}"
                );
                for fault in 0..5 {
                    let mut candidate = clone_execution_cfg_binding_v29(binding, &mut 0, budget)?;
                    let SemanticValueBindingV1::Aggregate(fields) = &mut candidate else {
                        panic!("original holder Aggregate");
                    };
                    assert_eq!(fields.len(), 2);
                    if fault == 0 {
                        fields.swap(0, 1);
                    } else {
                        let SemanticValueBindingV1::SourceReference(reference) = &mut fields[1]
                        else {
                            panic!("exact Field(1) terminal safe binding");
                        };
                        match fault {
                            1 => reference.owner ^= 1,
                            2 => {
                                reference.origin =
                                    SourceReferenceBindingOriginV29::SingleLoan(usize::MAX)
                            }
                            3 => reference.source_type = U32,
                            4 => reference.values[0].id = ValueId(u32::MAX),
                            _ => unreachable!(),
                        }
                    }
                    let refused = check_source_scalar_loan_binding_v29(
                        plan, &checked, &candidate, place, pointer, budget,
                    );
                    let expected = match fault {
                        0 => "typed object source payload differs from its actual operation",
                        1..=3 => "source reference binding belongs to another owner",
                        4 => {
                            "source raw address differs from its actual formation or memory history"
                        }
                        _ => unreachable!(),
                    };
                    assert!(
                        matches!(refused, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }) if detail == expected),
                        "{access:?}/fault={fault}: {refused:?}"
                    );
                    if (1..=3).contains(&fault) {
                        let family = source_object_private_holder_family_v29(
                            plan, &candidate, place, budget,
                        );
                        assert!(
                            matches!(
                                family,
                                Err(ProductionSemanticKirErrorV1::Unsupported {
                                    detail: "source reference binding belongs to another owner",
                                    ..
                                })
                            ),
                            "{family:?}"
                        );
                    }
                    drop(candidate);
                }
                check_source_scalar_loan_binding_v29(
                    plan, &checked, binding, place, pointer, budget,
                )?;
                assert!(source_object_private_holder_family_v29(
                    plan, binding, place, budget
                )?);
                Ok(())
            })?;
            assert_eq!(budget.storage(), floor);
            let (reads, writes) = SCALAR_STATIC_LOAN_QUERY_COUNTS_V29.get();
            SCALAR_STATIC_LOAN_QUERY_COUNTS_V29.set(match access {
                SourceReferenceAccessV29::Read => (reads + 1, writes),
                SourceReferenceAccessV29::Write => (reads, writes + 1),
                _ => unreachable!(),
            });
        }
    }
    Ok(())
}

#[test]
fn original_scalar_static_holder_read_and_store_queries_reject_foreign_archive_components() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_scalar_static_loan_queries_v29)),
    );
    SCALAR_STATIC_LOAN_QUERY_COUNTS_V29.set((0, 0));
    let result = run_scalar_static_loans_v29(true, 0, LIMIT, LIMIT);
    assert!(result.0.is_ok() && result.3, "{:?}", result.0);
    assert_eq!(SCALAR_STATIC_LOAN_VISITS_V29.get(), 3);
    let (reads, writes) = SCALAR_STATIC_LOAN_QUERY_COUNTS_V29.get();
    assert!(
        reads >= 12 && writes >= 6,
        "all original repeated helper read/write queries across three emissions: {reads}/{writes}"
    );
}
