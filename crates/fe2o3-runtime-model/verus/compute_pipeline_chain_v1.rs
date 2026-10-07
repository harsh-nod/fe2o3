// Extends the shared publication proof with promotion, quarantine and the full
// metadata epoch interval. Native receipt and outer terminal gates remain separate.
include!("compute_pipeline_publication_v1.rs");
include!("../../fe2o3-runtime/src/kfd_backend/compute_pipeline_lifecycle_body.rs");

verus! {

spec fn first_epoch_match<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, epoch: u64, out: Option<usize>) -> bool {
    match out {
        None => forall|j: int| 0 <= j < s.len() ==> !hit(s[j], Some(epoch)),
        Some(i) => i < s.len() && hit(s[i as int], Some(epoch))
            && forall|j: int| 0 <= j < i ==> !hit(s[j], Some(epoch)),
    }
}

fn first_epoch<T>(slots: &[RuntimeComputePipelineSlotV1<T>], epoch: u64) -> (out: Option<usize>)
    ensures first_epoch_match(slots@, epoch, out),
{
    compute_pipeline_first_epoch_body!(@annotated verus_exec_expr, slots, epoch, i,
        [invariant i <= slots.len(),
            forall|j: int| 0 <= j < i ==> !hit(slots@[j], Some(epoch)),
         decreases slots.len() - i,])
}

spec fn promotion_exists<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, frontier: Option<u64>,
    staged: Option<RuntimeComputePipelineIdentityV1>) -> bool {
    staged.is_none() && frontier.is_some()
        && exists|i: int| 0 <= i < s.len() && hit(s[i], frontier)
}

spec fn emptied<T>(slot: RuntimeComputePipelineSlotV1<T>) -> RuntimeComputePipelineSlotV1<T> {
    RuntimeComputePipelineSlotV1 { generation: slot.generation, entry: None }
}

spec fn promotion_frame<T>(before: Seq<RuntimeComputePipelineSlotV1<T>>, after: Seq<RuntimeComputePipelineSlotV1<T>>,
    epoch: u64, phase: RuntimeComputePipelinePhaseV1, active: ActiveSubmissionV1<T>, index: usize) -> bool {
    first_epoch_match(before, epoch, Some(index))
        && before[index as int].entry.unwrap().phase == phase
        && before[index as int].entry.unwrap().active == active
        && after == before.update(index as int, emptied(before[index as int]))
}

fn take_frontier<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize,
    frontier: &mut Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>)
    -> (out: Option<(RuntimeComputePipelinePhaseV1, ActiveSubmissionV1<T>)>)
    // The raw helper otherwise removes the entry before checked_sub panics.
    requires promotion_exists(old(slots)@, *old(frontier), staged) ==> *old(live) > 0,
    ensures out.is_some() == promotion_exists(old(slots)@, *old(frontier), staged),
        out.is_none() ==> final(slots)@ == old(slots)@ && *final(live) == *old(live)
            && *final(frontier) == *old(frontier),
        out.is_some() ==> *final(live) == *old(live) - 1,
        out.is_some() ==> *final(frontier) == (if *old(live) == 1 { None } else { successor(*old(frontier)) }),
        out.is_some() ==> exists|i: usize| promotion_frame(old(slots)@, final(slots)@,
                (*old(frontier)).unwrap(), out.unwrap().0, out.unwrap().1, i),
{
    compute_pipeline_take_frontier_body!(@annotated verus_exec_expr, slots, live, frontier, staged,
        epoch, index, entry, result, [let ghost before = slots@;],
        [proof {
            assert(exists|i: int| 0 <= i < slots.len() && hit(slots@[i], Some(epoch))) by {
                assert(hit(slots@[index as int], Some(epoch)));
            }
        }],
        [proof {
            assert(before == old(slots)@);
            assert(epoch == (*old(frontier)).unwrap());
            assert(promotion_frame(before, slots@, epoch, result.unwrap().0, result.unwrap().1, index));
            assert(exists|i: usize| promotion_frame(before, slots@, epoch, result.unwrap().0, result.unwrap().1, i));
        }])
}

