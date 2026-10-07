verus! {

spec fn retirement_roster_view_v1(roster: Seq<AllocationReferenceV1>) -> Seq<logical::AllocationReferenceV1> {
    roster.map(|_i, reference| allocation_reference_view(reference))
}

proof fn retirement_scan_correspondence_v1(actual: JournalContentsV1, model: logical::JournalContentsV1,
    roster: Seq<AllocationReferenceV1>, index: nat, previous: Option<AllocationKeyV1>)
    requires represents(actual, model),
    ensures retirement_scan_v1(actual, roster, index, previous)
        == begin_result_from(logical::retirement_scan_v1(model, retirement_roster_view_v1(roster), index,
            owner_previous_view(previous))),
    decreases roster.len() - index,
{
    if index < roster.len() {
        begin_allocation_correspondence(actual, model, roster[index as int]);
        if let Some(key) = previous { enrollment_order_correspondence(key, roster[index as int].key); }
        retirement_scan_correspondence_v1(actual, model, roster, index + 1, Some(roster[index as int].key));
    }
}

proof fn retirement_decision_correspondence_v1(actual: JournalContentsV1, model: logical::JournalContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize)
    requires represents(actual, model),
    ensures retirement_decision_v1(actual, roster, capacity)
        == begin_result_from(logical::retirement_decision_v1(model, retirement_roster_view_v1(roster), capacity)),
{
    retirement_scan_correspondence_v1(actual, model, roster, 0, None);
}

proof fn retirement_stable_correspondence_v1(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    roster: Seq<AllocationReferenceV1>, index: nat)
    requires stable_represents(actual, model),
    ensures retirement_stable_unread_v1(actual, roster, index)
        == begin_result_from(logical::retirement_stable_unread_v1(model, retirement_roster_view_v1(roster), index)),
        retirement_stable_safe_v1(actual, roster, index)
        == logical::retirement_stable_safe_v1(model, retirement_roster_view_v1(roster), index),
    decreases roster.len() - index,
{
    if index < roster.len() {
        allocation_lookup_correspondence(actual.journal, model.journal, roster[index as int]);
        retirement_stable_correspondence_v1(actual, model, roster, index + 1);
    }
}

proof fn retirement_producer_correspondence_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    roster: Seq<AllocationReferenceV1>, index: nat)
    requires producer_represents(actual, model),
    ensures retirement_producer_unread_v1(actual, roster, index)
        == begin_result_from(logical::retirement_producer_unread_v1(model, retirement_roster_view_v1(roster), index)),
        retirement_producer_safe_v1(actual, roster, index)
        == logical::retirement_producer_safe_v1(model, retirement_roster_view_v1(roster), index),
    decreases roster.len() - index,
{
    if index < roster.len() {
        allocation_lookup_correspondence(actual.stable.journal, model.stable.journal, roster[index as int]);
        producer_count_correspondence(actual, model, roster[index as int]);
        retirement_producer_correspondence_v1(actual, model, roster, index + 1);
    }
}

