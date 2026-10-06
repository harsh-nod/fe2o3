const SELECTED_MEMORY_LIMIT: usize = 1_000_000_000;

include!("production_source_reference_selection_mixed_v30_tests.rs");

#[test]
fn selected_memory_fixed_receipt_headers_match_an_independent_type_oracle() {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let expected = h::<PendingSourceSelectedAccessV30>()
        + h::<PendingSourceSelectedDescriptorV30>()
        + h::<PendingSourceSelectedLeafV30>()
        + h::<PendingSourceSelectedGuardV30>()
        + h::<PendingSourceSelectedObligationV30>()
        + h::<Vec<PendingSourceSelectedAccessV30>>()
        + h::<Vec<PendingSourceSelectedLeafV30>>()
        + h::<Vec<PendingSourceSelectedGuardV30>>()
        + h::<Vec<PendingSourceSelectedObligationV30>>()
        + h::<Vec<usize>>()
        + h::<Vec<Option<usize>>>()
        + h::<Vec<bool>>()
        + h::<Vec<(usize, BlockId, Option<usize>)>>()
        + h::<(usize, BlockId, Option<usize>)>()
        + h::<SourceSelectedDescriptorGuardV30>()
        + h::<Vec<SourceSelectedDescriptorGuardV30>>()
        + h::<SourceSelectedGuardCacheV30>()
        + h::<Option<Vec<SourceSelectedDescriptorGuardV30>>>()
        + h::<Vec<SourceIssuedGuardV29>>()
        + h::<SourceAddressValueAccessV29>()
        + h::<SourceIssuedRecipeV29>()
        + h::<(SourceIssuedRootTransportV29, PendingSourceIssuedIssuerV29)>()
        + h::<SourceIssuedActualValueV29<'_>>()
        + h::<SourceSelectorDefinitionV29<'_>>()
        + h::<CheckedSourceDescriptorGuardV29<'_>>()
        + h::<Option<CheckedSourceDescriptorGuardV29<'_>>>()
        + h::<SourceReferenceDescriptorUseV29>()
        + h::<SourceReferenceDescriptorGuardUseV29>()
        + h::<&SemanticValueBindingV1>()
        + h::<&Operation>()
        + h::<Option<(BlockId, u32)>>()
        + h::<(ValueId, ScalarType)>()
        + h::<()>()
        + std::mem::size_of::<Result<(), fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1>>();
    assert_eq!(
        source_reference_selection_memory_headers_v30().unwrap(),
        expected
    );
    for short in [false, true] {
        let limit = 17 + expected - usize::from(short);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let result =
            budget.reserve_storage(source_reference_selection_memory_headers_v30().unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == 17 + expected && error.limit() == limit));
            assert_eq!(budget.failed_storage(), Some(17 + expected));
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.storage(), 17);
    }
}

fn run_selected_memory_owner(
    owner: ProductionSemanticSsaOwnerV1,
    work: usize,
    storage: usize,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_owner_v18(
        owner,
        work,
        storage,
        &std::cell::Cell::new(None),
        consume,
    )
}

fn retained_selected_memory<'a>(
    original: &'a ProductionSourceCorrespondenceV18<'_>,
) -> &'a PendingSourceIssuedRolesV29 {
    scoped_raw_admission_v29::issued_role_tests_v29::issued_rows_v18(original)
}

fn selected_memory_storage_oracle(rows: &PendingSourceIssuedRolesV29) -> usize {
    use std::mem::size_of;
    rows.sources.capacity() * size_of::<PendingSourceIssuedSiteV29>()
        + rows.issuers.capacity() * size_of::<PendingSourceIssuedIssuerV29>()
        + rows.accesses.capacity() * size_of::<PendingSourceIssuedAccessV29>()
        + rows.selected.capacity() * size_of::<PendingSourceSelectedAccessV30>()
        + rows
            .selected
            .iter()
            .map(|row| {
                row.selection.nodes.capacity() * size_of::<SourceReferenceSelectionActualNodeV30>()
                    + row.selection.edges.capacity()
                        * size_of::<SourceReferenceSelectionActualEdgeV30>()
                    + row.leaves.capacity() * size_of::<PendingSourceSelectedLeafV30>()
                    + row.guards.capacity() * size_of::<PendingSourceSelectedGuardV30>()
                    + row.obligations.capacity() * size_of::<PendingSourceSelectedObligationV30>()
            })
            .sum::<usize>()
}

