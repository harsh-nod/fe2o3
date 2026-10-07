use super::*;
use crate::queue::completion::{
    CompletionDispatchGenerationBindingV1, CompletionPacketTemplateV1,
    ComputeDependencyTargetSubstitutionV1, substitute_dependency_target_component_for_test,
};
use crate::queue::submit::{
    initialize_invalid_ring, publish_slot_header_release, write_unpublished_slot,
};
use core::sync::atomic::{AtomicU64, Ordering};
use fe2o3_aql::{AqlDispatchGeometryV1, AqlDispatchOrderingV1, ObservedGpuAddressV1};
use fe2o3_runtime_model::{
    AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, DeviceKeyV1, MappingIdV1,
    MemoryAllocationKeyV1, MemoryMappingKeyV1, PhysicalDeviceIdV1, QueueGenerationV1,
    QueueInstanceIdV1, QueueKeyV1, VmIdV1, VmKeyV1,
};
use sha2::{Digest, Sha256};

#[repr(align(64))]
struct DependencyRing([u8; 4_096]);

struct DependencyBackend {
    ring: DependencyRing,
    write: AtomicU64,
    read: AtomicU64,
    checks: usize,
    fail_check: Option<usize>,
    panic_check: Option<usize>,
}

struct CompletionBackend {
    observation: fe2o3_aql::AqlCompletionObservationV1,
    reset_calls: usize,
}

impl NativeCompletionSignalBackendV1 for CompletionBackend {
    fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
        Ok(())
    }

    fn observe_one_acquire_in_current_scope(
        &mut self,
        _slot_index: u32,
    ) -> Result<fe2o3_aql::AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        Ok(self.observation)
    }

    fn observe_batch_acquire_in_current_scope(
        &mut self,
        slot_indices: &[u32],
    ) -> Result<Vec<fe2o3_aql::AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
        Ok(vec![self.observation; slot_indices.len()])
    }

    fn reset_pending_release(&mut self, _slot_index: u32) -> Result<(), Gfx942CompletionErrorV1> {
        self.reset_calls += 1;
        Ok(())
    }
}

impl DependencyBackend {
    fn new(write: u64, read: u64) -> Self {
        let mut ring = DependencyRing([0; 4_096]);
        initialize_invalid_ring(&mut ring.0).unwrap();
        Self {
            ring,
            write: AtomicU64::new(write),
            read: AtomicU64::new(read),
            checks: 0,
            fail_check: None,
            panic_check: None,
        }
    }
}

impl NativeAqlSubmissionBackendV1 for DependencyBackend {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.checks += 1;
        assert_ne!(self.panic_check, Some(self.checks), "injected claim panic");
        if self.fail_check == Some(self.checks) {
            Err(NativeAqlSubmissionErrorV1::Currentness)
        } else {
            Ok(())
        }
    }

    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
        Ok((
            self.write.load(Ordering::Acquire),
            self.read.load(Ordering::Acquire),
        ))
    }

    fn fetch_add_write_acq_rel(
        &mut self,
        increment: u64,
    ) -> Result<u64, NativeAqlSubmissionErrorV1> {
        Ok(self.write.fetch_add(increment, Ordering::AcqRel))
    }

    fn write_unpublished(
        &mut self,
        slot: u32,
        packet: &[u8; 64],
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        write_unpublished_slot(&mut self.ring.0, slot, packet)
    }

    fn publish_release_header(
        &mut self,
        slot: u32,
        header: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        publish_slot_header_release(&mut self.ring.0, slot, header)
    }

    fn ring_doorbell_release(&mut self, _packet_id: u64) -> Result<(), NativeAqlSubmissionErrorV1> {
        Ok(())
    }
}

fn queue(id: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: VmKeyV1 {
            device: DeviceKeyV1 {
                physical: PhysicalDeviceIdV1(1),
                generation: DeviceGenerationV1(1),
            },
            id: VmIdV1(1),
        },
        id: QueueInstanceIdV1(id),
        generation: QueueGenerationV1(1),
    }
}

fn mapping(id: u64) -> MemoryMappingKeyV1 {
    MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 {
            vm: queue(1).vm,
            id: AllocationIdV1(id),
            generation: AllocationGenerationV1(1),
        },
        id: MappingIdV1(id),
    }
}

