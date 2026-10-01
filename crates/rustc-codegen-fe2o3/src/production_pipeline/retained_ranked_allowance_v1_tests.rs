//! Typed-construction and synthetic ownership controls, not admitted BF16 source.
//! The ordinary fixture uses the real closed verifier; constructor/Context and
//! Display allocations are explicitly outside this component's resource quote.
use super::super::ranked_allowance::quote_values;
use super::*;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedAnalysisAllowanceV1 as Analysis,
    ProductionRankedCompileErrorV1, ProductionRankedKernelLoweringInputV1,
    ProductionRankedSnapshotAllowanceV1 as Presentation, ProductionSessionErrorV1,
    ProductionSessionLimitsV1,
};

fn allowances() -> (Analysis, Presentation) {
    (
        Analysis::new(11, 13).unwrap(),
        Presentation::new(17, 19).unwrap(),
    )
}
fn quote(a: Analysis, s: Presentation) -> (usize, usize) {
    quote_values(a.max_work(), s.max_work(), a.max_peak_storage()).unwrap()
}
fn last_account(events: &Rc<RefCell<Vec<Event>>>) -> Snapshot {
    let rows = events.borrow();
    let Some(Event::Account(snapshot)) = rows.last() else {
        panic!("original account drops last")
    };
    *snapshot
}

#[test]
fn paid_quote_has_exact_window_terms_and_checked_overflow() {
    let (a, s) = allowances();
    assert_eq!(
        quote(a, s),
        (
            11 + 19 + Budget::STORAGE_WINDOW_WORK_V1,
            13 + Budget::STORAGE_WINDOW_SCRATCH_V1
        )
    );
    for values in [(usize::MAX, 1, 0), (usize::MAX, 0, 0), (0, 0, usize::MAX)] {
        assert!(matches!(
            quote_values(values.0, values.1, values.2),
            Err(Resource::Arithmetic)
        ));
    }
}

#[test]
fn zero_payload_allowances_still_pay_original_window_controls() {
    let a = Analysis::new(0, 0).unwrap();
    let s = Presentation::new(0, 0).unwrap();
    let (work, storage) = quote(a, s);
    let phase = RetainedMaterializationPhaseV1::start(work, storage, |_| Ok(5u32))
        .unwrap()
        .try_map_with_ranked_allowances(a, s, |value, _permit, budget| {
            assert_eq!((budget.work(), budget.storage()), (work, storage));
            Ok(value)
        })
        .unwrap();
    assert_eq!(
        (phase.account.ledger.work(), phase.account.ledger.storage()),
        (work, 0)
    );
    assert_eq!(phase.finish_copy(), 5);
}

#[test]
fn exact_paid_floor_survives_return_and_two_original_account_moves() {
    let (a, s) = allowances();
    let (work, storage) = quote(a, s);
    let (events, _audit) = audit();
    let first = RetainedMaterializationPhaseV1::start(work + 3, storage + 7, |budget| {
        budget
            .charge_work(3)
            .map_err(materialization_resource_error_v29)?;
        budget
            .reserve_storage(7)
            .map_err(materialization_resource_error_v29)?;
        Ok(payload("source", &events))
    })
    .unwrap();
    let identity = Snapshot::of(&first.account.ledger).address;
    let paid = first
        .try_map_with_ranked_allowances(a, s, |source, permit, budget| {
            assert_eq!((budget.work(), budget.storage()), (work + 3, storage + 7));
            drop(source);
            Ok((payload("retained", &events), permit))
        })
        .unwrap();
    assert_eq!(Snapshot::of(&paid.account.ledger).address, identity);
    assert_eq!(
        (paid.account.ledger.work(), paid.account.ledger.storage()),
        (work + 3, 7 + a.max_peak_storage())
    );
    let moved = paid
        .try_map(|owned, budget| {
            assert_eq!(budget.storage(), 7 + a.max_peak_storage());
            Ok(owned)
        })
        .unwrap();
    assert_eq!(Snapshot::of(&moved.account.ledger).address, identity);
    drop(moved);
    let rows = events.borrow();
    assert_eq!(rows[0], Event::Payload("source"));
    assert_eq!(rows[1], Event::Payload("retained"));
    let Event::Account(account) = rows[2] else {
        panic!("account last")
    };
    assert_eq!(
        (account.work, account.storage, account.peak),
        (work + 3, 20, storage + 7)
    );
}

