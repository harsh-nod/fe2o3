//! Genuine source-owner components. Unsigned fixtures cannot make a full worker.
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::fixtures;
use crate::production_ranked_projection_v1::{
    with_backend_forwarding_erased_prefix_v1, with_backend_licm_direct_prefix_v1,
};
use fe2o3_kernel_analysis::CanonicalKirInductionRefinementOriginV1 as Origin;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::error::Error as _;

fn with_native(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(
        PreparedRefinedForwardingNativeOutputV1,
        PreparedRefinedForwardingHistoryClaimsV1,
        Ranked,
        &mut Budget<'_>,
    ),
) {
    let run = |prefix, ranked, budget: &mut Budget<'_>| {
        let floor = budget.storage();
        let (native, receipt) = super::super::prepare(
            prefix,
            profile,
            Limits::default(),
            ForwardingLimits::default(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let history = PreparedRefinedForwardingHistoryClaimsV1::prepare(&native, budget).unwrap();
        let history_storage = history.retained_storage();
        budget.reserve_storage(history_storage).unwrap();
        next(native, history, ranked, budget);
        budget
            .release_storage(history_storage + receipt.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    };
    if erased {
        with_backend_forwarding_erased_prefix_v1(profile, mutation, |owner, ranked, budget| {
            run(Prefix6::Erased(owner), ranked, budget)
        });
    } else {
        with_backend_licm_direct_prefix_v1(profile, mutation, |owner, ranked, budget| {
            run(Prefix6::Direct(owner), ranked, budget)
        });
    }
}
fn with_catalog<T>(
    native: &PreparedRefinedForwardingNativeOutputV1,
    budget: &mut Budget<'_>,
    next: impl FnOnce(&Catalog, &mut Budget<'_>) -> T,
) -> T {
    let semantic = match &native.owner {
        Composed::Direct(v) => *v
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .source_semantic_kir()
            .semantic()
            .semantic()
            .semantic_sha256()
            .as_bytes(),
        Composed::Erased(v) => *v
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .original_source()
            .semantic_ssa()
            .source_semantic()
            .semantic_sha256()
            .as_bytes(),
    };
    let (catalog, receipt) = Catalog::from_rows_with_budget(semantic, &[], &[], budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let value = next(&catalog, budget);
    drop(catalog);
    budget.release_storage(receipt.retained_storage()).unwrap();
    value
}
fn shape(native: &PreparedRefinedForwardingNativeOutputV1, mutation: bool) {
    assert_eq!(
        native
            .refinement_origins()
            .iter()
            .filter(|row| matches!(row.canonical_origin(), Origin::CheckedAddSplit { .. }))
            .count(),
        if mutation { 2 } else { 0 }
    );
    assert_eq!(
        native
            .origins()
            .iter()
            .filter(|row| row.canonical_origin().store.is_some())
            .count(),
        if mutation { 2 } else { 0 }
    );
    for row in native
        .origins()
        .iter()
        .filter(|row| row.canonical_origin().store.is_some())
    {
        assert!(row.original_source_statement().is_some());
        assert_ne!(
            row.canonical_origin().input.block,
            row.canonical_origin().store.unwrap().block
        );
    }
    assert_eq!(native.output().module().kernels.len(), 2);
    assert!(!native.grants_artifact_or_launch_authority());
    if mutation {
        assert_ne!(
            native.licm_input().canonical().canonical_bytes(),
            native.refinement_output().canonical().canonical_bytes()
        );
        assert_ne!(
            native.refinement_output().canonical().canonical_bytes(),
            native.output().canonical().canonical_bytes()
        );
    }
}
fn components(mutation: bool) {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_native(
                erased,
                profile,
                mutation,
                |native, history, ranked, budget| {
                    shape(&native, mutation);
                    let floor = budget.storage();
                    let checked = history.check(&native, budget).unwrap();
                    let storage = checked.retained_storage();
                    budget.reserve_storage(storage).unwrap();
                    let (_, _, semantic, actual) = checked.observed();
                    assert!(std::ptr::eq(semantic, native.output()));
                    assert!(std::ptr::eq(actual, native.output()));
                    drop(checked);
                    budget.release_storage(storage).unwrap();
                    assert_eq!(ranked.root_count(), 2);
                    with_catalog(&native, budget, |catalog, budget| {
                        let roots = fixtures::typed_roots(final_owner(&native));
                        fixtures::assert_final_subject(final_owner(&native));
                        let (prepared, receipt) =
                            prepare_handoff(&native, catalog, &roots, None, budget).unwrap();
                        budget.reserve_storage(receipt).unwrap();
                        assembly::tests::assert_bound_output(
                            final_owner(&native),
                            catalog,
                            profile,
                            &prepared,
                            budget,
                        );
                        let (rows, storage) = formal_rows(&native, budget).unwrap();
                        budget.reserve_storage(storage).unwrap();
                        assert_eq!(rows.len(), 2);
                        for (bytes, report) in rows.iter().zip(reports(&native)) {
                            assert_eq!(
                                bytes,
                                &Formal::from_current_obligations(report)
                                    .unwrap()
                                    .into_canonical_bytes()
                            );
                        }
                        drop(rows);
                        budget.release_storage(storage).unwrap();
                        drop(prepared);
                        budget.release_storage(receipt).unwrap();
                    });
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}

#[test]
fn final_worker_components_replay_nonempty_direct_and_unit_local_f() {
    components(true);
}
#[test]
fn final_worker_components_preserve_real_noop_routes() {
    components(false);
}

#[test]
fn final_worker_actual_sources_require_signed_ranked_receipts() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_native(erased, profile, true, |native, _history, ranked, budget| {
                shape(&native, true);
                assert!(
                    ranked
                        .roots()
                        .iter()
                        .all(|root| root.verification().aggregate_verus_execution().is_none())
                );
                let expected = ranked.roots()[0].semantic_root().index();
                let floor = budget.storage();
                let error = match prepare_source(&native, ranked, budget) {
                    Ok(_) => panic!("unsigned fixture cannot manufacture signed source custody"),
                    Err(error) => error,
                };
                assert!(matches!(worker_error(&error), E::Source(error)
                    if matches!(**error, NativeSourceLineageErrorV1::MissingSignedRankedReceipt { root }
                        if root == expected)));
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
fn worker_error(error: &ProductionPipelineError) -> &E {
    match error {
        ProductionPipelineError::InductionRefinementNativeStage(
            InductionRefinementNativeStageErrorV1::ForwardingComposition(
                RefinedForwardingNativeStageErrorV1::Worker(error),
            ),
        ) => error,
        _ => panic!("exact typed final-worker error: {error}"),
    }
}

#[test]
fn final_worker_source_preparation_preserves_first_work_refusal() {
    for erased in [false, true] {
        with_native(
            erased,
            Profile::Gfx942,
            true,
            |native, _history, ranked, parent| {
                let mut work = Work::new(22);
                {
                    let mut budget = Budget::new(&mut work, usize::MAX);
                    budget.reserve_storage(parent.storage()).unwrap();
                    budget.charge_work(17).unwrap();
                    let error = match prepare_source(&native, ranked, &mut budget) {
                        Ok(_) => panic!("six-unit source preflight must refuse"),
                        Err(error) => error,
                    };
                    assert!(matches!(worker_error(&error), E::Source(error)
                    if matches!(**error, NativeSourceLineageErrorV1::Resource(Resource::Work(limit))
                        if limit.actual() == 23 && limit.limit() == 22)));
                    assert_eq!(budget.work(), 17);
                    assert_eq!(budget.storage(), parent.storage());
                    assert_eq!(budget.failed_storage(), None);
                }
                assert_eq!(work.failed_work(), Some(23));
            },
        );
    }
}

#[test]
fn final_worker_descriptor_rejects_each_original_root_abi_launch_mutation() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_native(
                erased,
                profile,
                true,
                |native, _history, _ranked, budget| {
                    with_catalog(&native, budget, |catalog, budget| {
                        for case in 0..8 {
                            let mut roots = fixtures::typed_roots(final_owner(&native));
                            let expected = fixtures::hostile(&mut roots, case);
                            let floor = budget.storage();
                            let error =
                                match prepare_handoff(&native, catalog, &roots, None, budget) {
                                    Ok(_) => panic!("hostile typed descriptor accepted"),
                                    Err(error) => error,
                                };
                            assert!(matches!(worker_error(&error), E::Assembly(error)
                            if matches!(error.as_ref(), FinalWorkerAssemblyErrorV1::Descriptor(error)
                                if matches!(error.as_ref(), RefinedForwardingDescriptorErrorV1::Descriptor(error)
                                    if matches!(error.as_ref(), crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(detail)
                                        if *detail == expected)))));
                            assert_eq!(budget.storage(), floor);
                        }
                    });
                },
            );
        }
    }
}

#[test]
fn final_worker_replay_rejects_changed_digest_profile_and_text() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_native(
                erased,
                profile,
                true,
                |native, _history, _ranked, budget| {
                    with_catalog(&native, budget, |catalog, budget| {
                        let roots = fixtures::typed_roots(final_owner(&native));
                        let (mut prepared, receipt) =
                            prepare_handoff(&native, catalog, &roots, None, budget).unwrap();
                        budget.reserve_storage(receipt).unwrap();
                        assembly::tests::assert_changed_digest_and_profile_refuse(
                            final_owner(&native),
                            catalog,
                            profile,
                            &mut prepared,
                            budget,
                        );
                        assembly::tests::assert_changed_text_refuses(
                            final_owner(&native),
                            catalog,
                            profile,
                            &mut prepared,
                            budget,
                        );
                        drop(prepared);
                        budget.release_storage(receipt).unwrap();
                    });
                },
            );
        }
    }
}

struct Measured {
    error: Option<ProductionPipelineError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(
    native: &PreparedRefinedForwardingNativeOutputV1,
    catalog: &Catalog,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Measured {
    let roots = fixtures::typed_roots(final_owner(native));
    let mut work = Work::new(work_limit);
    let (error, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error = match prepare_handoff(native, catalog, &roots, None, &mut budget) {
            Ok((value, _)) => {
                drop(value);
                None
            }
            Err(error) => Some(error),
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            error,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Measured {
        error,
        work: accepted,
        peak,
        failed_storage,
        failed_work: work.failed_work(),
    }
}

#[test]
fn final_worker_component_exact_work_peak_and_terminal_work_short() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_native(
                    erased,
                    profile,
                    mutation,
                    |native, _history, _ranked, parent| {
                        with_catalog(&native, parent, |catalog, parent| {
                            let sibling = vec![0x31u8; 31];
                            let floor =
                                parent.storage() + size_of::<Vec<u8>>() + sibling.capacity();
                            let observed = measure(&native, catalog, floor, usize::MAX, usize::MAX);
                            assert!(observed.error.is_none());
                            assert_eq!(
                                (observed.failed_work, observed.failed_storage),
                                (None, None)
                            );
                            let exact =
                                measure(&native, catalog, floor, observed.work, observed.peak);
                            assert!(exact.error.is_none());
                            assert_eq!((exact.work, exact.peak), (observed.work, observed.peak));
                            let short =
                                measure(&native, catalog, floor, observed.work - 1, observed.peak);
                            assert!(
                                matches!(worker_error(short.error.as_ref().unwrap()), E::Resource(Resource::Work(error))
                            if error.actual() == observed.work && error.limit() == observed.work - 1)
                            );
                            assert_eq!(short.work, observed.work - 1);
                            assert_eq!(short.failed_work, Some(observed.work));
                            assert_eq!(short.failed_storage, None);
                            assert_eq!(sibling, [0x31; 31]);
                        });
                    },
                );
            }
        }
    }
}

#[test]
fn final_worker_first_engine_reservation_has_exact_public_extent() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_native(
                erased,
                profile,
                true,
                |native, _history, _ranked, parent| {
                    with_catalog(&native, parent, |catalog, parent| {
                        let floor = parent.storage();
                        let expected = 3 * dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
                            + 2 * fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES
                            + fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V2
                            + size_of::<Handoff>()
                            + size_of::<FinalWorkerInputsV1<'static>>()
                            + size_of::<String>();
                        let observed =
                            measure(&native, catalog, floor, usize::MAX, floor + expected - 1);
                        assert!(matches!(worker_error(observed.error.as_ref().unwrap()),
                        E::Resource(Resource::Storage(error))
                            if error.actual() == floor + expected && error.limit() == floor + expected - 1));
                        assert_eq!(observed.work, 20);
                        assert_eq!(observed.peak, floor);
                        assert_eq!(observed.failed_storage, Some(floor + expected));
                        assert_eq!(observed.failed_work, None);
                    });
                },
            );
        }
    }
}

