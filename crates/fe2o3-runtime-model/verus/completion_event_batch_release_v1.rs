// The included bodies are the production host-ledger algorithm. Numeric identity
// newtypes project to u64, preserving their complete structural equality. Reader
// storage is an arbitrary non-Copy payload. No native/device effects are modeled.
#![allow(unused_macros)]
#![feature(allocator_api)]
#[path = "completion_hash_reserve_contracts_v1.rs"]
mod reserve_contracts;
use std::collections::{HashMap, HashSet};
use vstd::std_specs::iter::IteratorSpec;
use vstd::prelude::*;

// Verify the debug check even in configurations where Rust would erase it.
macro_rules! debug_assert_eq {
    ($left:expr, $right:expr) => { verus_exec_expr!({ assert($left == $right); }) };
}

include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");
include!("../../fe2o3-kfd/src/queue_completion/batch_event_release_body.rs");

macro_rules! structural_eq {
    ($($name:ident),+ $(,)?) => { $(verus! {
        impl vstd::std_specs::cmp::PartialEqSpecImpl for $name {
            open spec fn obeys_eq_spec() -> bool { true }
            open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
        }
    })+ };
}

verus! {
#[derive(Clone, Copy, PartialEq, Eq)]
struct DeviceKeyV1 { physical: u64, generation: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct VmKeyV1 { device: DeviceKeyV1, id: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct QueueKeyV1 { vm: VmKeyV1, id: u64, generation: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct MemoryAllocationKeyV1 { vm: VmKeyV1, id: u64, generation: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct MemoryMappingKeyV1 { allocation: MemoryAllocationKeyV1, id: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletionSlotLeaseV1 { index: u32, generation: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ExactCompletionOccurrenceV1 {
    session_occurrence: u64, source_acceptance_epoch: u64, batch_id: u64,
    queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1,
    slot: CompletionSlotLeaseV1, dispatch_generation: u64, packet_id: Option<u64>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionOwnerPhaseV1 { Ready, ProbeActive, Poisoned }
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionSlotPhaseV1 {
    Available, Bound { batch_id: u64 }, Published { batch_id: u64 }, Completed { batch_id: u64 },
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletionSlotRecordV1 {
    generation: u64, phase: CompletionSlotPhaseV1, event_pins: u32, native_reader_pins: u32,
}
enum Gfx942CompletionErrorV1 { Poisoned, StaleEventOccurrence, DuplicateDependency, DependencyLedgerAllocation }
struct Gfx942ComputeEventOccurrenceV1 { event_id: u64, exact: ExactCompletionOccurrenceV1 }
struct Gfx942ComputeEventReleaseObservationV1;
struct CompletionDependencyLedgerV1<R> {
    next_event_id: u64, next_reader_lease_id: u64,
    events: HashMap<u64, ExactCompletionOccurrenceV1>, readers: R,
}
struct CompletionSignalArenaOwnerV1<R> {
    queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1,
    gpu_base: u64, next_batch_id: u64,
    slots: Box<[CompletionSlotRecordV1; 8192]>,
    dependency_ledger: Box<CompletionDependencyLedgerV1<R>>,
    phase: CompletionOwnerPhaseV1,
}
struct State<R> {
    queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1,
    gpu_base: u64, next_batch_id: u64,
    slots: Seq<CompletionSlotRecordV1>,
    events: Map<u64, ExactCompletionOccurrenceV1>, readers: R,
    next_event_id: u64, next_reader_lease_id: u64, phase: CompletionOwnerPhaseV1,
}
}

verus! {
struct ReservationTrace { ids: Option<bool>, budgets: Option<bool> }

spec fn suffix(rows: Seq<Gfx942ComputeEventOccurrenceV1>, i: int) -> Seq<Gfx942ComputeEventOccurrenceV1> {
    rows.subrange(i, rows.len() as int)
}

proof fn suffix_step(rows: Seq<Gfx942ComputeEventOccurrenceV1>, i: int)
    requires 0 <= i < rows.len(),
    ensures suffix(rows, i).len() > 0, suffix(rows, i)[0] == rows[i],
        suffix(rows, i).drop_first() == suffix(rows, i + 1),
{
    assert(suffix(rows, i).drop_first() =~= suffix(rows, i + 1));
}

spec fn legacy_error<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>, seen: Set<u64>)
    -> Option<Gfx942CompletionErrorV1>
    decreases rows.len(),
{
    if rows.len() == 0 { None }
    else if seen.contains(rows[0].event_id) { Some(Gfx942CompletionErrorV1::DuplicateDependency) }
    else if !releasable(s, rows[0]) { Some(Gfx942CompletionErrorV1::StaleEventOccurrence) }
    else { legacy_error(s, rows.drop_first(), seen.insert(rows[0].event_id)) }
}

spec fn count(rows: Seq<Gfx942ComputeEventOccurrenceV1>, key: u32) -> nat
    decreases rows.len(),
{
    if rows.len() == 0 { 0 } else {
        count(rows.drop_first(), key) + if rows[0].exact.slot.index == key { 1nat } else { 0nat }
    }
}

spec fn capacity<R>(s: State<R>, remaining: Map<u32, u32>, key: u32) -> u32 {
    if remaining.contains_key(key) { remaining[key] }
    else if (key as int) < s.slots.len() { s.slots[key as int].event_pins } else { 0 }
}

spec fn budget_ok<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>, remaining: Map<u32, u32>) -> bool {
    forall|key: u32| #[trigger] count(rows, key) <= capacity(s, remaining, key)
}

spec fn all_live<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>) -> bool {
    forall|i: int| 0 <= i < rows.len() ==> releasable(s, #[trigger] rows[i])
}

proof fn legacy_live<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>, seen: Set<u64>)
    requires legacy_error(s, rows, seen).is_none(),
    ensures all_live(s, rows),
    decreases rows.len(),
{
    if rows.len() > 0 {
        legacy_live(s, rows.drop_first(), seen.insert(rows[0].event_id));
        assert forall|i: int| 0 <= i < rows.len() implies releasable(s, #[trigger] rows[i]) by {
            if i > 0 { assert(rows[i] == rows.drop_first()[i - 1]); }
        }
    }
}

proof fn debit_decision<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>, remaining: Map<u32, u32>)
    requires rows.len() > 0, capacity(s, remaining, rows[0].exact.slot.index) > 0,
    ensures budget_ok(s, rows, remaining) == budget_ok(s, rows.drop_first(), remaining.insert(
        rows[0].exact.slot.index, (capacity(s, remaining, rows[0].exact.slot.index) - 1) as u32)),
{
    let key = rows[0].exact.slot.index;
    let tail = rows.drop_first();
    let next = remaining.insert(key, (capacity(s, remaining, key) - 1) as u32);
    if budget_ok(s, rows, remaining) {
        assert forall|other: u32| #[trigger] count(tail, other) <= capacity(s, next, other) by {
            assert(count(rows, other) <= capacity(s, remaining, other));
        }
    }
    if budget_ok(s, tail, next) {
        assert forall|other: u32| #[trigger] count(rows, other) <= capacity(s, remaining, other) by {
            assert(count(tail, other) <= capacity(s, next, other));
        }
    }
}

spec fn decision<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>, trace: ReservationTrace)
    -> Option<Result<nat, Gfx942CompletionErrorV1>>
{
    if s.phase != CompletionOwnerPhaseV1::Ready {
        if trace.ids.is_none() && trace.budgets.is_none() { Some(Err(Gfx942CompletionErrorV1::Poisoned)) } else { None }
    } else if trace.ids == Some(false) {
        if trace.budgets.is_none() { Some(Err(Gfx942CompletionErrorV1::DependencyLedgerAllocation)) } else { None }
    } else if trace.ids != Some(true) { None }
    else if let Some(error) = legacy_error(s, rows, Set::empty()) {
        if trace.budgets.is_none() { Some(Err(error)) } else { None }
    } else if rows.len() <= 1 {
        if trace.budgets.is_none() { Some(Ok(rows.len())) } else { None }
    } else if trace.budgets == Some(false) { Some(Err(Gfx942CompletionErrorV1::DependencyLedgerAllocation)) }
    else if trace.budgets != Some(true) { None }
    else if budget_ok(s, rows, Map::empty()) { Some(Ok(rows.len())) }
    else { Some(Err(Gfx942CompletionErrorV1::StaleEventOccurrence)) }
}

spec fn batch_released<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>) -> State<R>
    decreases rows.len(),
{
    if rows.len() == 0 { s } else { batch_released(released(s, rows[0]), rows.drop_first()) }
}

// Fresh IDs and aggregate counts, not a global ledger/cardinality invariant,
// justify each successive removal even when distinct events alias one slot.
proof fn release_legacy_tail<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>,
    head: Gfx942ComputeEventOccurrenceV1, seen: Set<u64>)
    requires legacy_error(s, rows, seen).is_none(), seen.contains(head.event_id),
        releasable(s, head), budget_ok(s, rows.push(head), Map::empty()),
    ensures legacy_error(released(s, head), rows, seen).is_none(),
    decreases rows.len(),
{
    if rows.len() > 0 {
        let tail = rows.drop_first();
        assert(rows[0].event_id != head.event_id);
        let key = rows[0].exact.slot.index;
        count_push(rows, head, key);
        assert(count(rows, key) > 0);
        assert(count(rows.push(head), key) <= capacity(s, Map::empty(), key));
        assert(releasable(released(s, head), rows[0]));
        assert forall|key: u32| #[trigger] count(tail.push(head), key) <= capacity(s, Map::empty(), key) by {
            count_push(rows, head, key); count_push(tail, head, key);
            assert(count(rows.push(head), key) <= capacity(s, Map::empty(), key));
        }
        release_legacy_tail(s, tail, head, seen.insert(rows[0].event_id));
    }
}