fn template(queue: QueueKeyV1, generation: u64) -> CompletionPacketTemplateV1 {
    CompletionPacketTemplateV1::new(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        AqlDispatchOrderingV1::WaitForPrior,
        0,
        0,
        ObservedGpuAddressV1::new(0x40_0000 + generation * 0x100).unwrap(),
        ObservedGpuAddressV1::new(0x50_0000 + generation * 0x100).unwrap(),
        16,
        CompletionDispatchGenerationBindingV1::new(
            queue,
            mapping(100 + generation),
            mapping(200 + generation),
            generation,
        ),
    )
}

fn real_prepared_target() -> (
    CompletionSignalArenaOwnerV1,
    Gfx942CompletionBatchV1<1>,
    CompletionSignalArenaOwnerV1,
    ComputeDependencySessionOwnerV1,
    PreparedComputeDependencyTargetUseV1,
    NativeAqlSubmissionOwnerV1,
) {
    const SESSION: u64 = 7;
    let source_queue = queue(1);
    let target_queue = queue(2);
    let mut source_owner =
        CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(source_queue);
    let source_bound = source_owner
        .bind_batch([template(source_queue, 11)])
        .unwrap();
    let (_source_packets, source_retention) = source_bound.into_parts();
    let source_event = source_owner
        .record_dependency_event_v1(SESSION, 1, &source_retention)
        .unwrap();
    let source_batch = source_owner.mark_published(source_retention, 100).unwrap();
    let source_event = source_owner
        .bind_dependency_event_v1(source_event, &source_batch)
        .unwrap();

    let mut acceptance_owner = ComputeDependencySessionOwnerV1::new(SESSION).unwrap();
    let source_epoch = acceptance_owner.reserve_acceptance_epoch().unwrap();
    assert_eq!(source_epoch.epoch(), 1);
    let acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
    let reader =
        retain_dependency_reader_for_target_v1(&mut source_owner, source_event, &acceptance)
            .unwrap();

    let mut target_owner =
        CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(target_queue);
    let target_bound = target_owner
        .bind_batch([template(target_queue, 21)])
        .unwrap();
    let target = target_owner
        .prepare_dependency_target_v1(
            acceptance.session_occurrence(),
            acceptance.epoch(),
            target_bound,
        )
        .unwrap();
    let prepared = acceptance_owner
        .begin_target_use(&target_owner, acceptance, target, vec![reader])
        .unwrap();
    (
        source_owner,
        source_batch,
        target_owner,
        acceptance_owner,
        prepared,
        NativeAqlSubmissionOwnerV1::new(4_096).unwrap(),
    )
}

fn published_source_event(
    owner: &mut CompletionSignalArenaOwnerV1,
    source_queue: QueueKeyV1,
    generation: u64,
    packet_id: u64,
) -> Gfx942ComputeEventOccurrenceV1 {
    let bound = owner
        .bind_batch([template(source_queue, generation)])
        .unwrap();
    let (_packets, retention) = bound.into_parts();
    let event = owner.record_dependency_event_v1(7, 1, &retention).unwrap();
    let batch = owner.mark_published(retention, packet_id).unwrap();
    owner.bind_dependency_event_v1(event, &batch).unwrap()
}

fn occurrence(
    session: u64,
    epoch: u64,
    queue_id: u64,
    slot: u32,
    packet_id: Option<u64>,
) -> ComputeDependencyOccurrenceIdentityV1 {
    ComputeDependencyOccurrenceIdentityV1 {
        session_occurrence: session,
        acceptance_epoch: epoch,
        batch_id: epoch,
        queue: queue(queue_id),
        signal_mapping: mapping(queue_id),
        slot_index: slot,
        slot_generation: epoch,
        dispatch_generation: epoch,
        packet_id,
    }
}

#[test]
fn acceptance_epochs_are_unique_monotonic_and_burned() {
    let mut owner = ComputeDependencySessionOwnerV1::new(7).unwrap();
    {
        let first = owner.reserve_acceptance_epoch().unwrap();
        assert_eq!(first.epoch, 1);
    }
    let second = owner.reserve_acceptance_epoch().unwrap();
    assert_eq!(second.epoch, 2);
    assert_eq!(owner.next_epoch(), Some(3));
}

