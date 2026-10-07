verus! {

pub const CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1: usize = 1_048_576;

pub struct ConstructorObservationsV1 {
    pub outcomes: Vec<bool>,
    pub attempts: Vec<(u8, usize)>,
}

spec fn constructor_outcome_v1(outcomes: Seq<bool>, index: nat) -> bool {
    index < outcomes.len() && outcomes[index as int]
}

spec fn constructor_capacity_v1(site: u8, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize) -> usize {
    if site == 0 || site == 1 { writer_capacity }
    else if site == 7 || site == 8 || site == 10 || site == 11 { read_capacity }
    else { allocation_capacity }
}

spec fn constructor_requests_v1(allocation_capacity: usize, writer_capacity: usize, read_capacity: usize) -> Seq<(u8, usize)> {
    Seq::new(13, |i: int| (i as u8, constructor_capacity_v1(i as u8, allocation_capacity, writer_capacity, read_capacity)))
}

spec fn constructor_observations_same_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1) -> bool {
    before.outcomes@ == after.outcomes@ && before.attempts@ == after.attempts@
}

spec fn constructor_trace_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
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

spec fn constructor_journal_admission_v1(context: u64, allocation_capacity: usize, writer_capacity: usize)
    -> Option<ContextVersionJournalErrorV1>
{
    if context == 0 || context == u64::MAX { Some(ContextVersionJournalErrorV1::InvalidContextGeneration) }
    else if allocation_capacity == 0 || allocation_capacity > 1_048_576
        || writer_capacity == 0 || writer_capacity > 1_048_576 { Some(ContextVersionJournalErrorV1::InvalidCapacity) }
    else { None }
}

spec fn constructor_owner_admission_v1(context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize)
    -> Option<ContextVersionJournalErrorV1>
{
    if read_capacity == 0 || read_capacity > 1_048_576 { Some(ContextVersionJournalErrorV1::InvalidCapacity) }
    else { constructor_journal_admission_v1(context, allocation_capacity, writer_capacity) }
}

spec fn constructor_reverse_free_v1(capacity: usize) -> Seq<usize> {
    Seq::new(capacity as nat, |i: int| (capacity as int - 1 - i) as usize)
}

spec fn constructor_journal_initialized_v1(owner: ContextVersionJournalV1,
    context: u64, allocation_capacity: usize, writer_capacity: usize) -> bool
{
    &&& owner.context_generation == context
    &&& owner.allocation_capacity == allocation_capacity
    &&& owner.writer_capacity == writer_capacity
    &&& owner.registration_watermark == 0
    &&& owner.reserved_count == 0
    &&& owner.writers@ == Seq::new(writer_capacity as nat, |i: int| None)
    &&& owner.free@ == constructor_reverse_free_v1(writer_capacity)
    &&& owner.allocations@ == Seq::new(allocation_capacity as nat, |i: int| None)
    &&& owner.allocation_free@ == constructor_reverse_free_v1(allocation_capacity)
    &&& owner.members@ == Seq::new(allocation_capacity as nat, |i: int| None)
    &&& owner.member_free@ == constructor_reverse_free_v1(allocation_capacity)
    &&& owner.scratch@ == Seq::new(allocation_capacity as nat, |i: int| None)
}

spec fn constructor_stable_initialized_v1(owner: ContextReadLeasedJournalV1,
    context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize) -> bool
{
    &&& constructor_journal_initialized_v1(owner.journal, context, allocation_capacity, writer_capacity)
    &&& owner.leases@ == Seq::new(read_capacity as nat, |i: int| None)
    &&& owner.free_reads@ == constructor_reverse_free_v1(read_capacity)
    &&& owner.readers@ == Seq::new(allocation_capacity as nat, |i: int| 0usize)
    &&& owner.next_incarnation == 1
}

spec fn constructor_producer_initialized_v1(owner: ContextProducerReadJournalV1,
    context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize) -> bool
{
    &&& constructor_stable_initialized_v1(owner.stable, context, allocation_capacity, writer_capacity, read_capacity)
    &&& owner.reservations@ == Seq::new(read_capacity as nat, |i: int| None)
    &&& owner.free@ == constructor_reverse_free_v1(read_capacity)
    &&& owner.counts@ == Seq::new(allocation_capacity as nat, |i: int| 0usize)
    &&& owner.next_incarnation == 1
}