spec fn quarantined<T>(slot: RuntimeComputePipelineSlotV1<T>) -> RuntimeComputePipelineSlotV1<T> {
    RuntimeComputePipelineSlotV1 {
        generation: slot.generation,
        entry: match slot.entry {
            None => None,
            Some(e) => Some(RuntimeComputePipelineEntryV1 {
                identity: e.identity, phase: RuntimeComputePipelinePhaseV1::Quarantined, active: e.active,
            }),
        },
    }
}

fn quarantine<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>])
    ensures final(slots).len() == old(slots).len(),
        forall|j: int| 0 <= j < old(slots).len() ==> final(slots)@[j] == quarantined(old(slots)@[j]),
{
    compute_pipeline_quarantine_body!(@annotated verus_exec_expr, slots, i,
        [let ghost before = slots@;],
        [invariant i <= slots.len(), slots.len() == before.len(), before == old(slots)@,
            forall|j: int| 0 <= j < i ==> slots@[j] == quarantined(before[j]),
            forall|j: int| i <= j < slots.len() ==> slots@[j] == before[j],
         decreases slots.len() - i,], [])
}

spec fn logical_head(next: Option<u64>) -> int {
    match next { Some(epoch) => epoch as int, None => u64::MAX as int + 1 }
}

spec fn logical_lower(next: Option<u64>, frontier: Option<u64>) -> int {
    match frontier { Some(epoch) => epoch as int, None => logical_head(next) }
}

spec fn is_staged<T>(slot: RuntimeComputePipelineSlotV1<T>, staged: Option<RuntimeComputePipelineIdentityV1>) -> bool {
    slot.entry.is_some() && Some(slot.entry.unwrap().identity) == staged
}

spec fn has_epoch<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, epoch: u64,
    staged: Option<RuntimeComputePipelineIdentityV1>) -> bool {
    exists|i: int| 0 <= i < s.len() && hit(s[i], Some(epoch)) && !is_staged(s[i], staged)
}

spec fn chain<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, live: usize, next: Option<u64>,
    frontier: Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>) -> bool {
    let h = logical_head(next);
    let l = logical_lower(next, frontier);
    &&& 1 <= s.len() <= 65536
    &&& 1 <= h <= u64::MAX as int + 1
    &&& frontier.is_some() ==> 1 <= l < h
    &&& unique_roster(s)
    &&& occupied(s, s.len() as int) == live
    &&& live < s.len()
    &&& live as int == h - l + if staged.is_some() { 1int } else { 0int }
    &&& match staged {
        None => true,
        Some(id) => exact(s, id) && id.logical_epoch == h && next == Some(id.logical_epoch),
    }
    &&& forall|i: int| 0 <= i < s.len() && s[i].entry.is_some() && !is_staged(s[i], staged)
        ==> l <= s[i].entry.unwrap().identity.logical_epoch < h
    &&& forall|epoch: u64| l <= epoch < h ==> #[trigger] has_epoch(s, epoch, staged)
}

proof fn occupied_nonnegative<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int)
    requires 0 <= n <= s.len(),
    ensures occupied(s, n) >= 0,
    decreases n,
{
    if n > 0 { occupied_nonnegative(s, n - 1); }
}

proof fn occupied_positive<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, n: int, index: int)
    requires 0 <= index < n <= s.len(), s[index].entry.is_some(),
    ensures occupied(s, n) > 0,
    decreases n,
{
    if index < n - 1 { occupied_positive(s, n - 1, index); }
    else { occupied_nonnegative(s, n - 1); }
}

proof fn unique_hit<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, epoch: u64, index: usize)
    requires unique_roster(s), index < s.len(), hit(s[index as int], Some(epoch)),
    ensures found_matches(s, s.len() as int, Some(epoch), Some(index)),
{
    assert forall|j: int| 0 <= j < s.len() && hit(s[j], Some(epoch)) implies j == index by {
        if j < index {
            assert(s[j].entry.unwrap().identity.logical_epoch != s[index as int].entry.unwrap().identity.logical_epoch);
        } else if index < j {
            assert(s[index as int].entry.unwrap().identity.logical_epoch != s[j].entry.unwrap().identity.logical_epoch);
        }
    }
}

