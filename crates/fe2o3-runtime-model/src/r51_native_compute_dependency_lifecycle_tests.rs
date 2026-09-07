use alloc::{boxed::Box, vec, vec::Vec};

use super::r51_native_compute_dependency_lifecycle::*;

const OWNER: u64 = 71;
const SESSION: u64 = 73;
const SOURCE_LANE: u64 = 11;
const TARGET_LANE: u64 = 13;
const SOURCE_ARENA: u64 = 17;

fn owner() -> R51DependencyLifecycleV1 {
    R51DependencyLifecycleV1::new_model_only(OWNER, SESSION).unwrap()
}

fn signal(ordinal: u64) -> R51SignalIdentityV1 {
    R51SignalIdentityV1 {
        mapping: 1_000 + ordinal,
        slot: ordinal as u16,
        generation: 2_000 + ordinal,
    }
}

fn source(owner: &mut R51DependencyLifecycleV1, ordinal: u64) -> R51SourceEventV1 {
    owner
        .publish_source_event_model_only(
            SOURCE_LANE,
            SOURCE_ARENA,
            3_000 + ordinal,
            signal(ordinal),
        )
        .unwrap()
}

fn prepare(
    owner: &mut R51DependencyLifecycleV1,
    events: Vec<R51SourceEventV1>,
    ordinal: u64,
) -> R51PreparedTargetV1 {
    owner
        .begin_target_model_only(
            TARGET_LANE,
            4_000 + ordinal,
            5_000 + ordinal,
            signal(10_000 + ordinal),
            events,
        )
        .unwrap()
}

fn publish(
    owner: &mut R51DependencyLifecycleV1,
    prepared: R51PreparedTargetV1,
) -> (Box<R51PublishedTargetV1>, R51TargetEventV1) {
    let native = match owner
        .publish_native_model_only(prepared, R51NativePublicationFaultV1::None)
        .unwrap()
    {
        R51NativePublicationV1::Published(native) => native,
        R51NativePublicationV1::Retryable(_) => panic!("unexpected retry"),
    };
    owner.bind_published_target_model_only(native).unwrap()
}

#[test]
fn exact_lifecycle_releases_source_pins_once_and_preserves_public_custody() {
    let mut owner = owner();
    let event = source(&mut owner, 1);
    let source_completed = owner
        .completed_source_signal_model_only(&event, 6_001)
        .unwrap();
    owner.clear_trace_model_only();
    let prepared = prepare(&mut owner, vec![event], 1);
    let epoch = prepared.target_epoch_model_only();
    assert_eq!(
        owner.phase_model_only(epoch),
        Some(R51DependencyTargetPhaseV1::Prepared)
    );
    let native = match owner
        .publish_native_model_only(prepared, R51NativePublicationFaultV1::None)
        .unwrap()
    {
        R51NativePublicationV1::Published(native) => native,
        R51NativePublicationV1::Retryable(_) => panic!("unexpected retry"),
    };
    assert_eq!(
        owner.phase_model_only(epoch),
        Some(R51DependencyTargetPhaseV1::NativePublished)
    );
    let (published, target_event) = owner.bind_published_target_model_only(native).unwrap();
    assert_eq!(
        owner.phase_model_only(epoch),
        Some(R51DependencyTargetPhaseV1::Published)
    );
    let stable_dispatch_custody = published.dispatch_custody_model_only();
    let stable_box_address = (&*published) as *const R51PublishedTargetV1;
    let published = match owner
        .poll_model_only(published, R51CompletionObservationV1::Pending)
        .unwrap()
    {
        R51PollV1::Pending(published) => published,
        R51PollV1::Ready(_) => panic!("pending became ready"),
    };
    assert_eq!(
        published.dispatch_custody_model_only(),
        stable_dispatch_custody
    );
    assert_eq!(
        (&*published) as *const R51PublishedTargetV1,
        stable_box_address
    );

    let before_pinned_recycle = owner.snapshot_model_only();
    let pinned = owner
        .recycle_completed_signal_model_only(source_completed)
        .unwrap_err();
    assert_eq!(pinned.error, R51DependencyLifecycleErrorV1::SignalPinned);
    assert_eq!(pinned.completed.public_custody_model_only(), 6_001);
    assert_eq!(owner.snapshot_model_only(), before_pinned_recycle);

    let completed = match owner
        .poll_model_only(published, R51CompletionObservationV1::ExactCompleted)
        .unwrap()
    {
        R51PollV1::Ready(completed) => completed,
        R51PollV1::Pending(_) => panic!("exact completion stayed pending"),
    };
    assert_eq!(
        owner.phase_model_only(epoch),
        Some(R51DependencyTargetPhaseV1::Completed)
    );
    assert_eq!(
        completed.dispatch_custody_model_only(),
        stable_dispatch_custody
    );
    assert_eq!(completed.dependency_count_model_only(), 1);
    owner
        .recycle_completed_signal_model_only(pinned.completed)
        .unwrap();

    let released = owner
        .release_after_dependent_completion_model_only(completed)
        .unwrap();
    assert_eq!(
        released.phase_model_only(),
        R51DependencyTargetPhaseV1::Released
    );
    assert_eq!(
        owner.phase_model_only(epoch),
        Some(R51DependencyTargetPhaseV1::Released)
    );
    let target_completed = owner
        .completed_target_signal_model_only(&released, 6_002)
        .unwrap();
    let before_target_pinned = owner.snapshot_model_only();
    let target_pinned = owner
        .recycle_completed_signal_model_only(target_completed)
        .unwrap_err();
    assert_eq!(
        target_pinned.error,
        R51DependencyLifecycleErrorV1::SignalPinned
    );
    assert_eq!(owner.snapshot_model_only(), before_target_pinned);
    owner.release_target_event_model_only(target_event).unwrap();
    owner
        .recycle_completed_signal_model_only(target_pinned.completed)
        .unwrap();
    assert!(owner.teardown_allowed_model_only());
}

