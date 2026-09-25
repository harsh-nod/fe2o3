use alloc::vec;

use super::*;

fn event(context_generation: u64, event_id: u64) -> R14ObservedEventKeyV1 {
    R14ObservedEventKeyV1 {
        context_generation,
        event_id,
    }
}

#[test]
fn configuration_and_event_identities_are_bounded() {
    assert!(matches!(
        R14AsyncObserverModelV1::new_model_only(0),
        Err(R14AsyncObserverErrorV1::CapacityExceeded)
    ));
    assert!(matches!(
        R14AsyncObserverModelV1::new_model_only(MAX_R14_ASYNC_WAITERS_V1 + 1),
        Err(R14AsyncObserverErrorV1::CapacityExceeded)
    ));
    let mut model = R14AsyncObserverModelV1::new_model_only(1).unwrap();
    assert_eq!(
        model.register_model_only(event(0, 1), R14RuntimeEventStatusV1::Pending),
        Err(R14AsyncObserverErrorV1::InvalidIdentity)
    );
    assert!(model.pending_events().is_empty());
}

#[test]
fn duplicate_and_capacity_failures_are_atomic() {
    let mut model = R14AsyncObserverModelV1::new_model_only(1).unwrap();
    let first = event(1, 1);
    model
        .register_model_only(first, R14RuntimeEventStatusV1::Pending)
        .unwrap();
    assert_eq!(
        model.register_model_only(first, R14RuntimeEventStatusV1::Pending),
        Err(R14AsyncObserverErrorV1::DuplicateEvent)
    );
    assert_eq!(
        model.register_model_only(event(1, 2), R14RuntimeEventStatusV1::Pending),
        Err(R14AsyncObserverErrorV1::CapacityExceeded)
    );
    assert_eq!(model.pending_events(), &[first]);
}

#[test]
fn already_terminal_registration_is_ready_without_consuming_capacity() {
    let mut model = R14AsyncObserverModelV1::new_model_only(1).unwrap();
    let terminal = R14RuntimeEventStatusV1::Failed { code: 17 };
    assert_eq!(
        model.register_model_only(event(1, 1), terminal),
        Ok(R14AsyncRegistrationV1::Ready(terminal))
    );
    assert!(model.pending_events().is_empty());
}

#[test]
fn pending_observation_preserves_exact_registration() {
    let mut model = R14AsyncObserverModelV1::new_model_only(2).unwrap();
    let key = event(1, 8);
    model
        .register_model_only(key, R14RuntimeEventStatusV1::Pending)
        .unwrap();
    assert_eq!(
        model.observe_model_only(
            key,
            R14AsyncObservationV1::Status(R14RuntimeEventStatusV1::Pending)
        ),
        Ok(None)
    );
    assert_eq!(model.pending_events(), &[key]);
}

#[test]
fn terminal_status_and_runtime_error_are_not_substituted() {
    let mut model = R14AsyncObserverModelV1::new_model_only(2).unwrap();
    let first = event(1, 1);
    let second = event(1, 2);
    model
        .register_model_only(first, R14RuntimeEventStatusV1::Pending)
        .unwrap();
    model
        .register_model_only(second, R14RuntimeEventStatusV1::Pending)
        .unwrap();
    assert_eq!(
        model.observe_model_only(
            first,
            R14AsyncObservationV1::Status(R14RuntimeEventStatusV1::Failed { code: -9 })
        ),
        Ok(Some(R14AsyncOutcomeV1::Runtime(
            R14RuntimeEventStatusV1::Failed { code: -9 }
        )))
    );
    assert_eq!(
        model.observe_model_only(second, R14AsyncObservationV1::RuntimeError { code: 31 }),
        Ok(Some(R14AsyncOutcomeV1::RuntimeError { code: 31 }))
    );
    assert!(model.pending_events().is_empty());
}

#[test]
fn completion_order_is_independent_and_registry_order_is_stable() {
    let mut model = R14AsyncObserverModelV1::new_model_only(3).unwrap();
    for key in [event(2, 9), event(1, 4), event(1, 3)] {
        model
            .register_model_only(key, R14RuntimeEventStatusV1::Pending)
            .unwrap();
    }
    assert_eq!(
        model.pending_events(),
        &[event(1, 3), event(1, 4), event(2, 9)]
    );
    assert_eq!(
        model.observe_model_only(
            event(2, 9),
            R14AsyncObservationV1::Status(R14RuntimeEventStatusV1::Succeeded)
        ),
        Ok(Some(R14AsyncOutcomeV1::Runtime(
            R14RuntimeEventStatusV1::Succeeded
        )))
    );
    assert_eq!(model.pending_events(), &[event(1, 3), event(1, 4)]);
}

#[test]
fn abandon_and_stop_change_observation_but_not_external_custody() {
    let mut model = R14AsyncObserverModelV1::new_model_only(3).unwrap();
    let abandoned = event(1, 1);
    let stopped = event(1, 2);
    model
        .register_model_only(abandoned, R14RuntimeEventStatusV1::Pending)
        .unwrap();
    model
        .register_model_only(stopped, R14RuntimeEventStatusV1::Pending)
        .unwrap();
    let runtime_custody = (true, true);
    assert!(model.abandon_model_only(abandoned));
    assert_eq!(runtime_custody, (true, true));
    assert_eq!(
        model.stop_model_only(),
        vec![(stopped, R14AsyncOutcomeV1::EngineStopped)]
    );
    assert_eq!(runtime_custody, (true, true));
    model.validate_global_invariants().unwrap();
}

#[test]
fn stopped_observer_rejects_new_registration_without_mutation() {
    let mut model = R14AsyncObserverModelV1::new_model_only(1).unwrap();
    assert!(model.stop_model_only().is_empty());
    assert_eq!(
        model.register_model_only(event(1, 1), R14RuntimeEventStatusV1::Pending),
        Err(R14AsyncObserverErrorV1::EngineStopped)
    );
    assert!(model.pending_events().is_empty());
}