#[test]
fn final_nonzero_epoch_is_issued_once_then_exhaustion_poison_is_permanent() {
    let mut owner = ComputeDependencySessionOwnerV1::new(7).unwrap();
    owner.next_acceptance_epoch = Some(u64::MAX);
    assert_eq!(owner.reserve_acceptance_epoch().unwrap().epoch(), u64::MAX);
    assert_eq!(owner.next_epoch(), None);
    assert_eq!(
        owner.reserve_acceptance_epoch(),
        Err(ComputeDependencyTargetUseErrorV1::AcceptanceEpochExhausted)
    );
    assert_eq!(
        owner.reserve_acceptance_epoch(),
        Err(ComputeDependencyTargetUseErrorV1::Poisoned)
    );
}

#[test]
fn isolated_foundation_owners_show_why_production_construction_stays_private() {
    let mut first = ComputeDependencySessionOwnerV1::new(7).unwrap();
    let mut duplicate = ComputeDependencySessionOwnerV1::new(7).unwrap();
    assert_eq!(first.reserve_acceptance_epoch().unwrap().epoch(), 1);
    assert_eq!(duplicate.reserve_acceptance_epoch().unwrap().epoch(), 1);
}

#[test]
fn cross_batch_target_components_fail_before_active_or_native_mutation() {
    for substitution in [
        ComputeDependencyTargetSubstitutionV1::FinalDispatch,
        ComputeDependencyTargetSubstitutionV1::Retention,
        ComputeDependencyTargetSubstitutionV1::Event,
    ] {
        let target_queue = queue(2);
        let mut target_owner =
            CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(target_queue);
        let first_bound = target_owner
            .bind_batch([template(target_queue, 21)])
            .unwrap();
        let second_bound = target_owner
            .bind_batch([template(target_queue, 22)])
            .unwrap();
        let first = target_owner
            .prepare_dependency_target_v1(7, 1, first_bound)
            .unwrap();
        let second = target_owner
            .prepare_dependency_target_v1(7, 2, second_bound)
            .unwrap();
        let (hostile, _other) =
            substitute_dependency_target_component_for_test(first, second, substitution);

        let mut acceptance_owner = ComputeDependencySessionOwnerV1::new(7).unwrap();
        let acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
        let backend = DependencyBackend::new(0, 0);
        let before = backend.ring.0;
        let failure =
            match acceptance_owner.begin_target_use(&target_owner, acceptance, hostile, Vec::new())
            {
                Err(failure) => failure,
                Ok(_) => panic!("cross-batch target substitution was admitted"),
            };
        let (error, returned_acceptance, _returned_target, returned_readers) = failure.into_parts();
        assert!(matches!(
            error,
            ComputeDependencyTargetUseErrorV1::Completion(_)
        ));
        assert_eq!(returned_acceptance.epoch(), 1);
        assert!(returned_readers.is_empty());
        assert!(acceptance_owner.active.is_empty());
        assert_eq!(backend.write.load(Ordering::Relaxed), 0);
        assert_eq!(backend.ring.0, before);
    }
}