#[test]
fn success_trace_uses_named_production_transitions_without_claiming_refinement() {
    let mut owner = owner();
    let event = source(&mut owner, 1);
    owner.clear_trace_model_only();
    let prepared = prepare(&mut owner, vec![event], 1);
    let (published, target_event) = publish(&mut owner, prepared);
    let completed = match owner
        .poll_model_only(published, R51CompletionObservationV1::ExactCompleted)
        .unwrap()
    {
        R51PollV1::Ready(completed) => completed,
        R51PollV1::Pending(_) => panic!("exact completion stayed pending"),
    };
    let released = owner
        .release_after_dependent_completion_model_only(completed)
        .unwrap();
    assert_eq!(owner.trace_names_model_only(), R51_SUCCESS_TRACE_V1);
    assert_eq!(released.dispatch_custody_model_only(), 4_001);
    owner.release_target_event_model_only(target_event).unwrap();
}

#[test]
fn pure_preflight_rejections_do_not_burn_epochs_or_mutate_pins() {
    let mut owner = owner();
    let initial = owner.snapshot_model_only();
    let failure = owner
        .begin_target_model_only(TARGET_LANE, 1, 2, signal(99), vec![])
        .unwrap_err();
    assert_eq!(
        failure.error,
        R51DependencyLifecycleErrorV1::EmptyDependencies
    );
    assert_eq!(owner.snapshot_model_only(), initial);

    let first = source(&mut owner, 1);
    let second = owner
        .publish_source_event_model_only(SOURCE_LANE, SOURCE_ARENA + 1, 3_002, signal(2))
        .unwrap();
    let before = owner.snapshot_model_only();
    let failure = owner
        .begin_target_model_only(TARGET_LANE, 4_001, 5_001, signal(101), vec![first, second])
        .unwrap_err();
    assert_eq!(
        failure.error,
        R51DependencyLifecycleErrorV1::MultipleSourceArenas
    );
    assert_eq!(owner.snapshot_model_only(), before);
    for event in failure.events {
        owner.release_source_event_model_only(event).unwrap();
    }
}

#[test]
fn accepted_ring_full_attempt_burns_epoch_then_rolls_back_without_native_effect() {
    let mut owner = owner();
    let event = source(&mut owner, 1);
    let source_epoch = event.source_epoch_model_only();
    let before = owner.snapshot_model_only();
    owner.clear_trace_model_only();
    let prepared = prepare(&mut owner, vec![event], 1);
    let target_epoch = prepared.target_epoch_model_only();
    assert!(target_epoch > source_epoch);
    let retryable = match owner
        .publish_native_model_only(prepared, R51NativePublicationFaultV1::RingFullNoEffect)
        .unwrap()
    {
        R51NativePublicationV1::Retryable(retryable) => retryable,
        R51NativePublicationV1::Published(_) => panic!("ring full published"),
    };
    let events = owner.rollback_retryable_model_only(retryable).unwrap();
    let after = owner.snapshot_model_only();
    assert_eq!(after.next_acceptance_epoch, Some(target_epoch + 1));
    assert_eq!(after.active_targets, before.active_targets);
    assert_eq!(after.event_pins, before.event_pins);
    assert_eq!(after.reader_pins, before.reader_pins);
    assert_eq!(after.native_effects, before.native_effects);
    assert_eq!(after.recycle_mutations, before.recycle_mutations);
    assert_eq!(events[0].public_custody_model_only(), 3_001);
    assert_eq!(owner.trace_names_model_only(), R51_RETRY_ROLLBACK_TRACE_V1);
    owner
        .release_source_event_model_only(events.into_iter().next().unwrap())
        .unwrap();
}

