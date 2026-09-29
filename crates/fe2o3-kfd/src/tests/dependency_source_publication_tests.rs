//! The production source-publication flow with CPU receipts, not GPU execution.

use super::runtime_completion_tests::CpuSignals;
use super::runtime_publication_tests::{fixture, owner, restored};
use super::*;
use crate::queue::completion::ComputeDependencyOccurrenceIdentityV1;
use crate::queue::dispatch_binding::CpuDispatchOwnerSnapshotV1;
use crate::queue::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1 as DispatchOwner;
use fe2o3_aql::{AqlCompletionObservationV1, AqlRingReservationError};

include!("dependency_source_output_tests.rs");

struct Recipe {
    owner: DispatchOwner,
    fail_publish: bool,
    fail_cancel: bool,
    bound: Option<(DispatchEpochIdentityV1, CpuDispatchOwnerSnapshotV1)>,
    cancellations: Vec<DispatchEpochIdentityV1>,
}

impl Recipe {
    fn new() -> Self {
        Self {
            owner: DispatchOwner::new(),
            fail_publish: false,
            fail_cancel: false,
            bound: None,
            cancellations: Vec::new(),
        }
    }

    fn owner(&mut self) -> &mut DispatchOwner {
        &mut self.owner
    }
}

impl<const N: usize> super::super::fixed_dispatch::DependencySourceRecipeV1<N> for Recipe {
    fn bind(
        &mut self,
        session: &mut ComputeAqlQueueSessionV1,
    ) -> Result<
        (
            Box<[CompletionPacketTemplateV1; N]>,
            DispatchEpochIdentityV1,
        ),
        Gfx942DispatchBindingErrorV1,
    > {
        let templates: Box<[CompletionPacketTemplateV1; N]> =
            vec![test_completion_template(session.key, self.owner().next_generation()); N]
                .into_boxed_slice()
                .try_into()
                .unwrap();
        let identity = self.owner().reserve_batch(session.key, &templates)?;
        self.bound = Some((identity, self.owner.cpu_snapshot()));
        Ok((templates, identity))
    }

    fn mark_published(
        &mut self,
        _: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.fail_publish {
            self.owner().poison();
        }
        self.owner().mark_published(identity, completion)
    }

    fn cancel(
        &mut self,
        _: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.cancellations.push(identity);
        if self.fail_cancel {
            self.owner().poison();
        }
        self.owner().cancel(identity)
    }
}

fn ring_full() -> NativeAqlSubmissionFailureV1 {
    NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(NativeAqlSubmissionErrorV1::Ring(
        AqlRingReservationError::InsufficientSpace {
            requested: 3,
            available: 0,
        },
    ))
}

#[test]
fn maximum_source_retry_fits_two_mib_stack_and_preserves_custody() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            for auxiliary in [false, true] {
                let (mut session, lane) = fixture(auxiliary);
                let ordinal = usize::from(auxiliary);
                let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
                let mut recipe = Recipe::new();
                for attempt in 1..=2 {
                    let before = owner(&session, ordinal).state_snapshot_for_test();
                    let acceptance = session
                        .dependency_owner
                        .custody_snapshot_for_test()
                        .1
                        .unwrap();
                    let failure = session
                        .with_compute_lane_v1(lane, |selected| {
                            selected.session.submit_dependency_source_using_v1::<8192>(
                                selected.lane,
                                &mut recipe,
                                |session, packets| {
                                    assert_eq!(packets.packet_count(), 8192);
                                    assert_eq!(
                                        session
                                            .completion_owner
                                            .dependency_ledger_counts_for_test(),
                                        (1 + attempt * 8192, 8192, 0)
                                    );
                                    Err(ring_full())
                                },
                            )
                        })
                        .unwrap()
                        .unwrap_err();
                    assert!(matches!(
                        failure,
                        Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)
                    ));
                    assert_eq!(
                        owner(&session, ordinal).state_snapshot_for_test(),
                        (before.0 + 1, before.1)
                    );
                    assert_eq!(
                        owner(&session, ordinal).dependency_ledger_counts_for_test(),
                        (1 + attempt * 8192, 0, 0)
                    );
                    assert_eq!(recipe.owner().next_generation(), attempt + 1);
                    assert_eq!(recipe.owner().live_epoch_count(), 0);
                    assert_eq!(recipe.cancellations.len(), attempt as usize);
                    assert_eq!(
                        recipe.cancellations.last(),
                        Some(&recipe.bound.as_ref().unwrap().0)
                    );
                    assert_eq!(
                        session.dependency_owner.custody_snapshot_for_test().1,
                        Some(acceptance + 1)
                    );
                    assert_eq!(
                        owner(&session, 1 - ordinal).custody_snapshot_for_test(),
                        untouched
                    );
                    assert!(!session.terminal_poisoned);
                }
                owner(&session, ordinal).ensure_releasable().unwrap();
                recipe.owner().ensure_releasable().unwrap();
                restored(&session);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

