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