#[test]
fn target_use_requires_cross_queue_earlier_distinct_sources() {
    let acceptance = ComputeDependencyAcceptanceV1 {
        session_occurrence: 7,
        epoch: 4,
    };
    let target = occurrence(7, 4, 2, 0, None);
    let first = occurrence(7, 1, 1, 3, Some(10));
    let second = occurrence(7, 3, 1, 4, Some(11));
    assert_eq!(
        validate_target_use_v1(7, &acceptance, target, &[first, second]),
        Ok(())
    );

    let cases = [
        (
            occurrence(8, 1, 1, 3, Some(10)),
            ComputeDependencyTargetUseErrorV1::CrossSessionDependency,
        ),
        (
            occurrence(7, 1, 2, 3, Some(10)),
            ComputeDependencyTargetUseErrorV1::SameQueueDependency,
        ),
        (
            occurrence(7, 4, 1, 3, Some(10)),
            ComputeDependencyTargetUseErrorV1::SelfDependency,
        ),
        (
            occurrence(7, 5, 1, 3, Some(10)),
            ComputeDependencyTargetUseErrorV1::DependencyCycle,
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(
            validate_target_use_v1(7, &acceptance, target, &[source]),
            Err(expected)
        );
    }
    assert_eq!(
        validate_target_use_v1(7, &acceptance, target, &[first, first]),
        Err(ComputeDependencyTargetUseErrorV1::DuplicateDependency)
    );
    assert_eq!(
        validate_target_use_v1(
            7,
            &acceptance,
            target,
            &[first, occurrence(7, 2, 3, 4, Some(11))],
        ),
        Err(ComputeDependencyTargetUseErrorV1::SourceOwnerRosterMismatch)
    );
}

#[test]
fn target_use_rejects_empty_oversized_and_prepublished_sources() {
    let acceptance = ComputeDependencyAcceptanceV1 {
        session_occurrence: 7,
        epoch: 4,
    };
    let target = occurrence(7, 4, 2, 0, None);
    assert_eq!(
        validate_target_use_v1(7, &acceptance, target, &[]),
        Err(ComputeDependencyTargetUseErrorV1::EmptyDependencyRoster)
    );
    let oversized = vec![occurrence(7, 1, 1, 3, Some(10)); 257];
    assert_eq!(
        validate_target_use_v1(7, &acceptance, target, &oversized),
        Err(ComputeDependencyTargetUseErrorV1::TooManyDependencies)
    );
    assert_eq!(
        validate_target_use_v1(7, &acceptance, target, &[occurrence(7, 1, 1, 3, None)]),
        Err(ComputeDependencyTargetUseErrorV1::InvalidTargetIdentity)
    );

    let maximum = (0..AQL_MAX_DEPENDENCY_SIGNALS_V1)
        .map(|slot| occurrence(7, 1, 1, slot as u32, Some(u64::from(slot as u32) + 10)))
        .collect::<Vec<_>>();
    assert_eq!(
        validate_target_use_v1(7, &acceptance, target, &maximum),
        Ok(())
    );
}

#[test]
fn occupancy_rollback_releases_real_readers_in_reverse_and_returns_events_in_order() {
    const SESSION: u64 = 7;
    let source_queue = queue(1);
    let target_queue = queue(2);
    let mut source_owner =
        CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(source_queue);
    let source_bound = source_owner
        .bind_batch([
            template(source_queue, 11),
            template(source_queue, 12),
            template(source_queue, 13),
        ])
        .unwrap();
    let (_source_packets, source_retention) = source_bound.into_parts();
    let mut events = Vec::new();
    for index in 0..3 {
        events.push(
            source_owner
                .record_dependency_event_at_v1(SESSION, 1, &source_retention, index)
                .unwrap(),
        );
    }
    let source_batch = source_owner.mark_published(source_retention, 102).unwrap();
    let events = events
        .into_iter()
        .enumerate()
        .map(|(index, event)| {
            source_owner
                .bind_dependency_event_at_v1(event, &source_batch, index)
                .unwrap()
        })
        .collect::<Vec<_>>();

    let mut acceptance_owner = ComputeDependencySessionOwnerV1::new(SESSION).unwrap();
    let source_epoch = acceptance_owner.reserve_acceptance_epoch().unwrap();
    assert_eq!(source_epoch.epoch(), 1);
    let acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
    assert_eq!(acceptance.epoch(), 2);
    let mut readers = Vec::new();
    for event in events {
        readers.push(
            retain_dependency_reader_for_target_v1(&mut source_owner, event, &acceptance).unwrap(),
        );
    }

    let mut target_owner =
        CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(target_queue);
    let target_bound = target_owner
        .bind_batch([template(target_queue, 21)])
        .unwrap();
    let target = target_owner
        .prepare_dependency_target_v1(
            acceptance.session_occurrence(),
            acceptance.epoch(),
            target_bound,
        )
        .unwrap();
    let prepared = acceptance_owner
        .begin_target_use(&target_owner, acceptance, target, readers)
        .unwrap();

    let mut submission = NativeAqlSubmissionOwnerV1::from_counters(4_096, 64, 0).unwrap();
    let mut backend = DependencyBackend::new(64, 0);
    let retryable = match acceptance_owner
        .publish_native(prepared, &mut submission, &mut backend)
        .unwrap_err()
    {
        ComputeDependencyPublicationFailureV1::Retryable(custody) => custody,
        ComputeDependencyPublicationFailureV1::Terminal(_) => {
            panic!("ring occupancy unexpectedly became terminal")
        }
    };
    assert!(matches!(
        retryable.error,
        NativeAqlSubmissionErrorV1::Ring(fe2o3_aql::AqlRingReservationError::Full)
    ));
    let mut source_owners = [&mut source_owner];
    let cancelled = acceptance_owner
        .rollback_retryable_before_side_effect(retryable, &mut source_owners, &mut target_owner)
        .unwrap();
    assert_eq!(cancelled.events.len(), 3);
    let returned_slots = cancelled
        .events
        .iter()
        .map(|event| {
            source_owner
                .dependency_source_identity_v1(event)
                .unwrap()
                .slot_index
        })
        .collect::<Vec<_>>();
    assert_eq!(returned_slots, [0, 1, 2]);
    for event in cancelled.events {
        let (event, lease) = source_owner
            .retain_dependency_reader_v1(event, SESSION, 2)
            .unwrap();
        source_owner.release_dependency_reader_v1(lease).unwrap();
        source_owner.release_dependency_event_v1(event).unwrap();
    }
    target_owner
        .cancel_bound(cancelled.target_completion)
        .unwrap();
    assert_eq!(
        acceptance_owner.reserve_acceptance_epoch().unwrap().epoch(),
        3
    );
}

#[test]
fn claim_currentness_and_panic_keep_real_target_and_reader_authority_terminal() {
    for panic in [false, true] {
        let (
            source_owner,
            _source_batch,
            target_owner,
            mut acceptance_owner,
            prepared,
            mut submission,
        ) = real_prepared_target();
        let mut backend = DependencyBackend::new(0, 0);
        if panic {
            backend.panic_check = Some(3);
        } else {
            backend.fail_check = Some(3);
        }
        let terminal = match acceptance_owner
            .publish_native(prepared, &mut submission, &mut backend)
            .unwrap_err()
        {
            ComputeDependencyPublicationFailureV1::Terminal(custody) => custody,
            ComputeDependencyPublicationFailureV1::Retryable(_) => {
                panic!("claim attempt incorrectly yielded retryable target authority")
            }
        };
        assert_eq!(
            terminal.boundary,
            Some(AqlDependencyDispatchPublicationBoundaryV1::ClaimWriteIndex)
        );
        assert!(matches!(
            terminal.error,
            NativeAqlSubmissionErrorV1::Currentness | NativeAqlSubmissionErrorV1::CallbackPanic
        ));
        assert_eq!(terminal.readers.len(), 1);
        assert!(matches!(
            terminal.target_completion,
            TerminalComputeDependencyCompletionV1::Bound { .. }
        ));
        assert_eq!(
            acceptance_owner.reserve_acceptance_epoch(),
            Err(ComputeDependencyTargetUseErrorV1::Poisoned)
        );
        assert_eq!(
            target_owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::BatchStillRetained)
        );
        assert_eq!(
            source_owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::BatchStillRetained)
        );
    }
}

