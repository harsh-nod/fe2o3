verus! {

impl ContextVersionJournalV1 {
    fn validate_allocation_retirement_observed_v1(&self, roster: &[AllocationReferenceV1], capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        ensures result == retirement_decision_v1(*self, roster@, capacity),
    {
        retirement_preflight_body!(verus_exec_expr, self, roster, shared_retained_allocation_v1,
            shared_retained_allocation_less_v1, capacity, previous, index, [
                invariant index <= roster.len(), roster.len() <= self.allocation_capacity,
                    retirement_scan_v1(*self, roster@, 0, None) == retirement_scan_v1(*self, roster@, index as nat, previous),
                decreases roster.len() - index,
            ])
    }

    fn retire_allocations_observed_v1(&mut self, roster: &[AllocationReferenceV1], capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        ensures retirement_relation_v1(*old(self), *final(self), roster@, capacity, result),
    {
        retirement_execute_body!(verus_exec_expr, self, roster, validate_allocation_retirement_observed_v1, [, capacity],
            index, [
                let ghost before = *self;
                proof {
                    retirement_ready_v1(before, roster@, capacity);
                    assert(retirement_slots_v1(roster@, 0) =~= Seq::<usize>::empty());
                }
            ], [
                invariant index <= roster.len(), before == *old(self),
                    retirement_decision_v1(before, roster@, capacity).is_ok(),
                    enrollment_untouched_journal_v1(before, *self),
                    self.allocations@.len() == before.allocations@.len(),
                    self.allocations@ == retirement_prefix_v1(before.allocations@, roster@, index as nat),
                    self.allocation_free@ == before.allocation_free@ + retirement_slots_v1(roster@, index as nat),
                    forall|i: int| 0 <= i < roster.len() ==> (#[trigger] roster@[i]).slot < self.allocations@.len(),
                    before.allocation_free@.len() + roster.len() <= usize::MAX,
                decreases roster.len() - index,
            ], [proof {
                assert(retirement_slots_v1(roster@, index as nat).push(roster@[index as int].slot)
                    =~= retirement_slots_v1(roster@, index as nat + 1));
                assert((before.allocation_free@ + retirement_slots_v1(roster@, index as nat)).push(roster@[index as int].slot)
                    =~= before.allocation_free@ + retirement_slots_v1(roster@, index as nat + 1));
            }])
    }
}

impl ContextReadLeasedJournalV1 {
    fn require_unread_allocations(&self, roster: &[AllocationReferenceV1]) -> (result: Result<(), ReadErrorV1>)
        requires retirement_stable_safe_v1(*self, roster@, 0),
        ensures result == retirement_stable_unread_v1(*self, roster@, 0),
    {
        retirement_unread_body!(verus_exec_expr, self, roster, index, [
            invariant index <= roster.len(), retirement_stable_safe_v1(*self, roster@, index as nat),
                retirement_stable_unread_v1(*self, roster@, 0) == retirement_stable_unread_v1(*self, roster@, index as nat),
            decreases roster.len() - index,
        ])
    }

    fn validate_allocation_retirement_observed_v1(&self, roster: &[AllocationReferenceV1], capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        requires retirement_stable_safe_v1(*self, roster@, 0),
        ensures result == retirement_stable_decision_v1(*self, roster@, capacity),
    {
        retirement_owner_validate_body!(verus_exec_expr, self, journal, roster, validate_allocation_retirement_observed_v1, [, capacity], [])
    }

    fn retire_allocations_observed_v1(&mut self, roster: &[AllocationReferenceV1], capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        requires retirement_stable_safe_v1(*old(self), roster@, 0),
        ensures retirement_stable_relation_v1(*old(self), *final(self), roster@, capacity, result),
    {
        retirement_owner_execute_body!(verus_exec_expr, self, journal, roster, validate_allocation_retirement_observed_v1,
            retire_allocations_observed_v1, [, capacity], [])
    }
}

impl ContextProducerReadJournalV1 {
    fn require_unread_allocations(&self, roster: &[AllocationReferenceV1]) -> (result: Result<(), ReadErrorV1>)
        requires retirement_producer_safe_v1(*self, roster@, 0),
        ensures result == retirement_producer_unread_v1(*self, roster@, 0),
    {
        retirement_unread_body!(verus_exec_expr, self, roster, index, [
            invariant index <= roster.len(), retirement_producer_safe_v1(*self, roster@, index as nat),
                retirement_producer_unread_v1(*self, roster@, 0) == retirement_producer_unread_v1(*self, roster@, index as nat),
            decreases roster.len() - index,
        ])
    }

    fn validate_allocation_retirement_observed_v1(&self, roster: &[AllocationReferenceV1], capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        requires retirement_producer_safe_v1(*self, roster@, 0),
        ensures result == retirement_producer_decision_v1(*self, roster@, capacity),
    {
        retirement_owner_validate_body!(verus_exec_expr, self, stable, roster, validate_allocation_retirement_observed_v1, [, capacity], [
            proof { retirement_producer_to_stable_v1(*self, roster@, 0); }
        ])
    }

    fn retire_allocations_observed_v1(&mut self, roster: &[AllocationReferenceV1], capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        requires retirement_producer_safe_v1(*old(self), roster@, 0),
        ensures retirement_producer_relation_v1(*old(self), *final(self), roster@, capacity, result),
    {
        retirement_owner_execute_body!(verus_exec_expr, self, stable, roster, validate_allocation_retirement_observed_v1,
            retire_allocations_observed_v1, [, capacity], [proof {
                retirement_producer_to_stable_v1(*self, roster@, 0);
            }])
    }
}

}
