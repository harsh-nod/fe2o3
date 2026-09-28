use super::tests::{STORAGE, WORK, local, split, with_inventory};
use super::*;
use std::mem::size_of;

#[derive(Debug, Eq, PartialEq)]
enum DropEvent {
    Payload(usize),
    Refund { before: usize, floor: usize },
}
thread_local! {
    static DROP_ORDER: std::cell::RefCell<Option<Vec<DropEvent>>> = const {
        std::cell::RefCell::new(None)
    };
}
fn record(event: DropEvent) {
    DROP_ORDER.with(|events| {
        if let Some(events) = events.borrow_mut().as_mut() {
            events.push(event);
        }
    });
}
pub(super) fn observe_refund(before: usize, floor: usize) {
    record(DropEvent::Refund { before, floor });
}
fn start_drop_order() {
    DROP_ORDER.with(|events| *events.borrow_mut() = Some(Vec::new()));
}
fn take_drop_order() -> Vec<DropEvent> {
    DROP_ORDER.with(|events| events.borrow_mut().take().unwrap())
}
struct NestedPayload(usize);
impl Drop for NestedPayload {
    fn drop(&mut self) {
        record(DropEvent::Payload(self.0));
        if self.0 != 0 {
            std::panic::panic_any(NestedPayload(self.0 - 1));
        }
    }
}

// Independent copies of field layouts, never a production bound helper.
#[allow(dead_code)]
struct AddressLayout {
    allocation: usize,
    start: usize,
    length: usize,
    offset: usize,
    alignment: u32,
    stride: usize,
}
#[allow(dead_code)]
struct ProofLayout<'a, 'g> {
    inventory: &'a CanonicalKirInventoryV1<'g>,
    definitions: Vec<Option<AddressLayout>>,
    operations: Vec<bool>,
    latest_stores: Vec<Option<usize>>,
}
#[allow(dead_code)]
struct ScopeLayout<'a, 'w> {
    budget: &'a mut Budget<'w>,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    active: bool,
}
fn header() -> usize {
    size_of::<ScopeLayout<'_, '_>>()
        + size_of::<std::thread::Result<Result<(ProofLayout<'_, '_>, usize), Error>>>()
        + size_of::<std::thread::Result<()>>()
}
fn assert_exact_capacity<T>(count: usize) {
    let mut rows = Vec::<T>::new();
    rows.try_reserve_exact(count).unwrap();
    assert_eq!(rows.capacity(), count);
}
fn assert_layout() {
    assert_eq!(size_of::<usize>(), 8);
    assert_eq!(size_of::<Vec<usize>>(), 24);
    assert_eq!(size_of::<Option<usize>>(), 16);
    assert_eq!(size_of::<Option<u64>>(), 16);
    assert_eq!(size_of::<Address>(), 48);
    assert_eq!(size_of::<Option<Address>>(), 56);
    assert_eq!(size_of::<physical_cfg::Event>(), 24);
    assert_eq!(size_of::<PrivateMemory<'_, '_>>(), 80);
    assert_eq!(size_of::<ScopeLayout<'_, '_>>(), 40);
    assert_eq!(size_of::<std::thread::Result<()>>(), 16);
    assert_eq!(
        size_of::<Cleanup<'_, '_>>(),
        size_of::<ScopeLayout<'_, '_>>()
    );
    assert_eq!(
        size_of::<ProofLayout<'_, '_>>(),
        size_of::<PrivateMemory<'_, '_>>()
    );
    assert_eq!(
        size_of::<
            std::thread::Result<R<(PrivateMemory<'_, '_>, CanonicalKirPrivateMemoryStorageV1)>>,
        >(),
        size_of::<std::thread::Result<Result<(ProofLayout<'_, '_>, usize), Error>>>(),
    );
    // Guard every scratch element/count shape without measuring checker costs.
    assert_exact_capacity::<Option<u64>>(4);
    assert_exact_capacity::<Option<AddressLayout>>(4);
    assert_exact_capacity::<bool>(4);
    for count in [1, 2, 4] {
        assert_exact_capacity::<Option<usize>>(count);
    }
    assert_exact_capacity::<physical_cfg::Event>(4);
    assert_exact_capacity::<bool>(2);
    assert_exact_capacity::<usize>(2);
}

