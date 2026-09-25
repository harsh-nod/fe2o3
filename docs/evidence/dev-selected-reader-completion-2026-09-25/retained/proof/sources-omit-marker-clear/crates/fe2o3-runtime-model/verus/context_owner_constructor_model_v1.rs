// Independent constructor contents and reservation observations, not physical allocation.
use super::*;

verus! {

#[derive(Clone, Copy)]
pub enum ConstructorErrorV1 {
    InvalidContextGeneration,
    InvalidCapacity,
    StorageAllocationFailed,
}

pub struct ConstructorObservationsV1 {
    pub outcomes: Vec<bool>,
    pub attempts: Vec<(u8, usize)>,
}

pub open spec fn constructor_outcome_v1(outcomes: Seq<bool>, index: nat) -> bool {
    index < outcomes.len() && outcomes[index as int]
}

pub open spec fn constructor_capacity_v1(site: u8, allocations: usize, writers: usize, reads: usize) -> usize {
    if site == 0 || site == 1 { writers }
    else if site == 7 || site == 8 || site == 10 || site == 11 { reads }
    else { allocations }
}

pub open spec fn constructor_requests_v1(allocations: usize, writers: usize, reads: usize) -> Seq<(u8, usize)> {
    Seq::new(13, |i: int| (i as u8, constructor_capacity_v1(i as u8, allocations, writers, reads)))
}

pub open spec fn constructor_observations_same_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1) -> bool {
    before.outcomes@ == after.outcomes@ && before.attempts@ == after.attempts@
}

pub open spec fn constructor_trace_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    requests: Seq<(u8, usize)>, success: bool) -> bool
{
    let count = (after.attempts@.len() - before.attempts@.len()) as nat;
    &&& before.outcomes@ == after.outcomes@
    &&& before.attempts@.len() <= after.attempts@.len()
    &&& count <= requests.len()
    &&& after.attempts@ == before.attempts@ + requests.take(count as int)
    &&& forall|i: int| 0 <= i < count && (i + 1 < count || success)
        ==> #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat)
    &&& if success { count == requests.len() }
        else { count > 0 && !constructor_outcome_v1(before.outcomes@, (after.attempts@.len() - 1) as nat) }
}

pub open spec fn constructor_journal_admission_v1(context: u64, allocations: usize, writers: usize)
    -> Option<ConstructorErrorV1>
{
    if context == 0 || context == u64::MAX { Some(ConstructorErrorV1::InvalidContextGeneration) }
    else if allocations == 0 || allocations > 1_048_576 || writers == 0 || writers > 1_048_576 {
        Some(ConstructorErrorV1::InvalidCapacity)
    } else { None }
}

pub open spec fn constructor_owner_admission_v1(context: u64, allocations: usize, writers: usize, reads: usize)
    -> Option<ConstructorErrorV1>
{
    if reads == 0 || reads > 1_048_576 { Some(ConstructorErrorV1::InvalidCapacity) }
    else { constructor_journal_admission_v1(context, allocations, writers) }
}

pub open spec fn constructor_journal_relation_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, result: Result<JournalContentsV1, ConstructorErrorV1>) -> bool
{
    match constructor_journal_admission_v1(context, allocations, writers) {
        Some(error) => result == Err(error) && constructor_observations_same_v1(before, after),
        None => {
            &&& constructor_trace_v1(before, after, constructor_requests_v1(allocations, writers, 0).take(7), result.is_ok())
            &&& match result {
                Err(error) => error == ConstructorErrorV1::StorageAllocationFailed,
                Ok(contents) => constructor_contents_relation_v1(context, allocations, writers, Ok(contents)),
            }
        },
    }
}

