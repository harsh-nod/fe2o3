// Actual short-circuit cleanup and consuming adapters over one retained owner.
// Only normal host-ledger returns are covered, not outer terminalization/native authority.
#![allow(unused_macros)]
#![feature(allocator_api)]
#[path = "completion_hash_reserve_contracts_v1.rs"]
mod reserve_contracts;
use std::collections::{HashMap, HashSet};
use vstd::std_specs::iter::IteratorSpec;
use vstd::prelude::*;

include!("completion_owner_schema_v1.rs");
include!("completion_bound_cancel_execution_v1.rs");
include!("completion_event_batch_release_execution_v1.rs");
include!("../../fe2o3-kfd/src/queue_completion/source_rollback_body.rs");

verus! {
spec fn event_view<R>(s: OwnerState<CompletionDependencyLedgerV1<R>>) -> State<R> {
    State { queue: s.queue, signal_mapping: s.signal_mapping, gpu_base: s.gpu_base,
        next_batch_id: s.next_batch_id, slots: s.slots, phase: s.phase,
        events: s.dependency_ledger.events@, readers: s.dependency_ledger.readers,
        next_event_id: s.dependency_ledger.next_event_id,
        next_reader_lease_id: s.dependency_ledger.next_reader_lease_id }
}

spec fn retention_view<R>(s: State<R>) -> OwnerState<()> {
    OwnerState { queue: s.queue, signal_mapping: s.signal_mapping, gpu_base: s.gpu_base,
        next_batch_id: s.next_batch_id, slots: s.slots, phase: s.phase, dependency_ledger: () }
}

spec fn event_cancelled<R>(s: State<R>, slots: Seq<CompletionSlotLeaseV1>, n: int) -> State<R> {
    State { slots: cancelled(retention_view(s), slots, n).slots, ..s }
}

proof fn retention_bridge<R, const N: usize>(s: OwnerState<CompletionDependencyLedgerV1<R>>,
    retention: CompletionBatchRetentionV1<N>)
    ensures
        bound(s, retention) == bound(retention_view(event_view(s)), retention),
        unpinned(s, retention.slots@, N as int)
            == unpinned(retention_view(event_view(s)), retention.slots@, N as int),
{
    let t = retention_view(event_view(s));
    let expected = CompletionSlotPhaseV1::Bound { batch_id: retention.batch_id };
    assert forall|i: int| entry(s, retention, expected, i) == entry(t, retention, expected, i) by { }
}

proof fn cancellation_bridge<R>(s: OwnerState<CompletionDependencyLedgerV1<R>>,
    slots: Seq<CompletionSlotLeaseV1>, n: int)
    ensures event_view(cancelled(s, slots, n)) == event_cancelled(event_view(s), slots, n),
{
    assert(cancelled(s, slots, n).slots =~= cancelled(retention_view(event_view(s)), slots, n).slots);
}

spec fn can_cancel<R, const N: usize>(s: State<R>, retention: CompletionBatchRetentionV1<N>) -> bool {
    bound(retention_view(s), retention) && unpinned(retention_view(s), retention.slots@, N as int)
}

struct SourceRollbackTrace {
    release: ReservationTrace,
    cancellation: Option<bool>,
}

spec fn rollback_outcome<R, const N: usize>(s: State<R>, events: Seq<Gfx942ComputeEventOccurrenceV1>,
    retention: CompletionBatchRetentionV1<N>, trace: SourceRollbackTrace,
    out: Result<(), Gfx942CompletionErrorV1>, after: State<R>) -> bool
{
    match decision(s, events, trace.release) {
        Some(Err(_)) => trace.cancellation.is_none()
            && out == Err(Gfx942CompletionErrorV1::StaleEventOccurrence) && after == s,
        Some(Ok(_)) => {
            let released = batch_released(s, events);
            let success = can_cancel(released, retention);
            &&& trace.cancellation == Some(success)
            &&& out == if success { Ok(()) } else { Err(Gfx942CompletionErrorV1::StaleEventOccurrence) }
            &&& after == if success { event_cancelled(released, retention.slots@, N as int) } else { released }
        },
        None => false,
    }
}

impl<R> CompletionSignalArenaOwnerV1<CompletionDependencyLedgerV1<R>> {
    fn release_dependency_event_batch_v1(&mut self, events: Vec<Gfx942ComputeEventOccurrenceV1>)
        -> (out: (Result<usize, (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>)>, Ghost<ReservationTrace>))
        ensures match out.0 {
            Ok(n) => decision(old(self).state(), events@, out.1@) == Some(Ok(n as nat))
                && final(self).state() == batch_released(old(self).state(), events@),
            Err((error, returned)) => decision(old(self).state(), events@, out.1@) == Some(Err(error))
                && final(self).state() == old(self).state() && returned == events,
        },
    { completion_release_dependency_event_batch_body!(verus_exec_expr, self, events) }

    fn rollback_source<const N: usize>(&mut self, events: Vec<Gfx942ComputeEventOccurrenceV1>,
        retention: CompletionBatchRetentionV1<N>)
        -> (out: (Result<(), Gfx942CompletionErrorV1>, Ghost<SourceRollbackTrace>))
        ensures rollback_outcome(old(self).state(), events@, retention, out.1@, out.0, final(self).state()),
    {
        let ghost mut trace = SourceRollbackTrace {
            release: ReservationTrace { ids: None, budgets: None }, cancellation: None,
        };
        macro_rules! observed_release {
            ($owner:expr, $events:ident) => { verus_exec_expr!({
                let (result, Ghost(reservations)) = completion_source_release_call!($owner, $events);
                proof { trace.release = reservations; }
                result
            }) };
        }
        macro_rules! observed_cancel {
            ($owner:expr, $retention:ident) => { verus_exec_expr!({
                let ghost before = ($owner).owner_state();
                let ghost kept = $retention;
                proof { retention_bridge(before, kept); }
                let result = completion_source_cancel_call!($owner, $retention);
                proof {
                    trace.cancellation = Some(result.is_ok());
                    cancellation_bridge(before, kept.slots@, N as int);
                }
                result
            }) };
        }
        let result = completion_source_rollback_body!(@annotated verus_exec_expr, self, events, retention,
            observed_release, observed_cancel);
        (result, Ghost(trace))
    }
}
}

