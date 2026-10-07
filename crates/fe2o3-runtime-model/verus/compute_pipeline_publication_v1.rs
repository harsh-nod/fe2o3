// Shared borrowed-table implementation. T is the entire uninspected Active
// payload, without Copy/Clone assumptions. Native authority is outside this proof.
use vstd::prelude::*;

include!("../../fe2o3-runtime/src/kfd_backend/compute_pipeline_publication_body.rs");

verus! {

#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeComputePipelineIdentityV1 {
    slot: u16, slot_generation: u64, logical_epoch: u64, submission: u64,
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for RuntimeComputePipelineIdentityV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeComputePipelinePhaseV1 { Publishing, Published, Completed, PhysicallyRetired, Quarantined }

impl vstd::std_specs::cmp::PartialEqSpecImpl for RuntimeComputePipelinePhaseV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

struct ActiveSubmissionV1<T> { id: u64, payload: T }
struct RuntimeComputePipelineEntryV1<T> {
    identity: RuntimeComputePipelineIdentityV1,
    phase: RuntimeComputePipelinePhaseV1,
    active: ActiveSubmissionV1<T>,
}
struct RuntimeComputePipelineSlotV1<T> { generation: u64, entry: Option<RuntimeComputePipelineEntryV1<T>> }

spec fn eligible<T>(slot: RuntimeComputePipelineSlotV1<T>) -> bool {
    slot.entry.is_none() && slot.generation < u64::MAX
}

spec fn vacancy_prefix<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int,
    result: Option<(usize, u64)>) -> bool {
    match result {
        None => forall|j: int| 0 <= j < n ==> !eligible(s[j]),
        Some((i, g)) => i < n && eligible(s[i as int]) && g == s[i as int].generation + 1
            && forall|j: int| 0 <= j < i ==> !eligible(s[j]),
    }
}

spec fn vacancy_result<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, result: Option<(usize, u64)>) -> bool {
    vacancy_prefix(s, s.len() as int, result)
}

fn vacant_generation<T>(slot: &RuntimeComputePipelineSlotV1<T>) -> (out: Option<u64>)
    ensures out == if eligible(*slot) { Some((slot.generation + 1) as u64) } else { None },
{
    compute_pipeline_vacant_generation_body!(verus_exec_expr, slot)
}

fn first_vacant<T>(slots: &[RuntimeComputePipelineSlotV1<T>]) -> (out: Option<(usize, u64)>)
    ensures vacancy_result(slots@, out),
{
    compute_pipeline_first_vacant_body!(@annotated verus_exec_expr, slots, i,
        [invariant i <= slots.len(),
            forall|j: int| 0 <= j < i ==> !eligible(slots@[j]),
         decreases slots.len() - i,], [])
}

fn has_capacity<T>(slots: &[RuntimeComputePipelineSlotV1<T>], live: usize, next: Option<u64>,
    staged: Option<RuntimeComputePipelineIdentityV1>) -> (out: bool)
    requires slots.len() > 0,
    ensures out == (staged.is_none() && live < slots.len() - 1 && next.is_some()
        && exists|j: int| 0 <= j < slots.len() && eligible(slots@[j])),
{
    compute_pipeline_capacity_body!(verus_exec_expr, slots, live, next, staged)
}

spec fn exact<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, id: RuntimeComputePipelineIdentityV1) -> bool {
    let i = id.slot as int;
    i < s.len() && s[i].generation == id.slot_generation && s[i].entry.is_some()
        && s[i].entry.unwrap().identity == id && s[i].entry.unwrap().active.id == id.submission
}

fn exact_entry<T>(slots: &[RuntimeComputePipelineSlotV1<T>], identity: RuntimeComputePipelineIdentityV1)
    -> (out: Option<&RuntimeComputePipelineEntryV1<T>>)
    ensures out.is_some() == exact(slots@, identity),
        match out { Some(entry) => slots@[identity.slot as int].entry == Some(*entry), None => true },
{
    compute_pipeline_entry_body!(verus_exec_expr, slots, identity)
}