proof fn chain_frontier<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, live: usize,
    next: Option<u64>, frontier: Option<u64>)
    requires chain(s, live, next, frontier, None),
    ensures frontier_valid(s, live, frontier, None),
{
    if live > 0 {
        let f = frontier.unwrap();
        assert(has_epoch(s, f, None));
        let index = choose|i: int| 0 <= i < s.len() && hit(s[i], Some(f)) && !is_staged(s[i], None);
        unique_hit(s, f, index as usize);
        assert(exists|j: usize| found_matches(s, s.len() as int, frontier, Some(j))) by {
            assert(found_matches(s, s.len() as int, frontier, Some(index as usize)));
        }
        if live > 1 {
            let second = (f + 1) as u64;
            assert(has_epoch(s, second, None));
            let j = choose|j: int| 0 <= j < s.len() && hit(s[j], Some(second)) && !is_staged(s[j], None);
            unique_hit(s, second, j as usize);
            found_count(s, s.len() as int, Some(second), Some(j as usize));
        }
    }
}

proof fn occupancy_same<T>(a: Seq<RuntimeComputePipelineSlotV1<T>>, b: Seq<RuntimeComputePipelineSlotV1<T>>, n: int)
    requires a.len() == b.len(), 0 <= n <= a.len(),
        forall|i: int| 0 <= i < n ==> a[i].entry.is_some() == b[i].entry.is_some(),
    ensures occupied(a, n) == occupied(b, n),
    decreases n,
{
    if n > 0 { occupancy_same(a, b, n - 1); }
}

proof fn chain_intact<T>(s: Seq<RuntimeComputePipelineSlotV1<T>>, live: usize,
    next: Option<u64>, frontier: Option<u64>, id: RuntimeComputePipelineIdentityV1)
    requires chain(s, live, next, frontier, Some(id)),
        s[id.slot as int].entry.unwrap().phase == RuntimeComputePipelinePhaseV1::Publishing,
    ensures intact(s, live, next, frontier, Some(id), id),
{
    if live > 1 {
        let f = frontier.unwrap();
        assert(has_epoch(s, f, Some(id)));
        let i = choose|i: int| 0 <= i < s.len() && hit(s[i], Some(f)) && !is_staged(s[i], Some(id));
        unique_hit(s, f, i as usize);
        found_count(s, s.len() as int, Some(f), Some(i as usize));
    }
}

fn stage_preserves_chain<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize,
    next: Option<u64>, frontier: Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    active: ActiveSubmissionV1<T>) -> (out: Result<RuntimeComputePipelineIdentityV1, ActiveSubmissionV1<T>>)
    requires chain(old(slots)@, *old(live), next, frontier, *old(staged)),
    ensures chain(final(slots)@, *final(live), next, frontier, *final(staged)),
        out.is_ok() == stage_ready(old(slots)@, *old(live), next, frontier, *old(staged), active),
        match out {
            Ok(id) => vacancy_result(old(slots)@, Some((id.slot as usize, id.slot_generation)))
                && id.logical_epoch == next.unwrap() && id.submission == active.id
                && *final(live) == *old(live) + 1 && *final(staged) == Some(id)
                && final(slots)@ == old(slots)@.update(id.slot as int,
                    RuntimeComputePipelineSlotV1 { generation: id.slot_generation,
                        entry: Some(RuntimeComputePipelineEntryV1 {
                            identity: id, phase: RuntimeComputePipelinePhaseV1::Publishing, active,
                        }),
                    }),
            Err(owner) => owner == active && final(slots)@ == old(slots)@
                && *final(live) == *old(live) && *final(staged) == *old(staged),
        },
        match out { Ok(id) => intact(final(slots)@, *final(live), next, frontier, *final(staged), id), Err(_) => true },
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
            assert forall|epoch: u64| logical_lower(next, frontier) <= epoch < logical_head(next)
                implies has_epoch(slots@, epoch, *staged) by {
                assert(has_epoch(before, epoch, None));
                let i = choose|i: int| 0 <= i < before.len() && hit(before[i], Some(epoch)) && !is_staged(before[i], None);
                assert(i != index);
                assert(hit(slots@[i], Some(epoch)) && !is_staged(slots@[i], *staged));
            }
            chain_intact(slots@, *live, next, frontier, id);
        }
    }
    result
}