proof fn count_push(rows: Seq<Gfx942ComputeEventOccurrenceV1>, event: Gfx942ComputeEventOccurrenceV1, key: u32)
    ensures count(rows.push(event), key) == count(rows, key) + if event.exact.slot.index == key { 1nat } else { 0nat },
    decreases rows.len(),
{
    reveal_with_fuel(count, 2);
    if rows.len() > 0 {
        count_push(rows.drop_first(), event, key);
        assert(rows.push(event).drop_first() =~= rows.drop_first().push(event));
    }
}

proof fn release_step<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>)
    requires rows.len() > 0, legacy_error(s, rows, Set::empty()).is_none(),
        budget_ok(s, rows, Map::empty()),
    ensures releasable(s, rows[0]),
        legacy_error(released(s, rows[0]), rows.drop_first(), Set::empty()).is_none(),
        budget_ok(released(s, rows[0]), rows.drop_first(), Map::empty()),
{
    let head = rows[0]; let tail = rows.drop_first();
    assert forall|key: u32| #[trigger] count(tail.push(head), key) <= capacity(s, Map::empty(), key) by {
        count_push(tail, head, key);
        assert(count(rows, key) <= capacity(s, Map::empty(), key));
    }
    release_legacy_tail(s, tail, head, Set::empty().insert(head.event_id));
    legacy_seen_weaken(released(s, head), tail, Set::empty(), Set::empty().insert(head.event_id));
    assert forall|key: u32| #[trigger] count(tail, key) <= capacity(released(s, head), Map::empty(), key) by {
        assert(count(rows, key) <= capacity(s, Map::empty(), key));
    }
}