verus! {
fn rollback_short_circuit_witness<R>(readers: R) {
    let vm = VmKeyV1 { device: DeviceKeyV1 { physical: 0, generation: 0 }, id: 0 };
    let queue = QueueKeyV1 { vm, id: 0, generation: 0 };
    let mapping = MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 { vm, id: 0, generation: 0 }, id: 0,
    };
    let mut owner = CompletionSignalArenaOwnerV1 {
        queue, signal_mapping: mapping, gpu_base: 0, next_batch_id: u64::MAX,
        slots: Box::new([CompletionSlotRecordV1 {
            generation: u64::MAX, phase: CompletionSlotPhaseV1::Available,
            event_pins: u32::MAX, native_reader_pins: u32::MAX,
        }; 8192]),
        dependency_ledger: Box::new(CompletionDependencyLedgerV1 {
            next_event_id: u64::MAX, next_reader_lease_id: u64::MAX, events: HashMap::new(), readers,
        }), phase: CompletionOwnerPhaseV1::Poisoned,
    };
    owner.slots[63] = CompletionSlotRecordV1 {
        generation: 0, phase: CompletionSlotPhaseV1::Bound { batch_id: 0 },
        event_pins: 0, native_reader_pins: 0,
    };
    let retention = CompletionBatchRetentionV1 {
        batch_id: 0, queue, signal_mapping: mapping,
        slots: Box::new([CompletionSlotLeaseV1 { index: 63, generation: 0 }]),
        dispatches: Box::new([CompletionDispatchGenerationBindingV1 {
            queue, code: mapping, kernarg: mapping, dispatch_generation: 1,
        }]), last_packet_id: None,
    };
    let ghost before = owner.state();
    assert(can_cancel(before, retention));
    let (out, Ghost(trace)) = owner.rollback_source(Vec::new(), retention);
    assert(out == Err(Gfx942CompletionErrorV1::StaleEventOccurrence));
    assert(owner.state() == before);
    assert(trace.release.ids.is_none() && trace.release.budgets.is_none());
    assert(trace.cancellation.is_none());
}