fn exact_entry_mut<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], identity: RuntimeComputePipelineIdentityV1)
    -> (out: Option<&mut RuntimeComputePipelineEntryV1<T>>)
    ensures out.is_some() == exact(old(slots)@, identity),
        match out {
            None => final(slots)@ == old(slots)@,
            Some(entry) => old(slots)@[identity.slot as int].entry == Some(*entry)
                && final(slots)@ == old(slots)@.update(identity.slot as int,
                    RuntimeComputePipelineSlotV1 {
                        generation: old(slots)@[identity.slot as int].generation,
                        entry: Some(*final(entry)),
                    }),
        },
{
    compute_pipeline_entry_mut_body!(verus_exec_expr, slots, identity)
}

spec fn occupied<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int) -> int
    decreases n,
{
    if n <= 0 { 0 } else { occupied(s, n - 1) + if s[n - 1].entry.is_some() { 1int } else { 0int } }
}

spec fn hit<T>(slot: RuntimeComputePipelineSlotV1<T>, epoch: Option<u64>) -> bool {
    slot.entry.is_some() && Some(slot.entry.unwrap().identity.logical_epoch) == epoch
}

spec fn hits<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int, epoch: Option<u64>) -> int
    decreases n,
{
    if n <= 0 { 0 } else { hits(s, n - 1, epoch) + if hit(s[n - 1], epoch) { 1int } else { 0int } }
}

spec fn valid_slot<T>(slot: RuntimeComputePipelineSlotV1<T>, index: int) -> bool {
    match slot.entry {
        None => true,
        Some(e) => e.identity.slot == index && e.identity.slot_generation == slot.generation
            && e.identity.submission == e.active.id && e.identity.submission != 0,
    }
}

spec fn found_matches<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int,
    epoch: Option<u64>, found: Option<usize>) -> bool {
    match found {
        None => forall|j: int| 0 <= j < n ==> !hit(s[j], epoch),
        Some(i) => i < n && hit(s[i as int], epoch)
            && forall|j: int| 0 <= j < n && hit(s[j], epoch) ==> j == i,
    }
}

spec fn successor(epoch: Option<u64>) -> Option<u64> {
    match epoch { Some(e) => if e < u64::MAX { Some((e + 1) as u64) } else { None }, None => None }
}

spec fn frontier_valid<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, live: usize,
    frontier: Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>) -> bool {
    staged.is_none() && occupied(s, s.len() as int) == live
        && (forall|j: int| 0 <= j < s.len() ==> valid_slot(s[j], j))
        && if live == 0 { frontier.is_none() } else {
            (exists|i: usize| found_matches(s, s.len() as int, frontier, Some(i)))
                && (live > 1 ==> hits(s, s.len() as int, successor(frontier)) == 1)
        }
}

fn checked_frontier<T>(slots: &[RuntimeComputePipelineSlotV1<T>], live: usize,
    frontier: Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>)
    -> (out: Result<Option<usize>, ()>)
    ensures out.is_ok() == frontier_valid(slots@, live, frontier, staged),
        match out {
            Ok(None) => live == 0,
            Ok(Some(i)) => live > 0 && found_matches(slots@, slots.len() as int, frontier, Some(i)),
            Err(()) => true,
        },
{
    compute_pipeline_frontier_body!(@annotated verus_exec_expr, slots, live, frontier, staged,
        i, count, found, next_count, next, entry,
        [invariant i <= slots.len(), count <= i, next_count <= i,
            staged.is_none(), next == successor(frontier),
            count == occupied(slots@, i as int),
            next_count == hits(slots@, i as int, next),
            forall|j: int| 0 <= j < i ==> valid_slot(slots@[j], j),
            found_matches(slots@, i as int, frontier, found),
         decreases slots.len() - i,],
        [proof {
            assert(occupied(slots@, i as int) == occupied(slots@, i - 1)
                + if slots@[i - 1].entry.is_some() { 1int } else { 0int });
            assert(hits(slots@, i as int, next) == hits(slots@, i - 1, next)
                + if hit(slots@[i - 1], next) { 1int } else { 0int });
        }],
        [proof {
            if found.is_some() {
                assert(exists|j: usize| found_matches(slots@, slots.len() as int, frontier, Some(j))) by {
                    assert(found_matches(slots@, slots.len() as int, frontier, Some(found.unwrap())));
                }
            }
        }])
}