// Source transcript: local 152; split adds one block-reset charge2 then CFG183.
// Peak local: ref8 + five Vec headers120 + constants64 + addresses224 +
// operation bits4 + store anchors64 + one local cell16 =500.
// Split additionally pays events120 + input56 + reached26 + processings40 +
// state40 + queue scalar header16 + queue rows40 + queue bits26 =364.
// Scoped wrapper adds work4 at entry and4 for the three-capacity transfer sum.
// Actual retained proof: header80 + addresses224 + bits4 + anchors64 =372.
#[test]
fn independent_local_and_cfg_exact_work_peak_and_transfer_receipts() {
    assert_layout();
    for (input, legacy_work, legacy_peak) in [(local(), 152, 500), (split(), 337, 864)] {
        with_inventory(&input, |inventory, input_floor| {
            let floor = input_floor + 37;
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(legacy_work + 8);
            let mut budget = Budget::new(&mut work, floor + header() + legacy_peak);
            budget.reserve_storage(floor).unwrap();
            let (proof, receipt) = check_canonical_kir_private_memory_v1(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                &mut budget,
            )
            .unwrap();
            assert_eq!(proof.definitions.capacity(), 4);
            assert_eq!(proof.operations.capacity(), 4);
            assert_eq!(proof.latest_stores.capacity(), 4);
            assert_eq!(proof.latest_stores(), &[None, None, None, Some(2)]);
            assert_eq!(receipt.retained_storage(), 372);
            assert_eq!(budget.work(), legacy_work + 8);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor + header() + legacy_peak);
            assert_eq!(budget.failed_storage(), None);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor + 372);
            drop(proof);
            budget.release_storage(372).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(work.failed_work(), None);
        });
    }
}

#[test]
fn independent_last_work_denial_allows_an_explicit_later_accepted_suffix() {
    assert_layout();
    for (input, legacy_work, legacy_peak) in [(local(), 152, 500), (split(), 337, 864)] {
        with_inventory(&input, |inventory, floor| {
            let exact_work = legacy_work + 8;
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(exact_work - 1);
            {
                let mut budget = Budget::new(&mut work, floor + header() + legacy_peak);
                budget.reserve_storage(floor).unwrap();
                let error = check_canonical_kir_private_memory_v1(
                    inventory,
                    CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                    &mut budget,
                )
                .err()
                .expect("last receipt charge denied");
                assert!(matches!(error, Error::Resource(Resource::Work(error))
                    if error.actual() == exact_work && error.limit() == exact_work - 1));
                assert_eq!(budget.work(), exact_work - 4);
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), floor + header() + legacy_peak);
                budget.charge_work(1).unwrap();
                assert_eq!(budget.work(), exact_work - 3);
            }
            assert_eq!(work.failed_work(), Some(exact_work));
        });
    }
}

#[test]
fn independent_storage_one_short_denies_at_the_actual_new_global_peak() {
    assert_layout();
    for (input, peak, prefix, last_reserve) in [(local(), 500, 100, 40), (split(), 864, 251, 26)] {
        with_inventory(&input, |inventory, floor| {
            let exact_peak = floor + header() + peak;
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, exact_peak - 1);
            budget.reserve_storage(floor).unwrap();
            let error = check_canonical_kir_private_memory_v1(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                &mut budget,
            )
            .err()
            .expect("last scratch vector is the new global peak");
            assert!(matches!(error, Error::Resource(Resource::Storage(error))
                if error.actual() == exact_peak && error.limit() == exact_peak - 1));
            assert_eq!(budget.work(), prefix);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), exact_peak - last_reserve);
            assert_eq!(budget.failed_storage(), Some(exact_peak));
        });
    }
}

#[test]
fn independent_scope_header_cuts_are_before_the_physical_engine() {
    assert_layout();
    with_inventory(&split(), |inventory, floor| {
        for (limit, attempted, peak) in [
            (floor + 40 - 1, floor + 40, floor),
            (floor + header() - 16 - 1, floor + header() - 16, floor + 40),
            (
                floor + header() - 1,
                floor + header(),
                floor + header() - 16,
            ),
        ] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            let error = check_canonical_kir_private_memory_v1(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                &mut budget,
            )
            .err()
            .expect("scope header refusal");
            assert!(matches!(error, Error::Resource(Resource::Storage(error))
                if error.actual() == attempted && error.limit() == limit));
            assert_eq!(budget.work(), 4);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), peak);
        }
    });
}