#[test]
fn final_worker_formal_receipt_pays_actual_vec_backing_and_replays() {
    for erased in [false, true] {
        with_native(
            erased,
            Profile::Gfx942,
            true,
            |native, _history, _ranked, budget| {
                let floor = budget.storage();
                let (rows, retained) = formal_rows(&native, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(retained).unwrap();
                assert_eq!(
                    retained,
                    size_of::<Vec<Vec<u8>>>()
                        + rows.capacity() * size_of::<Vec<u8>>()
                        + rows.iter().map(Vec::capacity).sum::<usize>()
                );
                let (fresh, storage) = formal_rows(&native, budget).unwrap();
                budget.reserve_storage(storage).unwrap();
                assert_eq!(rows, fresh);
                drop(fresh);
                budget.release_storage(storage).unwrap();
                drop(rows);
                budget.release_storage(retained).unwrap();
                assert_eq!(budget.storage(), floor);
            },
        );
    }
}

#[test]
fn final_worker_formal_replay_rejects_missing_permuted_and_changed_rows() {
    for erased in [false, true] {
        with_native(
            erased,
            Profile::Gfx942,
            true,
            |native, _history, _ranked, budget| {
                let (mut rows, storage) = formal_rows(&native, budget).unwrap();
                budget.reserve_storage(storage).unwrap();
                let floor = budget.storage();
                let missing = check_formal_rows(&native, &rows[..1], budget).unwrap_err();
                assert!(matches!(
                    worker_error(&missing),
                    E::Mismatch("complete final F formal payloads")
                ));
                rows.swap(0, 1);
                let permuted = check_formal_rows(&native, &rows, budget);
                rows.swap(0, 1);
                assert!(matches!(
                    worker_error(&permuted.unwrap_err()),
                    E::Mismatch("fresh final F formal payload")
                ));
                rows[0][0] ^= 1;
                let changed = check_formal_rows(&native, &rows, budget);
                rows[0][0] ^= 1;
                assert!(matches!(
                    worker_error(&changed.unwrap_err()),
                    E::Mismatch("fresh final F formal payload")
                ));
                check_formal_rows(&native, &rows, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                drop(rows);
                budget.release_storage(storage).unwrap();
            },
        );
    }
}

#[test]
fn final_worker_moved_ranked_header_is_credited_only_once() {
    for erased in [false, true] {
        with_native(
            erased,
            Profile::Gfx942,
            false,
            |native, _history, _ranked, _budget| {
                assert_eq!(
                    wrapper_header(&native).unwrap()
                        + size_of::<RefinedForwardingNativeProductionCompilationV1>()
                        + source_header(&native)
                        - size_of::<Ranked>()
                        + size_of::<Handoff>()
                        + size_of::<Vec<Vec<u8>>>(),
                    size_of::<PreparedRefinedForwardingWorkerHandoffV1>()
                );
            },
        );
    }
}

#[test]
fn final_worker_complete_owner_remains_neither_clone_nor_copy() {
    trait CloneAmbiguity<A> {
        fn check() {}
    }
    impl<T: ?Sized> CloneAmbiguity<()> for T {}
    impl<T: ?Sized + Clone> CloneAmbiguity<u8> for T {}
    trait CopyAmbiguity<A> {
        fn check() {}
    }
    impl<T: ?Sized> CopyAmbiguity<()> for T {}
    impl<T: ?Sized + Copy> CopyAmbiguity<u8> for T {}
    let _ = <PreparedRefinedForwardingWorkerHandoffV1 as CloneAmbiguity<_>>::check;
    let _ = <PreparedRefinedForwardingWorkerHandoffV1 as CopyAmbiguity<_>>::check;
}

#[test]
fn final_worker_nested_source_error_retains_its_source_chain() {
    let error = worker(E::Source(Box::new(
        NativeSourceLineageErrorV1::MissingSignedRankedReceipt { root: 7 },
    )));
    let E::Source(source) = worker_error(&error) else {
        panic!("source variant")
    };
    assert!(matches!(
        **source,
        NativeSourceLineageErrorV1::MissingSignedRankedReceipt { root: 7 }
    ));
    assert!(
        worker_error(&error)
            .source()
            .unwrap()
            .is::<NativeSourceLineageErrorV1>()
    );
}

#[test]
fn final_worker_reuses_scope_drop_before_refund_on_unwind() {
    use std::cell::Cell;
    struct Probe<'a>(&'a Cell<bool>);
    impl Drop for Probe<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = Cell::new(false);
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1024);
    budget.reserve_storage(41).unwrap();
    let error = scoped::<()>(41, &mut budget, |budget| {
        budget
            .reserve_storage(size_of::<Probe<'_>>())
            .map_err(resource)?;
        let _probe = Probe(&dropped);
        panic!("typed owning-scope unwind control");
    })
    .unwrap_err();
    assert!(matches!(error, ProductionPipelineError::CheckedOutputPolicy7Stage(
        crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1::Panicked)));
    assert!(dropped.get());
    assert_eq!(budget.storage(), 41);
}

#[test]
fn final_worker_component_refuses_a_missing_native_floor_before_work() {
    for erased in [false, true] {
        with_native(
            erased,
            Profile::Gfx942,
            true,
            |native, _history, _ranked, parent| {
                with_catalog(&native, parent, |catalog, _parent| {
                    let floor = native.retained_floor - 1;
                    let observed = measure(&native, catalog, floor, usize::MAX, usize::MAX);
                    assert!(matches!(observed.error, Some(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)))));
                    assert_eq!((observed.work, observed.peak), (17, floor));
                    assert_eq!(
                        (observed.failed_work, observed.failed_storage),
                        (None, None)
                    );
                });
            },
        );
    }
}