proof fn legacy_seen_weaken<R>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>, a: Set<u64>, b: Set<u64>)
    requires a.subset_of(b), legacy_error(s, rows, b).is_none(),
    ensures legacy_error(s, rows, a).is_none(),
    decreases rows.len(),
{
    if rows.len() > 0 {
        legacy_seen_weaken(s, rows.drop_first(), a.insert(rows[0].event_id), b.insert(rows[0].event_id));
    }
}

impl<R> CompletionSignalArenaOwnerV1<R> {
    fn release_compute_event_batch(&mut self, events: Vec<Gfx942ComputeEventOccurrenceV1>)
        -> (out: (Result<usize, (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>)>, Ghost<ReservationTrace>))
        ensures match out.0 {
            Ok(n) => decision(old(self).state(), events@, out.1@) == Some(Ok(n as nat))
                && final(self).state() == batch_released(old(self).state(), events@),
            Err((error, returned)) => decision(old(self).state(), events@, out.1@) == Some(Err(error))
                && final(self).state() == old(self).state() && returned == events,
        },
    {
        let ghost s = self.state(); let ghost rows = events@;
        let ghost mut trace = ReservationTrace { ids: None, budgets: None };
        macro_rules! finish { ($out:expr) => { verus_exec_expr!(($out, Ghost(trace))) }; }
        completion_release_event_batch_body!(@annotated verus_exec_expr, self, events, finish,
            ids, reservation, i, remaining, j, index, available, pending, event, released_count,
            [proof { trace.ids = Some(reservation.is_ok()); assert(suffix(rows, 0) =~= rows); }],
            [invariant self.state() == s, events@ == rows, i <= rows.len(),
                s.phase == CompletionOwnerPhaseV1::Ready, trace.ids == Some(true), trace.budgets.is_none(),
                legacy_error(s, rows, Set::empty()) == legacy_error(s, suffix(rows, i as int), ids@),
             decreases rows.len() - i,],
            [let ghost seen = ids@; proof { suffix_step(rows, i as int); }],
            [proof { assert(legacy_error(s, suffix(rows, i as int), seen)
                == legacy_error(s, suffix(rows, i + 1), ids@)); }],
            [proof { assert(suffix(rows, rows.len() as int) =~= Seq::empty()); legacy_live(s, rows, Set::empty()); }],
            [proof { trace.budgets = Some(reservation.is_ok()); assert(suffix(rows, 0) =~= rows); }],
            [invariant self.state() == s, events@ == rows, j <= rows.len(), rows.len() > 1,
                all_live(s, rows), legacy_error(s, rows, Set::empty()).is_none(),
                trace.ids == Some(true), trace.budgets == Some(true), s.phase == CompletionOwnerPhaseV1::Ready,
                budget_ok(s, rows, Map::empty()) == budget_ok(s, suffix(rows, j as int), remaining@),
             decreases rows.len() - j,],
            [let ghost prior = remaining@; proof { suffix_step(rows, j as int); }],
            [proof { assert(*available == capacity(s, prior, index));
                assert(count(suffix(rows, j as int), index) > 0); }],
            [proof { assert(capacity(s, prior, index) > 0);
                assert(remaining@ =~= prior.insert(index, (capacity(s, prior, index) - 1) as u32));
                debit_decision(s, suffix(rows, j as int), prior); }],
            [proof {
                if rows.len() <= 1 {
                    assert forall|key: u32| #[trigger] count(rows, key) <= capacity(s, Map::empty(), key) by {
                        reveal_with_fuel(count, 2);
                        if rows.len() == 1 {
                            assert(rows.drop_first() =~= Seq::empty());
                            assert(releasable(s, rows[0]));
                        }
                    }
                } else { assert(suffix(rows, rows.len() as int) =~= Seq::empty()); }
                assert(budget_ok(s, rows, Map::empty()));
                assert(decision(s, rows, trace) == Some(Ok(rows.len())));
            } let ghost mut todo = rows;],
            [invariant released_count == rows.len(), decision(s, rows, trace) == Some(Ok(rows.len())),
                s == old(self).state(), rows == events@,
                pending.remaining() == todo,
                pending.obeys_prophetic_iter_laws(), pending.decrease().is_some(),
                legacy_error(self.state(), pending.remaining(), Set::empty()).is_none(),
                budget_ok(self.state(), pending.remaining(), Map::empty()),
                batch_released(self.state(), pending.remaining()) == batch_released(s, rows),
             decreases pending.decrease().unwrap(),],
            [let ghost before = self.state(); let ghost tail = todo;
             proof { if tail.len() > 0 { release_step(before, tail); } }],
            [proof { assert(event == tail[0]); assert(pending.remaining() == tail.drop_first());
                assert(self.state() == released(before, event)); todo = tail.drop_first(); }])
    }
}
}