fn submit(
    selected: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    recipe: &mut Recipe,
    native: impl FnOnce(
        &mut ComputeAqlQueueSessionV1,
        AqlPreparedKernelDispatchBatchV2<3>,
    ) -> Result<u64, NativeAqlSubmissionFailureV1>,
) -> Result<Gfx942ComputeDependencySourceBatchV1<3>, Gfx942FixedDispatchSubmissionFailureV1> {
    selected
        .session
        .submit_dependency_source_using_v1(selected.lane, recipe, native)
}

fn identities(
    session: &ComputeAqlQueueSessionV1,
    source: &Gfx942ComputeDependencySourceBatchV1<3>,
) -> Vec<ComputeDependencyOccurrenceIdentityV1> {
    source
        .events
        .iter()
        .map(|event| {
            owner(session, event.lane.ordinal)
                .dependency_source_identity_v1(&event.event)
                .unwrap()
        })
        .collect()
}

fn publish(
    session: &mut ComputeAqlQueueSessionV1,
    lane: ComputeAqlQueueLaneV1,
    recipe: &mut Recipe,
    last_packet: u64,
) -> Gfx942ComputeDependencySourceBatchV1<3> {
    let epoch = session
        .dependency_owner
        .custody_snapshot_for_test()
        .1
        .unwrap();
    let source = session
        .with_compute_lane_v1(lane, |selected| {
            let source = submit(selected, recipe, |_, packets| {
                assert_eq!(packets.packet_count(), 3);
                Ok(last_packet)
            })
            .unwrap();
            let (batch, events) = source.into_parts();
            let (completion, identity) = unwrap_published(batch);
            let occurrence = recipe
                .owner()
                .validate_published(identity, &completion)
                .unwrap();
            assert_eq!(occurrence.packet_count, 3);
            assert_eq!(occurrence.first_packet_id, last_packet - 2);
            assert_eq!(occurrence.last_packet_id, last_packet);
            assert_eq!(events.len(), 3);
            let mut slots = std::collections::HashSet::new();
            for (index, event) in events.iter().enumerate() {
                assert_eq!(event.lane, lane);
                let exact = selected
                    .session
                    .completion_owner
                    .dependency_source_identity_v1(&event.event)
                    .unwrap();
                assert_eq!(
                    exact.session_occurrence,
                    selected.session.dependency_owner.session_occurrence()
                );
                assert_eq!(exact.acceptance_epoch, epoch);
                assert_eq!(exact.batch_id, occurrence.batch_id);
                assert_eq!(exact.queue, occurrence.queue);
                assert_eq!(exact.signal_mapping, occurrence.signal_mapping);
                assert_eq!(
                    exact.dispatch_generation,
                    occurrence.dispatch_roster.dispatch_generation
                );
                assert_eq!(exact.packet_id, Some(last_packet - 2 + index as u64));
                assert!(slots.insert(exact.slot_index));
            }
            Gfx942ComputeDependencySourceBatchV1 {
                batch: wrap_published(completion, identity),
                events,
            }
        })
        .unwrap();
    restored(session);
    source
}