fn confirm_preserves_chain<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: usize,
    next: &mut Option<u64>, frontier: &mut Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1) -> (out: Result<(), ()>)
    requires chain(old(slots)@, live, *old(next), *old(frontier), *old(staged)),
    ensures chain(final(slots)@, live, *final(next), *final(frontier), *final(staged)),
        out.is_ok() == intact(old(slots)@, live, *old(next), *old(frontier), *old(staged), identity),
        match out {
            Err(()) => final(slots)@ == old(slots)@ && *final(next) == *old(next)
                && *final(frontier) == *old(frontier) && *final(staged) == *old(staged),
            Ok(()) => final(slots)@ == old(slots)@.update(identity.slot as int, published(old(slots)@[identity.slot as int]))
                && *final(next) == successor(Some(identity.logical_epoch)) && (*final(staged)).is_none()
                && *final(frontier) == (if (*old(frontier)).is_none() { Some(identity.logical_epoch) } else { *old(frontier) }),
        },
{
    let ghost before = slots@;
    let ghost previous_next = *next;
    let ghost previous_frontier = *frontier;
    let result = confirm(slots, live, next, frontier, staged, identity);
    proof {
        if result.is_ok() {
            let index = identity.slot as int;
            occupancy_update(before, before.len() as int, index, slots@[index]);
            assert forall|epoch: u64| logical_lower(*next, *frontier) <= epoch < logical_head(*next)
                implies has_epoch(slots@, epoch, *staged) by {
                if epoch == identity.logical_epoch {
                    assert(hit(slots@[index], Some(epoch)) && !is_staged(slots@[index], *staged));
                } else {
                    assert(has_epoch(before, epoch, Some(identity)));
                    let i = choose|i: int| 0 <= i < before.len() && hit(before[i], Some(epoch)) && !is_staged(before[i], Some(identity));
                    assert(i != index);
                    assert(hit(slots@[i], Some(epoch)) && !is_staged(slots@[i], *staged));
                }
            }
        }
    }
    result
}

fn withdraw_preserves_chain<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize,
    next: Option<u64>, frontier: Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1) -> (out: Option<ActiveSubmissionV1<T>>)
    requires chain(old(slots)@, *old(live), next, frontier, *old(staged)),
    ensures chain(final(slots)@, *final(live), next, frontier, *final(staged)),
        out.is_some() == intact(old(slots)@, *old(live), next, frontier, *old(staged), identity),
        match out {
            None => final(slots)@ == old(slots)@ && *final(live) == *old(live) && *final(staged) == *old(staged),
            Some(owner) => owner == old(slots)@[identity.slot as int].entry.unwrap().active
                && *final(live) == *old(live) - 1 && (*final(staged)).is_none()
                && final(slots)@ == old(slots)@.update(identity.slot as int, emptied(old(slots)@[identity.slot as int])),
        },
{
    let ghost before = slots@;
    let result = withdraw(slots, live, next, frontier, staged, identity);
    proof {
        if result.is_some() {
            let index = identity.slot as int;
            occupancy_update(before, before.len() as int, index, slots@[index]);
            assert forall|epoch: u64| logical_lower(next, frontier) <= epoch < logical_head(next)
                implies has_epoch(slots@, epoch, *staged) by {
                assert(has_epoch(before, epoch, Some(identity)));
                let i = choose|i: int| 0 <= i < before.len() && hit(before[i], Some(epoch)) && !is_staged(before[i], Some(identity));
                assert(i != index);
                assert(hit(slots@[i], Some(epoch)) && !is_staged(slots@[i], *staged));
            }
        }
    }
    result
}