#[test]
fn work_denial_before_prepayment_or_window_never_calls_continuation() {
    let (a, s) = allowances();
    let (work, storage) = quote(a, s);
    for limit in [0, work - 1] {
        let (events, _audit) = audit();
        let calls = Cell::new(0);
        let phase = RetainedMaterializationPhaseV1::start(limit, storage, |_| {
            Ok(payload("source", &events))
        })
        .unwrap();
        let result = phase.try_map_with_ranked_allowances(a, s, |_, _, _| {
            calls.set(calls.get() + 1);
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(calls.get(), 0);
        assert_eq!(events.borrow()[0], Event::Payload("source"));
        let account = last_account(&events);
        assert!(account.failed_work.is_some());
        assert_eq!(account.work_limit, limit);
        assert_eq!(account.failed_storage, None);
    }
}

#[test]
fn one_short_storage_preserves_precharged_work_and_original_denial() {
    let (a, s) = allowances();
    let (work, storage) = quote(a, s);
    let (events, _audit) = audit();
    let calls = Cell::new(0);
    let phase = RetainedMaterializationPhaseV1::start(work, storage - 1, |_| {
        Ok(payload("source", &events))
    })
    .unwrap();
    assert!(
        phase
            .try_map_with_ranked_allowances(a, s, |_, _, _| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    let account = last_account(&events);
    assert_eq!(account.work, work - Budget::STORAGE_WINDOW_WORK_V1);
    assert_eq!(account.storage, 0);
    assert_eq!(account.failed_storage, Some(storage));
    assert_eq!(account.failed_work, None);
}

#[test]
fn original_sticky_failure_skips_paid_continuation() {
    let (a, s) = allowances();
    let (work, storage) = quote(a, s);
    let (events, _audit) = audit();
    let mut phase =
        RetainedMaterializationPhaseV1::start(work, storage, |_| Ok(payload("source", &events)))
            .unwrap();
    phase
        .account
        .ledger
        .with_budget(|budget| assert!(budget.charge_work(work + 1).is_err()));
    assert!(
        phase
            .try_map_with_ranked_allowances(a, s, |_, _, _| -> Result<()> {
                panic!("denied original account")
            })
            .is_err()
    );
    let account = last_account(&events);
    assert_eq!(account.failed_work, Some(work + 1));
    assert_eq!(account.work, 0);
}

#[test]
fn live_protected_window_rejects_release_of_paid_analysis_floor() {
    let (a, s) = allowances();
    let (work, storage) = quote(a, s);
    let phase = RetainedMaterializationPhaseV1::start(work, storage, |_| Ok(()))
        .unwrap()
        .try_map_with_ranked_allowances(a, s, |(), _permit, budget| {
            assert!(matches!(
                budget.release_storage(1),
                Err(Resource::Accounting)
            ));
            assert_eq!(budget.storage(), storage);
            Ok(())
        })
        .unwrap();
    assert_eq!(phase.account.ledger.storage(), a.max_peak_storage());
}

#[test]
fn callback_error_drops_payload_before_account_and_only_releases_window_scratch() {
    let (a, s) = allowances();
    let (work, storage) = quote(a, s);
    let (events, _audit) = audit();
    let phase =
        RetainedMaterializationPhaseV1::start(work, storage, |_| Ok(payload("source", &events)))
            .unwrap();
    let result = phase.try_map_with_ranked_allowances::<()>(a, s, |source, _, _| {
        let _source = source;
        Err(accounting())
    });
    rejected(result);
    assert_eq!(events.borrow()[0], Event::Payload("source"));
    let account = last_account(&events);
    assert_eq!(
        (account.work, account.storage, account.peak),
        (work, a.max_peak_storage(), storage)
    );
}

#[test]
fn callback_unwind_drops_payload_before_account_without_refunding_work_or_storage() {
    let (a, s) = allowances();
    let (work, storage) = quote(a, s);
    let (events, _audit) = audit();
    let phase =
        RetainedMaterializationPhaseV1::start(work, storage, |_| Ok(payload("source", &events)))
            .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: Result<RetainedMaterializationPhaseV1<()>> = phase
                .try_map_with_ranked_allowances(a, s, |source, _, _| {
                    let _source = source;
                    panic!("controlled paid continuation");
                });
        }))
        .is_err()
    );
    assert_eq!(events.borrow()[0], Event::Payload("source"));
    let account = last_account(&events);
    assert_eq!((account.work, account.storage), (work, storage));
}