fn retire(
    session: &mut ComputeAqlQueueSessionV1,
    lane: ComputeAqlQueueLaneV1,
    recipe: &mut Recipe,
    source: Gfx942ComputeDependencySourceBatchV1<3>,
    retain_reader: bool,
) {
    session
        .with_compute_lane_v1(lane, |selected| {
            let (batch, mut events) = source.into_parts();
            let (completion, identity) = unwrap_published(batch);
            let occurrence = recipe
                .owner()
                .validate_published(identity, &completion)
                .unwrap();
            let slots: Vec<_> = events
                .iter()
                .map(|event| {
                    selected
                        .session
                        .completion_owner
                        .dependency_source_identity_v1(&event.event)
                        .unwrap()
                        .slot_index
                })
                .collect();
            let reader = if retain_reader {
                let acceptance = selected
                    .session
                    .dependency_owner
                    .reserve_acceptance_epoch()
                    .unwrap();
                let event = events.pop().unwrap();
                let (event, reader) = selected
                    .session
                    .completion_owner
                    .retain_dependency_reader_v1(
                        event.event,
                        acceptance.session_occurrence(),
                        acceptance.epoch(),
                    )
                    .unwrap();
                events.push(Gfx942ComputeDependencyEventV1 { lane, event });
                Some(reader)
            } else {
                None
            };
            let mut signals = CpuSignals::new(AqlCompletionObservationV1::Completed);
            let Gfx942CompletionPollWithProgressV1::Ready { mut completed, .. } = selected
                .session
                .completion_owner
                .observe_once_with_progress(completion, &mut signals)
                .unwrap()
            else {
                panic!("CPU signals are complete")
            };
            assert_eq!(signals.observed, slots);
            assert_eq!(completed.occurrence_v1().unwrap(), occurrence);
            recipe.owner().mark_completed(identity, occurrence).unwrap();
            let mut resets = CpuSignals::new(AqlCompletionObservationV1::Pending);
            for (index, event) in events.into_iter().enumerate() {
                let (error, retained) = selected
                    .session
                    .completion_owner
                    .recycle_retaining(completed, &mut resets)
                    .unwrap_err();
                assert_eq!(
                    error,
                    Gfx942CompletionErrorV1::SignalPinned {
                        slot: slots[index],
                        event_pins: 1,
                        native_reader_pins: u32::from(retain_reader && index == 2),
                    }
                );
                completed = retained;
                assert_eq!(
                    recipe
                        .owner()
                        .validate_completed(identity, &completed)
                        .unwrap(),
                    occurrence
                );
                assert_eq!(resets.checks, 0);
                assert!(resets.resets.is_empty());
                selected
                    .session
                    .completion_owner
                    .release_dependency_event_v1(event.event)
                    .unwrap();
            }
            if let Some(reader) = reader {
                let (error, retained) = selected
                    .session
                    .completion_owner
                    .recycle_retaining(completed, &mut resets)
                    .unwrap_err();
                assert_eq!(
                    error,
                    Gfx942CompletionErrorV1::SignalPinned {
                        slot: slots[2],
                        event_pins: 0,
                        native_reader_pins: 1,
                    }
                );
                completed = retained;
                assert_eq!(resets.checks, 0);
                assert!(resets.resets.is_empty());
                selected
                    .session
                    .completion_owner
                    .release_dependency_reader_v1(reader)
                    .unwrap();
            }
            let observation = selected
                .session
                .completion_owner
                .recycle_retaining(completed, &mut resets)
                .unwrap();
            assert_eq!(observation.packet_count(), 3);
            assert_eq!(resets.resets, slots);
            recipe.owner().mark_recycled(identity, occurrence).unwrap();
        })
        .unwrap();
    restored(session);
}

#[test]
fn source_bundle_preserves_each_packet_pin_and_reader_until_explicit_release() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let ordinal = usize::from(auxiliary);
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let mut recipe = Recipe::new();
        let older = publish(&mut session, lane, &mut recipe, 42);
        let newer = publish(&mut session, lane, &mut recipe, 45);
        let older_exact = identities(&session, &older);
        assert_eq!(
            owner(&session, ordinal).dependency_ledger_counts_for_test(),
            (7, 6, 0)
        );
        retire(&mut session, lane, &mut recipe, newer, true);
        assert_eq!(identities(&session, &older), older_exact);
        assert_eq!(
            owner(&session, ordinal).dependency_ledger_counts_for_test(),
            (7, 3, 0)
        );
        retire(&mut session, lane, &mut recipe, older, false);
        recipe.owner().ensure_releasable().unwrap();
        owner(&session, ordinal).ensure_releasable().unwrap();
        assert_eq!(
            owner(&session, ordinal).dependency_ledger_counts_for_test(),
            (7, 0, 0)
        );
        assert_eq!(
            owner(&session, 1 - ordinal).custody_snapshot_for_test(),
            untouched
        );
        assert!(!session.terminal_poisoned);
    }
}

