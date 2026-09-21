use vstd::prelude::*;
verus! {

pub proof fn reader_count_addition_fits_usize_v1(contents: ProducerReadContentsV1, allocation: usize)
    requires producer_invariant_v1(contents), allocation < contents.counts@.len(),
    ensures contents.stable.readers@[allocation as int] + contents.counts@[allocation as int] <= 2_097_152,
        contents.stable.readers@[allocation as int] + contents.counts@[allocation as int] <= usize::MAX,
{
    lease_count_bound_v1(contents.stable.leases@, allocation);
    producer_count_bound_v1(contents.reservations@, allocation);
}

pub open spec fn producer_reader_count_decision_v1(contents: ProducerReadContentsV1, allocation: AllocationReferenceV1)
    -> Result<usize, ReadErrorV1>
{
    match allocation_decision_v1(contents.stable.journal, allocation) {
        Err(error) => Err(error),
        Ok(_) => Ok((contents.stable.readers@[allocation.slot as int] + contents.counts@[allocation.slot as int]) as usize),
    }
}

pub fn producer_reader_count_exec_v1(contents: &ProducerReadContentsV1, allocation: AllocationReferenceV1)
    -> (result: Result<usize, ReadErrorV1>)
    requires producer_invariant_v1(*contents),
    ensures exact_decision_v1(result, producer_reader_count_decision_v1(*contents, allocation)),
{
    match allocation_lookup_exec_v1(&contents.stable.journal, allocation) {
        Err(error) => return Err(error), Ok(_) => {},
    }
    proof { reader_count_addition_fits_usize_v1(*contents, allocation.slot); }
    Ok(contents.stable.readers[allocation.slot] + contents.counts[allocation.slot])
}

pub open spec fn producer_unread_scan_v1(contents: ProducerReadContentsV1, allocations: Seq<AllocationReferenceV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases allocations.len() - index,
{
    if index >= allocations.len() { Ok(()) }
    else { match producer_reader_count_decision_v1(contents, allocations[index as int]) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) } else { producer_unread_scan_v1(contents, allocations, index + 1) },
    } }
}

pub fn producer_require_unread_exec_v1(contents: &ProducerReadContentsV1, allocations: &[AllocationReferenceV1])
    -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*contents),
    ensures exact_decision_v1(result, producer_unread_scan_v1(*contents, allocations@, 0)),
{
    let mut index = 0;
    while index < allocations.len()
        invariant index <= allocations.len(), producer_invariant_v1(*contents),
            producer_unread_scan_v1(*contents, allocations@, 0) == producer_unread_scan_v1(*contents, allocations@, index as nat),
        decreases allocations.len() - index,
    {
        let count = match producer_reader_count_exec_v1(contents, allocations[index]) {
            Err(error) => return Err(error), Ok(value) => value,
        };
        if count != 0 { return Err(ReadErrorV1::AllocationBusy); }
        index += 1;
    }
    Ok(())
}

}
