// Actual event issuance under a monotonic ledger frontier; allocator outcomes
// remain unconstrained by the two explicit std contents-preservation contracts.
#![allow(unused_macros)]
#![feature(allocator_api)]
#[path = "completion_issue_reserve_contracts_v1.rs"]
mod reserve_contracts;
use std::collections::HashMap;
use vstd::prelude::*;
use vstd::std_specs::iter::IteratorSpec;

// Prove the diagnostic assertion even when release Rust would erase it.
macro_rules! debug_assert {
    ($condition:expr) => { verus_exec_expr!({ assert($condition); }) };
}

include!("completion_owner_schema_v1.rs");
include!("completion_bound_cancel_execution_v1.rs");
include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");
include!("completion_event_core_v1.rs");
include!("../../fe2o3-kfd/src/queue_completion/event_bind_body.rs");
include!("completion_event_occurrence_v1.rs");
include!("../../fe2o3-kfd/src/queue_completion/event_issue_body.rs");
include!("../../fe2o3-kfd/src/queue_live/dependency_source_output_body.rs");

verus! {
const GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1: usize = 8192;
struct IssueTrace { ledger: Option<bool>, output: Option<bool> }

spec fn below(map: Map<u64, ExactCompletionOccurrenceV1>, frontier: int) -> bool {
    forall|key: u64| #[trigger] map.contains_key(key) ==> key < frontier
}
spec fn fresh<R>(s: State<R>) -> bool { s.next_event_id > 0 && below(s.events, s.next_event_id as int) }
spec fn logical_error(session: u64, epoch: u64) -> Option<Gfx942CompletionErrorV1> {
    if session == 0 { Some(Gfx942CompletionErrorV1::InvalidSessionOccurrence) }
    else if epoch == 0 { Some(Gfx942CompletionErrorV1::InvalidAcceptanceEpoch) } else { None }
}
spec fn common_error<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>, session: u64, epoch: u64)
    -> Option<Gfx942CompletionErrorV1> {
    if s.phase != CompletionOwnerPhaseV1::Ready { Some(Gfx942CompletionErrorV1::Poisoned) }
    else if !bound(table(s), r) { Some(bound_error(r)) }
    else { logical_error(session, epoch) }
}
spec fn pin_room<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>, n: int) -> bool {
    forall|i: int| 0 <= i < n ==> s.slots[#[trigger] r.slots@[i].index as int].event_pins < u32::MAX
}
spec fn batch_error<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>, session: u64, epoch: u64)
    -> Option<Gfx942CompletionErrorV1> {
    if let Some(error) = common_error(s, r, session, epoch) { Some(error) }
    else if s.events.len() + N > 8192 { Some(Gfx942CompletionErrorV1::EventCapacityExhausted) }
    else if s.next_event_id + N > u64::MAX { Some(Gfx942CompletionErrorV1::EventIdentityExhausted) }
    else if !pin_room(s, r, N as int) { Some(Gfx942CompletionErrorV1::SignalPinCountExhausted) }
    else { None }
}
spec fn single_error<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>, session: u64, epoch: u64, i: usize)
    -> Option<Gfx942CompletionErrorV1> {
    if let Some(error) = common_error(s, r, session, epoch) { Some(error) }
    else if s.events.len() >= 8192 { Some(Gfx942CompletionErrorV1::EventCapacityExhausted) }
    else if i >= N { Some(Gfx942CompletionErrorV1::StaleBatchGeneration) }
    else if s.next_event_id == u64::MAX { Some(Gfx942CompletionErrorV1::EventIdentityExhausted) }
    else if s.slots[r.slots@[i as int].index as int].event_pins == u32::MAX {
        Some(Gfx942CompletionErrorV1::SignalPinCountExhausted)
    } else { None }
}
spec fn issue_decision(error: Option<Gfx942CompletionErrorV1>, trace: IssueTrace, batch: bool)
    -> Option<Result<(), Gfx942CompletionErrorV1>> {
    if let Some(error) = error {
        if trace.ledger.is_none() && trace.output.is_none() { Some(Err(error)) } else { None }
    } else if trace.ledger == Some(false) {
        if trace.output.is_none() { Some(Err(Gfx942CompletionErrorV1::DependencyLedgerAllocation)) } else { None }
    } else if trace.ledger != Some(true) { None }
    else if !batch { if trace.output.is_none() { Some(Ok(())) } else { None } }
    else if trace.output == Some(false) { Some(Err(Gfx942CompletionErrorV1::DependencyLedgerAllocation)) }
    else if trace.output == Some(true) { Some(Ok(())) } else { None }
}
spec fn issue_rows<const N: usize>(first: u64, session: u64, epoch: u64, r: CompletionBatchRetentionV1<N>, n: nat)
    -> Seq<Gfx942ComputeEventOccurrenceV1> {
    Seq::new(n, |i: int| Gfx942ComputeEventOccurrenceV1 {
        event_id: (first + i) as u64, exact: occurrence(session, epoch, r, i, None) })
}
spec fn issue_map<const N: usize>(map: Map<u64, ExactCompletionOccurrenceV1>, first: u64,
    session: u64, epoch: u64, r: CompletionBatchRetentionV1<N>, n: int) -> Map<u64, ExactCompletionOccurrenceV1>
    decreases n,
{
    if n <= 0 { map } else { issue_map(map, first, session, epoch, r, n - 1)
        .insert((first + n - 1) as u64, occurrence(session, epoch, r, n - 1, None)) }
}
spec fn issue_prefix<R, const N: usize>(s: State<R>, session: u64, epoch: u64, r: CompletionBatchRetentionV1<N>, n: int)
    -> State<R> {
    State { events: issue_map(s.events, s.next_event_id, session, epoch, r, n),
        slots: Seq::new(s.slots.len(), |i: int| {
            if present(r.slots@, n, i as u32) {
                CompletionSlotRecordV1 { event_pins: (s.slots[i].event_pins + 1) as u32, ..s.slots[i] }
            } else { s.slots[i] }
        }), ..s }
}
spec fn issued<R, const N: usize>(s: State<R>, session: u64, epoch: u64, r: CompletionBatchRetentionV1<N>) -> State<R> {
    State { next_event_id: (s.next_event_id + N) as u64, ..issue_prefix(s, session, epoch, r, N as int) }
}
spec fn single_issued<R>(s: State<R>, exact: ExactCompletionOccurrenceV1) -> State<R> {
    let i = exact.slot.index as int;
    State { next_event_id: (s.next_event_id + 1) as u64, events: s.events.insert(s.next_event_id, exact),
        slots: s.slots.update(i, CompletionSlotRecordV1 { event_pins: (s.slots[i].event_pins + 1) as u32, ..s.slots[i] }), ..s }
}
spec fn batch_outcome<R, const N: usize>(before: State<R>, after: State<R>, session: u64, epoch: u64,
    retention: CompletionBatchRetentionV1<N>, result: Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942CompletionErrorV1>, trace: IssueTrace) -> bool {
    fresh(after) && match result {
        Ok(events) => issue_decision(batch_error(before, retention, session, epoch), trace, true) == Some(Ok(()))
            && events@ == issue_rows(before.next_event_id, session, epoch, retention, N as nat)
            && after == issued(before, session, epoch, retention),
        Err(error) => issue_decision(batch_error(before, retention, session, epoch), trace, true) == Some(Err(error))
            && after == before,
    }
}
struct BoundCompletionBatchV1<const N: usize, P> { packets: P, retention: CompletionBatchRetentionV1<N> }