structural_eq!(DeviceKeyV1, VmKeyV1, QueueKeyV1, MemoryAllocationKeyV1,
    MemoryMappingKeyV1, CompletionSlotLeaseV1, ExactCompletionOccurrenceV1,
    CompletionOwnerPhaseV1, CompletionSlotPhaseV1, CompletionSlotRecordV1);

verus! {
broadcast use vstd::std_specs::hash::group_hash_axioms;

spec fn active<R>(s: State<R>, event: Gfx942ComputeEventOccurrenceV1) -> bool {
    s.events.contains_key(event.event_id) && s.events[event.event_id] == event.exact
}
spec fn live<R>(s: State<R>, exact: ExactCompletionOccurrenceV1) -> bool {
    let i = exact.slot.index as int;
    exact.queue == s.queue && exact.signal_mapping == s.signal_mapping
        && i < s.slots.len() && s.slots[i].generation == exact.slot.generation
        && match s.slots[i].phase {
            CompletionSlotPhaseV1::Bound { batch_id }
            | CompletionSlotPhaseV1::Published { batch_id }
            | CompletionSlotPhaseV1::Completed { batch_id } => batch_id == exact.batch_id,
            _ => false,
        }
}
spec fn releasable<R>(s: State<R>, event: Gfx942ComputeEventOccurrenceV1) -> bool {
    s.phase == CompletionOwnerPhaseV1::Ready && active(s, event) && live(s, event.exact)
        && s.slots[event.exact.slot.index as int].event_pins > 0
}
spec fn refusal<R>(s: State<R>) -> Gfx942CompletionErrorV1 {
    if s.phase == CompletionOwnerPhaseV1::Ready {
        Gfx942CompletionErrorV1::StaleEventOccurrence
    } else { Gfx942CompletionErrorV1::Poisoned }
}
spec fn released<R>(s: State<R>, event: Gfx942ComputeEventOccurrenceV1) -> State<R> {
    let i = event.exact.slot.index as int;
    State {
        slots: s.slots.update(i, CompletionSlotRecordV1 {
            event_pins: (s.slots[i].event_pins - 1) as u32, ..s.slots[i]
        }),
        events: s.events.remove(event.event_id), ..s
    }
}

impl<R> CompletionSignalArenaOwnerV1<R> {
    spec fn state(&self) -> State<R> {
        State { queue: self.queue, signal_mapping: self.signal_mapping,
            gpu_base: self.gpu_base, next_batch_id: self.next_batch_id,
            slots: (*self.slots)@, events: self.dependency_ledger.events@,
            readers: self.dependency_ledger.readers,
            next_event_id: self.dependency_ledger.next_event_id,
            next_reader_lease_id: self.dependency_ledger.next_reader_lease_id,
            phase: self.phase }
    }
    fn require_ready(&self) -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if self.phase == CompletionOwnerPhaseV1::Ready { Ok(()) }
            else { Err(Gfx942CompletionErrorV1::Poisoned) },
    { completion_require_ready_body!(verus_exec_expr, self) }

    fn validate_active_event(&self, event: &Gfx942ComputeEventOccurrenceV1)
        -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if active(self.state(), *event) { Ok(()) }
            else { Err(Gfx942CompletionErrorV1::StaleEventOccurrence) },
    { completion_validate_active_event_body!(verus_exec_expr, self, event) }

    fn validate_live_occurrence(&self, exact: ExactCompletionOccurrenceV1)
        -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if live(self.state(), exact) { Ok(()) }
            else { Err(Gfx942CompletionErrorV1::StaleEventOccurrence) },
    { completion_validate_live_occurrence_body!(verus_exec_expr, self, exact) }

}
}

