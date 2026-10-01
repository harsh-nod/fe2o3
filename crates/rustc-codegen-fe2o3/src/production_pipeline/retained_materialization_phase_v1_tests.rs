//! Custody-only controls; synthetic payloads are not authenticated source owners.
//! The existing genuine BF16 CPU observer separately uses the production entry.
use super::*;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    address: usize,
    work: usize,
    storage: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    work_limit: usize,
    storage_limit: usize,
}
impl Snapshot {
    fn of(ledger: &OwnedBudget) -> Self {
        Self {
            address: ledger as *const OwnedBudget as usize,
            work: ledger.work(),
            storage: ledger.storage(),
            peak: ledger.peak_storage(),
            failed_work: ledger.failed_work(),
            failed_storage: ledger.failed_storage(),
            work_limit: ledger.work_limit(),
            storage_limit: ledger.storage_limit(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Payload(&'static str),
    Account(Snapshot),
}
thread_local! {
    static AUDIT: RefCell<Option<Rc<RefCell<Vec<Event>>>>> = const { RefCell::new(None) };
}
pub(super) fn observe_account_drop(ledger: &OwnedBudget) {
    AUDIT.with(|slot| {
        if let Some(events) = slot.borrow().as_ref() {
            events
                .borrow_mut()
                .push(Event::Account(Snapshot::of(ledger)));
        }
    });
}
struct AuditScope {
    old: Option<Rc<RefCell<Vec<Event>>>>,
}
impl Drop for AuditScope {
    fn drop(&mut self) {
        let old = self.old.take();
        AUDIT.with(|slot| *slot.borrow_mut() = old);
    }
}
fn audit() -> (Rc<RefCell<Vec<Event>>>, AuditScope) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let old = AUDIT.with(|slot| slot.borrow_mut().replace(events.clone()));
    (events, AuditScope { old })
}
struct Payload {
    name: &'static str,
    events: Rc<RefCell<Vec<Event>>>,
}
impl Drop for Payload {
    fn drop(&mut self) {
        self.events.borrow_mut().push(Event::Payload(self.name));
    }
}
fn payload(name: &'static str, events: &Rc<RefCell<Vec<Event>>>) -> Payload {
    Payload {
        name,
        events: events.clone(),
    }
}
fn rejected<T>(result: Result<T>) {
    assert!(matches!(
        result,
        Err(error) if matches!(*error,
            ProductionPipelineError::PreRankedMaterialization(
                fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Canonical(
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Resource(
                        Resource::Accounting
                    )
                )
            )
        )
    ));
}

#[test]
fn original_account_survives_two_consuming_moves_with_exact_counters() {
    let (events, _audit) = audit();
    let first = RetainedMaterializationPhaseV1::start(30, 40, |budget| {
        budget
            .charge_work(3)
            .map_err(materialization_resource_error_v29)?;
        budget
            .reserve_storage(7)
            .map_err(materialization_resource_error_v29)?;
        Ok(payload("first", &events))
    })
    .unwrap();
    let original = Snapshot::of(&first.account.ledger);
    assert_eq!((original.work, original.storage, original.peak), (3, 7, 7));
    let second = first
        .try_map(|first, budget| {
            assert_eq!((budget.work(), budget.storage()), (3, 7));
            drop(first);
            budget
                .charge_work(5)
                .map_err(materialization_resource_error_v29)?;
            budget
                .reserve_storage(11)
                .map_err(materialization_resource_error_v29)?;
            Ok(payload("second", &events))
        })
        .unwrap();
    assert_eq!(
        Snapshot::of(&second.account.ledger).address,
        original.address
    );
    assert_eq!(
        (
            second.account.ledger.work(),
            second.account.ledger.storage()
        ),
        (8, 18)
    );
    let final_value = second
        .try_map(|second, budget| {
            drop(second);
            assert_eq!((budget.work(), budget.storage()), (8, 18));
            Ok(42u32)
        })
        .unwrap();
    assert_eq!(
        Snapshot::of(&final_value.account.ledger).address,
        original.address
    );
    assert_eq!(final_value.finish_copy(), 42);
    let rows = events.borrow();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], Event::Payload("first"));
    assert_eq!(rows[1], Event::Payload("second"));
    let Event::Account(last) = rows[2] else {
        panic!("account must drop last")
    };
    assert_eq!((last.work, last.storage, last.peak), (8, 18, 18));
    assert_eq!((last.work_limit, last.storage_limit), (30, 40));
}

#[test]
fn ordinary_owner_drop_destroys_payload_before_live_account() {
    let (events, _audit) = audit();
    let owned = RetainedMaterializationPhaseV1::start(20, 30, |budget| {
        budget
            .reserve_storage(9)
            .map_err(materialization_resource_error_v29)?;
        Ok(payload("owned", &events))
    })
    .unwrap();
    assert!(events.borrow().is_empty());
    drop(owned);
    let rows = events.borrow();
    assert_eq!(rows[0], Event::Payload("owned"));
    assert!(matches!(
        rows[1],
        Event::Account(Snapshot { storage: 9, .. })
    ));
}

#[test]
fn start_result_error_drops_captured_payload_before_account() {
    let (events, _audit) = audit();
    let captured = payload("captured", &events);
    let result = RetainedMaterializationPhaseV1::<()>::start(20, 30, move |budget| {
        budget
            .reserve_storage(4)
            .map_err(materialization_resource_error_v29)?;
        let _captured = captured;
        Err(accounting())
    });
    rejected(result);
    let rows = events.borrow();
    assert_eq!(rows[0], Event::Payload("captured"));
    assert!(matches!(
        rows[1],
        Event::Account(Snapshot { storage: 4, .. })
    ));
}

#[test]
fn transition_result_error_drops_input_and_output_before_account() {
    let (events, _audit) = audit();
    let owned = RetainedMaterializationPhaseV1::start(20, 30, |budget| {
        budget
            .reserve_storage(4)
            .map_err(materialization_resource_error_v29)?;
        Ok(payload("input", &events))
    })
    .unwrap();
    let result = owned.try_map::<()>(|input, budget| {
        let _input = input;
        let _output = payload("temporary", &events);
        budget
            .reserve_storage(3)
            .map_err(materialization_resource_error_v29)?;
        Err(accounting())
    });
    rejected(result);
    let rows = events.borrow();
    assert_eq!(
        &rows[..2],
        &[Event::Payload("temporary"), Event::Payload("input")]
    );
    assert!(matches!(
        rows[2],
        Event::Account(Snapshot { storage: 7, .. })
    ));
}

#[test]
fn start_unwind_preserves_payload_then_account_drop_order() {
    let (events, _audit) = audit();
    let captured = payload("captured", &events);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = RetainedMaterializationPhaseV1::<()>::start(20, 30, move |budget| {
            budget.reserve_storage(6).unwrap();
            let _captured = captured;
            panic!("controlled source callback unwind");
        });
    }));
    assert!(result.is_err());
    let rows = events.borrow();
    assert_eq!(rows[0], Event::Payload("captured"));
    assert!(matches!(
        rows[1],
        Event::Account(Snapshot { storage: 6, .. })
    ));
}