proof fn issue_frontier<const N: usize>(map: Map<u64, ExactCompletionOccurrenceV1>, first: u64,
    session: u64, epoch: u64, r: CompletionBatchRetentionV1<N>, n: int)
    requires 0 <= n <= N, below(map, first as int), first + n <= u64::MAX,
    ensures below(issue_map(map, first, session, epoch, r, n), first + n),
    decreases n,
{
    if n > 0 { issue_frontier(map, first, session, epoch, r, n - 1); }
}

fn validate_logical_identity(session_occurrence: u64, acceptance_epoch: u64)
    -> (out: Result<(), Gfx942CompletionErrorV1>)
    ensures out == match logical_error(session_occurrence, acceptance_epoch) { Some(error) => Err(error), None => Ok(()) },
{ completion_logical_identity_body!(verus_exec_expr, session_occurrence, acceptance_epoch) }

impl<K, V> CompletionDependencyLedgerV1<HashMap<K, V>> {
    fn new() -> (out: Self)
        ensures out.next_event_id == 1, out.next_reader_lease_id == 1,
            out.events@ == Map::<u64, ExactCompletionOccurrenceV1>::empty(), out.readers@ == Map::<K, V>::empty(),
            below(out.events@, out.next_event_id as int),
    { completion_dependency_ledger_new_body!(verus_exec_expr) }
}