spec fn older_or_same<T>(slot: RuntimeComputePipelineSlotV1<T>, id: RuntimeComputePipelineIdentityV1) -> bool {
    match slot.entry {
        None => true,
        Some(e) => e.identity == id || e.identity.logical_epoch < id.logical_epoch,
    }
}

spec fn intact<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, live: usize, next: Option<u64>,
    frontier: Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>,
    id: RuntimeComputePipelineIdentityV1) -> bool {
    staged == Some(id) && id.slot_generation != 0 && id.logical_epoch != 0 && id.submission != 0
        && next == Some(id.logical_epoch) && live != 0 && exact(s, id)
        && s[id.slot as int].entry.unwrap().phase == RuntimeComputePipelinePhaseV1::Publishing
        && occupied(s, s.len() as int) == live
        && (forall|j: int| 0 <= j < s.len() ==> older_or_same(s[j], id))
        && if live == 1 { frontier.is_none() } else {
            frontier.is_some() && frontier.unwrap() < id.logical_epoch
                && hits(s, s.len() as int, frontier) == 1
        }
}

fn staged_intact<T>(slots: &[RuntimeComputePipelineSlotV1<T>], live: usize, next: Option<u64>,
    frontier: Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1) -> (out: bool)
    ensures out == intact(slots@, live, next, frontier, staged, identity),
{
    compute_pipeline_staged_intact_body!(@annotated verus_exec_expr, slots, live, next, frontier,
        staged, identity, i, count, frontier_count, entry,
        [invariant i <= slots.len(), count <= i, frontier_count <= i,
            staged == Some(identity), identity.slot_generation != 0,
            identity.logical_epoch != 0, identity.submission != 0,
            next == Some(identity.logical_epoch), live != 0, exact(slots@, identity),
            slots@[identity.slot as int].entry.unwrap().phase == RuntimeComputePipelinePhaseV1::Publishing,
            count == occupied(slots@, i as int),
            frontier_count == hits(slots@, i as int, frontier),
            forall|j: int| 0 <= j < i ==> older_or_same(slots@[j], identity),
         decreases slots.len() - i,],
        [proof {
            assert(occupied(slots@, i as int) == occupied(slots@, i - 1)
                + if slots@[i - 1].entry.is_some() { 1int } else { 0int });
            assert(hits(slots@, i as int, frontier) == hits(slots@, i - 1, frontier)
                + if hit(slots@[i - 1], frontier) { 1int } else { 0int });
        }], [])
}

spec fn stage_sibling<T>(slot: RuntimeComputePipelineSlotV1<T>, id: u64, epoch: u64) -> bool {
    match slot.entry { None => true, Some(e) => e.active.id != id && e.identity.logical_epoch < epoch }
}

spec fn stage_ready<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, live: usize, next: Option<u64>,
    frontier: Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>, active: ActiveSubmissionV1<T>) -> bool {
    active.id != 0 && staged.is_none() && live < s.len() - 1 && next.is_some() && next.unwrap() != 0
        && frontier_valid(s, live, frontier, staged)
        && (exists|j: int| 0 <= j < s.len() && eligible(s[j]))
        && (forall|j: int| 0 <= j < s.len() ==> stage_sibling(s[j], active.id, next.unwrap()))
}