pub open spec fn constructor_stable_relation_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize,
    result: Result<ReadContentsV1, ConstructorErrorV1>) -> bool
{
    match constructor_owner_admission_v1(context, allocations, writers, reads) {
        Some(error) => result == Err(error) && constructor_observations_same_v1(before, after),
        None => {
            &&& constructor_trace_v1(before, after, constructor_requests_v1(allocations, writers, reads).take(10), result.is_ok())
            &&& match result {
                Err(error) => error == ConstructorErrorV1::StorageAllocationFailed,
                Ok(contents) => reader_constructor_relation_v1(context, allocations, writers, reads, Ok(contents)),
            }
        },
    }
}

pub open spec fn constructor_producer_relation_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize,
    result: Result<ProducerReadContentsV1, ConstructorErrorV1>) -> bool
{
    match constructor_owner_admission_v1(context, allocations, writers, reads) {
        Some(error) => result == Err(error) && constructor_observations_same_v1(before, after),
        None => {
            &&& constructor_trace_v1(before, after, constructor_requests_v1(allocations, writers, reads), result.is_ok())
            &&& match result {
                Err(error) => error == ConstructorErrorV1::StorageAllocationFailed,
                Ok(contents) => producer_constructor_relation_v1(context, allocations, writers, reads, Ok(contents)),
            }
        },
    }
}

pub proof fn constructor_trace_compose_v1(before: ConstructorObservationsV1, middle: ConstructorObservationsV1,
    after: ConstructorObservationsV1, first: Seq<(u8, usize)>, second: Seq<(u8, usize)>, success: bool)
    requires constructor_trace_v1(before, middle, first, true), constructor_trace_v1(middle, after, second, success),
    ensures constructor_trace_v1(before, after, first + second, success),
{
    let count = (after.attempts@.len() - middle.attempts@.len()) as int;
    assert(middle.attempts@.len() == before.attempts@.len() + first.len());
    assert(0 <= count <= second.len());
    assert(after.attempts@.len() - before.attempts@.len() == first.len() + count);
    assert(first.take(first.len() as int) =~= first);
    assert((first + second).take(first.len() as int + count) =~= first + second.take(count));
    assert((before.attempts@ + first) + second.take(count) =~= before.attempts@ + (first + second.take(count)));
    assert forall|i: int| 0 <= i < first.len() + count && (i + 1 < first.len() + count || success)
        implies #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat) by {
        if i < first.len() {
            assert(constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat));
        } else {
            let j = i - first.len();
            assert(0 <= j < count && (j + 1 < count || success));
            assert(constructor_outcome_v1(middle.outcomes@, (middle.attempts@.len() + j) as nat));
        }
    }
}

pub proof fn constructor_trace_extend_failed_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    prefix: Seq<(u8, usize)>, whole: Seq<(u8, usize)>)
    requires constructor_trace_v1(before, after, prefix, false), prefix.len() <= whole.len(),
        prefix == whole.take(prefix.len() as int),
    ensures constructor_trace_v1(before, after, whole, false),
{
    let count = (after.attempts@.len() - before.attempts@.len()) as int;
    assert(prefix.take(count) =~= whole.take(count));
}

pub fn constructor_reserve_model_exec_v1(observations: &mut ConstructorObservationsV1, site: u8, capacity: usize)
    -> (result: bool)
    requires old(observations).attempts@.len() < usize::MAX,
    ensures final(observations).outcomes@ == old(observations).outcomes@,
        final(observations).attempts@ == old(observations).attempts@.push((site, capacity)),
        result == constructor_outcome_v1(old(observations).outcomes@, old(observations).attempts@.len()),
{
    let index = observations.attempts.len();
    let result = if index < observations.outcomes.len() { observations.outcomes[index] } else { false };
    observations.attempts.push((site, capacity));
    result
}