#[test]
fn successful_native_publication_binds_exact_target_without_source_polling() {
    let (
        source_owner,
        _source_batch,
        mut target_owner,
        mut acceptance_owner,
        prepared,
        mut submission,
    ) = real_prepared_target();
    let mut backend = DependencyBackend::new(0, 0);
    let native = acceptance_owner
        .publish_native(prepared, &mut submission, &mut backend)
        .unwrap();
    assert_eq!(backend.checks, 4);
    assert_eq!(backend.write.load(Ordering::Relaxed), 2);
    let (published, target_event) = acceptance_owner
        .bind_published_target(native, &mut target_owner)
        .unwrap()
        .into_parts();
    assert_eq!(
        published.key.phase,
        ComputeDependencyTargetUsePhaseV1::Published
    );
    assert_eq!(published.key.target.packet_id, Some(1));
    assert_eq!(published.readers.len(), 1);
    assert_eq!(
        target_event.binding_state(),
        super::super::completion::Gfx942ComputeEventBindingStateV1::Bound
    );
    assert_eq!(published.publication.dependency_count(), 1);
    assert_eq!(
        source_owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::BatchStillRetained)
    );
    assert_eq!(
        target_owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::BatchStillRetained)
    );
}

#[test]
fn published_target_owner_substitution_is_terminal() {
    let (
        mut source_owner,
        _source_batch,
        target_owner,
        mut acceptance_owner,
        prepared,
        mut submission,
    ) = real_prepared_target();
    let mut backend = DependencyBackend::new(0, 0);
    let native = acceptance_owner
        .publish_native(prepared, &mut submission, &mut backend)
        .unwrap();
    let terminal = acceptance_owner
        .bind_published_target(native, &mut source_owner)
        .unwrap_err();
    assert_eq!(
        terminal.boundary,
        Some(AqlDependencyDispatchPublicationBoundaryV1::Doorbell)
    );
    assert!(matches!(
        terminal.target_completion,
        TerminalComputeDependencyCompletionV1::Bound { .. }
    ));
    assert_eq!(
        acceptance_owner.reserve_acceptance_epoch(),
        Err(ComputeDependencyTargetUseErrorV1::Poisoned)
    );
    assert_eq!(
        target_owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::BatchStillRetained)
    );
}