#[test]
fn source_signal_capacity_retry_refunds_dispatch_without_calling_native() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let ordinal = usize::from(auxiliary);
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let full = session
            .with_compute_lane_v1(lane, |selected| {
                selected
                    .session
                    .completion_owner
                    .fill_all_signals_for_test(test_completion_template(selected.session.key, 99))
            })
            .unwrap();
        let before = owner(&session, ordinal).custody_snapshot_for_test();
        let mut recipe = Recipe::new();
        let failure = session
            .with_compute_lane_v1(lane, |selected| {
                submit(selected, &mut recipe, |_, _| {
                    panic!("full signals must not call native")
                })
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            failure,
            Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::Completion(
                    Gfx942CompletionErrorV1::InsufficientSignals
                )
            )
        ));
        assert_eq!(owner(&session, ordinal).custody_snapshot_for_test(), before);
        assert_eq!(
            owner(&session, ordinal).dependency_ledger_counts_for_test(),
            (1, 0, 0)
        );
        assert_eq!(recipe.owner().live_epoch_count(), 0);
        assert_eq!(recipe.owner().next_generation(), 2);
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test().1,
            Some(2)
        );
        session
            .with_compute_lane_v1(lane, |selected| {
                selected
                    .session
                    .completion_owner
                    .complete_and_recycle_all_for_test(full)
            })
            .unwrap();
        let source = publish(&mut session, lane, &mut recipe, 62);
        retire(&mut session, lane, &mut recipe, source, false);
        recipe.owner().ensure_releasable().unwrap();
        owner(&session, ordinal).ensure_releasable().unwrap();
        assert_eq!(
            owner(&session, 1 - ordinal).custody_snapshot_for_test(),
            untouched
        );
        assert!(!session.terminal_poisoned);
    }
}

#[test]
fn source_ring_retry_refunds_all_three_events_preserves_neighbors_and_burns_identities() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let other_lane = if auxiliary {
            session.primary_compute_lane_v1()
        } else {
            ComputeAqlQueueLaneV1 {
                session: session.key,
                ordinal: 1,
                generation: 7,
            }
        };
        let ordinal = usize::from(auxiliary);
        let mut recipe = Recipe::new();
        let mut other_recipe = Recipe::new();
        let neighbor = publish(&mut session, lane, &mut recipe, 72);
        let other = publish(&mut session, other_lane, &mut other_recipe, 82);
        let neighbor_exact = identities(&session, &neighbor);
        let other_exact = identities(&session, &other);
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let before = owner(&session, ordinal).state_snapshot_for_test();
        let epoch = session
            .dependency_owner
            .custody_snapshot_for_test()
            .1
            .unwrap();
        let failure = session
            .with_compute_lane_v1(lane, |selected| {
                submit(selected, &mut recipe, |session, packets| {
                    assert_eq!(packets.packet_count(), 3);
                    assert_eq!(
                        session.completion_owner.dependency_ledger_counts_for_test(),
                        (7, 6, 0)
                    );
                    Err(ring_full())
                })
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            failure,
            Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)
        ));
        assert_eq!(
            owner(&session, ordinal).state_snapshot_for_test(),
            (before.0 + 1, before.1)
        );
        assert_eq!(
            owner(&session, ordinal).dependency_ledger_counts_for_test(),
            (7, 3, 0)
        );
        assert_eq!(recipe.owner().live_epoch_count(), 1);
        assert_eq!(recipe.owner().next_generation(), 3);
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test().1,
            Some(epoch + 1)
        );
        assert_eq!(identities(&session, &neighbor), neighbor_exact);
        assert_eq!(identities(&session, &other), other_exact);
        assert_eq!(
            owner(&session, 1 - ordinal).custody_snapshot_for_test(),
            untouched
        );
        let retry = publish(&mut session, lane, &mut recipe, 75);
        for exact in identities(&session, &retry) {
            assert_eq!(exact.acceptance_epoch, epoch + 1);
            assert_eq!(exact.dispatch_generation, 3);
            assert_eq!(exact.batch_id, before.0 + 1);
        }
        retire(&mut session, lane, &mut recipe, retry, false);
        assert_eq!(identities(&session, &neighbor), neighbor_exact);
        retire(&mut session, other_lane, &mut other_recipe, other, false);
        retire(&mut session, lane, &mut recipe, neighbor, false);
        recipe.owner().ensure_releasable().unwrap();
        other_recipe.owner().ensure_releasable().unwrap();
        owner(&session, 0).ensure_releasable().unwrap();
        owner(&session, 1).ensure_releasable().unwrap();
        assert!(!session.terminal_poisoned);
    }
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    EventRelease,
    CompletionCancel,
    DispatchCancel,
    NativeTerminal,
    CompletionPublish,
    EventBind,
    DispatchPublish,
    Unwind,
}

