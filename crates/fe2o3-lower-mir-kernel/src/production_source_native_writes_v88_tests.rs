pub(super) fn test_source_native_writes_v88(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    count: usize,
    receiver: usize,
    mode: u8,
    reached: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    struct Capture<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let floor = budget.storage();
    let denied_floor = &std::cell::Cell::new(None);
    let result = source_scalar_normalization_scratch_v18(
        original.source.cleanup,
        budget,
        0,
        |budget| {
            let output = optimized.output_inventory(budget)?;
            let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
            let metadata_storage = metadata
                .storage_extent(budget)
                .map_err(slice_completion_ranked_error_v25)?;
            budget.reserve_storage(metadata_storage)?;
            let (candidate, receipt) =
                build_canonical_ranked_candidate_v18(output, &metadata, budget)
                    .map_err(slice_completion_ranked_error_v25)?;
            budget.reserve_storage(receipt.retained_storage())?;
            let layouts = original.source.limits(budget)?.storage_layout_limits();
            with_checked_canonical_ranked_view_v18(output, &metadata, &candidate, budget, |checked, budget| {
            Ok::<_, CanonicalRankedViewErrorV1>(
                fe2o3_pliron::with_pending_canonical_ranked_source_roles_v18(checked, layouts, budget, |pending, budget| {
                    let mut selected = None;
                    let callback = |native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>, budget: &mut ArgumentBudgetV1<'_>| {
                        let inner_floor = budget.storage();
                        let dropped = std::cell::Cell::new(0);
                        let capture = Capture(&dropped);
                        if mode == 1 { reached.set(true); }
                        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            original.with_pending_source_native_writes_v88(optimized, 0, native, budget, move |view, budget| {
                                let _capture = capture;
                                assert_eq!(view.root, 0);
                                assert_eq!(view.original_call_count(budget)?, count);
                                for ordinal in 0..count {
                                    let endpoint = view.write(ordinal, budget)?.unwrap();
                                    assert_eq!(endpoint.fact.source.root_parameter, receiver);
                                    assert_eq!(endpoint.fact.source.element, ScalarType::U32);
                                    assert_eq!(endpoint.fact.tail.predicate, endpoint.fact.tail.extent);
                                    assert_eq!(endpoint.guard.wire_v86().edge(), None);
                                    assert!(endpoint.guard.require_cfg_v26().is_err());
                                    assert!(!endpoint.grants_memory_or_launch_authority());
                                    assert!(endpoint.requires_address_formation_domain());
                                    let store = source_operation_row_v18(output, endpoint.fact.output[6], budget)?.operation;
                                    assert!(matches!(store.kind, OperationKind::GuardedStore { pointer, predicate, value, .. }
                                        if pointer == endpoint.fact.tail.pointer && predicate == endpoint.fact.tail.predicate && value == endpoint.fact.value));
                                }
                                reached.set(true);
                                match mode {
                                    0 => Ok(()),
                                    2 => view.write(count, budget).map(|_| ()),
                                    3 => Err(ProductionSourceOwnedViewErrorV18::Binding("V88 callback refusal")),
                                    4 => std::panic::panic_any(880u32),
                                    5 | 6 => {
                                        let before = (budget.work(), budget.storage());
                                        let error = if mode == 5 {
                                            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                                            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
                                            foreign.reserve_storage(view.required).unwrap();
                                            let error = view.check(&mut foreign).unwrap_err();
                                            assert!(matches!(view.check(&mut foreign), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                                            assert_eq!((foreign.work(), foreign.storage()), (0, view.required));
                                            assert_eq!((budget.work(), budget.storage()), before);
                                            error
                                        } else {
                                            budget.release_storage(1).unwrap();
                                            let error = view.check(budget).unwrap_err();
                                            assert!(matches!(view.check(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                                            assert_eq!((budget.work(), budget.storage()), (before.0, before.1 - 1));
                                            error
                                        };
                                        assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)));
                                        assert!(original.source.cleanup.refund_denied());
                                        denied_floor.set(Some(budget.storage()));
                                        Err(error)
                                    }
                                    16..=23 => {
                                        let mut fact = *view.write(0, budget)?.unwrap().fact;
                                        match mode {
                                            16 => fact.receiver = fact.index,
                                            17 => fact.index = fact.receiver,
                                            18 => fact.value = fact.receiver,
                                            19 => fact.tail.predicate = fact.tail.zero,
                                            20 => fact.tail.extent = fact.tail.length,
                                            21 => fact.tail.pointer = fact.tail.data,
                                            22 => fact.output[6] = fact.output[5],
                                            23 => fact.output[3] = fact.output[2],
                                            _ => unreachable!(),
                                        }
                                        check_source_native_write_v88(original, optimized, native, &fact, budget)
                                    }
                                    _ => panic!("unexpected V88 test mode"),
                                }
                            })
                        }));
                        assert_eq!(dropped.get(), 1);
                        assert_eq!(budget.storage(), denied_floor.get().unwrap_or(inner_floor));
                        selected = Some(match caught {
                            Ok(value) => value,
                            Err(payload) => {
                                assert_eq!(mode, 4); assert_eq!(*payload.downcast::<u32>().unwrap(), 880);
                                Err(ProductionSourceOwnedViewErrorV18::Binding("V88 observed callback unwind"))
                            }
                        });
                        Ok(())
                    };
                    if mode == 1 { pending.with_pending_global_accesses_v18(output.owner(), budget, callback)?; }
                    else { pending.with_pending_global_accesses_v87(output.owner(), budget, callback)?; }
                    Ok(selected.expect("native callback executed"))
                }).map_err(slice_entry_native_error_v25).and_then(|value| value)
            )
        }).map_err(slice_completion_ranked_error_v25)??;
            drop(candidate);
            drop(metadata);
            Ok(())
        },
    );
    assert_eq!(budget.storage(), denied_floor.get().unwrap_or(floor));
    result
}