#[test]
fn exact_target_completion_releases_source_pins_once_then_enables_recycle() {
    let (
        mut source_owner,
        source_batch,
        mut target_owner,
        mut acceptance_owner,
        prepared,
        mut submission,
    ) = real_prepared_target();

    let mut native_backend = DependencyBackend::new(0, 0);
    let native = acceptance_owner
        .publish_native(prepared, &mut submission, &mut native_backend)
        .unwrap();
    let (published, target_event) = acceptance_owner
        .bind_published_target(native, &mut target_owner)
        .unwrap()
        .into_parts();

    let mut pending_backend = CompletionBackend {
        observation: fe2o3_aql::AqlCompletionObservationV1::Pending,
        reset_calls: 0,
    };
    let published = match acceptance_owner
        .observe_published_target_once(published, &mut target_owner, &mut pending_backend)
        .unwrap()
    {
        ComputeDependencyTargetPollV1::Pending(published) => published,
        ComputeDependencyTargetPollV1::Ready(_) => panic!("pending target completed"),
    };

    let mut complete_backend = CompletionBackend {
        observation: fe2o3_aql::AqlCompletionObservationV1::Completed,
        reset_calls: 0,
    };
    let source_completed = match source_owner
        .observe_once(source_batch, &mut complete_backend)
        .unwrap()
    {
        super::super::completion::Gfx942CompletionPollV1::Ready(completed) => completed,
        super::super::completion::Gfx942CompletionPollV1::Pending(_) => {
            panic!("completed source remained pending")
        }
    };
    let source_completed =
        match source_owner.recycle_retaining(source_completed, &mut complete_backend) {
            Err((Gfx942CompletionErrorV1::SignalPinned { .. }, completed)) => completed,
            other => panic!("source recycle was not pinned: {other:?}"),
        };
    assert_eq!(complete_backend.reset_calls, 0);

    let completed = match acceptance_owner
        .observe_published_target_once(published, &mut target_owner, &mut complete_backend)
        .unwrap()
    {
        ComputeDependencyTargetPollV1::Ready(completed) => completed,
        ComputeDependencyTargetPollV1::Pending(_) => {
            panic!("completed target remained pending")
        }
    };
    let released = acceptance_owner
        .release_after_dependent_completion(completed, &target_owner, &mut source_owner)
        .unwrap();
    assert_eq!(released.dependency_count, 1);
    assert!(acceptance_owner.active.is_empty());

    source_owner
        .recycle_retaining(source_completed, &mut complete_backend)
        .unwrap();
    assert_eq!(complete_backend.reset_calls, 1);
    let target_completed = released.target_completion;
    let target_completed =
        match target_owner.recycle_retaining(target_completed, &mut complete_backend) {
            Err((Gfx942CompletionErrorV1::SignalPinned { .. }, completed)) => completed,
            other => panic!("target recycle was not event-pinned: {other:?}"),
        };
    target_owner
        .release_dependency_event_v1(target_event)
        .unwrap();
    target_owner
        .recycle_retaining(target_completed, &mut complete_backend)
        .unwrap();
    assert_eq!(complete_backend.reset_calls, 2);
    assert_eq!(source_owner.ensure_releasable(), Ok(()));
    assert_eq!(target_owner.ensure_releasable(), Ok(()));
}