#[test]
fn final_worker_formal_header_and_row_backing_refuse_before_allocation() {
    for erased in [false, true] {
        with_native(
            erased,
            Profile::Gfx942,
            true,
            |native, _history, _ranked, parent| {
                for row_backing in [false, true] {
                    let floor = parent.storage();
                    let header = size_of::<Vec<Vec<u8>>>();
                    let backing = reports(&native).len() * size_of::<Vec<u8>>();
                    let attempted = floor + header + if row_backing { backing } else { 0 };
                    let mut work = Work::new(1_000_000);
                    let mut budget = Budget::new(&mut work, attempted - 1);
                    budget.reserve_storage(floor).unwrap();
                    budget.charge_work(17).unwrap();
                    let error = match formal_rows(&native, &mut budget) {
                        Ok(_) => panic!("prepaid formal allocation must refuse"),
                        Err(error) => error,
                    };
                    assert!(
                        matches!(worker_error(&error), E::Resource(Resource::Storage(limit))
                    if limit.actual() == attempted && limit.limit() == attempted - 1)
                    );
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.work(), 17);
                    assert_eq!(
                        budget.peak_storage(),
                        floor + if row_backing { header } else { 0 }
                    );
                    assert_eq!(budget.failed_storage(), Some(attempted));
                }
            },
        );
    }
}

