use super::*;

thread_local! {
    static SOURCE_INDEX_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SOURCE_INDEX_ASSERTIONS_FINISHED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn source_index_observer(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    _plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let before = budget.storage();
    match SOURCE_INDEX_FAULT.get() {
        0 => {
            let index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
            assert_eq!(
                pending.coordinates.sources.rows.len(),
                instances.instances().len()
            );
            let mut count = 0;
            for ordinal in 0..instances.instances().len() {
                let instance = instances.id_at(ordinal).unwrap();
                if instances.instance_reachable(instance) == Some(false) {
                    assert!(
                        pending
                            .active_instances
                            .sidecar_ordinal(
                                ordinal,
                                instances.instances().len(),
                                &pending.sidecars.rows,
                                budget,
                            )?
                            .is_none()
                    );
                    continue;
                }
                let sidecar = index.sidecar(instance, budget)?;
                assert_eq!(sidecar.source_call_instance, Some(instance));
                for span in &sidecar.statement_operation_spans {
                    let actual = index.statement(
                        SourceReferenceSiteV29 {
                            instance,
                            block: span.semantic_block,
                            statement: Some(span.statement_ordinal as usize),
                        },
                        budget,
                    )?;
                    assert!(std::ptr::eq(actual, span));
                    count += 1;
                }
            }
            assert!(count > 0);
            assert_eq!(count, index.statements.len());
            // Independent field mirrors preserve explicit header premises.
            #[allow(dead_code)]
            struct Header<'a> {
                pending: &'a PendingScopedRootEmissionV29,
                statements: Vec<Statement<'a>>,
                terminators: Vec<Terminator<'a>>,
                emitted: Emitted<'a>,
                storage: usize,
                slot: usize,
                ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
                required: usize,
            }
            #[allow(dead_code)]
            struct Statement<'a> {
                instance: usize,
                block: u32,
                statement: u32,
                span: &'a SemanticKirStatementOperationSpanV1,
            }
            #[allow(dead_code)]
            struct Terminator<'a> {
                instance: usize,
                block: u32,
                span: &'a SemanticKirTerminatorOperationSpanV1,
            }
            #[allow(dead_code)]
            struct Emitted<'a> {
                spans: Vec<EmittedSpan<'a>>,
                anchors: Vec<&'a InstanceCallAnchorV1>,
                blocks: Vec<(u32, &'a BasicBlock)>,
                storage: usize,
            }
            #[allow(dead_code)]
            struct EmittedSpan<'a> {
                span: &'a InstanceMappedSpanV1,
                first: (usize, u32, u32),
                end: (usize, u32, u32),
                subtree_end: (usize, u32, u32),
            }
            let expected = std::mem::size_of::<Header<'_>>()
                + index.statements.capacity() * std::mem::size_of::<Statement<'_>>()
                + index.terminators.capacity() * std::mem::size_of::<Terminator<'_>>()
                + index.emitted.spans.capacity() * std::mem::size_of::<EmittedSpan<'_>>()
                + index.emitted.anchors.capacity() * std::mem::size_of::<&InstanceCallAnchorV1>()
                + index.emitted.blocks.capacity() * std::mem::size_of::<(u32, &BasicBlock)>();
            assert_eq!(index.storage, expected);
            assert_eq!(budget.storage(), before + expected);
            compare_emitted_index_v29(&index.emitted, pending, budget)?;
            index.discard(budget)?;
            assert_eq!(budget.storage(), before);
            check_emitted_index_resources_v29(pending);
            check_emitted_index_scale_v29(pending);
            SOURCE_INDEX_ASSERTIONS_FINISHED.set(true);
            Ok(())
        }
        fault @ 5..=9 => {
            let result = hostile_emitted_index_v29(pending, fault, budget);
            SOURCE_INDEX_ASSERTIONS_FINISHED.set(true);
            result
        }
        fault => {
            let result = match fault {
                1 => {
                    // Reordering sidecars cannot silently replace the retained
                    // original-ID index with their new compact ordinals.
                    assert!(pending.sidecars.rows.len() > 1);
                    pending.sidecars.rows.reverse();
                    let result = SourceAddressSourceIndexV29::new(instances, pending, budget);
                    assert!(result.is_err());
                    let error = result.err().unwrap();
                    pending.sidecars.rows.reverse();
                    Err(error)
                }
                2 => {
                    assert!(pending.sidecars.rows.len() > 1);
                    let original = pending.sidecars.rows[1].source_call_instance;
                    pending.sidecars.rows[1].source_call_instance =
                        pending.sidecars.rows[0].source_call_instance;
                    let result = SourceAddressSourceIndexV29::new(instances, pending, budget);
                    assert!(result.is_err());
                    let error = result.err().unwrap();
                    pending.sidecars.rows[1].source_call_instance = original;
                    Err(error)
                }
                3 => {
                    let ordinal = pending
                        .sidecars
                        .rows
                        .iter()
                        .position(|row| !row.statement_operation_spans.is_empty())
                        .unwrap();
                    let original = pending.sidecars.rows[ordinal]
                        .statement_operation_spans
                        .pop()
                        .unwrap();
                    let result = SourceAddressSourceIndexV29::new(instances, pending, budget);
                    assert!(result.is_err());
                    let error = result.err().unwrap();
                    pending.sidecars.rows[ordinal]
                        .statement_operation_spans
                        .push(original);
                    Err(error)
                }
                4 => {
                    let ordinal = pending
                        .sidecars
                        .rows
                        .iter()
                        .position(|row| !row.statement_operation_spans.is_empty())
                        .unwrap();
                    let original =
                        pending.sidecars.rows[ordinal].statement_operation_spans[0].kernel_ir_block;
                    pending.sidecars.rows[ordinal].statement_operation_spans[0].kernel_ir_block =
                        BlockId(u32::MAX);
                    let result = SourceAddressSourceIndexV29::new(instances, pending, budget);
                    assert!(result.is_err());
                    let error = result.err().unwrap();
                    pending.sidecars.rows[ordinal].statement_operation_spans[0].kernel_ir_block =
                        original;
                    Err(error)
                }
                _ => unreachable!("unknown original source index hostile case"),
            };
            SOURCE_INDEX_ASSERTIONS_FINISHED.set(true);
            // Restored hostile metadata permits genuine error teardown only;
            // the original refusal propagates and no root output is accepted.
            result
        }
    }
}