impl<R> CompletionSignalArenaOwnerV1<CompletionDependencyLedgerV1<R>> {
    fn record_unbound_compute_event_batch<const N: usize>(&mut self, session_occurrence: u64,
        source_acceptance_epoch: u64, retention: &CompletionBatchRetentionV1<N>)
        -> (out: (Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942CompletionErrorV1>, Ghost<IssueTrace>))
        requires fresh(old(self).state()),
        ensures batch_outcome(old(self).state(), final(self).state(), session_occurrence, source_acceptance_epoch, *retention, out.0, out.1@),
    {
        let ghost s = self.state();
        let ghost r = *retention;
        let ghost mut trace = IssueTrace { ledger: None, output: None };
        macro_rules! finish { ($out:expr) => { verus_exec_expr!(($out, Ghost(trace))) }; }
        completion_record_event_batch_body!(@annotated verus_exec_expr, self, session_occurrence,
            source_acceptance_epoch, retention, N, finish, i, events, reservation, next, exact,
            [],
            [invariant 0 <= i <= N, self.state() == s, s == old(self).state(), fresh(s), *retention == r,
                common_error(s, r, session_occurrence, source_acceptance_epoch).is_none(),
                s.events.len() + N <= 8192, s.next_event_id + N <= u64::MAX,
                next == s.next_event_id + N, pin_room(s, r, i as int),
                trace.ledger.is_none(), trace.output.is_none(),
             decreases N - i,],
            [proof { assert(pin_room(s, r, i as int + 1)); }],
            [proof { trace.ledger = Some(reservation.is_ok()); }],
            [proof { trace.output = Some(reservation.is_ok()); }],
            [proof {
                assert(issue_prefix(s, session_occurrence, source_acceptance_epoch, r, 0).slots =~= s.slots);
                assert(issue_prefix(s, session_occurrence, source_acceptance_epoch, r, 0) == s);
            }],
            [invariant 0 <= i <= N, *retention == r, s == old(self).state(), fresh(s),
                batch_error(s, r, session_occurrence, source_acceptance_epoch).is_none(),
                next == s.next_event_id + N,
                self.state() == issue_prefix(s, session_occurrence, source_acceptance_epoch, r, i as int),
                below(self.dependency_ledger.events@, s.next_event_id + i),
                events@ == issue_rows(s.next_event_id, session_occurrence, source_acceptance_epoch, r, i as nat),
                trace.ledger == Some(true), trace.output == Some(true),
             decreases N - i,],
            [proof {
                issue_frontier(s.events, s.next_event_id, session_occurrence, source_acceptance_epoch, r, i as int);
                assert(entry(table(s), r, CompletionSlotPhaseV1::Bound { batch_id: r.batch_id }, i as int));
                assert(!present(r.slots@, i as int, r.slots@[i as int].index));
                assert(!self.dependency_ledger.events@.contains_key((s.next_event_id + i) as u64));
            }],
            [proof {
                assert(events@ =~= issue_rows(s.next_event_id, session_occurrence, source_acceptance_epoch, r, i as nat + 1));
                assert(self.state().slots =~= issue_prefix(s, session_occurrence, source_acceptance_epoch, r, i as int + 1).slots);
                assert(self.state() == issue_prefix(s, session_occurrence, source_acceptance_epoch, r, i as int + 1));
                issue_frontier(s.events, s.next_event_id, session_occurrence, source_acceptance_epoch, r, i as int + 1);
            }])
    }

    fn record_dependency_event_batch_v1<const N: usize>(&mut self, session_occurrence: u64,
        source_acceptance_epoch: u64, retention: &CompletionBatchRetentionV1<N>)
        -> (out: (Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942CompletionErrorV1>, Ghost<IssueTrace>))
        requires fresh(old(self).state()),
        ensures batch_outcome(old(self).state(), final(self).state(), session_occurrence, source_acceptance_epoch, *retention, out.0, out.1@),
    { completion_record_dependency_batch_body!(verus_exec_expr, self, session_occurrence, source_acceptance_epoch, retention) }

    fn record_dependency_event_batch_for_bound_v1<const N: usize, P>(&mut self, session_occurrence: u64,
        source_acceptance_epoch: u64, bound: &BoundCompletionBatchV1<N, P>)
        -> (out: (Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942CompletionErrorV1>, Ghost<IssueTrace>))
        requires fresh(old(self).state()),
        ensures batch_outcome(old(self).state(), final(self).state(), session_occurrence, source_acceptance_epoch, bound.retention, out.0, out.1@),
    { completion_record_bound_dependency_batch_body!(verus_exec_expr, self, session_occurrence, source_acceptance_epoch, bound) }

