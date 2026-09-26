use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayStorageV18, CanonicalKernelIrWorkBudgetV1 as Work, StorageFieldV1,
    StorageLayoutV1, StorageVariantV1,
};
use std::mem::size_of;

// Fixed typed-header reservation independently enumerated from the call graph.
// These Rust sizes are pinned-host premises, not a portable ABI or RSS model.
fn headers() -> usize {
    // One selected primary error; four payload bindings/captures and the
    // catch wrapper; constructor/caller catch outcomes; range and loop counter.
    size_of::<KirBridgeErrorV18>()
        + 4 * size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<AssertUnwindSafe<Box<dyn std::any::Any + Send>>>()
        + 2 * size_of::<std::thread::Result<()>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<resources::Scope<'_, '_>>()
        + 2 * size_of::<Result<resources::Scope<'_, '_>, ResourceError>>()
        + 2 * size_of::<KirPlironGraphV18<'_>>()
        + 8 * size_of::<Result<(KirPlironGraphV18<'_>, KirBridgeStorageV18), KirBridgeErrorV18>>()
        + 8 * size_of::<
            Result<
                (
                    VerifiedCanonicalKernelIrModuleV18,
                    KirBridgeReportV18,
                    KirBridgeStorageV18,
                ),
                KirBridgeErrorV18,
            >,
        >()
        + 2 * size_of::<Module>()
        + 2 * size_of::<Vec<KirBridgeCorrespondenceV1>>()
        + 2 * size_of::<Result<(Module, Vec<KirBridgeCorrespondenceV1>), KirBridgeErrorV18>>()
        + 2 * size_of::<
            std::thread::Result<
                Result<(Module, Vec<KirBridgeCorrespondenceV1>), KirBridgeErrorV18>,
            >,
        >()
        + 2 * size_of::<
            Result<
                (
                    VerifiedCanonicalKernelIrModuleV18,
                    CanonicalKernelIrReplayStorageV18,
                ),
                CanonicalKernelIrReplayAdmissionErrorV18,
            >,
        >()
        + 2 * size_of::<
            Result<CanonicalStorageTableIdentityV18, CanonicalKernelIrReplayAdmissionErrorV18>,
        >()
        + 2 * size_of::<Result<(), KirBridgeErrorV18>>()
        + size_of::<crate::OperationGraphSnapshotV1>()
        + 2 * size_of::<Result<crate::OperationGraphSnapshotV1, OperationHandleError>>()
        + 2 * size_of::<Result<(), ResourceError>>()
        + 2 * size_of::<Result<usize, ResourceError>>()
        + 2 * size_of::<Result<storage_v18::ProfileV18<'_>, CanonicalKernelIrReplayAdmissionErrorV18>>(
        )
        + 2 * size_of::<
            std::thread::Result<
                Result<(KirPlironGraphV18<'_>, KirBridgeStorageV18), KirBridgeErrorV18>,
            >,
        >()
        + 2 * size_of::<
            std::thread::Result<
                Result<
                    (
                        VerifiedCanonicalKernelIrModuleV18,
                        KirBridgeReportV18,
                        KirBridgeStorageV18,
                    ),
                    KirBridgeErrorV18,
                >,
            >,
        >()
        + 2 * size_of::<Vec<StorageLayoutV1>>()
        + 2 * size_of::<Vec<StorageFieldV1>>()
        + 2 * size_of::<Vec<StorageVariantV1>>()
        + 2 * size_of::<Result<Vec<StorageLayoutV1>, KirBridgeErrorV18>>()
        + 2 * size_of::<Result<Vec<StorageFieldV1>, KirBridgeErrorV18>>()
        + 2 * size_of::<Result<Vec<StorageVariantV1>, KirBridgeErrorV18>>()
        + 2 * size_of::<Result<Box<[StorageFieldV1]>, KirBridgeErrorV18>>()
        + 2 * size_of::<Result<Box<[StorageVariantV1]>, KirBridgeErrorV18>>()
        + 2 * size_of::<Result<(), std::collections::TryReserveError>>()
}

const FLOOR: usize = 17;
const WIRE: usize = 41;
const TREE: usize = 3;
// Shared row codec: empty row-count bytes4; count1; emission4;
// entry1; hash(domain length4 + domain34 + policy2 + length8 + bytes4).
const KEY: usize = 1 + 1 + 4 + (4 + 34 + 2 + 8 + 4);
const VOLUME: usize = WIRE + TREE + 1;
const OPAQUE_WORK: usize = 4 * VOLUME * VOLUME + 8 * VOLUME;
const OPAQUE_STORAGE: usize = 64 * VOLUME + 4096;
const ENTRY_WORK: usize = 1 + 4;
const IMPORT_WORK: usize = ENTRY_WORK + KEY + WIRE + OPAQUE_WORK;
// V966 independent empty Module("m") whole factory: count11 + emit45,
// entry1 + decode(44+11+57) + layout8 + verify7 + hash94.
const CANONICAL: usize = 1 + 11 + 45 + (44 + 11 + 57) + 8 + 7 + 94;
const EXPORT_WORK: usize = ENTRY_WORK + WIRE + TREE + OPAQUE_WORK + CANONICAL + KEY + WIRE;
fn retained_graph() -> usize {
    OPAQUE_STORAGE + size_of::<KirPlironGraphV18<'_>>()
}
fn retained_output() -> usize {
    size_of::<VerifiedCanonicalKernelIrModuleV18>() + 1 + WIRE + size_of::<KirBridgeReportV18>()
}

#[test]
fn storage_v18_empty_whole_import_has_independent_work_and_transfer_reservation() {
    assert_eq!(resources::headers().unwrap(), headers());
    assert_eq!(KEY, 58);
    assert_eq!(CANONICAL, 278);
    assert_eq!(IMPORT_WORK, 8564);
    assert_eq!(EXPORT_WORK, 8886);
    let owner = tests::owner(&Module::new("m"));
    let mut work = Work::new(IMPORT_WORK);
    let mut budget = Budget::new(&mut work, tests::AMPLE);
    budget.reserve_storage(FLOOR).unwrap();
    let (graph, receipt) = KirPlironGraphV18::import(&owner, &mut budget).unwrap();
    assert_eq!(budget.work(), IMPORT_WORK);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(receipt.retained_storage(), retained_graph());
    assert_eq!(graph.retained_storage(), retained_graph());
    assert!(graph.correspondence().is_empty());
}

#[test]
fn storage_v18_empty_import_one_short_opaque_admission_keeps_atomic_prefix() {
    let owner = tests::owner(&Module::new("m"));
    let mut work = Work::new(IMPORT_WORK - 1);
    {
        let mut budget = Budget::new(&mut work, tests::AMPLE);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(KirPlironGraphV18::import(&owner, &mut budget).is_err());
        assert_eq!(budget.work(), ENTRY_WORK + KEY + WIRE);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
    assert_eq!(work.failed_work(), Some(IMPORT_WORK));
    // This tests the aggregate admission boundary, not imagined primitive cuts
    // inside opaque Pliron construction.
}

#[test]
fn storage_v18_real_entry_header_exact_prefix_and_one_short_storage_are_independent() {
    let owner = tests::owner(&Module::new("m"));
    let mut work = Work::new(tests::AMPLE);
    let mut budget = Budget::new(&mut work, FLOOR + headers() - 1);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(KirPlironGraphV18::import(&owner, &mut budget).is_err());
    assert_eq!(budget.work(), ENTRY_WORK);
    assert_eq!(budget.peak_storage(), FLOOR);
    assert_eq!(budget.failed_storage(), Some(FLOOR + headers()));
    assert_eq!(budget.storage(), FLOOR);
    let mut work = Work::new(ENTRY_WORK);
    let mut budget = Budget::new(&mut work, FLOOR + headers());
    budget.reserve_storage(FLOOR).unwrap();
    assert!(KirPlironGraphV18::import(&owner, &mut budget).is_err());
    assert_eq!(budget.peak_storage(), FLOOR + headers());
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn storage_v18_empty_fresh_export_exact_and_one_short_compare_preserve_owner_floor() {
    for short in [false, true] {
        let owner = tests::owner(&Module::new("m"));
        let mut work = Work::new(IMPORT_WORK + EXPORT_WORK - usize::from(short));
        {
            let mut budget = Budget::new(&mut work, tests::AMPLE);
            budget.reserve_storage(FLOOR).unwrap();
            let (mut graph, receipt) = KirPlironGraphV18::import(&owner, &mut budget).unwrap();
            assert_eq!(receipt.retained_storage(), retained_graph());
            budget.reserve_storage(retained_graph()).unwrap();
            let result = graph.extract_canonical_v18_o0(tests::LIMITS, &mut budget);
            if short {
                assert!(result.is_err());
                assert_eq!(budget.work(), IMPORT_WORK + EXPORT_WORK - WIRE);
            } else {
                let (output, report, receipt) = result.unwrap();
                assert_eq!(output.canonical_bytes(), owner.canonical_bytes());
                assert_eq!(report.input, report.output);
                assert_eq!(receipt.retained_storage(), retained_output());
                assert_eq!(budget.work(), IMPORT_WORK + EXPORT_WORK);
            }
            assert_eq!(budget.storage(), FLOOR + retained_graph());
        }
        assert_eq!(
            work.failed_work(),
            short.then_some(IMPORT_WORK + EXPORT_WORK)
        );
    }
}

#[test]
fn storage_v18_import_and_export_panics_drop_scratch_then_restore_exact_floor() {
    let owner = tests::owner(&tests::fixture());
    let mut work = Work::new(tests::AMPLE);
    let mut budget = Budget::new(&mut work, tests::AMPLE);
    budget.reserve_storage(FLOOR).unwrap();
    tests::PANIC.with(|slot| slot.set(tests::PanicAt::Import));
    assert!(matches!(
        KirPlironGraphV18::import(&owner, &mut budget),
        Err(KirBridgeErrorV18::Bridge(
            KirBridgeErrorV1::UpstreamPanicked
        ))
    ));
    assert_eq!(budget.storage(), FLOOR);
    let (mut graph, receipt) = KirPlironGraphV18::import(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    tests::PANIC.with(|slot| slot.set(tests::PanicAt::Extract));
    assert!(matches!(
        graph.extract_canonical_v18(tests::LIMITS, &mut budget),
        Err(KirBridgeErrorV18::Bridge(
            KirBridgeErrorV1::UpstreamPanicked
        ))
    ));
    assert_eq!(budget.storage(), floor);
    assert!(graph.session.poisoned);
    let work = budget.work();
    assert!(
        graph
            .extract_canonical_v18(tests::LIMITS, &mut budget)
            .is_err()
    );
    assert_eq!(budget.work(), work);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn storage_v18_table_copy_uses_independent_exact_payload_roster_and_no_pointer_referent_walk() {
    let source = tests::fixture();
    let expected = 2 * size_of::<StorageLayoutV1>() + 2 * size_of::<StorageFieldV1>();
    let mut work = Work::new(4);
    let mut budget = Budget::new(&mut work, expected);
    let payload = resources::table_bytes(&source, &mut budget).unwrap();
    assert_eq!(payload, expected);
    assert_eq!(budget.work(), 2 + 2);
    budget.reserve_storage(expected).unwrap();
    let copy = resources::copy_table(&source).unwrap();
    assert_eq!(copy, source.storage_layouts);
    assert_eq!(copy.capacity(), 2);
    assert_ne!(copy.as_ptr(), source.storage_layouts.as_ptr());
    drop(copy);
    budget.release_storage(expected).unwrap();
    assert_eq!(budget.storage(), 0);
    assert!(resources::envelope(usize::MAX, 1).is_err());
    assert!(resources::envelope(usize::MAX / 8, 1).is_err());
}

#[test]
fn storage_v18_whole_entry_keeps_first_prior_work_and_storage_denials() {
    let owner = tests::owner(&Module::new("m"));
    let mut work = Work::new(5 + IMPORT_WORK);
    work.charge_work(5).unwrap();
    assert!(work.charge_work(IMPORT_WORK + 1).is_err());
    {
        let mut budget = Budget::new(&mut work, tests::AMPLE);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(budget.reserve_storage(tests::AMPLE).is_err());
        KirPlironGraphV18::import(&owner, &mut budget).unwrap();
        assert_eq!(budget.work(), 5 + IMPORT_WORK);
        assert_eq!(budget.failed_storage(), Some(tests::AMPLE + FLOOR));
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(work.failed_work(), Some(IMPORT_WORK + 6));
}

#[test]
fn storage_v18_finite_payload_cleanup_keeps_independent_full_reservations_live() {
    const BEFORE_CANONICAL: usize = ENTRY_WORK + WIRE + TREE + OPAQUE_WORK;
    for panics in 0..=3 {
        let owner = tests::owner(&Module::new("m"));
        let mut work = Work::new(2 * IMPORT_WORK + BEFORE_CANONICAL);
        let mut budget = Budget::new(&mut work, tests::AMPLE);
        budget.reserve_storage(FLOOR).unwrap();
        tests::nested_panic_at(tests::PanicAt::Import, panics);
        assert!(matches!(
            KirPlironGraphV18::import(&owner, &mut budget),
            Err(KirBridgeErrorV18::Bridge(
                KirBridgeErrorV1::UpstreamPanicked
            ))
        ));
        assert_eq!(
            tests::CLEANUP.with(|slot| slot.get()),
            tests::CleanupObservation {
                started: true,
                storage: FLOOR + headers() + retained_graph(),
                poisoned: None,
                drops: panics + 1,
            }
        );
        assert_eq!(budget.work(), IMPORT_WORK);
        assert_eq!(budget.storage(), FLOOR);
        let (mut graph, receipt) = KirPlironGraphV18::import(&owner, &mut budget).unwrap();
        assert_eq!(receipt.retained_storage(), retained_graph());
        budget.reserve_storage(retained_graph()).unwrap();
        tests::nested_panic_at(tests::PanicAt::Extract, panics);
        assert!(matches!(
            graph.extract_canonical_v18(tests::LIMITS, &mut budget),
            Err(KirBridgeErrorV18::Bridge(
                KirBridgeErrorV1::UpstreamPanicked
            ))
        ));
        assert_eq!(
            tests::CLEANUP.with(|slot| slot.get()),
            tests::CleanupObservation {
                started: true,
                storage: FLOOR + retained_graph() + headers() + OPAQUE_STORAGE,
                poisoned: Some(true),
                drops: panics + 1,
            }
        );
        assert_eq!(budget.work(), 2 * IMPORT_WORK + BEFORE_CANONICAL);
        assert_eq!(budget.storage(), FLOOR + retained_graph());
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn storage_v18_cleanup_attempt_prepay_is_atomic_exact_and_one_short() {
    for short in [false, true] {
        let owner = tests::owner(&Module::new("m"));
        let mut work = Work::new(ENTRY_WORK - usize::from(short));
        {
            let mut budget = Budget::new(&mut work, tests::AMPLE);
            budget.reserve_storage(FLOOR).unwrap();
            tests::nested_panic_at(tests::PanicAt::Import, 3);
            assert!(matches!(
                KirPlironGraphV18::import(&owner, &mut budget),
                Err(KirBridgeErrorV18::Resource(ResourceError::Work(_)))
                    | Err(KirBridgeErrorV18::Canonical(
                        CanonicalKernelIrReplayAdmissionErrorV18::Resource(ResourceError::Work(_))
                    ))
            ));
            assert_eq!(budget.work(), if short { 0 } else { ENTRY_WORK });
            assert_eq!(
                budget.peak_storage(),
                if short { FLOOR } else { FLOOR + headers() }
            );
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(
                tests::CLEANUP.with(|slot| slot.get()),
                tests::CleanupObservation::default()
            );
            tests::clear_panic();
        }
        if short {
            assert_eq!(work.failed_work(), Some(ENTRY_WORK));
        } else {
            // The next shared table-key entry is a separate one-unit debit.
            assert_eq!(work.failed_work(), Some(ENTRY_WORK + 1));
        }
    }
}

#[test]
fn storage_v18_nested_cleanup_preserves_prior_denial_history_and_primary_panic() {
    let owner = tests::owner(&Module::new("m"));
    let mut work = Work::new(tests::AMPLE);
    assert!(work.charge_work(tests::AMPLE + 1).is_err());
    {
        let mut budget = Budget::new(&mut work, tests::AMPLE);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(budget.reserve_storage(tests::AMPLE).is_err());
        tests::nested_panic_at(tests::PanicAt::Import, 3);
        assert!(matches!(
            KirPlironGraphV18::import(&owner, &mut budget),
            Err(KirBridgeErrorV18::Bridge(
                KirBridgeErrorV1::UpstreamPanicked
            ))
        ));
        assert_eq!(tests::CLEANUP.with(|slot| slot.get()).drops, 4);
        assert_eq!(budget.failed_storage(), Some(tests::AMPLE + FLOOR));
        assert_eq!(budget.work(), IMPORT_WORK);
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(work.failed_work(), Some(tests::AMPLE + 1));
}

#[test]
fn storage_v18_foreign_ledger_does_not_enter_payload_cleanup_or_poison() {
    let owner = tests::owner(&Module::new("m"));
    let mut work = Work::new(tests::AMPLE);
    let mut budget = Budget::new(&mut work, tests::AMPLE);
    budget.reserve_storage(FLOOR).unwrap();
    let (mut graph, receipt) = KirPlironGraphV18::import(&owner, &mut budget).unwrap();
    assert_eq!(receipt.retained_storage(), retained_graph());
    budget.reserve_storage(retained_graph()).unwrap();
    let mut foreign_work = Work::new(tests::AMPLE);
    let mut foreign = Budget::new(&mut foreign_work, tests::AMPLE);
    foreign.reserve_storage(FLOOR + retained_graph()).unwrap();
    tests::nested_panic_at(tests::PanicAt::Extract, 3);
    assert!(matches!(
        graph.extract_canonical_v18(tests::LIMITS, &mut foreign),
        Err(KirBridgeErrorV18::Bridge(
            KirBridgeErrorV1::GraphIdentityMismatch
        ))
    ));
    assert_eq!(
        tests::CLEANUP.with(|slot| slot.get()),
        tests::CleanupObservation::default()
    );
    assert!(!graph.session.poisoned);
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), FLOOR + retained_graph());
    assert_eq!(budget.storage(), FLOOR + retained_graph());
    tests::clear_panic();
}