fn source_index_run(
    fault: u8,
    work: usize,
    storage: usize,
) -> (
    Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1>,
    usize,
    usize,
) {
    source_index_run_groups(fault, 2, work, storage)
}

fn source_index_run_groups(
    fault: u8,
    groups: u32,
    work: usize,
    storage: usize,
) -> (
    Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1>,
    usize,
    usize,
) {
    struct Reset(Option<RootExecutionArchiveObserverV29>);
    impl Drop for Reset {
        fn drop(&mut self) {
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.0);
        }
    }
    SOURCE_INDEX_FAULT.set(fault);
    SOURCE_INDEX_ASSERTIONS_FINISHED.set(false);
    let reset = Reset(ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(source_index_observer)));
    let result = run_lifecycle(
        false,
        Fault::Orchestrated {
            groups,
            limits: ProductionSemanticKirLimitsV1::default(),
            fixture: ScopedFixture::Repeated,
        },
        work,
        storage,
    );
    drop(reset);
    result
}

#[test]
fn raw_census_original_instance_index_borrows_actual_root_spans_without_ordinal_reinterpretation() {
    for fault in [0] {
        let result = source_index_run(fault, 10_000_000, 10_000_000).0;
        assert!(
            SOURCE_INDEX_ASSERTIONS_FINISHED.get(),
            "original source index assertions were caught or skipped"
        );
        result.unwrap();
    }
}

#[test]
fn raw_census_original_instance_index_rejects_duplicate_missing_and_foreign_spans_before_output() {
    for fault in [1, 2, 3, 4] {
        let result = source_index_run(fault, 10_000_000, 10_000_000).0;
        assert!(
            SOURCE_INDEX_ASSERTIONS_FINISHED.get(),
            "hostile source index assertions were caught or skipped"
        );
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
    }
}

#[test]
fn raw_census_original_instance_index_actual_root_exact_and_one_short_resources() {
    let (baseline, work, storage) = source_index_run(0, 10_000_000, 10_000_000);
    assert!(SOURCE_INDEX_ASSERTIONS_FINISHED.get());
    baseline.unwrap();
    assert!(work > 0 && storage > 0);
    let exact = source_index_run(0, work, storage).0;
    assert!(SOURCE_INDEX_ASSERTIONS_FINISHED.get());
    exact.unwrap();
    assert!(matches!(
        source_index_run(0, work - 1, storage).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work { .. }
            )
        )
    ));
    assert!(matches!(
        source_index_run(0, work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage { .. }
            )
        )
    ));
}