fn stage<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize, next: Option<u64>,
    frontier: Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>, active: ActiveSubmissionV1<T>)
    -> (out: Result<RuntimeComputePipelineIdentityV1, ActiveSubmissionV1<T>>)
    requires 1 <= old(slots).len() <= 65536,
    ensures out.is_ok() == stage_ready(old(slots)@, *old(live), next, frontier, *old(staged), active),
        match out {
            Err(returned) => returned == active && final(slots)@ == old(slots)@
                && *final(live) == *old(live) && *final(staged) == *old(staged),
            Ok(id) => vacancy_result(old(slots)@, Some((id.slot as usize, id.slot_generation)))
                && id.logical_epoch == next.unwrap() && id.submission == active.id
                && *final(live) == *old(live) + 1 && *final(staged) == Some(id)
                && final(slots)@ == old(slots)@.update(id.slot as int,
                    RuntimeComputePipelineSlotV1 {
                        generation: id.slot_generation,
                        entry: Some(RuntimeComputePipelineEntryV1 {
                            identity: id, phase: RuntimeComputePipelinePhaseV1::Publishing, active,
                        }),
                    }),
        },
{
    compute_pipeline_stage_body!(@annotated verus_exec_expr, slots, live, next, frontier, staged,
        active, i, candidate, epoch, index, generation, identity,
        [let ghost before = slots@;],
        [invariant 1 <= slots.len() <= 65536, i <= slots.len(), slots@ == before, before == old(slots)@,
            *live == *old(live), *staged == *old(staged),
            active.id != 0, staged.is_none(), *live < slots.len() - 1,
            next == Some(epoch), epoch != 0, frontier_valid(slots@, *live, frontier, *staged),
            vacancy_prefix(slots@, i as int, candidate),
            forall|j: int| 0 <= j < i ==> stage_sibling(slots@[j], active.id, epoch),
         decreases slots.len() - i,], [],
        [proof {
            assert(exists|j: int| 0 <= j < slots.len() && eligible(slots@[j])) by {
                assert(eligible(slots@[index as int]));
            }
        }], [])
}

spec fn published<T>(slot: RuntimeComputePipelineSlotV1<T>) -> RuntimeComputePipelineSlotV1<T> {
    RuntimeComputePipelineSlotV1 {
        generation: slot.generation,
        entry: Some(RuntimeComputePipelineEntryV1 {
            identity: slot.entry.unwrap().identity,
            phase: RuntimeComputePipelinePhaseV1::Published,
            active: slot.entry.unwrap().active,
        }),
    }
}

fn confirm<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: usize, next: &mut Option<u64>,
    frontier: &mut Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1) -> (out: Result<(), ()>)
    ensures out.is_ok() == intact(old(slots)@, live, *old(next), *old(frontier), *old(staged), identity),
        match out {
            Err(()) => final(slots)@ == old(slots)@ && *final(next) == *old(next)
                && *final(frontier) == *old(frontier) && *final(staged) == *old(staged),
            Ok(()) => final(slots)@ == old(slots)@.update(identity.slot as int,
                    published(old(slots)@[identity.slot as int]))
                && *final(next) == successor(Some(identity.logical_epoch)) && final(staged).is_none()
                && *final(frontier) == if old(frontier).is_none() { Some(identity.logical_epoch) } else { *old(frontier) },
        },
{
    compute_pipeline_confirm_body!(verus_exec_expr, slots, live, next, frontier, staged, identity)
}

fn withdraw<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize, next: Option<u64>,
    frontier: Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1) -> (out: Option<ActiveSubmissionV1<T>>)
    ensures out.is_some() == intact(old(slots)@, *old(live), next, frontier, *old(staged), identity),
        match out {
            None => final(slots)@ == old(slots)@ && *final(live) == *old(live)
                && *final(staged) == *old(staged),
            Some(active) => active == old(slots)@[identity.slot as int].entry.unwrap().active
                && *final(live) == *old(live) - 1 && final(staged).is_none()
                && final(slots)@ == old(slots)@.update(identity.slot as int,
                    RuntimeComputePipelineSlotV1 {
                        generation: old(slots)@[identity.slot as int].generation, entry: None,
                    }),
        },
{
    compute_pipeline_withdraw_body!(verus_exec_expr, slots, live, next, frontier, staged, identity)
}

// A separate stronger invariant, not a premise that hides representable
// corruption in the raw acceptance/refusal contracts above.
spec fn unique_roster<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>) -> bool {
    &&& forall|i: int| 0 <= i < s.len() ==> valid_slot(s[i], i)
        && (s[i].entry.is_some() ==> s[i].generation != 0 && s[i].entry.unwrap().identity.logical_epoch != 0)
    &&& forall|i: int, j: int| 0 <= i < j < s.len() && s[i].entry.is_some() && s[j].entry.is_some()
        ==> s[i].entry.unwrap().active.id != s[j].entry.unwrap().active.id
            && s[i].entry.unwrap().identity.logical_epoch != s[j].entry.unwrap().identity.logical_epoch
}