verus! {
// This witness proves the full method's decision/effects for constructed aliases
// on either actual reservation outcome. It does not promise allocator success.
fn alias_witness<R>(readers: R, pins: u32)
    requires pins > 0,
{
    let vm = VmKeyV1 { device: DeviceKeyV1 { physical: 0, generation: 0 }, id: 0 };
    let queue = QueueKeyV1 { vm, id: 0, generation: 0 };
    let signal_mapping = MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 { vm, id: 0, generation: 0 }, id: 0,
    };
    let exact = ExactCompletionOccurrenceV1 {
        session_occurrence: 0, source_acceptance_epoch: 0, batch_id: 0,
        queue, signal_mapping, slot: CompletionSlotLeaseV1 { index: 1, generation: 0 },
        dispatch_generation: 0, packet_id: None,
    };
    let mut ledger = HashMap::new();
    ledger.insert(7u64, exact); ledger.insert(8u64, exact);
    let mut owner = CompletionSignalArenaOwnerV1 {
        queue, signal_mapping, gpu_base: 0, next_batch_id: u64::MAX,
        slots: Box::new([CompletionSlotRecordV1 {
            generation: u64::MAX, phase: CompletionSlotPhaseV1::Available,
            event_pins: u32::MAX, native_reader_pins: u32::MAX,
        }; 8192]),
        dependency_ledger: Box::new(CompletionDependencyLedgerV1 {
            next_event_id: u64::MAX, next_reader_lease_id: u64::MAX, events: ledger, readers,
        }), phase: CompletionOwnerPhaseV1::Ready,
    };
    owner.slots[1] = CompletionSlotRecordV1 {
        generation: 0, phase: CompletionSlotPhaseV1::Bound { batch_id: 0 },
        event_pins: pins, native_reader_pins: u32::MAX,
    };
    let mut events = Vec::new();
    events.push(Gfx942ComputeEventOccurrenceV1 { event_id: 7, exact });
    events.push(Gfx942ComputeEventOccurrenceV1 { event_id: 8, exact });
    let ghost before = owner.state(); let ghost rows = events@;
    proof {
        reveal_with_fuel(legacy_error, 3);
        assert(legacy_error(before, rows, Set::empty()).is_none());
        reveal_with_fuel(count, 3);
        assert(count(rows, 1) == 2);
        if pins >= 2 {
            assert forall|key: u32| #[trigger] count(rows, key) <= capacity(before, Map::empty(), key) by { }
        }
        assert(budget_ok(before, rows, Map::empty()) == (pins >= 2));
    }
    let (out, Ghost(trace)) = owner.release_compute_event_batch(events);
    if let Ok(n) = out {
        assert(n == 2 && pins >= 2);
        proof { reveal_with_fuel(batch_released, 3); }
        assert(owner.slots[1].event_pins == pins - 2);
        assert(owner.slots[1].native_reader_pins == u32::MAX);
        assert(owner.state().events == Map::<u64, ExactCompletionOccurrenceV1>::empty());
        assert(owner.state().slots[0] == before.slots[0]);
        assert(owner.state().readers == before.readers);
    } else {
        assert(owner.state() == before);
        assert(out.unwrap_err().1 == events);
        assert(out.unwrap_err().0 == Gfx942CompletionErrorV1::DependencyLedgerAllocation
            || (pins == 1 && out.unwrap_err().0 == Gfx942CompletionErrorV1::StaleEventOccurrence));
    }
    assert((trace.ids == Some(true) && trace.budgets == Some(true)) ==
        (out.is_ok() || (pins == 1 && out.unwrap_err().0 == Gfx942CompletionErrorV1::StaleEventOccurrence)));
}
}