#[test]
fn transition_unwind_drops_in_flight_payload_before_original_account() {
    let (events, _audit) = audit();
    let owned = RetainedMaterializationPhaseV1::start(20, 30, |budget| {
        budget.reserve_storage(6).unwrap();
        Ok(payload("input", &events))
    })
    .unwrap();
    let original = Snapshot::of(&owned.account.ledger).address;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = owned.try_map::<()>(|input, budget| {
            let _input = input;
            budget.charge_work(4).unwrap();
            panic!("controlled consuming unwind");
        });
    }));
    assert!(result.is_err());
    let rows = events.borrow();
    assert_eq!(rows[0], Event::Payload("input"));
    assert!(
        matches!(rows[1], Event::Account(Snapshot { address, work: 4, storage: 6, .. }) if address == original)
    );
}

#[test]
fn swallowed_work_denial_cannot_publish_successful_payload() {
    let (events, _audit) = audit();
    let result = RetainedMaterializationPhaseV1::start(5, 30, |budget| {
        budget.charge_work(5).unwrap();
        assert!(budget.charge_work(1).is_err());
        Ok(payload("refused", &events))
    });
    rejected(result);
    let rows = events.borrow();
    assert_eq!(rows[0], Event::Payload("refused"));
    assert!(matches!(
        rows[1],
        Event::Account(Snapshot {
            work: 5,
            failed_work: Some(6),
            ..
        })
    ));
}

#[test]
fn swallowed_storage_denial_cannot_publish_successful_payload() {
    let (events, _audit) = audit();
    let result = RetainedMaterializationPhaseV1::start(20, 5, |budget| {
        budget.reserve_storage(5).unwrap();
        assert!(budget.reserve_storage(1).is_err());
        Ok(payload("refused", &events))
    });
    rejected(result);
    let rows = events.borrow();
    assert_eq!(rows[0], Event::Payload("refused"));
    assert!(matches!(
        rows[1],
        Event::Account(Snapshot {
            storage: 5,
            failed_storage: Some(6),
            ..
        })
    ));
}