#[test]
fn source_terminal_rollback_preserves_exact_prefix_neighbors_and_burned_identities() {
    for auxiliary in [false, true] {
        for (fault, unpinned_refusal) in [
            (Fault::EventRelease, false),
            (Fault::EventRelease, true),
            (Fault::CompletionCancel, false),
            (Fault::DispatchCancel, false),
        ] {
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            let (mut session, lane) = fixture(auxiliary);
            let ordinal = usize::from(auxiliary);
            let other_lane = if auxiliary {
                session.primary_compute_lane_v1()
            } else {
                ComputeAqlQueueLaneV1 {
                    session: session.key,
                    ordinal: 1,
                    generation: 7,
                }
            };
            let mut recipe = Recipe::new();
            let mut other_recipe = Recipe::new();
            let _neighbor = publish(&mut session, lane, &mut recipe, 72);
            let _other = publish(&mut session, other_lane, &mut other_recipe, 82);
            let untouched = owner(&session, 1 - ordinal).source_rollback_snapshot_for_test();
            let other_dispatch = other_recipe.owner.cpu_snapshot();
            let before = owner(&session, ordinal).state_snapshot_for_test();
            let event_id = owner(&session, ordinal)
                .dependency_ledger_counts_for_test()
                .0;
            let mut expected_dependency = session.dependency_owner.custody_snapshot_for_test();
            let epoch = expected_dependency.1.unwrap();
            expected_dependency.1 = Some(epoch + 1);
            let generation = recipe.owner.next_generation();
            recipe.fail_cancel = matches!(fault, Fault::DispatchCancel);
            let mut expected = None;
            let failure = session
                .with_compute_lane_v1(lane, |selected| {
                    submit(selected, &mut recipe, |session, packets| {
                        assert_eq!(packets.packet_count(), 3);
                        match fault {
                            Fault::EventRelease if unpinned_refusal => session
                                .completion_owner
                                .clear_bound_event_pins_for_test(before.0),
                            Fault::EventRelease => session.completion_owner.poison_owner(),
                            Fault::CompletionCancel => session
                                .completion_owner
                                .invalidate_last_bound_phase_for_test(before.0),
                            Fault::DispatchCancel => (),
                            _ => unreachable!(),
                        }
                        expected =
                            Some(session.completion_owner.source_rollback_snapshot_for_test());
                        Err(ring_full())
                    })
                })
                .unwrap()
                .unwrap_err();
            assert!(matches!(
                failure,
                Gfx942FixedDispatchSubmissionFailureV1::Terminal(_)
            ));
            match fault {
                Fault::DispatchCancel => assert!(matches!(
                    failure.into_error(),
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::StaleDispatchGeneration
                    )
                )),
                _ => assert!(matches!(
                    failure.into_error(),
                    ComputeAqlQueueSessionErrorV1::Completion(
                        Gfx942CompletionErrorV1::StaleEventOccurrence
                    )
                )),
            }
            let release = !matches!(fault, Fault::EventRelease);
            let cancel = matches!(fault, Fault::DispatchCancel);
            let mut expected = expected.unwrap();
            expected.expect_terminal_prefix(
                before.0,
                release,
                cancel,
                u32::from(!unpinned_refusal),
            );
            assert_eq!(
                owner(&session, ordinal).source_rollback_snapshot_for_test(),
                expected
            );
            assert_eq!(
                owner(&session, 1 - ordinal).source_rollback_snapshot_for_test(),
                untouched
            );
            assert_eq!(other_recipe.owner.cpu_snapshot(), other_dispatch);
            let (identity, mut expected_dispatch) = recipe.bound.take().unwrap();
            if cancel {
                expected_dispatch.expect_poison_for_test();
                assert_eq!(recipe.cancellations, [identity]);
            } else {
                assert!(recipe.cancellations.is_empty());
            }
            assert_eq!(recipe.owner.cpu_snapshot(), expected_dispatch);
            assert_eq!(recipe.owner.next_generation(), generation + 1);
            assert_eq!(recipe.owner.live_epoch_count(), 2);
            assert_eq!(
                owner(&session, ordinal).state_snapshot_for_test(),
                (before.0 + 1, before.1 - if cancel { 0 } else { 3 })
            );
            assert_eq!(
                owner(&session, ordinal).dependency_ledger_counts_for_test(),
                (event_id + 3, if release { 3 } else { 6 }, 0)
            );
            assert_eq!(
                session.dependency_owner.custody_snapshot_for_test(),
                expected_dependency
            );
            assert!(session.terminal_poisoned);
            assert!(take_dispatch_terminal_process_gate_record_v1());
            let terminal_dependency = session.dependency_owner.custody_snapshot_for_test();
            for rejected_lane in [lane, other_lane, lane] {
                let error = session
                    .with_compute_lane_v1(rejected_lane, |_| panic!("terminal lane cannot run"))
                    .unwrap_err();
                assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::Poisoned
                    )
                ));
                assert_eq!(
                    owner(&session, ordinal).source_rollback_snapshot_for_test(),
                    expected
                );
                assert_eq!(
                    owner(&session, 1 - ordinal).source_rollback_snapshot_for_test(),
                    untouched
                );
                assert_eq!(recipe.owner.cpu_snapshot(), expected_dispatch);
                assert_eq!(other_recipe.owner.cpu_snapshot(), other_dispatch);
                assert_eq!(
                    session.dependency_owner.custody_snapshot_for_test(),
                    terminal_dependency
                );
                assert_eq!(recipe.cancellations.len(), usize::from(cancel));
            }
            restored(&session);
            // CPU-only owners retain terminal metadata; this is not native disposal.
        }
    }
}