fn quarantine_preserves_chain<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: usize,
    next: Option<u64>, frontier: Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>)
    requires chain(old(slots)@, live, next, frontier, staged),
    ensures chain(final(slots)@, live, next, frontier, staged),
        final(slots).len() == old(slots).len(),
        forall|i: int| 0 <= i < old(slots).len() ==> final(slots)@[i] == quarantined(old(slots)@[i]),
{
    let ghost before = slots@;
    quarantine(slots);
    proof {
        occupancy_same(before, slots@, before.len() as int);
        assert forall|epoch: u64| logical_lower(next, frontier) <= epoch < logical_head(next)
            implies has_epoch(slots@, epoch, staged) by {
            assert(has_epoch(before, epoch, staged));
            let i = choose|i: int| 0 <= i < before.len() && hit(before[i], Some(epoch)) && !is_staged(before[i], staged);
            assert(hit(slots@[i], Some(epoch)) && !is_staged(slots@[i], staged));
        }
    }
}

fn promotion_preserves_chain<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize,
    next: Option<u64>, frontier: &mut Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>)
    -> (out: Option<(RuntimeComputePipelinePhaseV1, ActiveSubmissionV1<T>)>)
    requires chain(old(slots)@, *old(live), next, *old(frontier), staged),
    ensures chain(final(slots)@, *final(live), next, *final(frontier), staged),
        out.is_some() == (staged.is_none() && *old(live) > 0),
        out.is_some() ==> exists|i: usize| promotion_frame(old(slots)@, final(slots)@,
            (*old(frontier)).unwrap(), out.unwrap().0, out.unwrap().1, i),
{
    let ghost before = slots@;
    let ghost previous_frontier = *frontier;
    let ghost previous_live = *live;
    proof {
        if staged.is_none() && *live > 0 {
            assert(has_epoch(before, (*frontier).unwrap(), None));
        }
    }
    let result = take_frontier(slots, live, frontier, staged);
    proof {
        if result.is_some() {
            let epoch = previous_frontier.unwrap();
            let index = choose|i: usize| promotion_frame(before, slots@, epoch, result.unwrap().0, result.unwrap().1, i);
            occupancy_update(before, before.len() as int, index as int, emptied(before[index as int]));
            assert forall|i: int| 0 <= i < slots.len() && slots@[i].entry.is_some()
                && !is_staged(slots@[i], staged) implies logical_lower(next, *frontier)
                    <= slots@[i].entry.unwrap().identity.logical_epoch < logical_head(next) by {
                assert(i != index);
                if i < index {
                    assert(before[i].entry.unwrap().identity.logical_epoch != before[index as int].entry.unwrap().identity.logical_epoch);
                } else {
                    assert(before[index as int].entry.unwrap().identity.logical_epoch != before[i].entry.unwrap().identity.logical_epoch);
                }
            }
            assert forall|e: u64| logical_lower(next, *frontier) <= e < logical_head(next)
                implies has_epoch(slots@, e, staged) by {
                assert(has_epoch(before, e, staged));
                let i = choose|i: int| 0 <= i < before.len() && hit(before[i], Some(e)) && !is_staged(before[i], staged);
                assert(e != epoch && i != index);
                assert(hit(slots@[i], Some(e)) && !is_staged(slots@[i], staged));
            }
        }
    }
    result
}

fn quarantine_staged_refuses_settlement<T>(slots: &mut [RuntimeComputePipelineSlotV1<T>], live: &mut usize,
    next: &mut Option<u64>, frontier: &mut Option<u64>, staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1)
    requires chain(old(slots)@, *old(live), *old(next), *old(frontier), *old(staged)),
        *old(staged) == Some(identity),
    ensures chain(final(slots)@, *final(live), *final(next), *final(frontier), *final(staged)),
        *final(live) == *old(live), *final(next) == *old(next),
        *final(frontier) == *old(frontier), *final(staged) == *old(staged),
        forall|i: int| 0 <= i < old(slots).len() ==> final(slots)@[i] == quarantined(old(slots)@[i]),
{
    quarantine_preserves_chain(slots, *live, *next, *frontier, *staged);
    let ghost first = slots@;
    quarantine(slots);
    assert(slots@ =~= first);
    let confirmation = confirm(slots, *live, next, frontier, staged, identity);
    assert(confirmation.is_err());
    let withdrawal = withdraw(slots, live, *next, *frontier, staged, identity);
    assert(withdrawal.is_none());
    let promotion = take_frontier(slots, live, frontier, *staged);
    assert(promotion.is_none());
}