proof fn occupancy_update<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int, index: int,
    slot: RuntimeComputePipelineSlotV1<T>)
    requires 0 <= index < s.len(), 0 <= n <= s.len(),
    ensures occupied(s.update(index, slot), n) == occupied(s, n)
        + if index < n {
            (if slot.entry.is_some() { 1int } else { 0int })
                - (if s[index].entry.is_some() { 1int } else { 0int })
        } else { 0int },
    decreases n,
{
    if n > 0 { occupancy_update(s, n - 1, index, slot); }
}

proof fn hits_update<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int, index: int,
    slot: RuntimeComputePipelineSlotV1<T>, epoch: Option<u64>)
    requires 0 <= index < s.len(), 0 <= n <= s.len(),
    ensures hits(s.update(index, slot), n, epoch) == hits(s, n, epoch)
        + if index < n {
            (if hit(slot, epoch) { 1int } else { 0int })
                - (if hit(s[index], epoch) { 1int } else { 0int })
        } else { 0int },
    decreases n,
{
    if n > 0 { hits_update(s, n - 1, index, slot, epoch); }
}

proof fn found_count<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int,
    epoch: Option<u64>, found: Option<usize>)
    requires 0 <= n <= s.len(), found_matches(s, n, epoch, found),
    ensures hits(s, n, epoch) == if found.is_some() { 1int } else { 0int },
    decreases n,
{
    if n > 0 {
        if found.is_some() && found.unwrap() == n - 1 {
            found_count(s, n - 1, epoch, None);
        } else { found_count(s, n - 1, epoch, found); }
    }
}

fn stage_is_settleable<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize,
    next: Option<u64>, frontier: Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    active: ActiveSubmissionV1<T>) -> (out: Result<RuntimeComputePipelineIdentityV1, ActiveSubmissionV1<T>>)
    requires 1 <= old(slots).len() <= 65536,
    ensures match out {
        Ok(id) => intact(final(slots)@, *final(live), next, frontier, *final(staged), id),
        Err(_) => true,
    },
{
    let ghost before = slots@;
    let ghost previous_live = *live;
    let result = stage(slots, live, next, frontier, staged, active);
    proof {
        if let Ok(id) = result {
            let index = id.slot as int;
            occupancy_update(before, before.len() as int, index, slots@[index]);
            if previous_live > 0 {
                let found = choose|j: usize| found_matches(before, before.len() as int, frontier, Some(j));
                assert(stage_sibling(before[found as int], active.id, next.unwrap()));
                found_count(before, before.len() as int, frontier, Some(found));
                hits_update(before, before.len() as int, index, slots@[index], frontier);
            }
            assert forall|j: int| 0 <= j < slots.len() implies older_or_same(slots@[j], id) by {
                if j != index { assert(stage_sibling(before[j], active.id, next.unwrap())); }
            }
        }
    }
    result
}

fn stage_preserves_roster<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize,
    next: Option<u64>, frontier: Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    active: ActiveSubmissionV1<T>) -> (out: Result<RuntimeComputePipelineIdentityV1, ActiveSubmissionV1<T>>)
    requires 1 <= old(slots).len() <= 65536, unique_roster(old(slots)@),
        occupied(old(slots)@, old(slots).len() as int) == *old(live),
    ensures unique_roster(final(slots)@),
        occupied(final(slots)@, final(slots).len() as int) == *final(live),
{
    let ghost before = slots@;
    let result = stage(slots, live, next, frontier, staged, active);
    proof {
        if let Ok(id) = result {
            let index = id.slot as int;
            occupancy_update(before, before.len() as int, index, slots@[index]);
            assert forall|i: int| 0 <= i < slots.len() implies valid_slot(slots@[i], i)
                && (slots@[i].entry.is_some() ==> slots@[i].generation != 0
                    && slots@[i].entry.unwrap().identity.logical_epoch != 0) by {
                if i != index { assert(slots@[i] == before[i]); }
            }
            assert forall|i: int, j: int| 0 <= i < j < slots.len()
                && slots@[i].entry.is_some() && slots@[j].entry.is_some() implies
                slots@[i].entry.unwrap().active.id != slots@[j].entry.unwrap().active.id
                && slots@[i].entry.unwrap().identity.logical_epoch != slots@[j].entry.unwrap().identity.logical_epoch by {
                if i == index { assert(stage_sibling(before[j], active.id, next.unwrap())); }
                else if j == index { assert(stage_sibling(before[i], active.id, next.unwrap())); }
                else { assert(before[i].entry.unwrap().identity.logical_epoch != before[j].entry.unwrap().identity.logical_epoch); }
            }
        }
    }
    result
}