#[test]
fn final_worker_new_text_copy_refuses_before_requested_backing() {
    let mut work = Work::new(1000);
    let text = "actual final F text copy";
    let floor = 41 + size_of::<String>();
    let mut budget = Budget::new(&mut work, floor + text.len() - 1);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(17).unwrap();
    let error = scoped(floor, &mut budget, |budget| copy_text(text, budget)).unwrap_err();
    assert!(
        matches!(worker_error(&error), E::Resource(Resource::Storage(limit))
        if limit.actual() == floor + text.len() && limit.limit() == floor + text.len() - 1)
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor);
    assert_eq!(budget.work(), 17 + text.len() + 1);
    assert_eq!(budget.failed_storage(), Some(floor + text.len()));
}

#[test]
fn final_worker_reuses_mid_operation_ledger_swap_refusal() {
    let mut a = Work::new(1000);
    let mut b = Work::new(1000);
    let mut original = Budget::new(&mut a, 1024);
    let mut foreign = Budget::new(&mut b, 1024);
    original.reserve_storage(41).unwrap();
    original.charge_work(17).unwrap();
    foreign.reserve_storage(41).unwrap();
    foreign.charge_work(5).unwrap();
    let result = scoped(41, &mut original, |budget| {
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    std::mem::swap(&mut original, &mut foreign);
    assert!(matches!(result, Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
        crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1::Resource(Resource::Accounting)))));
    assert_eq!((original.storage(), original.work()), (41, 17));
    assert_eq!((foreign.storage(), foreign.work()), (41, 5));
}