#[test]
fn neutral_growth_overflow_and_cell_refusal_restore_the_original_floor() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    assert!(matches!(
        scoped(&mut budget, |budget| scratch::<usize>(usize::MAX, budget)),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.work(), 10);
    with_inventory(&split(), |inventory, floor| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            check_canonical_kir_private_memory_v1(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: 0 },
                &mut budget,
            ),
            Err(Error::Unsupported {
                phase: "private",
                detail: "bounded nonzero allocation extent"
            })
        ));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn neutral_allocator_capacity_error_discards_partial_construction_before_cleanup() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(37).unwrap();
    let request = (isize::MAX as usize).checked_add(1).unwrap();
    let result = scoped(&mut budget, |budget| scratch::<u8>(request, budget));
    assert!(matches!(result, Err(Error::Resource(Resource::Allocation))));
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.work(), 10);
    assert!(budget.peak_storage() > request);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn neutral_scope_discards_rejected_value_and_panic_payload_without_foreign_credit() {
    use std::{
        cell::Cell,
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    struct Payload(Arc<AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Rc::new(Cell::new(false));
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut other_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let mut foreign = Budget::new(&mut other_work, STORAGE);
    budget.reserve_storage(37).unwrap();
    foreign.reserve_storage(37).unwrap();
    let foreign_ledger = foreign.work_ledger_identity_v1();
    let result = scoped(&mut budget, |budget| {
        budget.reserve_storage(17)?;
        std::mem::swap(budget, &mut foreign);
        Ok(Dropped(Rc::clone(&dropped)))
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert!(dropped.get());
    assert!(budget.work_ledger_identity_v1() == foreign_ledger);
    assert_eq!((budget.work(), budget.storage()), (0, 37));
    assert!(foreign.storage() > 37);
    let payload_dropped = Arc::new(AtomicBool::new(false));
    let result: R<()> = scoped(&mut budget, |budget| {
        let rows = scratch::<u8>(17, budget)?;
        assert_eq!(rows.capacity(), 17);
        std::panic::panic_any(Payload(Arc::clone(&payload_dropped)));
    });
    assert!(matches!(result, Err(Error::Panicked)));
    assert!(payload_dropped.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), 37);
}

#[test]
fn neutral_success_preserves_preexisting_denial_history() {
    with_inventory(&split(), |inventory, floor| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(345);
        {
            let limit = floor + header() + 864;
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            assert!(budget.charge_work(350).is_err());
            assert!(budget.reserve_storage(header() + 865).is_err());
            let (proof, receipt) = check_canonical_kir_private_memory_v1(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                &mut budget,
            )
            .unwrap();
            assert_eq!(budget.work(), 345);
            assert_eq!(budget.failed_storage(), Some(limit + 1));
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            drop(proof);
            budget.release_storage(receipt.retained_storage()).unwrap();
        }
        assert_eq!(work.failed_work(), Some(350));
    });
}

#[test]
fn nested_payloads_drop_before_refund_with_exact_floor_and_denial_history() {
    for depth in [1, 2] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4);
        {
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(37).unwrap();
            let peak = 37
                + size_of::<ScopeLayout<'_, '_>>()
                + size_of::<std::thread::Result<R<()>>>()
                + size_of::<std::thread::Result<()>>()
                + 17;
            start_drop_order();
            let result: R<()> = scoped(&mut budget, |budget| {
                budget.reserve_storage(17)?;
                assert!(budget.charge_work(1).is_err());
                assert!(budget.reserve_storage(STORAGE).is_err());
                std::panic::panic_any(NestedPayload(depth));
            });
            assert!(matches!(result, Err(Error::Panicked)));
            let mut expected = (0..=depth)
                .rev()
                .map(DropEvent::Payload)
                .collect::<Vec<_>>();
            expected.push(DropEvent::Refund {
                before: peak,
                floor: 37,
            });
            assert_eq!(take_drop_order(), expected);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (4, 37, peak)
            );
            assert_eq!(budget.failed_storage(), Some(peak + STORAGE));
        }
        assert_eq!(work.failed_work(), Some(5));
    }
}

#[test]
fn rejected_results_drain_nested_panics_without_foreign_credit() {
    for depth in [1, 2] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut other_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let mut foreign = Budget::new(&mut other_work, STORAGE);
        budget.reserve_storage(37).unwrap();
        foreign.reserve_storage(41).unwrap();
        let ledger = foreign.work_ledger_identity_v1();
        let retained = 37
            + size_of::<ScopeLayout<'_, '_>>()
            + size_of::<std::thread::Result<R<NestedPayload>>>()
            + size_of::<std::thread::Result<()>>()
            + 17;
        start_drop_order();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(17)?;
            std::mem::swap(budget, &mut foreign);
            Ok(NestedPayload(depth))
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(
            take_drop_order(),
            (0..=depth)
                .rev()
                .map(DropEvent::Payload)
                .collect::<Vec<_>>()
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!((budget.work(), budget.storage()), (0, 41));
        assert_eq!((foreign.work(), foreign.storage()), (4, retained));
    }
}

#[test]
fn undercut_results_drain_nested_panics_without_refund() {
    for depth in [1, 2] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        start_drop_order();
        let result = scoped(&mut budget, |budget| {
            budget.release_storage(budget.storage() - 36)?;
            Ok(NestedPayload(depth))
        });
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
        assert_eq!(
            take_drop_order(),
            (0..=depth)
                .rev()
                .map(DropEvent::Payload)
                .collect::<Vec<_>>()
        );
        assert_eq!((budget.work(), budget.storage()), (4, 36));
    }
}