// Concrete nonempty MAX boundary, retry, staged refusal and opaque-owner
// witnesses. No Copy/Clone or native-receipt assumptions are imposed on T.
#[verifier::spinoff_prover]
fn exhausted_chain_witness<T>(first_payload: T, second_payload: T) {
    let mut slots = [RuntimeComputePipelineSlotV1 { generation: 0, entry: None },
        RuntimeComputePipelineSlotV1 { generation: 0, entry: None },
        RuntimeComputePipelineSlotV1 { generation: 0, entry: None }];
    let mut live = 0usize;
    let mut staged = None;
    let mut next = Some(u64::MAX - 1);
    let mut frontier = None;
    let first = ActiveSubmissionV1 { id: 7, payload: first_payload };
    let second = ActiveSubmissionV1 { id: 8, payload: second_payload };
    let ghost first_owner = first;
    let ghost second_owner = second;
    proof {
        reveal_with_fuel(occupied, 4);
        assert(chain(slots@, live, next, frontier, staged));
        assert(eligible(slots@[0]));
        assert(stage_ready(slots@, live, next, frontier, staged, first));
    }
    let Ok(a) = stage_preserves_chain(&mut slots, &mut live, next, frontier, &mut staged, first)
        else { assert(false); return; };
    assert(a.slot == 0 && a.logical_epoch == u64::MAX - 1);
    confirm_preserves_chain(&mut slots, live, &mut next, &mut frontier, &mut staged, a).unwrap();
    proof {
        chain_frontier(slots@, live, next, frontier);
        assert(eligible(slots@[1]));
        assert(stage_ready(slots@, live, next, frontier, staged, second));
    }
    let Ok(b) = stage_preserves_chain(&mut slots, &mut live, next, frontier, &mut staged, second)
        else { assert(false); return; };
    assert(b.slot == 1 && b.logical_epoch == u64::MAX && b.slot_generation == 1);
    let blocked = take_frontier(&mut slots, &mut live, &mut frontier, staged);
    assert(blocked.is_none());
    let returned = withdraw_preserves_chain(&mut slots, &mut live, next, frontier, &mut staged, b).unwrap();
    assert(returned == second_owner);
    proof {
        chain_frontier(slots@, live, next, frontier);
        assert(eligible(slots@[1]));
        assert(stage_ready(slots@, live, next, frontier, staged, returned));
    }
    let Ok(c) = stage_preserves_chain(&mut slots, &mut live, next, frontier, &mut staged, returned)
        else { assert(false); return; };
    assert(c.logical_epoch == b.logical_epoch && c.slot == b.slot && c.slot_generation == 2);
    confirm_preserves_chain(&mut slots, live, &mut next, &mut frontier, &mut staged, c).unwrap();
    assert(next.is_none() && live == 2);
    quarantine_preserves_chain(&mut slots, live, next, frontier, staged);
    let (phase, owner) = promotion_preserves_chain(&mut slots, &mut live, next, &mut frontier, staged).unwrap();
    assert(phase == RuntimeComputePipelinePhaseV1::Quarantined && owner == first_owner);
    let (phase, owner) = promotion_preserves_chain(&mut slots, &mut live, next, &mut frontier, staged).unwrap();
    assert(phase == RuntimeComputePipelinePhaseV1::Quarantined && owner == second_owner);
    assert(chain(slots@, live, next, frontier, staged));
    assert(live == 0 && next.is_none() && frontier.is_none());
    let exhausted = promotion_preserves_chain(&mut slots, &mut live, next, &mut frontier, staged);
    assert(exhausted.is_none());
    let rejected = stage_preserves_chain(&mut slots, &mut live, next, frontier, &mut staged, owner);
    match rejected {
        Err(returned) => assert(returned == second_owner),
        Ok(_) => assert(false),
    }
}

}