#[test]
fn source_rollback_and_publication_faults_are_terminal_and_preserve_uncertain_custody() {
    for auxiliary in [false, true] {
        for fault in [
            Fault::EventRelease,
            Fault::CompletionCancel,
            Fault::DispatchCancel,
            Fault::NativeTerminal,
            Fault::CompletionPublish,
            Fault::EventBind,
            Fault::DispatchPublish,
            Fault::Unwind,
        ] {
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert!(!take_lane_unwind_process_gate_record_v1());
            let (mut session, lane) = fixture(auxiliary);
            let ordinal = usize::from(auxiliary);
            let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
            let available = owner(&session, ordinal).state_snapshot_for_test().1;
            let mut recipe = Recipe::new();
            recipe.fail_cancel = matches!(fault, Fault::DispatchCancel);
            recipe.fail_publish = matches!(fault, Fault::DispatchPublish);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                session
                    .with_compute_lane_v1(lane, |selected| {
                        submit(selected, &mut recipe, |session, _| match fault {
                            Fault::EventRelease => {
                                session.completion_owner.poison_owner();
                                Err(ring_full())
                            }
                            Fault::CompletionCancel => {
                                session.completion_owner.invalidate_bound_phase_for_test();
                                Err(ring_full())
                            }
                            Fault::DispatchCancel => Err(ring_full()),
                            Fault::NativeTerminal => Err(NativeAqlSubmissionFailureV1::Terminal(
                                NativeAqlSubmissionErrorV1::CallbackPanic,
                            )),
                            Fault::CompletionPublish => {
                                session.completion_owner.poison_owner();
                                Ok(102)
                            }
                            Fault::DispatchPublish => Ok(102),
                            Fault::EventBind => Ok(1),
                            Fault::Unwind => std::panic::panic_any("source-publication-unwind"),
                        })
                    })
                    .unwrap()
            }));
            if matches!(fault, Fault::Unwind) {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"source-publication-unwind")
                );
                assert!(take_lane_unwind_process_gate_record_v1());
            } else {
                let failure = result.unwrap().unwrap_err();
                assert!(
                    matches!(failure, Gfx942FixedDispatchSubmissionFailureV1::Terminal(_)),
                    "{fault:?}"
                );
                if matches!(fault, Fault::DispatchCancel) {
                    assert!(matches!(
                        failure.into_error(),
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::StaleDispatchGeneration
                        )
                    ));
                }
                assert!(!take_lane_unwind_process_gate_record_v1());
            }
            assert!(take_dispatch_terminal_process_gate_record_v1());
            assert!(session.terminal_poisoned);
            assert_eq!(recipe.owner().live_epoch_count(), 1);
            let released = matches!(fault, Fault::CompletionCancel | Fault::DispatchCancel);
            assert_eq!(
                owner(&session, ordinal).dependency_ledger_counts_for_test(),
                (4, if released { 0 } else { 3 }, 0)
            );
            assert_eq!(
                owner(&session, ordinal).state_snapshot_for_test().1,
                available
                    - if matches!(fault, Fault::DispatchCancel) {
                        0
                    } else {
                        3
                    }
            );
            assert_eq!(
                owner(&session, 1 - ordinal).custody_snapshot_for_test(),
                untouched
            );
            restored(&session);
            assert!(
                session
                    .with_compute_lane_v1(lane, |_| panic!("terminal lane cannot run"))
                    .is_err()
            );
            // No native resources in these fixtures. Do not pretend retained custody was recycled.
        }
    }
}