    fn record_unbound_compute_event<const N: usize>(&mut self, session_occurrence: u64,
        source_acceptance_epoch: u64, retention: &CompletionBatchRetentionV1<N>, batch_index: usize)
        -> (out: (Result<Gfx942ComputeEventOccurrenceV1, Gfx942CompletionErrorV1>, Ghost<IssueTrace>))
        requires fresh(old(self).state()),
        ensures fresh(final(self).state()), match out.0 {
            Ok(event) => issue_decision(single_error(old(self).state(), *retention, session_occurrence, source_acceptance_epoch, batch_index), out.1@, false) == Some(Ok(()))
                && event.event_id == old(self).state().next_event_id
                && event.exact == occurrence(session_occurrence, source_acceptance_epoch, *retention, batch_index as int, None)
                && final(self).state() == single_issued(old(self).state(), event.exact),
            Err(error) => issue_decision(single_error(old(self).state(), *retention, session_occurrence, source_acceptance_epoch, batch_index), out.1@, false) == Some(Err(error))
                && final(self).state() == old(self).state(),
        },
    {
        let ghost mut trace = IssueTrace { ledger: None, output: None };
        macro_rules! finish { ($out:expr) => { verus_exec_expr!(($out, Ghost(trace))) }; }
        completion_record_single_event_body!(@annotated verus_exec_expr, self, session_occurrence,
            source_acceptance_epoch, retention, batch_index, finish, reservation, [],
            [proof { trace.ledger = Some(reservation.is_ok()); }])
    }
}

// Constructed Bound metadata witnesses ledger composition, not native authority.
fn constructor_issue_witness() {
    let vm = VmKeyV1 { device: DeviceKeyV1 { physical: 0, generation: 0 }, id: 0 };
    let queue = QueueKeyV1 { vm, id: 0, generation: 0 };
    let mapping = MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 { vm, id: 0, generation: 0 }, id: 0,
    };
    let ledger = CompletionDependencyLedgerV1::<HashMap<u64, u64>>::new();
    let mut owner = CompletionSignalArenaOwnerV1 {
        queue, signal_mapping: mapping, gpu_base: 0, next_batch_id: u64::MAX,
        slots: Box::new([CompletionSlotRecordV1 {
            generation: u64::MAX, phase: CompletionSlotPhaseV1::Available,
            event_pins: u32::MAX, native_reader_pins: u32::MAX,
        }; 8192]), dependency_ledger: Box::new(ledger), phase: CompletionOwnerPhaseV1::Ready,
    };
    let record = CompletionSlotRecordV1 {
        generation: 0, phase: CompletionSlotPhaseV1::Bound { batch_id: 0 },
        event_pins: 2, native_reader_pins: 7,
    };
    owner.slots[63] = record;
    owner.slots[64] = CompletionSlotRecordV1 { event_pins: 4, ..record };
    let dispatch = CompletionDispatchGenerationBindingV1 {
        queue, code: mapping, kernarg: mapping, dispatch_generation: 1,
    };
    let retention = CompletionBatchRetentionV1 {
        batch_id: 0, queue, signal_mapping: mapping,
        slots: Box::new([CompletionSlotLeaseV1 { index: 63, generation: 0 },
            CompletionSlotLeaseV1 { index: 64, generation: 0 }]),
        dispatches: Box::new([dispatch; 2]), last_packet_id: None,
    };
    let ghost before = owner.state();
    assert(fresh(before) && before.next_event_id == 1 && before.events.len() == 0);
    assert(batch_error(before, retention, 9, 11).is_none());
    let (result, trace) = owner.record_unbound_compute_event_batch(9, 11, &retention);
    assert(result.is_ok() == (trace@.ledger == Some(true) && trace@.output == Some(true)));
    match result {
        Ok(events) => {
            reveal_with_fuel(issue_map, 3);
            assert(owner.state().events == before.events.insert(1, occurrence(9, 11, retention, 0, None))
                .insert(2, occurrence(9, 11, retention, 1, None)));
            assert(events@ == issue_rows(1, 9, 11, retention, 2));
            assert(events@[0].event_id == 1 && events@[1].event_id == 2);
            assert(owner.state().next_event_id == 3);
            assert(owner.state().slots[63].event_pins == 3 && owner.state().slots[64].event_pins == 5);
        },
        Err(error) => {
            assert(error == Gfx942CompletionErrorV1::DependencyLedgerAllocation);
            assert(owner.state() == before);
        },
    }
    assert(owner.state().slots[0] == before.slots[0]);
    assert(owner.state().readers == before.readers);
    let ghost middle = owner.state();
    assert(middle.events.len() <= 2);
    assert(middle.next_event_id <= 3);
    assert(bound(table(middle), retention));
    assert(middle.slots[64].event_pins <= 5);
    assert(single_error(middle, retention, 9, 11, 1).is_none());
    let (result, trace) = owner.record_unbound_compute_event(9, 11, &retention, 1);
    assert(result.is_ok() == (trace@.ledger == Some(true)));
    match result {
        Ok(event) => {
            assert(event.event_id == middle.next_event_id);
            assert(event.exact == occurrence(9, 11, retention, 1, None));
            assert(owner.state() == single_issued(middle, event.exact));
        },
        Err(error) => {
            assert(error == Gfx942CompletionErrorV1::DependencyLedgerAllocation);
            assert(owner.state() == middle);
        },
    }
}