#[test]
fn published_target_event_can_feed_a_second_target_before_first_completion() {
    let (
        mut source_owner,
        _source_batch,
        mut middle_owner,
        mut acceptance_owner,
        prepared_middle,
        mut submission,
    ) = real_prepared_target();
    let mut native_backend = DependencyBackend::new(0, 0);
    let native_middle = acceptance_owner
        .publish_native(prepared_middle, &mut submission, &mut native_backend)
        .unwrap();
    let (published_middle, middle_event) = acceptance_owner
        .bind_published_target(native_middle, &mut middle_owner)
        .unwrap()
        .into_parts();

    let final_acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
    assert_eq!(final_acceptance.epoch(), 3);
    let middle_reader =
        retain_dependency_reader_for_target_v1(&mut middle_owner, middle_event, &final_acceptance)
            .unwrap();
    let final_queue = queue(3);
    let mut final_owner =
        CompletionSignalArenaOwnerV1::for_persistent_compute_cancellation_test(final_queue);
    let final_bound = final_owner.bind_batch([template(final_queue, 31)]).unwrap();
    let final_target = final_owner
        .prepare_dependency_target_v1(
            final_acceptance.session_occurrence(),
            final_acceptance.epoch(),
            final_bound,
        )
        .unwrap();
    let _prepared_final = acceptance_owner
        .begin_target_use(
            &final_owner,
            final_acceptance,
            final_target,
            vec![middle_reader],
        )
        .unwrap();
    assert_eq!(acceptance_owner.active.len(), 2);

    let mut completed_backend = CompletionBackend {
        observation: fe2o3_aql::AqlCompletionObservationV1::Completed,
        reset_calls: 0,
    };
    let completed_middle = match acceptance_owner
        .observe_published_target_once(published_middle, &mut middle_owner, &mut completed_backend)
        .unwrap()
    {
        ComputeDependencyTargetPollV1::Ready(completed) => completed,
        ComputeDependencyTargetPollV1::Pending(_) => panic!("middle target remained pending"),
    };
    acceptance_owner
        .release_after_dependent_completion(completed_middle, &middle_owner, &mut source_owner)
        .unwrap();
    assert_eq!(acceptance_owner.active.len(), 1);
    assert_eq!(
        middle_owner.ensure_releasable(),
        Err(Gfx942CompletionErrorV1::BatchStillRetained)
    );
}

#[test]
fn active_target_capacity_accepts_128_rejects_129_without_effect_and_reuses_release() {
    let (
        mut source_owner,
        _source_batch,
        mut target_owner,
        mut acceptance_owner,
        first_prepared,
        mut submission,
    ) = real_prepared_target();
    assert!(acceptance_owner.active.capacity() >= MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1);

    for epoch in 10_000..10_126 {
        let target = occurrence(7, epoch, 100 + epoch, epoch as u32, None);
        assert!(
            acceptance_owner
                .active
                .insert(
                    epoch,
                    ActiveComputeDependencyTargetUseV1 {
                        target,
                        dependency_count: 1,
                        phase: ComputeDependencyTargetUsePhaseV1::Published,
                    },
                )
                .is_none()
        );
    }
    assert_eq!(acceptance_owner.active.len(), 127);

    let second_acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
    let second_event = published_source_event(&mut source_owner, queue(1), 12, 101);
    let second_reader =
        retain_dependency_reader_for_target_v1(&mut source_owner, second_event, &second_acceptance)
            .unwrap();
    let second_bound = target_owner.bind_batch([template(queue(2), 22)]).unwrap();
    let second_target = target_owner
        .prepare_dependency_target_v1(7, second_acceptance.epoch(), second_bound)
        .unwrap();
    let _second_prepared = acceptance_owner
        .begin_target_use(
            &target_owner,
            second_acceptance,
            second_target,
            vec![second_reader],
        )
        .unwrap();
    assert_eq!(
        acceptance_owner.active.len(),
        MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1
    );

    let rejected_acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
    let rejected_epoch = rejected_acceptance.epoch();
    let rejected_event = published_source_event(&mut source_owner, queue(1), 13, 102);
    let rejected_reader = retain_dependency_reader_for_target_v1(
        &mut source_owner,
        rejected_event,
        &rejected_acceptance,
    )
    .unwrap();
    let rejected_bound = target_owner.bind_batch([template(queue(2), 23)]).unwrap();
    let rejected_target = target_owner
        .prepare_dependency_target_v1(7, rejected_epoch, rejected_bound)
        .unwrap();
    let rejected_identity = rejected_target.identity();
    let native_backend = DependencyBackend::new(0, 0);
    let ring_before = native_backend.ring.0;
    let failure = match acceptance_owner.begin_target_use(
        &target_owner,
        rejected_acceptance,
        rejected_target,
        vec![rejected_reader],
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("the 129th active target was admitted"),
    };
    let (error, returned_acceptance, returned_target, returned_readers) = failure.into_parts();
    assert_eq!(
        error,
        ComputeDependencyTargetUseErrorV1::ActiveTargetCapacity
    );
    assert_eq!(returned_acceptance.epoch(), rejected_epoch);
    assert_eq!(returned_target.identity(), rejected_identity);
    assert_eq!(returned_readers.len(), 1);
    assert_eq!(native_backend.write.load(Ordering::Relaxed), 0);
    assert_eq!(native_backend.ring.0, ring_before);
    assert_eq!(
        acceptance_owner.active.len(),
        MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1
    );

    let mut native_backend = DependencyBackend::new(0, 0);
    let native_first = acceptance_owner
        .publish_native(first_prepared, &mut submission, &mut native_backend)
        .unwrap();
    let (published_first, _target_event) = acceptance_owner
        .bind_published_target(native_first, &mut target_owner)
        .unwrap()
        .into_parts();
    let mut completion_backend = CompletionBackend {
        observation: fe2o3_aql::AqlCompletionObservationV1::Completed,
        reset_calls: 0,
    };
    let completed_first = match acceptance_owner
        .observe_published_target_once(published_first, &mut target_owner, &mut completion_backend)
        .unwrap()
    {
        ComputeDependencyTargetPollV1::Ready(completed) => completed,
        ComputeDependencyTargetPollV1::Pending(_) => panic!("target remained pending"),
    };
    acceptance_owner
        .release_after_dependent_completion(completed_first, &target_owner, &mut source_owner)
        .unwrap();
    assert_eq!(acceptance_owner.active.len(), 127);

    acceptance_owner
        .begin_target_use(
            &target_owner,
            returned_acceptance,
            returned_target,
            returned_readers,
        )
        .expect("a completed target must release one active-record slot");
    assert_eq!(
        acceptance_owner.active.len(),
        MAX_ACTIVE_DEPENDENCY_TARGETS_PER_SESSION_V1
    );
}