fn compare_emitted_index_v29(
    index: &SourceAddressEmittedIndexV29<'_>,
    pending: &PendingScopedRootEmissionV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut removed = 0;
    let mut retained = 0;
    for span in &pending.coordinates.spans.rows {
        let original = span.source.coordinates().2;
        for point in original.first..original.first + original.count {
            let expected = ScopedEmittedPointsV29 {
                coordinates: &pending.coordinates,
                relocation: &pending.slot_relocation,
                budget,
            }
            .emitted_point(span.instance, original.block, point, false)
            .map_err(source_address_point_error_v29)?;
            let actual = index.point(span.instance, original.block, point, budget)?;
            assert_eq!(actual, expected);
            match expected {
                Some((block, operation)) => {
                    budget.charge_work(pending.function.body.as_ref().unwrap().blocks.len())?;
                    let expected = pending
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .find(|row| row.id == block)
                        .unwrap()
                        .operations
                        .get(operation as usize)
                        .unwrap();
                    let actual =
                        index.operation(span.instance, original.block, point as usize, budget)?;
                    assert!(std::ptr::eq(actual, expected));
                    retained += 1;
                }
                None => removed += 1,
            }
        }
    }
    assert!(
        removed > 0 && retained > 0,
        "real repeated calls must exercise tombstones and emitted operations"
    );
    Ok(())
}

// The index is inert metadata, so its independent resource test does not claim
// source admission under this fresh ledger. Production keeps the parent owner.
fn emitted_index_resource_run_v29(
    pending: &PendingScopedRootEmissionV29,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    const FLOOR: usize = 19;
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let index = SourceAddressEmittedIndexV29::new(pending, budget)?;
        let owned = index.storage;
        assert_eq!(budget.storage(), FLOOR + owned);
        for span in &pending.coordinates.spans.rows {
            let original = span.source.coordinates().2;
            if original.count > 0 {
                index.point(span.instance, original.block, original.first, budget)?;
                index.point(
                    span.instance,
                    original.block,
                    original.first + original.count - 1,
                    budget,
                )?;
            }
        }
        drop(index);
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

fn check_emitted_index_resources_v29(pending: &PendingScopedRootEmissionV29) {
    let (result, work, storage) = emitted_index_resource_run_v29(pending, 10_000_000, 10_000_000);
    result.unwrap();
    emitted_index_resource_run_v29(pending, work, storage)
        .0
        .unwrap();
    assert!(matches!(
        emitted_index_resource_run_v29(pending, work - 1, storage).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work { .. }
            )
        )
    ));
    assert!(matches!(
        emitted_index_resource_run_v29(pending, work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage { .. }
            )
        )
    ));
}