#[derive(Clone, Copy)]
struct ComputeAqlQueueLaneV1 { session: QueueKeyV1, ordinal: usize, generation: u64 }
struct Gfx942ComputeDependencyEventV1 { lane: ComputeAqlQueueLaneV1, event: Gfx942ComputeEventOccurrenceV1 }
enum ComputeAqlQueueSessionErrorV1 { Contract(&'static str) }
enum FixedDispatchSubmissionFailureV1 {
    RejectedBeforeSideEffect(ComputeAqlQueueSessionErrorV1),
    RetryableBeforeSideEffect(ComputeAqlQueueSessionErrorV1),
    Terminal(ComputeAqlQueueSessionErrorV1),
}

fn reserve_source_output<const N: usize>() -> (out: (Result<Vec<Gfx942ComputeDependencyEventV1>, FixedDispatchSubmissionFailureV1>, Ghost<bool>))
    ensures match out.0 {
        Ok(output) => out.1@ && output@.len() == 0,
        Err(error) => !out.1@ && error == FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
            ComputeAqlQueueSessionErrorV1::Contract("dependency source event output allocation")),
    },
{
    let ghost mut success = false;
    macro_rules! finish { ($out:expr) => { verus_exec_expr!(($out, Ghost(success))) }; }
    dependency_source_output_reserve_body!(@annotated verus_exec_expr, N, finish, reservation,
        [proof { success = reservation.is_ok(); }])
}

fn pack_source_output(output: Vec<Gfx942ComputeDependencyEventV1>, events: Vec<Gfx942ComputeEventOccurrenceV1>, lane: ComputeAqlQueueLaneV1)
    -> (out: Vec<Gfx942ComputeDependencyEventV1>)
    ensures out@ == output@ + Seq::new(events.len() as nat, |i: int| Gfx942ComputeDependencyEventV1 { lane, event: events@[i] }),
{
    let mut packed = output;
    dependency_source_output_pack_body!(@annotated verus_exec_expr, packed, events, lane, pending, event,
        [let ghost initial = packed@; let ghost rows = events@; let ghost mut consumed = 0nat;],
        [invariant 0 <= consumed <= rows.len(), rows == events@, initial == output@,
            pending.remaining() == rows.subrange(consumed as int, rows.len() as int),
            pending.obeys_prophetic_iter_laws(), pending.decrease().is_some(),
            packed@ == initial + Seq::new(consumed, |i: int| Gfx942ComputeDependencyEventV1 { lane, event: rows[i] }),
         ensures packed@ == output@ + Seq::new(events.len() as nat, |i: int| Gfx942ComputeDependencyEventV1 { lane, event: events@[i] }),
         decreases pending.decrease().unwrap(),],
        [let ghost tail = pending.remaining();],
        [proof {
            assert(event == rows[consumed as int]);
            consumed = consumed + 1;
            assert(pending.remaining() == tail.drop_first());
            assert(pending.remaining() =~= rows.subrange(consumed as int, rows.len() as int));
            assert(packed@ =~= initial + Seq::new(consumed, |i: int| Gfx942ComputeDependencyEventV1 { lane, event: rows[i] }));
        }],
        [proof {
            assert(tail.len() == 0);
            assert(consumed == rows.len());
            assert(packed@ =~= output@ + Seq::new(events.len() as nat, |i: int| Gfx942ComputeDependencyEventV1 { lane, event: events@[i] }));
        }])
}
}