fn check_selected_memory_rows(
    original: &ProductionSourceCorrespondenceV18<'_>,
    parallel: bool,
    recurrence: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let rows = retained_selected_memory(original);
    assert_eq!(rows.sources.len(), 2);
    assert_eq!(rows.issuers.len(), 2);
    assert!(rows.accesses.is_empty());
    assert_eq!(
        rows.selected.len(),
        2,
        "the original helper loads and stores"
    );
    assert_eq!(rows.selected.iter().filter(|row| row.writing).count(), 1);
    assert_eq!(
        rows.retained_storage(budget)?,
        selected_memory_storage_oracle(rows)
    );
    for row in &rows.selected {
        assert_eq!(row.instance.index(), 1);
        assert_eq!(row.leaves.len(), 2);
        assert_eq!(row.memory.address_space, AddressSpace::Generic);
        assert_eq!(row.selection.nodes[0].pointer, row.pointer);
        assert!(
            row.leaves
                .iter()
                .all(|leaf| matches!(leaf.origin, PendingSourceSelectedLeafOriginV30::Issued(_)))
        );
        let mut original_incoming = Vec::new();
        let mut has_recurrence = false;
        for (node, actual) in row.selection.nodes.iter().enumerate() {
            if let SourceReferenceSelectionStepV29::Parameter { first, count, .. } =
                actual.original.step
            {
                for edge in &row.selection.edges[first..first + count] {
                    original_incoming.push((edge.source, edge.ordinal));
                    has_recurrence |= edge.original.input == node;
                }
            }
        }
        if parallel {
            assert!(original_incoming.iter().any(|left| {
                original_incoming
                    .iter()
                    .any(|right| left.0 == right.0 && left.1 != right.1)
            }));
        }
        if recurrence {
            assert!(has_recurrence);
        }
        assert!(row.obligations.len() >= 2);
        for obligation in &row.obligations {
            let guard = &row.guards[obligation.guard.expect("every injection is guarded")];
            assert_eq!(guard.leaf, obligation.leaf);
            assert!(
                !obligation.at_access,
                "neither branch-local guard dominates the join"
            );
            match obligation.usage {
                PendingSourceSelectedUseV30::Incoming(edge) => {
                    assert_eq!(row.selection.edges[edge].source, obligation.block);
                }
                PendingSourceSelectedUseV30::Invocation(node) => {
                    assert_eq!(
                        row.selection.nodes[node].invocation.unwrap().source,
                        obligation.block
                    );
                }
                PendingSourceSelectedUseV30::Access => panic!("the root is a real selection"),
            }
        }
    }
    Ok(())
}

pub(super) fn check_distinct_selection_memory() {
    let completed = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        distinct_origin_join_owner(),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            check_selected_memory_rows(original, false, false, budget)?;
            completed.set(true);
            Ok(())
        },
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(completed.get());
}

#[test]
fn selected_memory_diamond_preserves_distinct_roots_parallel_edges_and_recurrence() {
    for (parallel, recurrence) in [(false, false), (true, false), (false, true), (true, true)] {
        let completed = std::cell::Cell::new(false);
        let (result, _, _) = run_selected_memory_owner(
            selection_owner(parallel, recurrence),
            SELECTED_MEMORY_LIMIT,
            SELECTED_MEMORY_LIMIT,
            |original, budget| {
                check_selected_memory_rows(original, parallel, recurrence, budget)?;
                let rows = retained_selected_memory(original);
                assert_ne!(
                    rows.issuers[0].root_parameter,
                    rows.issuers[1].root_parameter
                );
                assert_ne!(rows.issuers[0].root_input, rows.issuers[1].root_input);
                completed.set(true);
                Ok(())
            },
        );
        assert!(
            result.is_ok(),
            "parallel={parallel} recurrence={recurrence}: {result:?}"
        );
        assert!(completed.get());
    }
}