spec fn constructor_journal_relation_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocation_capacity: usize, writer_capacity: usize,
    result: Result<ContextVersionJournalV1, ContextVersionJournalErrorV1>) -> bool
{
    match constructor_journal_admission_v1(context, allocation_capacity, writer_capacity) {
        Some(error) => result == Err(error) && constructor_observations_same_v1(before, after),
        None => {
            &&& constructor_trace_v1(before, after, constructor_requests_v1(allocation_capacity, writer_capacity, 0).take(7), result.is_ok())
            &&& match result {
                Err(error) => error == ContextVersionJournalErrorV1::StorageAllocationFailed,
                Ok(owner) => constructor_journal_initialized_v1(owner, context, allocation_capacity, writer_capacity),
            }
        },
    }
}

spec fn constructor_stable_relation_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize,
    result: Result<ContextReadLeasedJournalV1, ContextVersionJournalErrorV1>) -> bool
{
    match constructor_owner_admission_v1(context, allocation_capacity, writer_capacity, read_capacity) {
        Some(error) => result == Err(error) && constructor_observations_same_v1(before, after),
        None => {
            &&& constructor_trace_v1(before, after, constructor_requests_v1(allocation_capacity, writer_capacity, read_capacity).take(10), result.is_ok())
            &&& match result {
                Err(error) => error == ContextVersionJournalErrorV1::StorageAllocationFailed,
                Ok(owner) => constructor_stable_initialized_v1(owner, context, allocation_capacity, writer_capacity, read_capacity),
            }
        },
    }
}

spec fn constructor_producer_relation_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize,
    result: Result<ContextProducerReadJournalV1, ContextVersionJournalErrorV1>) -> bool
{
    match constructor_owner_admission_v1(context, allocation_capacity, writer_capacity, read_capacity) {
        Some(error) => result == Err(error) && constructor_observations_same_v1(before, after),
        None => {
            &&& constructor_trace_v1(before, after, constructor_requests_v1(allocation_capacity, writer_capacity, read_capacity), result.is_ok())
            &&& match result {
                Err(error) => error == ContextVersionJournalErrorV1::StorageAllocationFailed,
                Ok(owner) => constructor_producer_initialized_v1(owner, context, allocation_capacity, writer_capacity, read_capacity),
            }
        },
    }
}

impl ConstructorObservationsV1 {
    fn reserve<T>(&mut self, site: u8, capacity: usize, storage: &mut Vec<T>) -> (result: bool)
        requires old(self).attempts@.len() < usize::MAX,
        ensures final(self).outcomes@ == old(self).outcomes@,
            final(self).attempts@ == old(self).attempts@.push((site, capacity)),
            final(storage)@ == old(storage)@,
            result == constructor_outcome_v1(old(self).outcomes@, old(self).attempts@.len()),
    {
        let index = self.attempts.len();
        let result = if index < self.outcomes.len() { self.outcomes[index] } else { false };
        self.attempts.push((site, capacity));
        result
    }
}

fn constructor_vacant_slots_v1<T>(capacity: usize, site: u8, allocator: &mut ConstructorObservationsV1)
    -> (result: Result<Vec<Option<T>>, ContextVersionJournalErrorV1>)
    requires old(allocator).attempts@.len() < usize::MAX,
    ensures final(allocator).outcomes@ == old(allocator).outcomes@,
        final(allocator).attempts@ == old(allocator).attempts@.push((site, capacity)),
        result.is_ok() == constructor_outcome_v1(old(allocator).outcomes@, old(allocator).attempts@.len()),
        match result {
            Ok(slots) => slots@ == Seq::new(capacity as nat, |i: int| None::<T>),
            Err(error) => error == ContextVersionJournalErrorV1::StorageAllocationFailed,
        },
{
    constructor_vacant_body!(verus_exec_expr, T, capacity, site, allocator, slots,
        [let ghost before = *allocator;],
        [invariant before == *old(allocator), slots@.len() <= capacity,
            forall|i: int| 0 <= i < slots@.len() ==> #[trigger] slots@[i] == None::<T>,
            allocator.outcomes@ == before.outcomes@,
            allocator.attempts@ == before.attempts@.push((site, capacity)),
            constructor_outcome_v1(before.outcomes@, before.attempts@.len()),
         decreases capacity - slots@.len(),],
        [proof { assert(slots@ =~= Seq::new(capacity as nat, |i: int| None::<T>)); }])
}

