use super::*;
use canonical_assertion_v1::QualificationJoinFaultV1 as JoinFault;
use std::{cell::Cell, mem::size_of};

fn accepted(owner: &ProductionCanonicalScalarFixedPointOwnerV1) {
    history_run(owner, |view, budget| final_reports(owner, view, budget)).unwrap();
}

fn reader_error(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    donor: &ProductionCanonicalScalarFixedPointOwnerV1,
    read: impl FnOnce(&mut Budget<'_>) -> HistoryResult<()>,
) -> HistoryError {
    let before = snapshots(owner);
    let donor_before = snapshots(donor);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + donor.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let error = read(&mut budget)
        .err()
        .expect("isolated unauthenticated reader input must refuse");
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(snapshots(owner), before);
    assert_eq!(snapshots(donor), donor_before);
    error
}

#[test]
fn qualification_literal_has_exact_scalar_selection_and_internal_connector() {
    use fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1;
    for expected in [false, true] {
        let owner = prepare(literal(expected, false));
        // One changing scalar round and one real terminal no-change round.
        // Integer normalization leaves the Bool branch alone in both rounds.
        assert_eq!(owner.history().rounds().len(), 2);
        let rounds = owner.history().rounds();
        let n = owner
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes();
        let f = owner.output().canonical().canonical_bytes();
        assert_ne!(n, f);
        assert_eq!(rounds[0].integer().owner().canonical().canonical_bytes(), n);
        assert_eq!(rounds[0].scalar().owner().canonical().canonical_bytes(), f);
        assert_eq!(rounds[1].integer().owner().canonical().canonical_bytes(), f);
        assert_eq!(rounds[1].scalar().owner().canonical().canonical_bytes(), f);
        history_run(&owner, |view, budget| {
            final_reports(&owner, view, budget)?;
            let row = view.assertion(0, budget)?;
            let selection = row.selection().expect("real checked selection event");
            assert_eq!(selection.round(), 0);
            assert!(!selection.is_integer());
            assert!(matches!(
                row.success_placement(),
                CanonicalKirEdgePlacementV1::InternalConnector(_)
            ));
            assert_eq!(row.original_binding().expected(), expected);
            assert_eq!(row.proof_kind(), Proof::ExactRange);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn qualification_literal_inventory_source_and_range_cuts_are_source_derived() {
    let owner = prepare(literal(true, false));
    accepted(&owner);
    canonical_assertion_v1::qualification_cost_components_v1(&owner, false);
}

#[test]
fn qualification_dynamic_inventory_and_source_construction_are_source_derived() {
    let owner = prepare(dynamic_source(true, false, false));
    accepted(&owner);
    canonical_assertion_v1::qualification_cost_components_v1(&owner, true);
}

#[test]
fn qualification_final_selector_operand_definition_and_polarity_are_isolated() {
    let owner = prepare(payload_source());
    accepted(&owner);
    for (fault, detail) in [
        (JoinFault::SelectorOperand, "assertion selector occurrence"),
        (
            JoinFault::SelectorDefinition,
            "actual assertion selector or ordered successors",
        ),
        (
            JoinFault::SuccessPolarity,
            "actual assertion selector or ordered successors",
        ),
    ] {
        let error = reader_error(&owner, &owner, |budget| {
            canonical_assertion_v1::qualification_final_reader_v1(&owner, &owner, fault, budget)
        });
        assert!(
            matches!(error, HistoryError::Invalid(actual) if actual == detail),
            "{fault:?}: {error:?}"
        );
    }
    accepted(&owner);
}

#[test]
fn qualification_final_alias_order_and_synthetic_association_are_independent() {
    let owner = prepare(dynamic_source(true, false, true));
    accepted(&owner);
    for (fault, detail) in [
        (
            JoinFault::FinalAliasOrder,
            "complete final source alias order",
        ),
        (
            JoinFault::FinalSyntheticAlias,
            "final trap lacks exact original synthetic alias",
        ),
    ] {
        let error = reader_error(&owner, &owner, |budget| {
            canonical_assertion_v1::qualification_final_reader_v1(&owner, &owner, fault, budget)
        });
        assert!(
            matches!(error, HistoryError::Assertion(Af::Binding { detail: actual, .. }) if actual == detail),
            "{fault:?}: {error:?}"
        );
    }
    accepted(&owner);
}

#[test]
fn qualification_same_bytes_foreign_original_and_final_owners_keep_exact_custody() {
    let owner = prepare(dynamic_source(true, false, false));
    let donor = prepare(dynamic_source(true, false, false));
    accepted(&owner);
    accepted(&donor);
    let original = reader_error(&owner, &donor, |budget| {
        canonical_assertion_v1::qualification_foreign_original_v1(&owner, &donor, budget)
    });
    assert!(
        matches!(original, HistoryError::InputCustody),
        "{original:?}"
    );
    let final_owner = reader_error(&owner, &donor, |budget| {
        canonical_assertion_v1::qualification_final_reader_v1(
            &owner,
            &donor,
            JoinFault::ForeignFinalOwner,
            budget,
        )
    });
    assert!(
        matches!(final_owner, HistoryError::InputCustody),
        "{final_owner:?}"
    );
    accepted(&owner);
    accepted(&donor);
}

#[test]
fn qualification_real_adjacent_payload_reader_rejects_one_changed_occurrence() {
    let owner = prepare(payload_source());
    accepted(&owner);
    let reached = Cell::new(false);
    let error = reader_error(&owner, &owner, |budget| {
        canonical_assertion_v1::qualification_payload_reader_v1(&owner, &reached, budget)
    });
    assert!(
        reached.get(),
        "must reach a nonempty retained actual success payload"
    );
    assert!(
        matches!(
            error,
            HistoryError::Invalid("assertion payload occurrence transport")
        ),
        "{error:?}"
    );
    accepted(&owner);
}

#[test]
fn qualification_public_history_nested_panic_destructors_keep_paid_floor_and_postflight() {
    use std::{
        panic::panic_any,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    struct PanicTrace {
        next: AtomicUsize,
        counts: [AtomicUsize; 3],
        order: [AtomicUsize; 3],
    }
    struct Payload {
        trace: Arc<PanicTrace>,
        generation: usize,
        terminal: usize,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.trace.counts[self.generation].fetch_add(1, Ordering::SeqCst);
            let order = self.trace.next.fetch_add(1, Ordering::SeqCst) + 1;
            self.trace.order[self.generation].store(order, Ordering::SeqCst);
            if self.generation < self.terminal {
                panic_any(Self {
                    trace: Arc::clone(&self.trace),
                    generation: self.generation + 1,
                    terminal: self.terminal,
                });
            }
        }
    }
    struct PaidProbe<'a, 'w> {
        budget: &'a Budget<'w>,
        observation: &'a Cell<Option<(usize, usize)>>,
    }
    impl Drop for PaidProbe<'_, '_> {
        fn drop(&mut self) {
            self.observation
                .set(Some((self.budget.work(), self.budget.storage())));
        }
    }
    for terminal in [1, 2] {
        let owner = prepare(literal(true, false));
        accepted(&owner);
        let before = snapshots(&owner);
        let trace = Arc::new(PanicTrace {
            next: AtomicUsize::new(0),
            counts: std::array::from_fn(|_| AtomicUsize::new(0)),
            order: std::array::from_fn(|_| AtomicUsize::new(0)),
        });
        let callback = Cell::new(None);
        let native = Cell::new(None);
        let native_history = Cell::new(None);
        let drop_observation = Cell::new(None);
        let mut work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        // Caller-owned payload/probe credit is explicit. Arc/atomic recording
        // and Rust's panic allocator are not a production allocation oracle.
        let floor = owner.retained_storage_floor_v1()
            + SIBLING
            + (terminal + 1) * size_of::<Payload>()
            + size_of::<PaidProbe<'_, '_>>();
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&budget) as usize;
        let error = owner
            .with_assertion_policy_checks_v1(&mut budget, |view, budget| -> HistoryResult<()> {
                final_reports(&owner, view, budget)?;
                let policies = view.policies(budget)?;
                native.set(Some(policies.observation(budget)?));
                native_history.set(Some(policies.history(0, budget)?));
                callback.set(Some((
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                )));
                let _probe = PaidProbe {
                    budget,
                    observation: &drop_observation,
                };
                panic_any(Payload {
                    trace: Arc::clone(&trace),
                    generation: 0,
                    terminal,
                });
            })
            .unwrap_err();
        let (at_work, paid, peak) = callback
            .get()
            .expect("actual public assertion-history callback");
        if terminal == 1 {
            assert!(
                matches!(error, HistoryError::Assertion(Af::Panicked)),
                "{error:?}"
            );
        } else {
            let HistoryError::FinalPolicy(error) = error else {
                panic!("{error:?}")
            };
            assert!(matches!(
                error.failure(),
                fe2o3_pliron::CanonicalRankedPolicyFailureV1::Panicked
            ));
            assert_eq!(error.last_invocation(), native_history.get());
            let observed = native.get().unwrap();
            assert_eq!(
                error.observation().work_upper_bound(),
                observed.work_upper_bound()
            );
            assert_eq!(
                error.observation().peak_storage_units(),
                observed.peak_storage_units()
            );
            assert_eq!(error.observation().retained_storage_units(), 0);
            assert_eq!(error.observation().first_denial(), None);
            assert!(!error.observation().caught_panic());
        }
        // Only the facade postflight debits: lineage/source/C-owner queries.
        // This exact local continuation is not a whole-entry cost oracle.
        assert_eq!(budget.work(), at_work + 3);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), peak);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(drop_observation.get(), Some((at_work, paid)));
        assert_eq!(std::ptr::from_ref(&budget) as usize, slot);
        assert!(budget.work_ledger_identity_v1() == ledger);
        for generation in 0..3 {
            let reached = generation <= terminal;
            assert_eq!(
                trace.counts[generation].load(Ordering::SeqCst),
                usize::from(reached)
            );
            assert_eq!(
                trace.order[generation].load(Ordering::SeqCst),
                if reached { generation + 1 } else { 0 }
            );
        }
        assert_eq!(trace.next.load(Ordering::SeqCst), terminal + 1);
        assert_eq!(snapshots(&owner), before);
        accepted(&owner);
        drop(owner);
        drop(trace);
        budget.release_storage(floor - SIBLING).unwrap();
        assert_eq!(budget.storage(), SIBLING);
        assert_eq!(work.failed_work(), None);
    }
}