#[test]
fn source_public_rejections_preserve_legacy_errors_and_acceptance_burn_order() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let before = owner(&session, usize::from(auxiliary)).custody_snapshot_for_test();
        let classified = session
            .with_compute_lane_v1(lane, |selected| {
                selected.submit_fixed_dispatch_with_dependency_events_classified_v1::<3>()
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            classified,
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            )
        ));
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test().1,
            Some(2)
        );
        let legacy = session
            .with_compute_lane_v1(lane, |selected| {
                selected.submit_fixed_dispatch_with_dependency_events_v1::<3>()
            })
            .unwrap()
            .unwrap_err();
        assert_eq!(
            format!("{:?}", classified.into_error()),
            format!("{legacy:?}")
        );
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test().1,
            Some(3)
        );
        let invalid = session
            .with_compute_lane_v1(lane, |selected| {
                selected.submit_fixed_dispatch_with_dependency_events_classified_v1::<0>()
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            invalid,
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "dependency source packet count must be 1 through 8192"
                )
            )
        ));
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test().1,
            Some(3)
        );
        assert_eq!(
            owner(&session, usize::from(auxiliary)).custody_snapshot_for_test(),
            before
        );
        assert!(!session.terminal_poisoned);
        restored(&session);
    }
}

#[test]
fn source_acceptance_exhaustion_terminalizes_before_recipe_or_native_work() {
    for auxiliary in [false, true] {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let (mut session, lane) = fixture(auxiliary);
        let before = owner(&session, usize::from(auxiliary)).custody_snapshot_for_test();
        session
            .dependency_owner
            .advance_to_last_acceptance_epoch_for_test();
        let refused = session
            .with_compute_lane_v1(lane, |selected| {
                selected.submit_fixed_dispatch_with_dependency_events_classified_v1::<3>()
            })
            .unwrap();
        assert!(matches!(
            refused,
            Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(_))
        ));
        assert_eq!(session.dependency_owner.custody_snapshot_for_test().1, None);
        let mut recipe = Recipe::new();
        let failure = session
            .with_compute_lane_v1(lane, |selected| {
                submit(selected, &mut recipe, |_, _| {
                    panic!("exhaustion cannot reach native work")
                })
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            failure,
            Gfx942FixedDispatchSubmissionFailureV1::Terminal(_)
        ));
        assert!(take_dispatch_terminal_process_gate_record_v1());
        assert!(session.terminal_poisoned);
        assert_eq!(recipe.owner().next_generation(), 1);
        assert_eq!(recipe.owner().live_epoch_count(), 0);
        assert_eq!(
            owner(&session, usize::from(auxiliary)).custody_snapshot_for_test(),
            before
        );
        restored(&session);
    }
}

#[test]
fn source_persistent_gate_and_poisoned_acceptance_preserve_no_effect_rejection() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let primary = session.key;
        let mut attached = persistent_compute_gate_test_session_v1(primary, 3);
        session.persistent_compute = attached.persistent_compute.take();
        let acceptance = session.dependency_owner.custody_snapshot_for_test();
        let before = owner(&session, usize::from(auxiliary)).custody_snapshot_for_test();
        let public = session.with_compute_lane_v1(lane, |selected| {
            selected.submit_fixed_dispatch_with_dependency_events_classified_v1::<3>()
        });
        if auxiliary {
            assert!(matches!(
                public,
                Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                ))
            ));
        } else {
            assert!(matches!(
                public,
                Ok(Err(
                    Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::ResourcePhase
                        )
                    )
                ))
            ));
        }
        // AUX selection rejects even earlier. Independently check the source
        // wrapper's persistent guard, which also protects primary publication.
        let failure = session
            .submit_fixed_dispatch_with_dependency_events_inner_v1::<3>(lane)
            .unwrap_err();
        assert!(matches!(
            failure,
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            )
        ));
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test(),
            acceptance
        );
        assert_eq!(
            owner(&session, usize::from(auxiliary)).custody_snapshot_for_test(),
            before
        );
        assert!(!session.terminal_poisoned);
        restored(&session);

        let (mut session, lane) = fixture(auxiliary);
        session.dependency_owner.poison();
        let acceptance = session.dependency_owner.custody_snapshot_for_test();
        let mut recipe = Recipe::new();
        let failure = session
            .with_compute_lane_v1(lane, |selected| {
                submit(selected, &mut recipe, |_, _| {
                    panic!("poisoned acceptance cannot reach native submission")
                })
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            failure,
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(_)
        ));
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test(),
            acceptance
        );
        assert_eq!(recipe.owner().next_generation(), 1);
        // Preserve the legacy pathological-state behavior: owner poison itself
        // rejects this source attempt without newly poisoning the session.
        assert!(!session.terminal_poisoned);
        restored(&session);
    }
}