#[test]
fn mixed_source_owners_are_rejected_before_native_publication() {
    let first_queue = queue(1);
    let second_queue = queue(2);
    let target_queue = queue(3);
    let mut first_owner =
        CompletionSignalArenaOwnerV1::for_dependency_test(first_queue, 101, 0x10_000);
    let mut second_owner =
        CompletionSignalArenaOwnerV1::for_dependency_test(second_queue, 102, 0x20_000);
    let first_event = published_source_event(&mut first_owner, first_queue, 11, 100);
    let second_event = published_source_event(&mut second_owner, second_queue, 12, 200);
    let mut acceptance_owner = ComputeDependencySessionOwnerV1::new(7).unwrap();
    acceptance_owner.reserve_acceptance_epoch().unwrap();
    let acceptance = acceptance_owner.reserve_acceptance_epoch().unwrap();
    let first_reader =
        retain_dependency_reader_for_target_v1(&mut first_owner, first_event, &acceptance).unwrap();
    let second_reader =
        retain_dependency_reader_for_target_v1(&mut second_owner, second_event, &acceptance)
            .unwrap();
    let mut target_owner =
        CompletionSignalArenaOwnerV1::for_dependency_test(target_queue, 103, 0x30_000);
    let target_bound = target_owner
        .bind_batch([template(target_queue, 21)])
        .unwrap();
    let target = target_owner
        .prepare_dependency_target_v1(7, acceptance.epoch(), target_bound)
        .unwrap();
    let failure = acceptance_owner.begin_target_use(
        &target_owner,
        acceptance,
        target,
        vec![first_reader, second_reader],
    );
    let failure = match failure {
        Err(failure) => failure,
        Ok(_) => panic!("mixed-owner foundation preparation was admitted"),
    };
    let (error, _acceptance, _target, readers) = failure.into_parts();
    assert_eq!(
        error,
        ComputeDependencyTargetUseErrorV1::SourceOwnerRosterMismatch
    );
    assert_eq!(readers.len(), 2);
    assert!(acceptance_owner.active.is_empty());
    assert_eq!(first_owner.dependency_reader_count_for_test(), 1);
    assert_eq!(second_owner.dependency_reader_count_for_test(), 1);
}

#[test]
fn dependency_publisher_foundation_manifest_digest_is_frozen() {
    let digest = Sha256::digest(GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        rendered,
        GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_SHA256_V1
    );
}