#[test]
fn selected_memory_receipt_equality_rejects_ordered_edge_leaf_guard_and_payload_mutations() {
    let (result, _, _) = run_selected_memory_owner(
        selection_owner(true, true),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            let rows = retained_selected_memory(original);
            check_selected_memory_rows(original, true, true, budget)?;
            for fault in 0..9 {
                let floor = budget.storage();
                let mut copied = PendingSourceIssuedRolesV29::empty();
                copied.selected = copied_selected_rows_v30(&rows.selected, budget)
                    .map_err(source_emission_error_v18)?;
                let row = &mut copied.selected[0];
                assert!(
                    row.matches(&rows.selected[0], budget)
                        .map_err(source_emission_error_v18)?
                );
                match fault {
                    0 => row.selection.edges[0].ordinal += 1,
                    1 => row.selection.edges.swap(0, 1),
                    2 => row.leaves.swap(0, 1),
                    3 => row.guards[0].edge ^= 1,
                    4 => row.obligations[0].guard = None,
                    5 => row.obligations[0].at_access = !row.obligations[0].at_access,
                    6 => row.writing = !row.writing,
                    7 => row.memory.alignment *= 2,
                    8 => row.subject.instance = ProductionCallInstanceIdV1(0),
                    _ => unreachable!(),
                }
                assert!(
                    !row.matches(&rows.selected[0], budget)
                        .map_err(source_emission_error_v18)?,
                    "fault {fault}"
                );
                drop(copied);
                budget.release_storage(budget.storage() - floor)?;
                assert_eq!(budget.storage(), floor);
            }
            Ok(())
        },
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn selected_memory_cannot_enter_the_singleton_final_issued_consumer() {
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        selection_owner(false, false),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            check_selected_memory_rows(original, false, false, budget)?;
            reached.set(true);
            scoped_raw_admission_v29::checked_issued_source_rows_v18(original, 0, budget)?;
            panic!("conditional selection must not become singleton authority");
        },
    );
    assert!(reached.get());
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "selected reference access requires its conditional V30 consumer"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn structural_selection_without_memory_retains_no_selected_access_authority() {
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        full_selection_bind_owner(false),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, _| {
            let rows = retained_selected_memory(original);
            assert_eq!(rows.sources.len(), 2);
            assert!(rows.selected.is_empty());
            assert!(rows.accesses.is_empty());
            reached.set(true);
            Ok(())
        },
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(reached.get());
}

#[test]
fn selected_memory_complete_transaction_has_exact_and_one_short_resource_limits() {
    let run = |work, storage| {
        run_selected_memory_owner(
            selection_owner(true, true),
            work,
            storage,
            |original, budget| check_selected_memory_rows(original, true, true, budget),
        )
    };
    let (positive, work, storage) = run(SELECTED_MEMORY_LIMIT, SELECTED_MEMORY_LIMIT);
    positive.unwrap();
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    for (work_limit, storage_limit, work_cut) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, _, _) = run(work_limit, storage_limit);
        let resource = scoped_raw_admission_v29::issued_role_tests_v29::issued_role_resource_v18(
            result.unwrap_err(),
        );
        match (work_cut, resource) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!((error.actual(), error.limit()), (work, work - 1));
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!((error.actual(), error.limit()), (storage, storage - 1));
            }
            other => panic!("exact selected-memory resource boundary: {other:?}"),
        }
    }
}

#[test]
fn legacy_issued_admission_errors_never_enter_selected_reference_dispatch() {
    for fault in 1..=5 {
        SOURCE_SELECTED_ACCESS_ATTEMPTS_V30.set(0);
        run_original(0);
        assert_eq!(SOURCE_SELECTED_ACCESS_ATTEMPTS_V30.get(), 0);
        run_original(fault);
        assert_eq!(
            SOURCE_SELECTED_ACCESS_ATTEMPTS_V30.get(),
            0,
            "legacy fault {fault}"
        );
        run_original(0);
        assert_eq!(SOURCE_SELECTED_ACCESS_ATTEMPTS_V30.get(), 0);
    }
}

#[test]
fn selected_memory_guard_replay_rejects_false_missing_and_wrong_injection_obligations() {
    let (result, _, _) = run_selected_memory_owner(
        selection_owner(true, true),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            let rows = retained_selected_memory(original);
            let root = original.source.root(0, budget)?.1;
            let function = original.inventory.functions()[root].function;
            for fault in 0..5 {
                let floor = budget.storage();
                let mut copies = copied_selected_rows_v30(&rows.selected, budget)
                    .map_err(source_emission_error_v18)?;
                let actual = SourceIssuedActualV29::from_function(function, budget)
                    .map_err(source_emission_error_v18)?;
                let row = &mut copies[0];
                for obligation in &mut row.obligations {
                    obligation.guard = None;
                    obligation.at_access = false;
                }
                check_source_reference_selection_guards_v30(function, &actual, row, budget)
                    .map_err(source_emission_error_v18)?;
                assert!(
                    row.matches(&rows.selected[0], budget)
                        .map_err(source_emission_error_v18)?
                );
                for obligation in &mut row.obligations {
                    obligation.guard = None;
                    obligation.at_access = false;
                }
                match fault {
                    0 => {
                        let first = row.leaves[0].first_guard;
                        let end = first + row.leaves[0].guard_count;
                        for guard in &mut row.guards[first..end] {
                            guard.edge ^= 1;
                        }
                    }
                    1 => row.guards.clear(),
                    2 => row.obligations[0].block = row.operation.0,
                    3 => row.leaves[0].pointer = row.leaves[1].pointer,
                    4 => {
                        row.obligations.remove(0);
                    }
                    _ => unreachable!(),
                };
                let error =
                    check_source_reference_selection_guards_v30(function, &actual, row, budget)
                        .unwrap_err();
                assert!(
                    matches!(
                        error,
                        ProductionSemanticKirErrorV1::Unsupported {
                            function: 0,
                            block: None,
                            statement: None,
                            detail: "selected reference access differs from its conditional source obligations",
                        }
                    ),
                    "fault {fault}: {error:?}"
                );
                drop((copies, actual));
                budget.release_storage(budget.storage() - floor)?;
                assert_eq!(budget.storage(), floor);
            }
            Ok(())
        },
    );
    assert!(result.is_ok(), "{result:?}");
}