#[test]
fn source_returned_bundle_survives_lane_callback_unwind_without_recycle() {
    for auxiliary in [false, true] {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        assert!(!take_lane_unwind_process_gate_record_v1());
        let (mut session, lane) = fixture(auxiliary);
        let ordinal = usize::from(auxiliary);
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let mut recipe = Recipe::new();
        let mut retained = None;
        let mut snapshot = None;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = session.with_compute_lane_v1(lane, |selected| {
                retained = Some(submit(selected, &mut recipe, |_, _| Ok(122)).unwrap());
                snapshot = Some(
                    selected
                        .session
                        .completion_owner
                        .custody_snapshot_for_test(),
                );
                std::panic::panic_any("after-source-bundle-return")
            });
        }))
        .unwrap_err();
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"after-source-bundle-return")
        );
        assert!(take_lane_unwind_process_gate_record_v1());
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        assert!(session.terminal_poisoned);
        assert_eq!(
            owner(&session, ordinal).custody_snapshot_for_test(),
            snapshot.unwrap()
        );
        assert_eq!(
            owner(&session, ordinal).dependency_ledger_counts_for_test(),
            (4, 3, 0)
        );
        let (batch, events) = retained.unwrap().into_parts();
        let (completion, identity) = unwrap_published(batch);
        let occurrence = recipe
            .owner()
            .validate_published(identity, &completion)
            .unwrap();
        assert_eq!(occurrence.first_packet_id, 120);
        assert_eq!(occurrence.last_packet_id, 122);
        assert_eq!(events.len(), 3);
        for event in &events {
            assert_eq!(event.lane, lane);
            assert_eq!(
                event.event.binding_state(),
                super::super::super::completion::Gfx942ComputeEventBindingStateV1::Bound
            );
        }
        assert_eq!(
            owner(&session, 1 - ordinal).custody_snapshot_for_test(),
            untouched
        );
        assert_eq!(recipe.owner().live_epoch_count(), 1);
        restored(&session);
    }
}

#[test]
fn source_native_forwarding_source_wiring_smoke() {
    fn span(source: &str, start: &str, end: &str) -> String {
        source
            .split_once(start)
            .unwrap()
            .1
            .split_once(end)
            .unwrap()
            .0
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }
    let facade = include_str!("../queue_live.rs");
    let fixed = include_str!("../queue_live/fixed_dispatch.rs");
    let legacy = span(
        facade,
        "pub fn submit_fixed_dispatch_with_dependency_events_v1",
        "pub fn submit_fixed_dispatch_with_dependency_events_classified_v1",
    );
    assert!(legacy.contains("self.submit_fixed_dispatch_with_dependency_events_classified_v1::<N>().map_err(Gfx942FixedDispatchSubmissionFailureV1::into_error)"));
    let classified = span(
        facade,
        "pub fn submit_fixed_dispatch_with_dependency_events_classified_v1",
        "pub fn poll_fixed_dispatch",
    );
    assert!(classified.contains(
        "self.session.submit_fixed_dispatch_with_dependency_events_inner_v1::<N>(self.lane)"
    ));
    let native = span(
        fixed,
        "fn submit_fixed_dispatch_with_dependency_events_inner_v1",
        "fn submit_dependency_source_using_v1",
    );
    assert!(native.contains("self.submit_dependency_source_using_v1(lane,&mutNativeDependencySourceRecipeV1,|session,packets|session.submit_prepared_batch_classified(packets),)"));
    let recipe = span(
        fixed,
        "impl<const N: usize> DependencySourceRecipeV1<N> for NativeDependencySourceRecipeV1",
        "impl CheckedGfx942XnackMinusDevice",
    );
    assert!(recipe.contains(".bind_templates::<N>(session.key)"));
    assert!(recipe.contains(".mark_published(identity,completion)"));
    assert!(recipe.contains(
        "native_dependency_source_cancel_body!(dependency_source_rust_expr,session,identity)"
    ));
    let forwarding: String = include_str!("../queue_live/dependency_source_failure_body.rs")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(forwarding.contains("$session.dispatch.as_mut().expect(\"dependencysourcedispatchownerremainsretained\").cancel_binding($identity)"));
}