// The event roster has two aliases but retention has one slot. It may even
// name a disjoint batch slot: the implementation has no provenance precondition.
fn rollback_release_prefix_witness<R>(readers: R, invalid_retention: bool, disjoint: bool) {
    let vm = VmKeyV1 { device: DeviceKeyV1 { physical: 0, generation: 0 }, id: 0 };
    let queue = QueueKeyV1 { vm, id: 0, generation: 0 };
    let mapping = MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 { vm, id: 0, generation: 0 }, id: 0,
    };
    let exact = ExactCompletionOccurrenceV1 {
        session_occurrence: 0, source_acceptance_epoch: 0, batch_id: 0,
        queue, signal_mapping: mapping, slot: CompletionSlotLeaseV1 { index: 1, generation: 0 },
        dispatch_generation: 0, packet_id: None,
    };
    let mut events_map = HashMap::new();
    events_map.insert(7u64, exact); events_map.insert(8u64, exact);
    let mut owner = CompletionSignalArenaOwnerV1 {
        queue, signal_mapping: mapping, gpu_base: 0, next_batch_id: u64::MAX,
        slots: Box::new([CompletionSlotRecordV1 {
            generation: u64::MAX, phase: CompletionSlotPhaseV1::Available,
            event_pins: u32::MAX, native_reader_pins: u32::MAX,
        }; 8192]),
        dependency_ledger: Box::new(CompletionDependencyLedgerV1 {
            next_event_id: u64::MAX, next_reader_lease_id: u64::MAX, events: events_map, readers,
        }), phase: CompletionOwnerPhaseV1::Ready,
    };
    owner.slots[1] = CompletionSlotRecordV1 {
        generation: 0, phase: CompletionSlotPhaseV1::Bound { batch_id: 0 },
        event_pins: 2, native_reader_pins: 0,
    };
    owner.slots[63] = CompletionSlotRecordV1 {
        generation: 0, phase: CompletionSlotPhaseV1::Bound { batch_id: 0 },
        event_pins: 0, native_reader_pins: 0,
    };
    let index = if disjoint { 63u32 } else { 1u32 };
    let retention = CompletionBatchRetentionV1 {
        batch_id: 0, queue, signal_mapping: mapping,
        slots: Box::new([CompletionSlotLeaseV1 { index, generation: 0 }]),
        dispatches: Box::new([CompletionDispatchGenerationBindingV1 {
            queue, code: mapping, kernarg: mapping, dispatch_generation: 1,
        }]), last_packet_id: if invalid_retention { Some(0) } else { None },
    };
    let mut events = Vec::new();
    events.push(Gfx942ComputeEventOccurrenceV1 { event_id: 7, exact });
    events.push(Gfx942ComputeEventOccurrenceV1 { event_id: 8, exact });
    let ghost before = owner.state(); let ghost rows = events@;
    let ghost released = batch_released(before, rows);
    proof {
        reveal_with_fuel(legacy_error, 3);
        assert(legacy_error(before, rows, Set::empty()).is_none());
        reveal_with_fuel(count, 3);
        assert forall|key: u32| #[trigger] count(rows, key) <= capacity(before, Map::empty(), key) by { }
        reveal_with_fuel(batch_released, 3);
        assert(released.slots[1].event_pins == 0);
        assert(released.slots[63] == before.slots[63]);
        assert(can_cancel(released, retention) == !invalid_retention);
    }
    let ghost kept = retention;
    let (out, Ghost(trace)) = owner.rollback_source(events, retention);
    proof {
    if trace.release.ids == Some(true) && trace.release.budgets == Some(true) {
        assert(trace.cancellation == Some(!invalid_retention));
        assert(out.is_ok() == !invalid_retention);
        if invalid_retention {
            assert(owner.state() == released);
            assert(out == Err(Gfx942CompletionErrorV1::StaleEventOccurrence));
            assert(owner.slots[1].phase == CompletionSlotPhaseV1::Bound { batch_id: 0 });
        } else {
            assert(owner.state() == event_cancelled(released, kept.slots@, 1));
            assert(owner.slots[index as int].phase == CompletionSlotPhaseV1::Available);
        }
        assert(owner.slots[1].event_pins == 0);
        assert(owner.state().events == Map::<u64, ExactCompletionOccurrenceV1>::empty());
        assert(owner.state().slots[0] == before.slots[0]);
        assert(owner.state().readers == before.readers);
    } else {
        assert(out == Err(Gfx942CompletionErrorV1::StaleEventOccurrence));
        assert(trace.cancellation.is_none());
        assert(owner.state() == before);
    }
    assert(owner.phase == CompletionOwnerPhaseV1::Ready);
    assert(owner.next_batch_id == u64::MAX);
    assert(owner.dependency_ledger.next_event_id == u64::MAX);
    assert(owner.dependency_ledger.next_reader_lease_id == u64::MAX);
    }
}
}
