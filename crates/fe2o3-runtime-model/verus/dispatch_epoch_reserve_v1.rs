// Exact host epoch selection/reservation, composed with the existing cancellation
// body. Arbitrary retained credits are framed; native publication is not modeled.
include!("dispatch_epoch_cancel_v1.rs");
include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_reserve_body.rs");

verus! {
const GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1: usize = 64;

impl FixedDispatchCapacityProfileV1 {
    fn slots(self) -> (out: usize)
        ensures out == profile_slots(self),
    { dispatch_capacity_slots_body!(verus_exec_expr, self) }
}

spec fn profile_slots(profile: FixedDispatchCapacityProfileV1) -> usize {
    match profile { FixedDispatchCapacityProfileV1::Default64 => 64,
        FixedDispatchCapacityProfileV1::Qualification1024 => 1024 }
}

spec fn reusable(slots: Seq<DispatchEpochSlotV1>, index: int) -> bool {
    0 <= index < slots.len() && slots[index].phase == DispatchEpochPhaseV1::Vacant
        && slots[index].slot_generation < u64::MAX
}

spec fn first_reusable(slots: Seq<DispatchEpochSlotV1>, index: int) -> bool {
    reusable(slots, index) && forall|i: int| 0 <= i < index ==> !reusable(slots, i)
}

spec fn preflight_error<C>(s: State<C>, queue: QueueKeyV1) -> Option<Gfx942DispatchBindingErrorV1> {
    if s.poisoned { Some(Gfx942DispatchBindingErrorV1::Poisoned) }
    else if s.recipe_queue.is_some() && s.recipe_queue != Some(queue) {
        Some(Gfx942DispatchBindingErrorV1::WrongQueueGeneration)
    } else if !(exists|i: int| reusable(s.slots, i)) {
        if exists|i: int| 0 <= i < s.slots.len() && s.slots[i].phase == DispatchEpochPhaseV1::Vacant {
            Some(Gfx942DispatchBindingErrorV1::GenerationExhausted)
        } else { Some(Gfx942DispatchBindingErrorV1::DispatchEpochCapacity { maximum: profile_slots(s.capacity_profile) }) }
    } else if s.next_generation == u64::MAX { Some(Gfx942DispatchBindingErrorV1::GenerationExhausted) }
    else { None }
}

spec fn roster_matches<C>(s: State<C>, queue: QueueKeyV1, roster: CompletionDispatchRosterV1) -> bool {
    &&& roster.queue == queue
    &&& roster.dispatch_generation == s.next_generation
    &&& roster.packet_count != 0
    &&& (s.capacity_profile == FixedDispatchCapacityProfileV1::Qualification1024 ==> roster.packet_count == 1)
}

spec fn reserve_error<C>(s: State<C>, queue: QueueKeyV1, roster: CompletionDispatchRosterV1)
    -> Option<Gfx942DispatchBindingErrorV1>
{
    if preflight_error(s, queue).is_some() { preflight_error(s, queue) }
    else if !roster_matches(s, queue, roster) { Some(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration) }
    else if !(exists|i: int| 0 <= i <= u16::MAX && reusable(s.slots, i)) {
        Some(Gfx942DispatchBindingErrorV1::ResourcePhase)
    } else { None }
}

spec fn reserved_state<C>(s: State<C>, queue: QueueKeyV1, roster: CompletionDispatchRosterV1, index: int) -> State<C> {
    State { recipe_queue: Some(queue), next_generation: (s.next_generation + 1) as u64,
        slots: s.slots.update(index, DispatchEpochSlotV1 {
            slot_generation: (s.slots[index].slot_generation + 1) as u64,
            phase: DispatchEpochPhaseV1::Reserved { dispatch_generation: s.next_generation, expected_roster: roster },
        }), ..s }
}

impl<C> DispatchGenerationOwnerV1<C> {
    fn preflight_reservation(&self, queue: QueueKeyV1)
        -> (out: Result<(usize, u64, u64), Gfx942DispatchBindingErrorV1>)
        ensures out.is_ok() == preflight_error(self.state(), queue).is_none(),
            match out {
                Err(error) => Some(error) == preflight_error(self.state(), queue),
                Ok((index, generation, slot_generation)) => {
                    &&& first_reusable(self.slots@, index as int)
                    &&& generation == self.next_generation
                    &&& generation < u64::MAX
                    &&& slot_generation == self.slots@[index as int].slot_generation + 1
                    &&& slot_generation > 0
                },
            },
    {
        dispatch_preflight_reservation_body!(@annotated verus_exec_expr, self, queue, index, vacant,
            [invariant 0 <= index <= self.slots.len(), !self.poisoned,
                self.recipe_queue.is_none() || self.recipe_queue == Some(queue),
                forall|i: int| 0 <= i < index ==> !reusable(self.slots@, i),
                vacant == (exists|i: int| 0 <= i < index && self.slots@[i].phase == DispatchEpochPhaseV1::Vacant),
             decreases self.slots.len() - index,],
            [proof { assert(reusable(self.slots@, index as int)); }],
            [proof {
                assert(!reusable(self.slots@, index as int));
                assert(vacant == (exists|i: int| 0 <= i < index + 1 && self.slots@[i].phase == DispatchEpochPhaseV1::Vacant));
            }])
    }

    fn reserve(&mut self, queue: QueueKeyV1, expected_roster: CompletionDispatchRosterV1)
        -> (out: Result<DispatchEpochIdentityV1, Gfx942DispatchBindingErrorV1>)
        ensures out.is_ok() == reserve_error(old(self).state(), queue, expected_roster).is_none(),
            match out {
                Err(error) => final(self).state() == old(self).state()
                    && Some(error) == reserve_error(old(self).state(), queue, expected_roster),
                Ok(id) => {
                    let before = old(self).state();
                    &&& id.queue == queue
                    &&& id.recipe_occurrence == before.recipe_occurrence
                    &&& first_reusable(before.slots, id.slot_index as int)
                    &&& id.slot_generation == before.slots[id.slot_index as int].slot_generation + 1
                    &&& id.slot_generation > 0
                    &&& id.dispatch_generation == before.next_generation
                    &&& id.dispatch_generation < u64::MAX
                    &&& final(self).state() == reserved_state(before, queue, expected_roster, id.slot_index as int)
                    &&& cancellable(final(self).state(), id)
                },
            },
    { dispatch_reserve_epoch_body!(verus_exec_expr, self, queue, expected_roster) }
}

// Real shared calls, no constructor premise: cancel frees only the phase and
// the next reservation consumes the same slot with both counters advanced.
fn reserve_cancel_reserve<C>(owner: &mut DispatchGenerationOwnerV1<C>, queue: QueueKeyV1,
    roster: CompletionDispatchRosterV1)
    requires reserve_error(old(owner).state(), queue, roster).is_none(),
        old(owner).next_generation < u64::MAX - 1,
        forall|i: int| first_reusable(old(owner).slots@, i) ==> old(owner).slots@[i].slot_generation < u64::MAX - 1,
    ensures final(owner).next_generation == old(owner).next_generation + 2,
{
    let ghost before = owner.state();
    let first = match owner.reserve(queue, roster) { Ok(id) => id, Err(_) => { assert(false); return; } };
    assert(before.slots[first.slot_index as int].slot_generation < u64::MAX - 1);
    let cancelled_result = owner.cancel_epoch(first);
    assert(cancelled_result == Ok(()));
    assert(owner.next_generation == before.next_generation + 1);
    assert(owner.slots@[first.slot_index as int].slot_generation == first.slot_generation);
    let next_roster = CompletionDispatchRosterV1 { dispatch_generation: first.dispatch_generation + 1, ..roster };
    assert forall|i: int| 0 <= i < first.slot_index implies !reusable(owner.slots@, i) by {
        assert(owner.slots@[i] == before.slots[i]);
        assert(!reusable(before.slots, i));
    }
    assert(first_reusable(owner.slots@, first.slot_index as int));
    assert(reserve_error(owner.state(), queue, next_roster).is_none());
    let next = match owner.reserve(queue, next_roster) { Ok(id) => id, Err(_) => { assert(false); return; } };
    assert(next.slot_index == first.slot_index);
    assert(next.slot_generation == first.slot_generation + 1);
    assert(next.dispatch_generation == first.dispatch_generation + 1);
    assert(owner.state().credits == before.credits);
}

fn exhausted_prefix_witness<C>(credits: C, queue: QueueKeyV1, digest: [u8; 32]) {
    let mut slots = Vec::new();
    slots.push(DispatchEpochSlotV1 { slot_generation: u64::MAX, phase: DispatchEpochPhaseV1::Vacant });
    slots.push(DispatchEpochSlotV1 { slot_generation: 0, phase: DispatchEpochPhaseV1::Vacant });
    let mut owner = DispatchGenerationOwnerV1 {
        next_generation: 0, recipe_occurrence: 0, recipe_queue: None,
        capacity_profile: FixedDispatchCapacityProfileV1::Default64, slots, credits,
        recycled_generation: Some(u64::MAX), predecessor_detached_generation: Some(0), poisoned: false,
    };
    let roster = CompletionDispatchRosterV1 { queue, dispatch_generation: 0,
        packet_count: usize::MAX, roster_sha256: digest };
    assert(reusable(owner.slots@, 1));
    assert(reserve_error(owner.state(), queue, roster).is_none());
    let id = match owner.reserve(queue, roster) { Ok(id) => id, Err(_) => { assert(false); return; } };
    assert(id.slot_index == 1 && id.slot_generation == 1 && id.dispatch_generation == 0);
    let result = owner.cancel_epoch(id);
    assert(result == Ok(()));
    assert(owner.next_generation == 1 && owner.recipe_queue == Some(queue));
    assert(owner.slots@[0].slot_generation == u64::MAX);
    assert(owner.slots@[1].slot_generation == 1);
    assert(owner.slots@[1].phase == DispatchEpochPhaseV1::Vacant);
    let ghost before = owner.state();
    let stale = owner.cancel_epoch(id);
    assert(stale == Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration));
    assert(owner.state() == before);
}
}
