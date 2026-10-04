use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 23;

enum Fault {
    Error,
    Panic,
    Payload(Arc<AtomicUsize>),
}
std::thread_local! {
    static FAULT: RefCell<Option<Fault>> = const { RefCell::new(None) };
}
pub(super) fn after_model_reservation(budget: &Budget<'_>) -> Result<(), Resource> {
    let Some(fault) = FAULT.with(|cell| cell.take()) else {
        return Ok(());
    };
    assert!(budget.storage() > FLOOR);
    match fault {
        Fault::Error => Err(Resource::Accounting),
        Fault::Panic => panic!("snapshot model reservation panic"),
        Fault::Payload(count) => {
            struct Payload(Arc<AtomicUsize>);
            impl Drop for Payload {
                fn drop(&mut self) {
                    self.0.fetch_add(1, Ordering::SeqCst);
                    panic!("snapshot model payload drop");
                }
            }
            std::panic::panic_any(Payload(count))
        }
    }
}
fn legacy(budget: &mut Budget<'_>) -> CapturedRankedSourceV1 {
    let stage = super::super::tests::project(budget);
    budget.reserve_storage(stage.retained_storage()).unwrap();
    stage
}

#[test]
fn attachment_reserves_all_reports_on_the_same_ledger_and_preserves_legacy_evidence() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let old = legacy(&mut budget);
    let old_retained = old.retained_storage();
    let evidence = fe2o3_mir_model::InertCanonicalSemanticU32InductionEvidenceV1::from_report(
        &old.roots[0].semantic_u32_induction,
    )
    .unwrap();
    let n = *old.capture().original().executable().canonical().identity();
    let ledger = budget.work_ledger_identity_v1();
    let before_work = budget.work();
    let stage = CapturedBoundSnapshotSourceV1::try_attach_v1(old, &mut budget).unwrap();
    assert!(budget.work() > before_work);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), FLOOR + stage.retained_storage());
    let payload: usize = stage
        .reports
        .iter()
        .map(|r| r.retained_storage() - size_of::<Report>())
        .sum();
    assert_eq!(
        stage.retained_storage(),
        old_retained + size_of::<CapturedBoundSnapshotSourceV1>()
            - size_of::<CapturedRankedSourceV1>()
            + table_bytes(stage.reports.capacity()).unwrap()
            + payload
    );
    assert_eq!(
        stage
            .capture()
            .original()
            .executable()
            .canonical()
            .identity(),
        &n
    );
    assert_eq!(
        evidence.canonical_bytes(),
        fe2o3_mir_model::InertCanonicalSemanticU32InductionEvidenceV1::from_report(
            &stage.captured.roots[0].semantic_u32_induction
        )
        .unwrap()
        .canonical_bytes()
    );
    let mut count = 0;
    stage
        .with_observations_v1(&mut budget, |row, budget| {
            assert!(budget.storage() >= FLOOR + stage.retained_storage());
            let Some(SnapshotConsistency::Joined(fact)) = row.outcome() else {
                panic!("direct new-family component join")
            };
            assert!(std::ptr::eq(fact.source(), stage.capture().original()));
            assert!(std::ptr::eq(row.report(), &stage.reports[0]));
            assert_eq!(fact.certificate(), row.report().certificates()[0]);
            assert_eq!(fact.certificate().bound_snapshot(), None);
            count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(budget.storage(), FLOOR + stage.retained_storage());
    let receipt = stage.retained_storage();
    drop(stage);
    budget.release_storage(receipt).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn attachment_has_exact_and_one_short_cumulative_work_and_storage() {
    fn execute(
        work_limit: usize,
        storage_limit: usize,
    ) -> (Result<usize, CaptureError>, usize, usize) {
        let mut setup_work = Work::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let old = super::super::tests::project(&mut setup);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget
            .reserve_storage(FLOOR + old.retained_storage())
            .unwrap();
        let result = CapturedBoundSnapshotSourceV1::try_attach_v1(old, &mut budget);
        let result = result.map(|stage| {
            let receipt = stage.retained_storage();
            assert_eq!(budget.storage(), FLOOR + receipt);
            drop(stage);
            budget.release_storage(receipt).unwrap();
            receipt
        });
        assert_eq!(budget.storage(), FLOOR);
        (result, budget.work(), budget.peak_storage())
    }
    let (result, used, peak) = execute(WORK, STORAGE);
    assert_eq!(execute(used, peak).0.unwrap(), result.unwrap());
    assert!(matches!(
        execute(used - 1, peak).0,
        Err(CaptureError::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        execute(used, peak - 1).0,
        Err(CaptureError::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn model_error_and_unwind_drop_owned_inputs_before_floor_cleanup() {
    for panic in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let old = legacy(&mut budget);
        let ledger = budget.work_ledger_identity_v1();
        assert!(
            FAULT
                .with(|cell| cell.replace(Some(if panic { Fault::Panic } else { Fault::Error })))
                .is_none()
        );
        let result = CapturedBoundSnapshotSourceV1::try_attach_v1(old, &mut budget);
        if panic {
            assert!(matches!(result, Err(CaptureError::Panicked)));
        } else {
            assert!(matches!(
                result,
                Err(CaptureError::Resource(Resource::Accounting))
            ));
        }
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn model_panic_payload_destruction_occurs_after_valid_ledger_floor_restoration() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let old = legacy(&mut budget);
    let count = Arc::new(AtomicUsize::new(0));
    assert!(
        FAULT
            .with(|cell| cell.replace(Some(Fault::Payload(count.clone()))))
            .is_none()
    );
    let result = catch_unwind(AssertUnwindSafe(|| {
        CapturedBoundSnapshotSourceV1::try_attach_v1(old, &mut budget)
    }));
    assert!(result.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn foreign_ledger_or_slot_cannot_mimic_the_reserved_attachment() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let old = legacy(&mut budget);
    let stage = CapturedBoundSnapshotSourceV1::try_attach_v1(old, &mut budget).unwrap();
    let mut foreign_work = Work::new(WORK);
    let mut foreign = Budget::new(&mut foreign_work, STORAGE);
    foreign.reserve_storage(budget.storage()).unwrap();
    let before = (foreign.work(), foreign.storage());
    assert!(matches!(
        stage.observation_count_v1(&mut foreign),
        Err(CaptureError::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        stage.with_observations_v1(&mut foreign, |_, _| Ok(())),
        Err(CaptureError::Resource(Resource::Accounting))
    ));
    assert_eq!((foreign.work(), foreign.storage()), before);
    let floor = budget.storage();
    budget
        .release_storage(floor - stage.retained_storage() + 1)
        .unwrap();
    let before = (budget.work(), budget.storage());
    assert!(matches!(
        stage.observation_count_v1(&mut budget),
        Err(CaptureError::Resource(Resource::Accounting))
    ));
    assert_eq!((budget.work(), budget.storage()), before);
    budget.reserve_storage(floor - budget.storage()).unwrap();
    let receipt = stage.retained_storage();
    drop(stage);
    budget.release_storage(receipt).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn missing_input_reservation_refuses_before_any_new_charge() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let old = super::super::tests::project(&mut budget);
    assert!(old.retained_storage() > FLOOR);
    let before = (budget.work(), budget.storage());
    assert!(matches!(
        CapturedBoundSnapshotSourceV1::try_attach_v1(old, &mut budget),
        Err(CaptureError::Resource(Resource::Accounting))
    ));
    assert_eq!((budget.work(), budget.storage()), before);
}