// Grouping precedes logical initialization; partial physical objects and their drops are not modeled.
pub fn constructor_reserve_range_model_exec_v1(observations: &mut ConstructorObservationsV1,
    start: u8, end: u8, allocations: usize, writers: usize, reads: usize) -> (result: bool)
    requires start <= end <= 13, old(observations).attempts@.len() + (end - start) <= usize::MAX,
    ensures constructor_trace_v1(*old(observations), *final(observations),
        constructor_requests_v1(allocations, writers, reads).subrange(start as int, end as int), result),
{
    let ghost before = *observations;
    let ghost requests = constructor_requests_v1(allocations, writers, reads).subrange(start as int, end as int);
    let mut site = start;
    while site < end
        invariant start <= site <= end <= 13,
            before == *old(observations),
            requests == constructor_requests_v1(allocations, writers, reads).subrange(start as int, end as int),
            requests.len() == end - start,
            before.attempts@.len() + (end - start) <= usize::MAX,
            observations.outcomes@ == before.outcomes@,
            observations.attempts@ == before.attempts@ + requests.take((site - start) as int),
            observations.attempts@.len() == before.attempts@.len() + (site - start),
            forall|i: int| 0 <= i < site - start
                ==> #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat),
        decreases end - site,
    {
        let capacity = if site == 0 || site == 1 { writers }
            else if site == 7 || site == 8 || site == 10 || site == 11 { reads }
            else { allocations };
        let success = constructor_reserve_model_exec_v1(observations, site, capacity);
        proof {
            assert(requests[(site - start) as int] == (site, capacity));
            assert(requests.take((site - start + 1) as int)
                =~= requests.take((site - start) as int).push((site, capacity)));
            assert(observations.attempts@ =~= before.attempts@ + requests.take((site - start + 1) as int));
        }
        if !success {
            proof {
                assert(observations.attempts@.len() == before.attempts@.len() + (site - start) + 1);
                assert(!constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + site - start) as nat));
                assert forall|i: int| 0 <= i < site - start + 1 && i + 1 < site - start + 1
                    implies #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat) by {
                    assert(i < site - start);
                }
                assert(constructor_trace_v1(before, *observations, requests, false));
            }
            return false;
        }
        site += 1;
    }
    true
}

pub fn constructor_journal_model_exec_v1(observations: &mut ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize) -> (result: Result<JournalContentsV1, ConstructorErrorV1>)
    requires old(observations).attempts@.len() + 7 <= usize::MAX,
    ensures constructor_journal_relation_v1(*old(observations), *final(observations), context, allocations, writers, result),
{
    if context == 0 || context == u64::MAX { return Err(ConstructorErrorV1::InvalidContextGeneration); }
    if allocations == 0 || allocations > 1_048_576 || writers == 0 || writers > 1_048_576 {
        return Err(ConstructorErrorV1::InvalidCapacity);
    }
    if !constructor_reserve_range_model_exec_v1(observations, 0, 7, allocations, writers, 0) {
        return Err(ConstructorErrorV1::StorageAllocationFailed);
    }
    let writers_slots = vacant_contents_exec_v1(writers);
    let free = free_contents_exec_v1(writers);
    let allocations_slots = vacant_contents_exec_v1(allocations);
    let allocation_free = free_contents_exec_v1(allocations);
    let members = vacant_contents_exec_v1(allocations);
    let member_free = free_contents_exec_v1(allocations);
    let scratch = vacant_contents_exec_v1(allocations);
    Ok(JournalContentsV1 {
        context_generation: context, allocation_capacity: allocations, writer_capacity: writers,
        registration_watermark: 0, reserved_count: 0, writers: writers_slots, free,
        allocations: allocations_slots, allocation_free, members, member_free, scratch,
    })
}