thread_local! {
    static SELECTED_MEMORY_ALIGNMENT_CALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn corrupt_selected_memory_alignment(
    pending: &mut PendingScopedRootEmissionV29,
    _: &ExecutionInstancesV29<'_>,
    _: &SourceReferencePlanV29<'_, '_>,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut loads = 0;
    for block in &mut pending.function.body.as_mut().unwrap().blocks {
        for operation in &mut block.operations {
            if let OperationKind::Load { access, .. } = &mut operation.kind {
                assert_eq!(access.alignment, 4);
                access.alignment = 8;
                loads += 1;
            }
        }
    }
    assert_eq!(loads, 1, "the actual selected helper load");
    SELECTED_MEMORY_ALIGNMENT_CALLBACKS.set(SELECTED_MEMORY_ALIGNMENT_CALLBACKS.get() + 1);
    Ok(())
}

fn install_selected_memory_alignment_observer(
    _: &ExecutionInstancesV29<'_>,
    _: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    _: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(Some(corrupt_selected_memory_alignment));
    Ok(())
}

#[test]
fn selected_memory_actual_helper_alignment_must_match_the_original_scalar_layout() {
    check_distinct_selection_memory();
    struct Restore(
        Option<ScopedSlotCustodyObserverV29>,
        Option<RootExecutionArchiveObserverV29>,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.1);
        }
    }
    {
        let _restore = Restore(
            SCOPED_SLOT_CUSTODY_OBSERVER_V29
                .replace(Some(install_selected_memory_alignment_observer)),
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.get(),
        );
        SELECTED_MEMORY_ALIGNMENT_CALLBACKS.set(0);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_selected_memory_owner(
            distinct_origin_join_owner(),
            SELECTED_MEMORY_LIMIT,
            SELECTED_MEMORY_LIMIT,
            |_, _| {
                reached.set(true);
                Ok(())
            },
        );
        assert!(!reached.get());
        assert_eq!(SELECTED_MEMORY_ALIGNMENT_CALLBACKS.get(), 1);
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            function: 0,
                            block: None,
                            statement: None,
                            detail: "selected reference access differs from its conditional source obligations",
                        }
                    )
                ))
            ),
            "{result:?}"
        );
    }
    check_distinct_selection_memory();
}

#[test]
fn selected_memory_single_origin_entry_loop_keeps_the_existing_issuer_route() {
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        entry_loop_selection_owner(),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, _| {
            let rows = retained_selected_memory(original);
            assert_eq!(
                (rows.sources.len(), rows.issuers.len(), rows.accesses.len()),
                (1, 1, 1)
            );
            assert!(rows.selected.is_empty());
            assert_ne!(rows.accesses[0].instance, rows.issuers[0].instance);
            assert_eq!(rows.accesses[0].issuer_instance, rows.issuers[0].instance);
            assert_eq!(rows.accesses[0].issuer, rows.issuers[0].definition);
            reached.set(true);
            Ok(())
        },
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(reached.get());
}

#[test]
fn selected_memory_foreign_ledger_is_rejected_before_receipt_access_and_retains_poison() {
    let retained = std::cell::Cell::new(None);
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_owner_v18(
        selection_owner(false, false),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        &retained,
        |original, budget| {
            check_selected_memory_rows(original, false, false, budget)?;
            let floor = budget.storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SELECTED_MEMORY_LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut work, SELECTED_MEMORY_LIMIT);
            foreign.reserve_storage(floor)?;
            let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
            let called = std::cell::Cell::new(false);
            let error = scoped_raw_admission_v29::with_checked_source_memory_v29(
                original,
                0,
                None,
                &mut foreign,
                |_, _| {
                    called.set(true);
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(
                matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ),
                "{error:?}"
            );
            assert!(!called.get());
            assert_eq!(
                (foreign.work(), foreign.storage(), foreign.peak_storage()),
                before
            );
            assert_eq!(budget.storage(), floor);
            let work = budget.work();
            assert!(matches!(
                original.query(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(budget.work(), work);
            retained.set(Some(floor));
            reached.set(true);
            Ok(())
        },
    );
    assert!(reached.get());
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ),
        "{result:?}"
    );
    assert!(retained.get().unwrap() > 37);
}
