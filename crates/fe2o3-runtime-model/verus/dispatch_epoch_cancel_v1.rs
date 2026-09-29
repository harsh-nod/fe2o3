// The shared production bodies operate on the retained table's slice payload.
// HostMetadataTableV1 dereferences its Vec; its untouched accounting payload is
// arbitrary C here. Allocation, Drop, native execution and the adapter are not proved.
use vstd::prelude::*;

include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_cancel_body.rs");

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
struct CompletionDispatchRosterV1 {
    queue: QueueKeyV1, packet_count: usize, dispatch_generation: u64, roster_sha256: [u8; 32],
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletionBatchOccurrenceV1 {
    batch_id: u64, queue: QueueKeyV1, signal_mapping: MemoryMappingKeyV1,
    packet_count: usize, first_packet_id: u64, last_packet_id: u64,
    roster_sha256: [u8; 32], dispatch_roster: CompletionDispatchRosterV1,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum DispatchEpochPhaseV1 {
    Vacant,
    Reserved { dispatch_generation: u64, expected_roster: CompletionDispatchRosterV1 },
    Published { dispatch_generation: u64, completion: CompletionBatchOccurrenceV1 },
    Completed { dispatch_generation: u64, completion: CompletionBatchOccurrenceV1 },
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct DispatchEpochSlotV1 { slot_generation: u64, phase: DispatchEpochPhaseV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct DispatchEpochIdentityV1 {
    queue: QueueKeyV1, recipe_occurrence: u64, slot_index: u16,
    slot_generation: u64, dispatch_generation: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum FixedDispatchCapacityProfileV1 { Default64, Qualification1024 }
enum Gfx942DispatchBindingErrorV1 {
    Poisoned, StaleDispatchGeneration, WrongQueueGeneration, GenerationExhausted,
    DispatchEpochCapacity { maximum: usize }, ResourcePhase,
}
struct DispatchGenerationOwnerV1<C> {
    next_generation: u64, recipe_occurrence: u64, recipe_queue: Option<QueueKeyV1>,
    capacity_profile: FixedDispatchCapacityProfileV1, slots: Vec<DispatchEpochSlotV1>,
    credits: C, recycled_generation: Option<u64>, predecessor_detached_generation: Option<u64>,
    poisoned: bool,
}
struct State<C> {
    next_generation: u64, recipe_occurrence: u64, recipe_queue: Option<QueueKeyV1>,
    capacity_profile: FixedDispatchCapacityProfileV1, slots: Seq<DispatchEpochSlotV1>,
    credits: C, recycled_generation: Option<u64>, predecessor_detached_generation: Option<u64>,
    poisoned: bool,
}
}

structural_eq!(DeviceKeyV1, VmKeyV1, QueueKeyV1, MemoryAllocationKeyV1, MemoryMappingKeyV1,
    CompletionDispatchRosterV1, CompletionBatchOccurrenceV1, DispatchEpochPhaseV1,
    DispatchEpochSlotV1, DispatchEpochIdentityV1, FixedDispatchCapacityProfileV1);

verus! {
spec fn identity_matches<C>(s: State<C>, id: DispatchEpochIdentityV1, expected: DispatchEpochPhaseV1) -> bool {
    &&& id.recipe_occurrence == s.recipe_occurrence
    &&& Some(id.queue) == s.recipe_queue
    &&& (id.slot_index as int) < s.slots.len()
    &&& s.slots[id.slot_index as int].slot_generation == id.slot_generation
    &&& s.slots[id.slot_index as int].phase == expected
}

spec fn reserved<C>(s: State<C>, id: DispatchEpochIdentityV1) -> bool {
    &&& (id.slot_index as int) < s.slots.len()
    &&& match s.slots[id.slot_index as int].phase {
        DispatchEpochPhaseV1::Reserved { dispatch_generation, .. } => dispatch_generation == id.dispatch_generation,
        _ => false,
    }
}

spec fn cancellable<C>(s: State<C>, id: DispatchEpochIdentityV1) -> bool {
    &&& !s.poisoned
    &&& reserved(s, id)
    &&& id.recipe_occurrence == s.recipe_occurrence
    &&& Some(id.queue) == s.recipe_queue
    &&& s.slots[id.slot_index as int].slot_generation == id.slot_generation
}

spec fn refusal<C>(s: State<C>) -> Gfx942DispatchBindingErrorV1 {
    if s.poisoned { Gfx942DispatchBindingErrorV1::Poisoned }
    else { Gfx942DispatchBindingErrorV1::StaleDispatchGeneration }
}

spec fn cancelled<C>(s: State<C>, id: DispatchEpochIdentityV1) -> State<C> {
    State { slots: s.slots.update(id.slot_index as int, DispatchEpochSlotV1 {
        phase: DispatchEpochPhaseV1::Vacant, ..s.slots[id.slot_index as int]
    }), ..s }
}

impl<C> DispatchGenerationOwnerV1<C> {
    spec fn state(&self) -> State<C> {
        State { next_generation: self.next_generation, recipe_occurrence: self.recipe_occurrence,
            recipe_queue: self.recipe_queue, capacity_profile: self.capacity_profile,
            slots: self.slots@, credits: self.credits, recycled_generation: self.recycled_generation,
            predecessor_detached_generation: self.predecessor_detached_generation, poisoned: self.poisoned }
    }

    fn ensure_not_poisoned(&self) -> (out: Result<(), Gfx942DispatchBindingErrorV1>)
        ensures out == if self.poisoned { Err(Gfx942DispatchBindingErrorV1::Poisoned) } else { Ok(()) },
    { dispatch_not_poisoned_body!(verus_exec_expr, self) }

    fn require_identity(&self, identity: DispatchEpochIdentityV1, expected: DispatchEpochPhaseV1)
        -> (out: Result<(), Gfx942DispatchBindingErrorV1>)
        ensures out == if !self.poisoned && identity_matches(self.state(), identity, expected) {
            Ok(())
        } else { Err(refusal(self.state())) },
    { dispatch_require_identity_body!(verus_exec_expr, self, identity, expected) }

    fn expected_roster(&self, identity: DispatchEpochIdentityV1)
        -> (out: Result<CompletionDispatchRosterV1, Gfx942DispatchBindingErrorV1>)
        ensures
            out.is_ok() == (!self.poisoned && reserved(self.state(), identity)),
            match out {
                Ok(roster) => (identity.slot_index as int) < self.slots@.len()
                    && self.slots@[identity.slot_index as int].phase == DispatchEpochPhaseV1::Reserved {
                    dispatch_generation: identity.dispatch_generation, expected_roster: roster },
                Err(error) => error == refusal(self.state()),
            },
    { dispatch_expected_roster_body!(verus_exec_expr, self, identity) }

    fn cancel_epoch(&mut self, identity: DispatchEpochIdentityV1)
        -> (out: Result<(), Gfx942DispatchBindingErrorV1>)
        ensures
            out == if cancellable(old(self).state(), identity) { Ok(()) } else { Err(refusal(old(self).state())) },
            final(self).state() == if out.is_ok() { cancelled(old(self).state(), identity) } else { old(self).state() },
    { dispatch_cancel_epoch_body!(verus_exec_expr, self, identity) }
}

// A real second slot with arbitrary roster and neighbor payload: cancellation
// authenticates identity, not roster internals, capacity consistency or counters.
fn cancellation_witness<C>(credits: C, queue: QueueKeyV1, roster: CompletionDispatchRosterV1,
    neighbor: DispatchEpochSlotV1, slot_generation: u64, generation: u64)
{
    let mut slots = Vec::new();
    slots.push(neighbor);
    slots.push(DispatchEpochSlotV1 { slot_generation, phase: DispatchEpochPhaseV1::Reserved {
        dispatch_generation: generation, expected_roster: roster } });
    let mut owner = DispatchGenerationOwnerV1 {
        next_generation: u64::MAX, recipe_occurrence: 0, recipe_queue: Some(queue),
        capacity_profile: FixedDispatchCapacityProfileV1::Qualification1024, slots, credits,
        recycled_generation: Some(0), predecessor_detached_generation: Some(u64::MAX), poisoned: false,
    };
    let identity = DispatchEpochIdentityV1 { queue, recipe_occurrence: 0, slot_index: 1,
        slot_generation, dispatch_generation: generation };
    let ghost before = owner.state();
    let result = owner.cancel_epoch(identity);
    assert(result == Ok(()));
    assert(owner.state() == cancelled(before, identity));
    assert(owner.slots@[0] == neighbor);
    assert(owner.slots@[1].slot_generation == slot_generation);
    assert(owner.next_generation == u64::MAX);
    let ghost released = owner.state();
    let again = owner.cancel_epoch(identity);
    assert(again == Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration));
    assert(owner.state() == released);
    owner.poisoned = true;
    let ghost poisoned = owner.state();
    let rejected = owner.cancel_epoch(identity);
    assert(rejected == Err(Gfx942DispatchBindingErrorV1::Poisoned));
    assert(owner.state() == poisoned);
}
}
