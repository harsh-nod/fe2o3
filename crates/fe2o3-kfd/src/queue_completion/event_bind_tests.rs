struct EventBindFixture {
    owner: CompletionSignalArenaOwnerV1,
    batch: Gfx942CompletionBatchV1<3>,
    events: Vec<Gfx942ComputeEventOccurrenceV1>,
    alias: (
        Gfx942ComputeEventOccurrenceV1,
        Gfx942ComputeDependencyReaderLeaseV1,
    ),
    neighbor: (
        Gfx942ComputeEventOccurrenceV1,
        Gfx942ComputeDependencyReaderLeaseV1,
    ),
    neighbor_batch: Gfx942CompletionBatchV1<1>,
}

fn event_bind_fixture(last: u64) -> EventBindFixture {
    let mut owner = owner();
    let (neighbor_batch, neighbor) = published(&mut owner);
    let neighbor = owner
        .retain_compute_dependency_reader(neighbor, SESSION, DEPENDENT_EPOCH)
        .unwrap();
    let (_, retention) = owner.bind_batch([template(); 3]).unwrap().into_parts();
    let mut events = Vec::with_capacity(11);
    for index in 0..3 {
        events.push(
            owner
                .record_unbound_compute_event(
                    SESSION + index as u64,
                    SOURCE_EPOCH + index as u64,
                    &retention,
                    index,
                )
                .unwrap(),
        );
    }
    let alias = owner
        .record_unbound_compute_event(SESSION, SOURCE_EPOCH, &retention, 0)
        .unwrap();
    let batch = owner.mark_published_retaining(retention, last).unwrap();
    let alias = owner
        .bind_compute_event_after_publication(alias, &batch, 0)
        .unwrap();
    let alias = owner
        .retain_compute_dependency_reader(alias, SESSION, DEPENDENT_EPOCH)
        .unwrap();
    EventBindFixture {
        owner,
        batch,
        events,
        alias,
        neighbor,
        neighbor_batch,
    }
}

fn bind_event_facts(
    events: &[Gfx942ComputeEventOccurrenceV1],
) -> Vec<(u64, ExactCompletionOccurrenceV1)> {
    events
        .iter()
        .map(|event| (event.event_id, event.exact))
        .collect()
}

fn finish_event_bind_fixture(mut fixture: EventBindFixture) {
    assert_eq!(
        fixture
            .owner
            .release_compute_event_batch(fixture.events)
            .unwrap(),
        3
    );
    for (event, reader) in [fixture.alias, fixture.neighbor] {
        fixture
            .owner
            .release_compute_dependency_reader(reader)
            .unwrap();
        fixture.owner.release_compute_event(event).unwrap();
    }
    let mut backend = CompletedBackend { reset_calls: 0 };
    let completed = complete(&mut fixture.owner, fixture.batch, &mut backend);
    fixture.owner.recycle(completed, &mut backend).unwrap();
    let completed = complete(&mut fixture.owner, fixture.neighbor_batch, &mut backend);
    fixture.owner.recycle(completed, &mut backend).unwrap();
    assert_eq!(backend.reset_calls, 4);
    fixture.owner.ensure_releasable().unwrap();
}

fn bind_fixture_events(fixture: &mut EventBindFixture, packets: [u64; 3]) {
    let events = std::mem::take(&mut fixture.events);
    let storage = (events.as_ptr(), events.capacity());
    let mut facts = bind_event_facts(&events);
    let mut expected = snapshot(&fixture.owner);
    for ((event_id, exact), packet_id) in facts.iter_mut().zip(packets) {
        exact.packet_id = Some(packet_id);
        expected.events.get_mut(event_id).unwrap().packet_id = Some(packet_id);
    }
    let (result, allocations) = crate::topology::tests::count_allocations_for_test(|| {
        fixture
            .owner
            .bind_compute_event_batch_after_publication(events, &fixture.batch)
    });
    assert_eq!(allocations, 0);
    fixture.events = result.unwrap();
    assert_eq!(
        (fixture.events.as_ptr(), fixture.events.capacity()),
        storage
    );
    assert_eq!(bind_event_facts(&fixture.events), facts);
    assert_eq!(snapshot(&fixture.owner), expected);
}

#[test]
fn event_batch_binding_is_allocation_free_and_frames_readers_neighbors_and_storage() {
    for (last, packets) in [
        (2, [0, 1, 2]),
        (u64::MAX - 1, [u64::MAX - 3, u64::MAX - 2, u64::MAX - 1]),
    ] {
        let mut fixture = event_bind_fixture(last);
        bind_fixture_events(&mut fixture, packets);
        finish_event_bind_fixture(fixture);
    }
}

#[test]
fn event_batch_binding_does_not_strengthen_issuance_or_pin_invariants() {
    let mut fixture = event_bind_fixture(101);
    let event = &mut fixture.events[2];
    // Hostile but ledger-authenticated metadata is not an issuance proof.
    event.exact.session_occurrence = 0;
    event.exact.source_acceptance_epoch = 0;
    *fixture
        .owner
        .dependency_ledger
        .events
        .get_mut(&event.event_id)
        .unwrap() = event.exact;
    let slot = event.exact.slot.index as usize;
    fixture.owner.slots[slot].event_pins = 0;
    fixture.owner.slots[slot].native_reader_pins = u32::MAX;
    bind_fixture_events(&mut fixture, [99, 100, 101]);
    fixture.owner.slots[slot].event_pins = 1;
    fixture.owner.slots[slot].native_reader_pins = 0;
    finish_event_bind_fixture(fixture);
}