#[test]
fn final_worker_component_preserves_earlier_work_and_storage_denials() {
    with_native(
        false,
        Profile::Gfx942,
        true,
        |native, _history, _ranked, parent| {
            with_catalog(&native, parent, |catalog, parent| {
                let roots = fixtures::typed_roots(final_owner(&native));
                let floor = parent.storage();
                let mut work = Work::new(17);
                {
                    let mut budget = Budget::new(&mut work, usize::MAX);
                    budget.reserve_storage(floor).unwrap();
                    budget.charge_work(17).unwrap();
                    assert!(budget.charge_work(1).is_err());
                    let error = match prepare_handoff(&native, catalog, &roots, None, &mut budget) {
                        Ok(_) => panic!("work denial"),
                        Err(error) => error,
                    };
                    assert!(
                        matches!(worker_error(&error), E::Resource(Resource::Work(limit))
                    if limit.actual() == 20 && limit.limit() == 17)
                    );
                    assert_eq!((budget.storage(), budget.work()), (floor, 17));
                }
                assert_eq!(work.failed_work(), Some(18));
                let mut work = Work::new(usize::MAX);
                let mut budget = Budget::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                budget.charge_work(17).unwrap();
                assert!(budget.reserve_storage(1).is_err());
                let error = match prepare_handoff(&native, catalog, &roots, None, &mut budget) {
                    Ok(_) => panic!("storage denial"),
                    Err(error) => error,
                };
                assert!(
                    matches!(worker_error(&error), E::Resource(Resource::Storage(limit))
                if limit.actual() > floor + 1 && limit.limit() == floor)
                );
                assert_eq!(
                    (budget.storage(), budget.peak_storage(), budget.work()),
                    (floor, floor, 20)
                );
                assert_eq!(budget.failed_storage(), Some(floor + 1));
            });
        },
    );
}