fn check_emitted_index_scale_v29(pending: &PendingScopedRootEmissionV29) {
    let template = *pending
        .coordinates
        .spans
        .rows
        .iter()
        .find(|row| row.removed_call.is_none() && row.source.coordinates().2.count > 0)
        .unwrap();
    // Synthetic disjoint locator rows measure the index only. They have no
    // original-source admission authority and never enter a root transaction.
    for count in [16_usize, 64, 256, 4096] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
        with_canonical_call_scratch_v1(&mut budget, |budget| {
            let mut spans = emission_vec_v1(count, budget)?;
            for ordinal in 0..count {
                let mut row = template;
                let first = u32::try_from(ordinal * 2).unwrap();
                match &mut row.source {
                    InstanceSpanSourceV1::Statement(source) => {
                        source.first_operation_ordinal = first;
                        source.operation_count = 1;
                    }
                    InstanceSpanSourceV1::Terminator(source) => {
                        source.first_operation_ordinal = first;
                        source.operation_count = 1;
                    }
                    InstanceSpanSourceV1::Synthetic(source) => {
                        source.first_operation_ordinal = first;
                        source.operation_count = 1;
                    }
                    InstanceSpanSourceV1::InvocationEntry(source) => {
                        source.first_operation_ordinal = first;
                        source.operation_count = 1;
                    }
                }
                spans.push(row);
            }
            let before = budget.work();
            let index = SourceAddressEmittedIndexV29::from_rows(&spans, &[], &[], budget)?;
            for span in &spans {
                let original = span.source.coordinates().2;
                assert!(index.point(span.instance, original.block, original.first, budget)?.is_some());
            }
            let measured = budget.work() - before;
            let height = (usize::BITS - count.leading_zeros()) as usize;
            assert!(measured <= count * (64 + 40 * height),
                "constructor plus all disjoint queries exceeded independent N log N bound: {count}, {measured}");
            drop(index);
            drop(spans);
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

fn hostile_emitted_index_v29(
    pending: &mut PendingScopedRootEmissionV29,
    fault: u8,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let selected = pending
        .coordinates
        .spans
        .rows
        .iter()
        .position(|row| row.removed_call.is_none() && row.source.coordinates().2.count > 0)
        .unwrap();
    let source = pending.coordinates.spans.rows[selected];
    let original = source.source.coordinates().2;
    match fault {
        5 | 6 => {
            let other = if selected == 0 { 1 } else { 0 };
            let saved = pending.coordinates.spans.rows[other];
            let mut duplicate = source;
            if fault == 6 {
                duplicate
                    .segments
                    .iter_mut()
                    .flatten()
                    .find(|row| row.count > 0)
                    .unwrap()
                    .first += 1;
            }
            pending.coordinates.spans.rows[other] = duplicate;
            let result = with_canonical_call_scratch_v1(budget, |budget| {
                let index = SourceAddressEmittedIndexV29::new(pending, budget)?;
                let expected =
                    ScopedEmittedPointsV29::operation_point(source, original.first, None)
                        .map_err(source_address_point_error_v29)?;
                let actual =
                    index.point(source.instance, original.block, original.first, budget)?;
                assert_eq!(actual, expected);
                drop(index);
                Ok(())
            });
            pending.coordinates.spans.rows[other] = saved;
            if fault == 6 {
                assert!(result.is_err());
            }
            result
        }
        7 | 8 => {
            let span = *pending
                .coordinates
                .spans
                .rows
                .iter()
                .find(|row| row.removed_call.is_some())
                .unwrap();
            let call = span.removed_call.unwrap();
            let selected = pending
                .coordinates
                .anchors
                .rows
                .iter()
                .position(|row| {
                    row.instance == call.caller
                        && row.source.semantic_block == call.block
                        && row.removed
                })
                .unwrap();
            let changed = if fault == 7 {
                selected
            } else {
                pending
                    .coordinates
                    .anchors
                    .rows
                    .iter()
                    .enumerate()
                    .find(|(i, row)| *i != selected && row.removed)
                    .unwrap()
                    .0
            };
            let saved = {
                let row = &pending.coordinates.anchors.rows[changed];
                (row.instance, row.source.semantic_block, row.removed)
            };
            let row = &mut pending.coordinates.anchors.rows[changed];
            row.instance = call.caller;
            row.source.semantic_block = call.block;
            row.removed = fault == 8;
            let result = with_canonical_call_scratch_v1(budget, |budget| {
                let index = SourceAddressEmittedIndexV29::new(pending, budget)?;
                let original = span.source.coordinates().2;
                index.point(span.instance, original.block, original.first, budget)?;
                drop(index);
                Ok(())
            });
            let row = &mut pending.coordinates.anchors.rows[changed];
            (row.instance, row.source.semantic_block, row.removed) = saved;
            assert!(result.is_err());
            result
        }
        9 => with_canonical_call_scratch_v1(budget, |budget| {
            let index = SourceAddressEmittedIndexV29::new(pending, budget)?;
            let result = index.point(source.instance, BlockId(u32::MAX), original.first, budget);
            assert!(result.is_err());
            result.map(|_| ())
        }),
        _ => unreachable!(),
    }
}

#[test]
fn raw_census_emitted_index_preserves_agreeing_overlap() {
    source_index_run(5, 10_000_000, 10_000_000).0.unwrap();
    assert!(SOURCE_INDEX_ASSERTIONS_FINISHED.get());
}

#[test]
fn raw_census_emitted_index_refuses_conflicts_missing_points_and_bad_removed_anchors() {
    for fault in [6, 7, 8, 9] {
        assert!(matches!(
            source_index_run(fault, 10_000_000, 10_000_000).0,
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        assert!(SOURCE_INDEX_ASSERTIONS_FINISHED.get());
    }
}

#[test]
fn raw_census_emitted_index_matches_scanned_repeated_helpers_across_launch_domains() {
    for groups in [1, 4, 12] {
        source_index_run_groups(0, groups, 10_000_000, 10_000_000)
            .0
            .unwrap();
        assert!(SOURCE_INDEX_ASSERTIONS_FINISHED.get());
    }
}