fn constructor_free_slots_v1(capacity: usize, site: u8, allocator: &mut ConstructorObservationsV1)
    -> (result: Result<Vec<usize>, ContextVersionJournalErrorV1>)
    requires old(allocator).attempts@.len() < usize::MAX,
    ensures final(allocator).outcomes@ == old(allocator).outcomes@,
        final(allocator).attempts@ == old(allocator).attempts@.push((site, capacity)),
        result.is_ok() == constructor_outcome_v1(old(allocator).outcomes@, old(allocator).attempts@.len()),
        match result {
            Ok(free) => free@ == constructor_reverse_free_v1(capacity),
            Err(error) => error == ContextVersionJournalErrorV1::StorageAllocationFailed,
        },
{
    constructor_free_body!(verus_exec_expr, capacity, site, allocator, free, next,
        [let ghost before = *allocator;],
        [invariant before == *old(allocator), next <= capacity, free@.len() + next as nat == capacity as nat,
            forall|i: int| 0 <= i < free@.len() ==> #[trigger] free@[i] == (capacity as int - 1 - i) as usize,
            allocator.outcomes@ == before.outcomes@,
            allocator.attempts@ == before.attempts@.push((site, capacity)),
            constructor_outcome_v1(before.outcomes@, before.attempts@.len()),
         decreases next,],
        [proof { assert(free@ =~= constructor_reverse_free_v1(capacity)); }])
}

fn constructor_zero_slots_v1(capacity: usize, site: u8, allocator: &mut ConstructorObservationsV1)
    -> (result: Result<Vec<usize>, ContextVersionJournalErrorV1>)
    requires old(allocator).attempts@.len() < usize::MAX,
    ensures final(allocator).outcomes@ == old(allocator).outcomes@,
        final(allocator).attempts@ == old(allocator).attempts@.push((site, capacity)),
        result.is_ok() == constructor_outcome_v1(old(allocator).outcomes@, old(allocator).attempts@.len()),
        match result {
            Ok(counts) => counts@ == Seq::new(capacity as nat, |i: int| 0usize),
            Err(error) => error == ContextVersionJournalErrorV1::StorageAllocationFailed,
        },
{
    constructor_zero_body!(verus_exec_expr, capacity, site, allocator, counts,
        [let ghost before = *allocator;],
        [invariant before == *old(allocator), counts@.len() <= capacity,
            forall|i: int| 0 <= i < counts@.len() ==> #[trigger] counts@[i] == 0usize,
            allocator.outcomes@ == before.outcomes@,
            allocator.attempts@ == before.attempts@.push((site, capacity)),
            constructor_outcome_v1(before.outcomes@, before.attempts@.len()),
         decreases capacity - counts@.len(),],
        [proof { assert(counts@ =~= Seq::new(capacity as nat, |i: int| 0usize)); }])
}

impl ContextVersionJournalV1 {
    fn new_with_allocator_v1(context: u64, allocation_capacity: usize, writer_capacity: usize,
        allocator: &mut ConstructorObservationsV1) -> (result: Result<Self, ContextVersionJournalErrorV1>)
        requires old(allocator).attempts@.len() + 7 <= usize::MAX,
        ensures constructor_journal_relation_v1(*old(allocator), *final(allocator), context, allocation_capacity, writer_capacity, result),
    {
        constructor_journal_body!(verus_exec_expr, context, allocation_capacity, writer_capacity, allocator,
            constructor_vacant_slots_v1, constructor_free_slots_v1, result,
            [let ghost before = *allocator;
             let ghost requests = constructor_requests_v1(allocation_capacity, writer_capacity, 0).take(7);],
            [proof {
                let count = (allocator.attempts@.len() - before.attempts@.len()) as int;
                assert(allocator.attempts@ =~= before.attempts@ + requests.take(count));
                assert forall|i: int| 0 <= i && i + 1 < count
                    implies #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat) by {}
                assert(constructor_trace_v1(before, *allocator, requests, false));
            }],
            [proof {
                assert(allocator.attempts@ =~= before.attempts@ + requests);
                assert forall|i: int| 0 <= i < 7
                    implies #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat) by {}
                assert(requests.take(requests.len() as int) =~= requests);
                assert(constructor_trace_v1(before, *allocator, requests, true));
            }], [])
    }

    fn new_observed_v1(context: u64, allocation_capacity: usize, writer_capacity: usize,
        allocator: &mut ConstructorObservationsV1) -> (result: Result<Self, ContextVersionJournalErrorV1>)
        requires old(allocator).attempts@.len() + 7 <= usize::MAX,
        ensures constructor_journal_relation_v1(*old(allocator), *final(allocator), context, allocation_capacity, writer_capacity, result),
    {
        constructor_entry_body!(Self::new_with_allocator_v1, allocator, context, allocation_capacity, writer_capacity)
    }
}