#[test]
fn one_hundred_twenty_eight_active_targets_is_exact_and_release_reopens_one_slot() {
    let mut owner = owner();
    let mut dispatches = Vec::new();
    let mut target_events = Vec::new();
    for ordinal in 1..=R51_MAX_ACTIVE_DEPENDENCY_TARGETS_V1 as u64 {
        let event = source(&mut owner, ordinal);
        let prepared = prepare(&mut owner, vec![event], ordinal);
        let (published, target_event) = publish(&mut owner, prepared);
        dispatches.push(published);
        target_events.push(target_event);
    }
    assert_eq!(
        owner.snapshot_model_only().active_targets,
        R51_MAX_ACTIVE_DEPENDENCY_TARGETS_V1
    );
    let overflow_event = source(&mut owner, 500);
    let before_rejection = owner.snapshot_model_only();
    let failure = owner
        .begin_target_model_only(
            TARGET_LANE,
            9_001,
            9_002,
            signal(20_001),
            vec![overflow_event],
        )
        .unwrap_err();
    assert_eq!(
        failure.error,
        R51DependencyLifecycleErrorV1::ActiveTargetCapacity
    );
    assert_eq!(owner.snapshot_model_only(), before_rejection);
    let overflow_event = failure.events.into_iter().next().unwrap();

    let first_dispatch = dispatches.remove(0);
    let first_target_event = target_events.remove(0);
    let completed = match owner
        .poll_model_only(first_dispatch, R51CompletionObservationV1::ExactCompleted)
        .unwrap()
    {
        R51PollV1::Ready(completed) => completed,
        R51PollV1::Pending(_) => panic!("exact completion stayed pending"),
    };
    owner
        .release_after_dependent_completion_model_only(completed)
        .unwrap();
    owner
        .release_target_event_model_only(first_target_event)
        .unwrap();
    let prepared = prepare(&mut owner, vec![overflow_event], 900);
    assert_eq!(
        owner.snapshot_model_only().active_targets,
        R51_MAX_ACTIVE_DEPENDENCY_TARGETS_V1
    );
    let retryable = match owner
        .publish_native_model_only(prepared, R51NativePublicationFaultV1::RingFullNoEffect)
        .unwrap()
    {
        R51NativePublicationV1::Retryable(retryable) => retryable,
        R51NativePublicationV1::Published(_) => panic!("ring full published"),
    };
    let _ = owner.rollback_retryable_model_only(retryable).unwrap();
}

#[test]
fn fan_in_bound_is_exact_and_all_events_must_name_one_source_arena() {
    let mut owner = owner();
    let events = (1..=R51_MAX_DEPENDENCY_EVENTS_V1 as u64)
        .map(|ordinal| source(&mut owner, ordinal))
        .collect::<Vec<_>>();
    let prepared = prepare(&mut owner, events, 1);
    assert_eq!(prepared.source_arena_model_only(), SOURCE_ARENA);
    let retryable = match owner
        .publish_native_model_only(prepared, R51NativePublicationFaultV1::RingFullNoEffect)
        .unwrap()
    {
        R51NativePublicationV1::Retryable(retryable) => retryable,
        R51NativePublicationV1::Published(_) => panic!("ring full published"),
    };
    assert_eq!(
        owner
            .rollback_retryable_model_only(retryable)
            .unwrap()
            .len(),
        256
    );

    let extra = source(&mut owner, 700);
    let events = (800..=1_055)
        .map(|ordinal| source(&mut owner, ordinal))
        .collect();
    let prepared = prepare(&mut owner, events, 2);
    let publication = owner
        .publish_native_model_only(prepared, R51NativePublicationFaultV1::RingFullNoEffect)
        .unwrap();
    let retryable = match publication {
        R51NativePublicationV1::Retryable(retryable) => retryable,
        R51NativePublicationV1::Published(_) => panic!("ring full published"),
    };
    let mut too_many = owner.rollback_retryable_model_only(retryable).unwrap();
    too_many.push(extra);
    let before = owner.snapshot_model_only();
    let failure = owner
        .begin_target_model_only(TARGET_LANE, 41, 43, signal(30_001), too_many)
        .unwrap_err();
    assert_eq!(
        failure.error,
        R51DependencyLifecycleErrorV1::TooManyDependencies
    );
    assert_eq!(owner.snapshot_model_only(), before);
}