fn construction(index: u64) -> ProductionConstructionV1 {
    use fe2o3_pliron::{
        ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedOperationV1 as Op,
        ProductionRankedTerminatorV1, ProductionRankedValueIdV1 as Id,
        ProductionRankedValueV1 as Value,
    };
    let view = Id::new(0);
    let idx = Id::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "checked",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                Op::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1; 3],
                    workgroup_extents: [1; 3],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                Op::View {
                    result: view,
                    element_width: 32,
                    writable: false,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                Op::IndexConstant {
                    result: idx,
                    value: index,
                },
                Op::Access {
                    kind: dialect_kernel::AccessKindAttr::Read,
                    view: Value::Local(view),
                    indices: vec![Value::Local(idx)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    ProductionConstructionV1::ranked_kernel("paid_root", kernel).unwrap()
}
fn real_phase() -> RetainedMaterializationPhaseV1<ProductionConstructionV1> {
    // Construction/transform allocation is deliberately NOT covered here.
    let construction = construction(0);
    RetainedMaterializationPhaseV1::start(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        |_| Ok(construction),
    )
    .unwrap()
}
struct ObservedLoweringDrop {
    input: Option<ProductionRankedKernelLoweringInputV1>,
    events: Rc<RefCell<Vec<Event>>>,
}
impl Drop for ObservedLoweringDrop {
    fn drop(&mut self) {
        // Record only after the actual input and its retained session have dropped.
        drop(self.input.take());
        self.events
            .borrow_mut()
            .push(Event::Payload("lowering-and-session"));
    }
}

#[test]
fn real_ordinary_compile_and_owning_move_keep_original_account_until_session_drops() {
    let (events, _audit) = audit();
    let a = Analysis::production_hard_ceiling();
    let s = Presentation::production_hard_ceiling();
    let (work, storage) = quote(a, s);
    let phase = real_phase();
    let address = Snapshot::of(&phase.account.ledger).address;
    let paid = phase
        .try_map_with_ranked_allowances(a, s, |construction, permit, budget| {
            assert_eq!((budget.work(), budget.storage()), (work, storage));
            let input = permit
                .compile(construction, ProductionSessionLimitsV1::default())
                .unwrap();
            assert!(input.all_mandatory_reports_are_clean());
            assert!(!input.grants_artifact_or_launch_authority());
            assert!(!input.grants_compiler_refinement_authority());
            assert!(
                input.production_analysis_peak_storage_upper_bound_v1() <= a.max_peak_storage()
            );
            Ok(ObservedLoweringDrop {
                input: Some(input),
                events: events.clone(),
            })
        })
        .unwrap();
    assert_eq!(Snapshot::of(&paid.account.ledger).address, address);
    assert_eq!(paid.account.ledger.storage(), a.max_peak_storage());
    assert!(events.borrow().is_empty());
    let moved = paid
        .try_map(|owned, budget| {
            assert_eq!(budget.work(), work);
            assert_eq!(budget.storage(), a.max_peak_storage());
            assert!(
                owned
                    .input
                    .as_ref()
                    .unwrap()
                    .all_mandatory_reports_are_clean()
            );
            Ok(owned)
        })
        .unwrap();
    assert_eq!(Snapshot::of(&moved.account.ledger).address, address);
    drop(moved);
    assert_eq!(events.borrow()[0], Event::Payload("lowering-and-session"));
    let account = last_account(&events);
    assert_eq!(
        (account.work, account.storage, account.peak),
        (work, a.max_peak_storage(), storage)
    );
}

#[test]
fn real_analysis_refusal_keeps_the_prepaid_account_and_returns_original_typed_error() {
    let a = Analysis::new(0, 0).unwrap();
    let s = Presentation::production_hard_ceiling();
    let (work, _) = quote(a, s);
    let phase = real_phase()
        .try_map_with_ranked_allowances(a, s, |construction, permit, _| {
            let error = permit
                .compile(construction, ProductionSessionLimitsV1::default())
                .unwrap_err();
            assert!(matches!(
                error,
                ProductionRankedCompileErrorV1::Session(
                    ProductionSessionErrorV1::AnalysisResourceLimit { .. }
                )
            ));
            Ok(())
        })
        .unwrap();
    // Inner policy refusal is a typed compile error, not a reset/retry or an
    // invented denial by the fully prepaid enclosing account.
    assert_eq!(phase.account.ledger.work(), work);
    assert_eq!(phase.account.ledger.failed_work(), None);
    assert_eq!(phase.account.ledger.failed_storage(), None);
    assert_eq!(phase.account.ledger.storage(), 0);
}

#[test]
fn real_snapshot_refusal_uses_selected_policy_without_publishing_a_lowering_input() {
    let a = Analysis::production_hard_ceiling();
    let s = Presentation::new(0, 0).unwrap();
    let (work, _) = quote(a, s);
    let phase = real_phase()
        .try_map_with_ranked_allowances(a, s, |construction, permit, _| {
            let error = permit
                .compile(construction, ProductionSessionLimitsV1::default())
                .unwrap_err();
            assert!(matches!(
                error,
                ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::Operation(
                    fe2o3_pliron::OperationHandleError::OperationGraphSnapshotResourceLimit {
                        resource: "presentation hash work"
                    }
                ))
            ));
            Ok(())
        })
        .unwrap();
    assert_eq!(phase.account.ledger.work(), work);
    assert_eq!(phase.account.ledger.storage(), a.max_peak_storage());
}