impl ContextReadLeasedJournalV1 {
    fn new_with_allocator_v1(context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize,
        allocator: &mut ConstructorObservationsV1) -> (result: Result<Self, ContextVersionJournalErrorV1>)
        requires old(allocator).attempts@.len() + 10 <= usize::MAX,
        ensures constructor_stable_relation_v1(*old(allocator), *final(allocator), context, allocation_capacity, writer_capacity, read_capacity, result),
    {
        constructor_stable_body!(verus_exec_expr, context, allocation_capacity, writer_capacity, read_capacity, allocator,
            ContextVersionJournalV1::new_with_allocator_v1, constructor_vacant_slots_v1, constructor_free_slots_v1,
            constructor_zero_slots_v1, result,
            [let ghost before = *allocator;
             let ghost requests = constructor_requests_v1(allocation_capacity, writer_capacity, read_capacity).take(10);],
            [proof {
                if constructor_owner_admission_v1(context, allocation_capacity, writer_capacity, read_capacity).is_none() {
                    let count = (allocator.attempts@.len() - before.attempts@.len()) as int;
                    assert(allocator.attempts@ =~= before.attempts@ + requests.take(count));
                    assert forall|i: int| 0 <= i && i + 1 < count
                        implies #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat) by {}
                    assert(constructor_trace_v1(before, *allocator, requests, false));
                }
            }],
            [proof {
                assert(allocator.attempts@ =~= before.attempts@ + requests);
                assert forall|i: int| 0 <= i < 10
                    implies #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat) by {}
                assert(requests.take(requests.len() as int) =~= requests);
                assert(constructor_trace_v1(before, *allocator, requests, true));
            }])
    }

    fn new_observed_v1(context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize,
        allocator: &mut ConstructorObservationsV1) -> (result: Result<Self, ContextVersionJournalErrorV1>)
        requires old(allocator).attempts@.len() + 10 <= usize::MAX,
        ensures constructor_stable_relation_v1(*old(allocator), *final(allocator), context, allocation_capacity, writer_capacity, read_capacity, result),
    {
        constructor_entry_body!(Self::new_with_allocator_v1, allocator, context, allocation_capacity, writer_capacity, read_capacity)
    }
}

impl ContextProducerReadJournalV1 {
    fn new_with_allocator_v1(context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize,
        allocator: &mut ConstructorObservationsV1) -> (result: Result<Self, ContextVersionJournalErrorV1>)
        requires old(allocator).attempts@.len() + 13 <= usize::MAX,
        ensures constructor_producer_relation_v1(*old(allocator), *final(allocator), context, allocation_capacity, writer_capacity, read_capacity, result),
    {
        constructor_producer_body!(verus_exec_expr, context, allocation_capacity, writer_capacity, read_capacity, allocator,
            ContextReadLeasedJournalV1::new_with_allocator_v1, constructor_vacant_slots_v1, constructor_free_slots_v1,
            constructor_zero_slots_v1, result,
            [let ghost before = *allocator;
             let ghost requests = constructor_requests_v1(allocation_capacity, writer_capacity, read_capacity);],
            [proof {
                if constructor_owner_admission_v1(context, allocation_capacity, writer_capacity, read_capacity).is_none() {
                    let count = (allocator.attempts@.len() - before.attempts@.len()) as int;
                    assert(allocator.attempts@ =~= before.attempts@ + requests.take(count));
                    assert forall|i: int| 0 <= i && i + 1 < count
                        implies #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat) by {}
                    assert(constructor_trace_v1(before, *allocator, requests, false));
                }
            }],
            [proof {
                assert(allocator.attempts@ =~= before.attempts@ + requests);
                assert forall|i: int| 0 <= i < 13
                    implies #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat) by {}
                assert(requests.take(requests.len() as int) =~= requests);
                assert(constructor_trace_v1(before, *allocator, requests, true));
            }])
    }

    fn new_observed_v1(context: u64, allocation_capacity: usize, writer_capacity: usize, read_capacity: usize,
        allocator: &mut ConstructorObservationsV1) -> (result: Result<Self, ContextVersionJournalErrorV1>)
        requires old(allocator).attempts@.len() + 13 <= usize::MAX,
        ensures constructor_producer_relation_v1(*old(allocator), *final(allocator), context, allocation_capacity, writer_capacity, read_capacity, result),
    {
        constructor_entry_body!(Self::new_with_allocator_v1, allocator, context, allocation_capacity, writer_capacity, read_capacity)
    }
}

}
