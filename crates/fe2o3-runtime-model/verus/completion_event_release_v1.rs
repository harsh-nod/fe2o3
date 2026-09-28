// The included bodies are the production host-ledger algorithm. Numeric identity
// newtypes project to u64, preserving their complete structural equality. Reader
// storage is an arbitrary non-Copy payload. No native/device effects are modeled.
use std::collections::HashMap;
use vstd::prelude::*;

// Verify the debug check even in configurations where Rust would erase it.
macro_rules! debug_assert_eq {
    ($left:expr, $right:expr) => { verus_exec_expr!({ assert($left == $right); }) };
}

include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");

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
enum Gfx942CompletionErrorV1 { Poisoned, StaleEventOccurrence }
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

    fn event_release_preflight(&self, event: &Gfx942ComputeEventOccurrenceV1)
        -> (out: Result<u32, Gfx942CompletionErrorV1>)
        ensures out == if releasable(self.state(), *event) {
            Ok((self.state().slots[event.exact.slot.index as int].event_pins - 1) as u32)
        } else { Err(refusal(self.state())) },
    { completion_event_release_preflight_body!(verus_exec_expr, self, event) }

    fn release_compute_event(&mut self, event: Gfx942ComputeEventOccurrenceV1)
        -> (out: Result<Gfx942ComputeEventReleaseObservationV1,
            (Gfx942CompletionErrorV1, Gfx942ComputeEventOccurrenceV1)>)
        ensures out.is_ok() == releasable(old(self).state(), event),
            match out {
                Ok(_) => final(self).state() == released(old(self).state(), event),
                Err((error, returned)) => final(self).state() == old(self).state()
                    && returned == event && error == refusal(old(self).state()),
            },
    { completion_release_event_body!(verus_exec_expr, self, event) }
}

// A constructed success witness for every accepted phase and positive pin count.
// Zero logical IDs, an unbound packet, surviving readers and malformed neighbors
// are intentional: this raw release contract must not assume creation provenance.
fn accepted_witness<R>(readers: R, phase: u8, pins: u32)
    requires phase < 3, pins > 0,
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
    let mut events = HashMap::new();
    events.insert(7u64, exact);
    let mut owner = CompletionSignalArenaOwnerV1 {
        queue, signal_mapping, gpu_base: 0, next_batch_id: u64::MAX,
        slots: Box::new([CompletionSlotRecordV1 {
            generation: u64::MAX, phase: CompletionSlotPhaseV1::Available,
            event_pins: u32::MAX, native_reader_pins: u32::MAX,
        }; 8192]),
        dependency_ledger: Box::new(CompletionDependencyLedgerV1 {
            next_event_id: u64::MAX, next_reader_lease_id: u64::MAX, events, readers,
        }),
        phase: CompletionOwnerPhaseV1::Ready,
    };
    owner.slots[1] = CompletionSlotRecordV1 {
        generation: 0, event_pins: pins, native_reader_pins: u32::MAX,
        phase: if phase == 0 { CompletionSlotPhaseV1::Bound { batch_id: 0 } }
            else if phase == 1 { CompletionSlotPhaseV1::Published { batch_id: 0 } }
            else { CompletionSlotPhaseV1::Completed { batch_id: 0 } },
    };
    let ghost before = owner.state();
    let result = owner.release_compute_event(Gfx942ComputeEventOccurrenceV1 { event_id: 7, exact });
    assert(result.is_ok());
    assert(owner.state().events == Map::<u64, ExactCompletionOccurrenceV1>::empty());
    assert(owner.slots[1].event_pins == pins - 1);
    assert(owner.slots[1].native_reader_pins == u32::MAX);
    assert(owner.state().readers == before.readers);
    assert(owner.state().slots[0] == before.slots[0]);
}
}