pub fn constructor_stable_model_exec_v1(observations: &mut ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize) -> (result: Result<ReadContentsV1, ConstructorErrorV1>)
    requires old(observations).attempts@.len() + 10 <= usize::MAX,
    ensures constructor_stable_relation_v1(*old(observations), *final(observations), context, allocations, writers, reads, result),
{
    let ghost before = *observations;
    let ghost requests = constructor_requests_v1(allocations, writers, reads);
    if reads == 0 || reads > 1_048_576 { return Err(ConstructorErrorV1::InvalidCapacity); }
    let journal = match constructor_journal_model_exec_v1(observations, context, allocations, writers) {
        Ok(value) => value,
        Err(error) => {
            proof {
                assert(constructor_requests_v1(allocations, writers, 0).take(7) =~= requests.take(7));
                if constructor_journal_admission_v1(context, allocations, writers).is_none() {
                    assert(requests.take(10).take(7) =~= requests.take(7));
                    constructor_trace_extend_failed_v1(before, *observations, requests.take(7), requests.take(10));
                }
            }
            return Err(error);
        },
    };
    let ghost middle = *observations;
    let success = constructor_reserve_range_model_exec_v1(observations, 7, 10, allocations, writers, reads);
    proof {
        assert(constructor_requests_v1(allocations, writers, 0).take(7) =~= requests.take(7));
        constructor_trace_compose_v1(before, middle, *observations, requests.take(7), requests.subrange(7, 10), success);
        assert(requests.take(7) + requests.subrange(7, 10) =~= requests.take(10));
    }
    if !success { return Err(ConstructorErrorV1::StorageAllocationFailed); }
    let leases = vacant_contents_exec_v1(reads);
    let free_reads = free_contents_exec_v1(reads);
    let readers = zero_readers_exec_v1(allocations);
    Ok(ReadContentsV1 { journal, leases, free_reads, readers, next_incarnation: 1 })
}

pub fn constructor_producer_model_exec_v1(observations: &mut ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize) -> (result: Result<ProducerReadContentsV1, ConstructorErrorV1>)
    requires old(observations).attempts@.len() + 13 <= usize::MAX,
    ensures constructor_producer_relation_v1(*old(observations), *final(observations), context, allocations, writers, reads, result),
        match result { Ok(contents) => producer_invariant_v1(contents), Err(_) => true },
{
    let ghost before = *observations;
    let ghost requests = constructor_requests_v1(allocations, writers, reads);
    let stable = match constructor_stable_model_exec_v1(observations, context, allocations, writers, reads) {
        Ok(value) => value,
        Err(error) => {
            proof {
                if constructor_owner_admission_v1(context, allocations, writers, reads).is_none() {
                    constructor_trace_extend_failed_v1(before, *observations, requests.take(10), requests);
                }
            }
            return Err(error);
        },
    };
    let ghost middle = *observations;
    let success = constructor_reserve_range_model_exec_v1(observations, 10, 13, allocations, writers, reads);
    proof {
        constructor_trace_compose_v1(before, middle, *observations, requests.take(10), requests.subrange(10, 13), success);
        assert(requests.take(10) + requests.subrange(10, 13) =~= requests);
    }
    if !success { return Err(ConstructorErrorV1::StorageAllocationFailed); }
    let reservations = vacant_contents_exec_v1(reads);
    let free = free_contents_exec_v1(reads);
    let counts = zero_readers_exec_v1(allocations);
    let contents = ProducerReadContentsV1 { stable, reservations, free, counts, next_incarnation: 1 };
    proof { producer_constructor_invariant_v1(context, allocations, writers, reads, contents); }
    Ok(contents)
}

pub proof fn constructor_producer_issued_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize,
    contents: ProducerReadContentsV1, storage: StorageCapacitiesV1)
    requires constructor_producer_relation_v1(before, after, context, allocations, writers, reads, Ok(contents)),
    ensures producer_invariant_v1(contents),
        storage_admission_v1(allocations, writers, storage) ==> issued_producer_v1(contents, storage, Seq::empty()),
{
    producer_constructor_invariant_v1(context, allocations, writers, reads, contents);
    if storage_admission_v1(allocations, writers, storage) {
        issued_constructor_v1(contents.stable.journal, context, allocations, writers, storage);
    }
}

}