fn confirm_preserves_roster<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: usize,
    next: &mut Option<u64>, frontier: &mut Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1) -> (out: Result<(), ()>)
    requires unique_roster(old(slots)@), occupied(old(slots)@, old(slots).len() as int) == live,
    ensures unique_roster(final(slots)@), occupied(final(slots)@, final(slots).len() as int) == live,
{
    let ghost before = slots@;
    let result = confirm(slots, live, next, frontier, staged, identity);
    proof {
        if result.is_ok() {
            occupancy_update(before, before.len() as int, identity.slot as int, slots@[identity.slot as int]);
        }
    }
    result
}

fn withdraw_preserves_roster<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize,
    next: Option<u64>, frontier: Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1) -> (out: Option<ActiveSubmissionV1<T>>)
    requires unique_roster(old(slots)@), occupied(old(slots)@, old(slots).len() as int) == *old(live),
    ensures unique_roster(final(slots)@),
        occupied(final(slots)@, final(slots).len() as int) == *final(live),
{
    let ghost before = slots@;
    let result = withdraw(slots, live, next, frontier, staged, identity);
    proof {
        if result.is_some() {
            occupancy_update(before, before.len() as int, identity.slot as int, slots@[identity.slot as int]);
        }
    }
    result
}

fn vacant_retry_witness<T>(payload: T) {
    let mut slots = [RuntimeComputePipelineSlotV1 { generation: 0, entry: None },
        RuntimeComputePipelineSlotV1 { generation: 0, entry: None }];
    let mut live = 0usize;
    let mut staged = None;
    let mut next = Some(u64::MAX);
    let mut frontier = None;
    let owner = ActiveSubmissionV1 { id: 7, payload };
    let ghost original = owner;
    proof {
        reveal_with_fuel(occupied, 3);
        assert(eligible(slots@[0]));
        assert(stage_ready(slots@, live, next, frontier, staged, owner));
    }
    let Ok(first) = stage(&mut slots, &mut live, next, frontier, &mut staged, owner)
        else { assert(false); return; };
    assert(first.slot == 0 && first.slot_generation == 1 && first.logical_epoch == u64::MAX);
    proof {
        reveal_with_fuel(occupied, 3);
        assert(intact(slots@, live, next, frontier, staged, first));
    }
    let returned = withdraw(&mut slots, &mut live, next, frontier, &mut staged, first).unwrap();
    assert(returned == original && live == 0 && next == Some(u64::MAX));
    proof {
        reveal_with_fuel(occupied, 3);
        assert(eligible(slots@[0]));
        assert(stage_ready(slots@, live, next, frontier, staged, returned));
    }
    let Ok(second) = stage(&mut slots, &mut live, next, frontier, &mut staged, returned)
        else { assert(false); return; };
    assert(second.slot == first.slot && second.slot_generation == 2 && second.logical_epoch == first.logical_epoch);
    proof {
        reveal_with_fuel(occupied, 3);
        assert(intact(slots@, live, next, frontier, staged, second));
    }
    confirm(&mut slots, live, &mut next, &mut frontier, &mut staged, second).unwrap();
    assert(next.is_none() && frontier == Some(u64::MAX));
    assert(slots@[0].entry.unwrap().active == original);
    assert(slots@[0].entry.unwrap().phase == RuntimeComputePipelinePhaseV1::Published);
}

}