#[test]
fn substituted_completion_is_terminal_and_cannot_release_pins() {
    let mut owner = owner();
    let event = source(&mut owner, 1);
    let prepared = prepare(&mut owner, vec![event], 1);
    let (published, _target_event) = publish(&mut owner, prepared);
    let before = owner.snapshot_model_only();
    assert_eq!(
        owner.poll_model_only(
            published,
            R51CompletionObservationV1::Substituted {
                target_epoch: 999,
                dispatch_custody: 998,
            },
        ),
        Err(R51DependencyLifecycleErrorV1::ExactCompletionMismatch)
    );
    let after = owner.snapshot_model_only();
    assert!(after.terminal);
    assert_eq!(after.active_targets, before.active_targets);
    assert_eq!(after.event_pins, before.event_pins);
    assert_eq!(after.reader_pins, before.reader_pins);
    assert!(!owner.teardown_allowed_model_only());
}

#[test]
fn post_claim_terminal_state_is_absorbing() {
    let mut owner = owner();
    let event = source(&mut owner, 1);
    let prepared = prepare(&mut owner, vec![event], 1);
    assert_eq!(
        owner
            .publish_native_model_only(prepared, R51NativePublicationFaultV1::AtOrAfterFirstClaim,),
        Err(R51DependencyLifecycleErrorV1::Terminal)
    );
    let terminal = owner.snapshot_model_only();
    assert!(terminal.terminal);
    assert_eq!(terminal.active_targets, 1);
    assert!(terminal.event_pins > 0);
    assert!(terminal.reader_pins > 0);
    assert_eq!(
        owner.publish_source_event_model_only(SOURCE_LANE, SOURCE_ARENA, 900, signal(900)),
        Err(R51DependencyLifecycleErrorV1::Terminal)
    );
    assert_eq!(owner.snapshot_model_only(), terminal);
    assert_eq!(
        owner.teardown_model_only(),
        Err(R51DependencyLifecycleErrorV1::TeardownBlocked)
    );
    assert_eq!(owner.snapshot_model_only(), terminal);
}

#[test]
fn teardown_is_admitted_exactly_when_active_and_pin_counts_are_zero() {
    let mut owner = owner();
    assert!(owner.teardown_allowed_model_only());
    let event = source(&mut owner, 1);
    assert!(!owner.teardown_allowed_model_only());
    assert_eq!(
        owner.teardown_model_only(),
        Err(R51DependencyLifecycleErrorV1::TeardownBlocked)
    );
    owner.release_source_event_model_only(event).unwrap();
    assert!(owner.teardown_allowed_model_only());
    let receipt = owner.teardown_model_only().unwrap();
    assert_eq!(receipt.active_targets, 0);
    assert_eq!(receipt.event_pins, 0);
    assert_eq!(receipt.reader_pins, 0);
}

#[test]
fn invalid_preflight_does_not_consume_a_nonzero_acceptance_epoch() {
    let mut owner = owner();
    let event = source(&mut owner, 1);
    assert_eq!(event.source_epoch_model_only(), 1);
    let before = owner.next_acceptance_epoch_model_only();
    let event = event.substitute_arena_model_only(0);
    let failure = owner
        .begin_target_model_only(TARGET_LANE, 71, 72, signal(71), vec![event])
        .unwrap_err();
    assert_eq!(failure.error, R51DependencyLifecycleErrorV1::InvalidCustody);
    assert_eq!(owner.next_acceptance_epoch_model_only(), before);
}

#[test]
fn maximum_nonzero_epoch_is_burned_before_the_next_reservation_fails_closed() {
    let mut owner = owner();
    let event = source(&mut owner, 1);
    owner.substitute_next_acceptance_epoch_model_only(Some(u64::MAX));
    let prepared = owner
        .begin_target_model_only(TARGET_LANE, 71, 72, signal(71), vec![event])
        .unwrap();
    assert_eq!(prepared.target_epoch_model_only(), u64::MAX);
    assert_eq!(owner.next_acceptance_epoch_model_only(), None);
    let retryable = match owner
        .publish_native_model_only(prepared, R51NativePublicationFaultV1::RingFullNoEffect)
        .unwrap()
    {
        R51NativePublicationV1::Retryable(retryable) => retryable,
        R51NativePublicationV1::Published(_) => panic!("ring full published"),
    };
    let event = owner
        .rollback_retryable_model_only(retryable)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let before_exhaustion = owner.snapshot_model_only();
    assert_eq!(
        owner.publish_source_event_model_only(SOURCE_LANE, SOURCE_ARENA, 73, signal(73)),
        Err(R51DependencyLifecycleErrorV1::EpochExhausted)
    );
    assert!(owner.terminal_model_only());
    assert_eq!(owner.next_acceptance_epoch_model_only(), None);
    let after_exhaustion = owner.snapshot_model_only();
    assert_eq!(
        after_exhaustion.active_targets,
        before_exhaustion.active_targets
    );
    assert_eq!(after_exhaustion.event_pins, before_exhaustion.event_pins);
    assert_eq!(after_exhaustion.reader_pins, before_exhaustion.reader_pins);
    assert_eq!(event.source_epoch_model_only(), 1);
}