#[test]
fn source_native_writes_v88_headers_precede_work_and_include_live_borrows() {
    // Keep the complete live-frame equation independent of the production
    // function so omitted callback/result/iterator envelopes are detected.
    type OracleFrame<'a> = (
        PendingSourceNativeWritesV88<'a, 'a>,
        PendingSourceNativeWriteV88<'a>,
        Vec<Option<OptimizedSourceWriteV87>>,
        Option<OptimizedSourceWriteV87>,
        [&'a (); 20],
        [usize; 12],
        [bool; 4],
        [SliceOperation; 7],
        [SliceDefinition; 2],
        [ValueId; 7],
        [&'a Operation; 7],
        [&'a CanonicalKirDefinitionRefV1<'a>; 2],
        [(&'a Operation, &'a ValueDef); 2],
        Option<[(&'a Operation, &'a ValueDef); 2]>,
        Result<
            Option<[(&'a Operation, &'a ValueDef); 2]>,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
        SourceOwnedResultV18<Vec<Option<OptimizedSourceWriteV87>>>,
        SourceOwnedResultV18<Option<PendingSourceNativeWriteV88<'a>>>,
        SourceOwnedResultV18<GlobalSourceGuardV85>,
        SourceOwnedResultV18<()>,
        std::thread::Result<SourceOwnedResultV18<()>>,
        std::slice::Iter<'a, Option<OptimizedSourceWriteV87>>,
        std::array::IntoIter<SliceOperation, 4>,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    );
    let base = optimized_write_headers_v87().unwrap();
    let headers = source_native_write_headers_v88(0, 1).unwrap();
    assert_eq!(
        size_of::<PendingSourceNativeWritesV88<'_, '_>>(),
        3 * size_of::<&()>()
            + size_of::<&[Option<OptimizedSourceWriteV87>]>()
            + 3 * size_of::<usize>()
            + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
    );
    assert_eq!(
        headers,
        base + size_of::<OracleFrame<'_>>()
            + std::mem::align_of::<OracleFrame<'_>>()
            + 2 * size_of::<SourceOwnedResultV18<OracleFrame<'_>>>()
            + 1
    );
    assert_eq!(
        source_native_write_headers_v88(29, 8).unwrap() - headers,
        3 * 29 + 7
    );
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, headers + 17 - usize::from(short));
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(headers);
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
            if error.actual() == headers + 17 && error.limit() == headers + 16));
        } else {
            result.unwrap();
            budget.release_storage(headers).unwrap();
        }
        assert_eq!((budget.work(), budget.storage()), (0, 17));
    }
}
