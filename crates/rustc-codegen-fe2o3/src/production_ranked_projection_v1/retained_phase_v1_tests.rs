use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn phase(work: usize, storage: usize) -> RetainedProjectionPhaseV1 {
    let mut original = Box::new(OwnedBudget::new(Work::new(work), storage));
    original.with_budget(|budget| {
        budget.charge_work(3).unwrap();
        budget.reserve_storage(7).unwrap();
    });
    RetainedProjectionPhaseV1::new(original)
}

#[test]
fn conditional_original_phase_survives_moves_and_callback_boundaries() {
    let mut first = phase(100, 100);
    first
        .with_budget(|budget| {
            budget.charge_work(11).map_err(Error::ConditionalResource)?;
            budget
                .reserve_storage(13)
                .map_err(Error::ConditionalResource)
        })
        .unwrap();
    let mut second = Box::new(first);
    second
        .with_budget(|budget| {
            assert_eq!((budget.work(), budget.storage()), (16, 20));
            budget.charge_work(5).map_err(Error::ConditionalResource)
        })
        .unwrap();
    assert_eq!((second.ledger.work(), second.ledger.storage()), (21, 20));
}

#[test]
fn conditional_original_phase_postchecks_keep_nested_callback_owned_storage() {
    use guarded_source_progress_v1::resources;
    let mut phase = phase(1000, 1000);
    let retained = phase
        .with_budget(|budget| {
            let ledger = budget.work_ledger_identity_v1();
            let floor = budget.storage();
            let retained = resources::owned(
                budget,
                0,
                Error::ConditionalResource,
                || Error::ConditionalResource(Resource::Accounting),
                |budget| {
                    let mut retained =
                        resources::table::<u8>(13, budget).map_err(Error::ConditionalResource)?;
                    retained.extend_from_slice(&[0; 13]);
                    let storage = retained.capacity();
                    Ok((retained, storage))
                },
            )?;
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage(), floor + retained.capacity());
            Ok(retained)
        })
        .unwrap();
    let storage = retained.capacity();
    assert_eq!(phase.ledger.storage(), 7 + storage);
    phase
        .with_budget(|budget| {
            assert_eq!(budget.storage(), 7 + storage);
            drop(retained);
            budget
                .release_storage(storage)
                .map_err(Error::ConditionalResource)
        })
        .unwrap();
    assert_eq!(phase.ledger.storage(), 7);
}

#[test]
fn conditional_original_phase_replay_denial_never_replenishes_work() {
    let mut phase = phase(5, 100);
    phase.with_budget(|_| Ok(())).unwrap();
    phase.with_budget(|_| Ok(())).unwrap();
    let result = phase.with_budget::<()>(|_| panic!("work denial ignored"));
    assert!(matches!(
        result,
        Err(Error::ConditionalResource(Resource::Work(_)))
    ));
    assert_eq!(
        (phase.ledger.work(), phase.ledger.failed_work()),
        (5, Some(6))
    );
}

#[test]
fn conditional_original_phase_keeps_storage_denial_across_views() {
    let mut phase = phase(100, 8);
    let result = phase.with_budget(|budget| {
        budget
            .reserve_storage(2)
            .map_err(Error::ConditionalResource)
    });
    assert!(matches!(
        result,
        Err(Error::ConditionalResource(Resource::Storage(_)))
    ));
    phase
        .with_budget(|budget| {
            assert_eq!((budget.storage(), budget.failed_storage()), (7, Some(9)));
            Ok(())
        })
        .unwrap();
}

#[test]
fn conditional_original_phase_replacement_is_terminal_and_preserves_original_state() {
    let mut phase = phase(100, 100);
    let replacement = Box::leak(Box::new(Work::new(100)));
    let result = phase.with_budget(|budget| {
        budget.charge_work(5).map_err(Error::ConditionalResource)?;
        *budget = Budget::new(replacement, 100);
        budget
            .reserve_storage(7)
            .map_err(Error::ConditionalResource)
    });
    assert!(matches!(
        result,
        Err(Error::ConditionalResource(Resource::Accounting))
    ));
    assert_eq!((phase.ledger.work(), phase.ledger.storage()), (9, 7));
    assert!(
        phase
            .with_budget::<()>(|_| panic!("poisoned phase reused"))
            .is_err()
    );
}

#[test]
fn conditional_original_phase_floor_damage_is_terminal() {
    let mut phase = phase(100, 100);
    let result = phase.with_budget(|budget| {
        budget
            .release_storage(1)
            .map_err(Error::ConditionalResource)
    });
    assert!(matches!(
        result,
        Err(Error::ConditionalResource(Resource::Accounting))
    ));
    assert!(phase.poisoned);
    assert_eq!((phase.ledger.work(), phase.ledger.storage()), (4, 6));
}