proof fn retirement_prefix_correspondence_v1(actual: JournalContentsV1, model: logical::JournalContentsV1,
    roster: Seq<AllocationReferenceV1>, count: nat)
    requires represents(actual, model), count <= roster.len(),
        forall|i: int| 0 <= i < roster.len() ==> (#[trigger] roster[i]).slot < actual.allocations@.len(),
    ensures retirement_prefix_v1(actual.allocations@, roster, count).len() == actual.allocations@.len(),
        retirement_prefix_v1(actual.allocations@, roster, count).map(|_i, entry| allocation_entry_slot_view(entry))
            == logical::retirement_prefix_v1(model.allocations@, retirement_roster_view_v1(roster), count),
        retirement_slots_v1(roster, count) == logical::retirement_slots_v1(retirement_roster_view_v1(roster), count),
    decreases count,
{
    if count > 0 {
        retirement_prefix_correspondence_v1(actual, model, roster, (count - 1) as nat);
        assert(retirement_prefix_v1(actual.allocations@, roster, count).map(|_i, entry| allocation_entry_slot_view(entry))
            =~= logical::retirement_prefix_v1(model.allocations@, retirement_roster_view_v1(roster), count));
    }
    assert(retirement_slots_v1(roster, count) =~= logical::retirement_slots_v1(retirement_roster_view_v1(roster), count));
}

proof fn retirement_paired_transition_v1(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires represents(before, model_before), retirement_relation_v1(before, after, roster, capacity, result),
        logical::retirement_relation_v1(model_before, model_after, retirement_roster_view_v1(roster), capacity, model_result),
    ensures result == begin_result_from(model_result), represents(after, model_after),
{
    retirement_decision_correspondence_v1(before, model_before, roster, capacity);
    if result.is_ok() {
        retirement_ready_v1(before, roster, capacity);
        retirement_prefix_correspondence_v1(before, model_before, roster, roster.len());
    }
}

proof fn retirement_owner_paired_transition_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    roster: Seq<AllocationReferenceV1>, capacity: usize,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires producer_represents(before, model_before), retirement_producer_relation_v1(before, after, roster, capacity, result),
        logical::retirement_producer_relation_v1(model_before, model_after, retirement_roster_view_v1(roster), capacity, model_result),
    ensures result == begin_result_from(model_result), producer_represents(after, model_after),
{
    retirement_producer_correspondence_v1(before, model_before, roster, 0);
    retirement_stable_correspondence_v1(before.stable, model_before.stable, roster, 0);
    if retirement_producer_unread_v1(before, roster, 0).is_ok()
        && retirement_stable_unread_v1(before.stable, roster, 0).is_ok() {
        retirement_paired_transition_v1(before.stable.journal, after.stable.journal, model_before.stable.journal,
            model_after.stable.journal, roster, capacity, result, model_result);
    }
}

// Each side executes independently; raw correspondence needs only reached-prefix count safety.
fn owner_retirement_paired_exec_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    roster: &[AllocationReferenceV1], model_roster: &[logical::AllocationReferenceV1], capacity: usize,
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)), retirement_roster_view_v1(roster@) == model_roster@,
        retirement_producer_safe_v1(*old(actual), roster@, 0),
    ensures results.0 == begin_result_from(results.1), producer_represents(*final(actual), *final(model)),
        retirement_producer_relation_v1(*old(actual), *final(actual), roster@, capacity, results.0),
        logical::retirement_producer_relation_v1(*old(model), *final(model), model_roster@, capacity, results.1),
        results.0.is_err() ==> *final(actual) == *old(actual), results.1.is_err() ==> *final(model) == *old(model),
        logical::producer_invariant_v1(*old(model)) ==> logical::producer_invariant_v1(*final(model)),
        logical::issued_producer_v1(*old(model), storage, history) ==> logical::issued_producer_v1(*final(model), storage, history),
        logical::producer_invariant_v1(*old(model)) ==> {
            &&& forall|reference: logical::ReadReferenceV1| #[trigger] logical::lease_decision_v1(final(model).stable, reference)
                == logical::lease_decision_v1(old(model).stable, reference)
            &&& forall|reference: logical::ProducerReadReferenceV1| #[trigger] logical::producer_lookup_decision_v1(*final(model), reference)
                == logical::producer_lookup_decision_v1(*old(model), reference)
            &&& forall|s: int| 0 <= s < old(model).reservations@.len() && (#[trigger] old(model).reservations@[s]).is_some()
                ==> logical::producer_status_decision_v1(final(model).stable.journal, old(model).reservations@[s].unwrap().request)
                    == logical::producer_status_decision_v1(old(model).stable.journal, old(model).reservations@[s].unwrap().request)
        },
{
    let ghost before = *actual;
    let ghost model_before = *model;
    proof { retirement_producer_correspondence_v1(before, model_before, roster@, 0); }
    let result = actual.retire_allocations_observed_v1(roster, capacity);
    let model_result = logical::retirement_producer_exec_v1(model, model_roster, capacity);
    proof {
        retirement_owner_paired_transition_v1(before, *actual, model_before, *model, roster@, capacity, result, model_result);
        logical::retirement_producer_preserves_v1(model_before, *model, model_roster@, capacity, model_result, storage, history);
    }
    (result, model_result)
}

proof fn owner_retirement_invariant_domain_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    roster: Seq<AllocationReferenceV1>)
    requires producer_represents(actual, model), logical::producer_invariant_v1(model),
    ensures retirement_producer_safe_v1(actual, roster, 0),
{
    logical::retirement_producer_safe_from_invariant_v1(model, retirement_roster_view_v1(roster), 0);
    retirement_producer_correspondence_v1(actual, model, roster, 0);
}

}