#[test]
fn event_batch_binding_refusal_is_allocation_free_atomic_and_ordered() {
    #[derive(Clone, Copy, Debug)]
    enum Fault {
        PoisonBeforeRetention,
        RetentionBeforeLength,
        RetentionGeneration,
        LengthBeforeRow,
        BoundBeforeLedger,
        LedgerBeforePacket,
        PacketBeforeLateRow,
        LateBound,
        LateMissing,
        LatePosition,
        DuplicateId,
        Reordered,
        BoundBeforeLateMissing,
        Underflow,
        Overflow,
    }
    for fault in [
        Fault::PoisonBeforeRetention,
        Fault::RetentionBeforeLength,
        Fault::RetentionGeneration,
        Fault::LengthBeforeRow,
        Fault::BoundBeforeLedger,
        Fault::LedgerBeforePacket,
        Fault::PacketBeforeLateRow,
        Fault::LateBound,
        Fault::LateMissing,
        Fault::LatePosition,
        Fault::DuplicateId,
        Fault::Reordered,
        Fault::BoundBeforeLateMissing,
        Fault::Underflow,
        Fault::Overflow,
    ] {
        let mut fixture = event_bind_fixture(101);
        let mut events = Vec::with_capacity(11);
        events.extend(fixture.events.iter().map(duplicate_event));
        let slots = *fixture.batch.retention.slots;
        let error = match fault {
            Fault::PoisonBeforeRetention => {
                fixture.owner.phase = CompletionOwnerPhaseV1::Poisoned;
                fixture.batch.retention.last_packet_id = None;
                Gfx942CompletionErrorV1::Poisoned
            }
            Fault::RetentionBeforeLength => {
                fixture.batch.retention.last_packet_id = None;
                events.pop();
                Gfx942CompletionErrorV1::StaleBatchGeneration
            }
            Fault::RetentionGeneration => {
                fixture.batch.retention.slots[2].generation += 1;
                Gfx942CompletionErrorV1::StaleBatchGeneration
            }
            Fault::LengthBeforeRow => {
                events.pop();
                events[0].exact.packet_id = Some(99);
                Gfx942CompletionErrorV1::StaleEventOccurrence
            }
            Fault::BoundBeforeLedger | Fault::BoundBeforeLateMissing => {
                events[0].exact.packet_id = Some(99);
                let index = if matches!(fault, Fault::BoundBeforeLedger) {
                    0
                } else {
                    2
                };
                fixture
                    .owner
                    .dependency_ledger
                    .events
                    .remove(&events[index].event_id);
                Gfx942CompletionErrorV1::EventAlreadyBound
            }
            Fault::LedgerBeforePacket => {
                fixture
                    .owner
                    .dependency_ledger
                    .events
                    .remove(&events[0].event_id);
                fixture.batch.retention.last_packet_id = Some(u64::MAX);
                Gfx942CompletionErrorV1::StaleEventOccurrence
            }
            Fault::PacketBeforeLateRow => {
                fixture.batch.retention.last_packet_id = Some(u64::MAX);
                events[2].exact.packet_id = Some(101);
                Gfx942CompletionErrorV1::StaleBatchGeneration
            }
            Fault::LateBound => {
                events[2].exact.packet_id = Some(101);
                Gfx942CompletionErrorV1::EventAlreadyBound
            }
            Fault::LateMissing => {
                fixture
                    .owner
                    .dependency_ledger
                    .events
                    .remove(&events[2].event_id);
                Gfx942CompletionErrorV1::StaleEventOccurrence
            }
            Fault::LatePosition => {
                events[2].exact.slot = events[0].exact.slot;
                *fixture
                    .owner
                    .dependency_ledger
                    .events
                    .get_mut(&events[2].event_id)
                    .unwrap() = events[2].exact;
                Gfx942CompletionErrorV1::StaleEventOccurrence
            }
            Fault::DuplicateId => {
                events[2] = duplicate_event(&events[0]);
                Gfx942CompletionErrorV1::StaleEventOccurrence
            }
            Fault::Reordered => {
                events.swap(0, 2);
                Gfx942CompletionErrorV1::StaleEventOccurrence
            }
            Fault::Underflow | Fault::Overflow => {
                fixture.batch.retention.last_packet_id =
                    Some(if matches!(fault, Fault::Underflow) {
                        1
                    } else {
                        u64::MAX
                    });
                Gfx942CompletionErrorV1::StaleBatchGeneration
            }
        };
        let before = snapshot(&fixture.owner);
        let facts = bind_event_facts(&events);
        let storage = (events.as_ptr(), events.capacity());
        let (result, allocations) = crate::topology::tests::count_allocations_for_test(|| {
            fixture
                .owner
                .bind_compute_event_batch_after_publication(events, &fixture.batch)
        });
        let (actual, events) = result.unwrap_err();
        assert_eq!(allocations, 0, "{fault:?}");
        assert_eq!(actual, error, "{fault:?}");
        assert_eq!((events.as_ptr(), events.capacity()), storage, "{fault:?}");
        assert_eq!(bind_event_facts(&events), facts, "{fault:?}");
        assert_eq!(snapshot(&fixture.owner), before, "{fault:?}");
        drop(events);
        // Restore only injected corruption; retry and dispose the genuine tokens.
        fixture.owner.phase = CompletionOwnerPhaseV1::Ready;
        fixture.batch.retention.last_packet_id = Some(101);
        *fixture.batch.retention.slots = slots;
        for event in &fixture.events {
            fixture
                .owner
                .dependency_ledger
                .events
                .insert(event.event_id, event.exact);
        }
        bind_fixture_events(&mut fixture, [99, 100, 101]);
        finish_event_bind_fixture(fixture);
    }
}