#[test]
fn conditional_original_phase_unwind_keeps_work_and_denials() {
    let mut phase = phase(100, 8);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = phase.with_budget::<()>(|budget| {
            let _ = budget.reserve_storage(2);
            budget.charge_work(5).unwrap();
            panic!("target replay unwound")
        });
    }));
    assert!(panic.is_err());
    assert_eq!(
        (
            phase.ledger.work(),
            phase.ledger.storage(),
            phase.ledger.failed_storage()
        ),
        (9, 7, Some(9))
    );
    phase.with_budget(|_| Ok(())).unwrap();
    assert_eq!(phase.ledger.work(), 10);
}

#[test]
fn conditional_original_phase_observes_exact_work_denial_and_drop_without_proof_events() {
    use observation::{Event, Outcome};
    let (_, observed) = observation::observe(|| {
        let mut phase = phase(5, 100);
        phase.with_budget(|_| Ok(())).unwrap();
        let mut moved = Box::new(phase);
        moved.with_budget(|_| Ok(())).unwrap();
        assert!(
            moved
                .with_budget::<()>(|_| panic!("denied callback entered"))
                .is_err()
        );
    });
    let [
        Event::PhaseRetained(initial),
        Event::PhaseFinished {
            before: a,
            after: b,
            outcome: Outcome::Accepted,
        },
        Event::PhaseFinished {
            before: c,
            after: d,
            outcome: Outcome::Accepted,
        },
        Event::PhaseFinished {
            before: e,
            after: f,
            outcome: Outcome::Rejected,
        },
        Event::PhaseDropped {
            before,
            after,
            poisoned: false,
        },
    ] = observed.events.as_slice()
    else {
        panic!("unexpected events: {:?}", observed.events)
    };
    assert_eq!(initial, a);
    assert_eq!(b, c);
    assert_eq!(d, e);
    assert_eq!(f, before);
    assert_eq!((initial.work, b.work, d.work, f.work), (3, 4, 5, 5));
    assert_eq!((f.storage, f.failed_work), (7, Some(6)));
    assert_eq!(
        (after.storage, after.work, after.failed_work),
        (0, 5, Some(6))
    );
}

#[test]
fn conditional_original_phase_observes_storage_denial_and_unwind_without_success() {
    use observation::{Event, Outcome};
    let (_, observed) = observation::observe(|| {
        let mut phase = phase(100, 8);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = phase.with_budget::<()>(|budget| {
                assert!(budget.reserve_storage(2).is_err());
                panic!("replay interrupted")
            });
        }));
        assert!(panic.is_err());
    });
    let [
        Event::PhaseRetained(initial),
        Event::PhaseFinished {
            before,
            after,
            outcome: Outcome::Unwind,
        },
        Event::PhaseDropped {
            before: dropped,
            after: released,
            poisoned: false,
        },
    ] = observed.events.as_slice()
    else {
        panic!("unexpected events: {:?}", observed.events)
    };
    assert_eq!(initial, before);
    assert_eq!(after, dropped);
    assert_eq!(
        (after.work, after.storage, after.failed_storage),
        (4, 7, Some(9))
    );
    assert_eq!((released.storage, released.failed_storage), (0, Some(9)));
}

#[test]
fn conditional_original_phase_observer_restores_after_unwind() {
    let panic = std::panic::catch_unwind(|| {
        observation::observe(|| {
            let _phase = phase(100, 100);
            panic!("fixture interrupted")
        });
    });
    assert!(panic.is_err());
    let (answer, observation) = observation::observe(|| 7);
    assert_eq!(answer, 7);
    assert!(observation.events.is_empty());
}

#[test]
fn retained_roster_phase_box_storage_is_exact_and_does_not_count_reserved_credits() {
    use fe2o3_kernel_ir::{
        LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as E,
        LogicalStorageLimitsV1 as Limits,
    };
    let phase = phase(100, 100);
    let bytes = std::mem::size_of::<OwnedBudget>();
    let mut exact = Counter::new(Limits {
        max_bytes: Some(bytes),
        max_items: 1,
    });
    phase.charge_retained_heap_storage_v1(&mut exact).unwrap();
    assert_eq!((exact.bytes(), exact.items()), (bytes, 1));
    assert_eq!((phase.ledger.work(), phase.ledger.storage()), (3, 7));
    let mut short = Counter::new(Limits {
        max_bytes: Some(bytes - 1),
        max_items: 1,
    });
    assert_eq!(
        phase.charge_retained_heap_storage_v1(&mut short),
        Err(E::ByteLimit)
    );
    assert_eq!((short.bytes(), short.items()), (0, 0));
    let mut short_items = Counter::new(Limits {
        max_bytes: Some(bytes),
        max_items: 0,
    });
    assert_eq!(
        phase.charge_retained_heap_storage_v1(&mut short_items),
        Err(E::ItemLimit)
    );
    assert_eq!((short_items.bytes(), short_items.items()), (0, 0));
}

#[test]
fn retained_roster_clean_gate_rejects_swallowed_original_storage_denial() {
    let mut phase = phase(100, 8);
    phase
        .with_budget(|budget| {
            assert!(budget.reserve_storage(2).is_err());
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        phase.require_clean_v1(),
        Err(Error::ConditionalResource(Resource::Storage(_)))
    ));
    assert_eq!(
        (phase.ledger.storage(), phase.ledger.failed_storage()),
        (7, Some(9))
    );
}