#[test]
fn propagated_resource_error_is_not_replaced_or_retried() {
    let (events, _audit) = audit();
    let calls = Cell::new(0);
    let result = RetainedMaterializationPhaseV1::<()>::start(0, 0, |budget| {
        calls.set(calls.get() + 1);
        budget
            .charge_work(1)
            .map_err(materialization_resource_error_v29)?;
        Ok(())
    });
    assert!(matches!(result, Err(error) if matches!(*error,
        ProductionPipelineError::PreRankedMaterialization(
            fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Canonical(
                fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Resource(Resource::Work(_))
            )
        )
    )));
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        events.borrow()[0],
        Event::Account(Snapshot {
            failed_work: Some(1),
            ..
        })
    ));
}

#[test]
fn successful_transition_cannot_release_incoming_retained_floor() {
    let (events, _audit) = audit();
    let owned = RetainedMaterializationPhaseV1::start(20, 30, |budget| {
        budget.reserve_storage(9).unwrap();
        Ok(payload("input", &events))
    })
    .unwrap();
    let result = owned.try_map(|input, budget| {
        drop(input);
        budget.release_storage(1).unwrap();
        Ok(payload("output", &events))
    });
    rejected(result);
    let rows = events.borrow();
    assert_eq!(
        &rows[..2],
        &[Event::Payload("input"), Event::Payload("output")]
    );
    assert!(matches!(
        rows[2],
        Event::Account(Snapshot { storage: 8, .. })
    ));
}

#[test]
fn failed_transition_also_checks_original_floor() {
    let (events, _audit) = audit();
    let owned = RetainedMaterializationPhaseV1::start(20, 30, |budget| {
        budget.reserve_storage(9).unwrap();
        Ok(payload("input", &events))
    })
    .unwrap();
    let result = owned.try_map::<()>(|input, budget| {
        drop(input);
        budget.release_storage(1).unwrap();
        Err(Box::new(materialization_resource_error_v29(
            Resource::Arithmetic,
        )))
    });
    rejected(result);
    assert!(matches!(
        events.borrow().last(),
        Some(Event::Account(Snapshot { storage: 8, .. }))
    ));
}

#[test]
fn first_denial_is_sticky_across_private_account_borrows() {
    let (_events, _audit) = audit();
    let mut account = OriginalMaterializationAccountV1::new(1, 2);
    rejected(account.run(|budget| {
        budget.charge_work(1).unwrap();
        assert!(budget.charge_work(1).is_err());
        Ok(())
    }));
    let before = Snapshot::of(&account.ledger);
    let called = Cell::new(false);
    rejected(account.run(|_| {
        called.set(true);
        Ok(())
    }));
    assert!(!called.get());
    assert_eq!(Snapshot::of(&account.ledger), before);
}

#[test]
fn temporary_scratch_can_restore_entry_without_refunding_payload_credit() {
    let (_events, _audit) = audit();
    let owned = RetainedMaterializationPhaseV1::start(20, 30, |budget| {
        budget.reserve_storage(7).unwrap();
        Ok(3u32)
    })
    .unwrap();
    let next = owned
        .try_map(|value, budget| {
            budget.reserve_storage(11).unwrap();
            budget.release_storage(11).unwrap();
            Ok(value + 1)
        })
        .unwrap();
    assert_eq!(
        (
            next.account.ledger.storage(),
            next.account.ledger.peak_storage()
        ),
        (7, 18)
    );
    assert_eq!(next.finish_copy(), 4);
}

#[test]
fn zero_limits_allow_empty_custody_but_no_resource_operation() {
    let (_events, _audit) = audit();
    let owned = RetainedMaterializationPhaseV1::start(0, 0, |_| Ok(())).unwrap();
    assert_eq!(
        (owned.account.ledger.work(), owned.account.ledger.storage()),
        (0, 0)
    );
    owned.finish_copy();
    rejected(RetainedMaterializationPhaseV1::start(0, 0, |budget| {
        assert!(budget.reserve_storage(1).is_err());
        Ok(())
    }));
}

#[test]
fn default_phase_limits_are_not_widened() {
    let work = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
    let storage = crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
    let owned = RetainedMaterializationPhaseV1::start(work, storage, |_| Ok(())).unwrap();
    assert_eq!(owned.account.ledger.work_limit(), work);
    assert_eq!(owned.account.ledger.storage_limit(), storage);
    owned.finish_copy();
}

#[path = "retained_ranked_allowance_v1_tests.rs"]
mod ranked_allowance_tests;