fn roster_phase_state(
    phase: &RetainedProjectionPhaseV1,
) -> (
    usize,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
    usize,
    bool,
) {
    (
        phase.ledger.work(),
        phase.ledger.storage(),
        phase.ledger.peak_storage(),
        phase.ledger.failed_work(),
        phase.ledger.failed_storage(),
        phase.floor,
        phase.poisoned,
    )
}

#[test]
fn retained_roster_clean_gate_preserves_original_work_denial_before_new_debit() {
    let mut phase = phase(5, 8);
    let original = phase.ledger.with_budget(|budget| {
        budget.charge_work(2).unwrap();
        budget.charge_work(4).unwrap_err() // accepted 5, original attempted 9
    });
    assert!(matches!(original, Resource::Work(_)));
    assert_eq!(
        (phase.ledger.work(), phase.ledger.failed_work()),
        (5, Some(9))
    );
    let state = roster_phase_state(&phase);
    let account = phase.ledger.as_ref() as *const OwnedBudget;
    for _ in 0..3 {
        let Err(Error::ConditionalResource(actual)) = phase.require_clean_v1() else {
            panic!("expected original typed Work refusal");
        };
        // The old gate returned attempted 6 here before inspecting original 9.
        assert_eq!(actual, original);
        assert_eq!(roster_phase_state(&phase), state);
        assert_eq!(phase.ledger.as_ref() as *const OwnedBudget, account);
    }
}

#[test]
fn retained_roster_clean_gate_preserves_original_storage_at_exhausted_work() {
    let mut phase = phase(5, 8);
    let original = phase.ledger.with_budget(|budget| {
        budget.charge_work(2).unwrap();
        budget.reserve_storage(2).unwrap_err() // accepted storage 7, attempted 9
    });
    assert!(matches!(original, Resource::Storage(_)));
    assert_eq!((phase.ledger.work(), phase.ledger.failed_work()), (5, None));
    let state = roster_phase_state(&phase);
    let account = phase.ledger.as_ref() as *const OwnedBudget;
    for _ in 0..3 {
        let Err(Error::ConditionalResource(actual)) = phase.require_clean_v1() else {
            panic!("expected original typed Storage refusal");
        };
        // No new Work denial is introduced to mask the existing Storage error.
        assert_eq!(actual, original);
        assert_eq!(phase.ledger.failed_work(), None);
        assert_eq!(roster_phase_state(&phase), state);
        assert_eq!(phase.ledger.as_ref() as *const OwnedBudget, account);
    }
}

#[test]
fn retained_roster_clean_gate_keeps_existing_work_first_history_policy() {
    let mut phase = phase(5, 8);
    let original_work = phase.ledger.with_budget(|budget| {
        budget.charge_work(2).unwrap();
        // Storage happened first chronologically. Existing policy is still
        // Work-first when BOTH per-resource original histories are present.
        assert!(budget.reserve_storage(2).is_err());
        budget.charge_work(4).unwrap_err()
    });
    assert_eq!(
        (phase.ledger.failed_work(), phase.ledger.failed_storage()),
        (Some(9), Some(9))
    );
    let state = roster_phase_state(&phase);
    for _ in 0..3 {
        let Err(Error::ConditionalResource(actual)) = phase.require_clean_v1() else {
            panic!("expected original Work-first refusal");
        };
        assert_eq!(actual, original_work);
        assert_eq!(roster_phase_state(&phase), state);
    }
}

#[test]
fn retained_roster_clean_gate_keeps_clean_charge_and_accounting_priority() {
    let mut clean = phase(4, 8);
    clean.require_clean_v1().unwrap();
    assert_eq!((clean.ledger.work(), clean.ledger.failed_work()), (4, None));
    let Err(Error::ConditionalResource(first)) = clean.require_clean_v1() else {
        panic!("expected new first Work refusal at the unchanged cap");
    };
    assert_eq!(
        (clean.ledger.work(), clean.ledger.failed_work()),
        (4, Some(5))
    );
    let state = roster_phase_state(&clean);
    let Err(Error::ConditionalResource(repeated)) = clean.require_clean_v1() else {
        panic!("expected retained Work refusal");
    };
    assert_eq!(first, repeated);
    assert_eq!(roster_phase_state(&clean), state);
    for poison in [false, true] {
        let mut damaged = phase(5, 8);
        damaged.ledger.with_budget(|budget| {
            budget.charge_work(2).unwrap();
            assert!(budget.reserve_storage(2).is_err());
            if !poison {
                budget.release_storage(1).unwrap();
            }
        });
        damaged.poisoned = poison;
        let state = roster_phase_state(&damaged);
        assert!(matches!(
            damaged.require_clean_v1(),
            Err(Error::ConditionalResource(Resource::Accounting))
        ));
        assert_eq!(roster_phase_state(&damaged), state);
    }
}
