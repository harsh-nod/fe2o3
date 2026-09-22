verus! {

spec fn enrollment_view(value: EnrollmentV1) -> logical::EnrollmentV1 {
    logical::EnrollmentV1 { key: allocation_key_view(value.key),
        device: device_key_view(value.device), byte_extent: value.byte_extent }
}

spec fn entries_view(values: Seq<EnrollmentV1>) -> Seq<logical::EnrollmentV1> {
    values.map(|_i, value| enrollment_view(value))
}

spec fn reference_slot_view(value: Option<AllocationReferenceV1>) -> Option<logical::AllocationReferenceV1> {
    match value { None => None, Some(value) => Some(allocation_reference_view(value)) }
}

spec fn references_view(values: Seq<Option<AllocationReferenceV1>>) -> Seq<Option<logical::AllocationReferenceV1>> {
    values.map(|_i, value| reference_slot_view(value))
}

spec fn enrollment_result_from(value: Result<(), logical::EnrollmentErrorV1>) -> Result<(), EnrollmentErrorV1> {
    match value { Ok(()) => Err(EnrollmentErrorV1::InvalidState), Err(error) => Err(enrollment_error_embed(error)) }
}

spec fn enrollment_error_option_from(value: Option<logical::EnrollmentErrorV1>) -> Option<EnrollmentErrorV1> {
    match value { None => None, Some(error) => Some(enrollment_error_embed(error)) }
}

proof fn enrollment_entry_correspondence(context: u64, entry: EnrollmentV1)
    ensures enrollment_entry_error_v1(context, entry)
        == enrollment_error_option_from(logical::enrollment_entry_error_v1(context, enrollment_view(entry))),
        allocation_entry_view(enrollment_value_v1(entry)) == logical::enrollment_value_v1(enrollment_view(entry)),
{}

proof fn enrollment_order_correspondence(left: AllocationKeyV1, right: AllocationKeyV1)
    ensures enrollment_key_less_v1(left, right)
        == logical::enrollment_key_less_v1(allocation_key_view(left), allocation_key_view(right)),
        (left == right) == (allocation_key_view(left) == allocation_key_view(right)),
{}

proof fn enrollment_scan_correspondence(context: u64, entries: Seq<EnrollmentV1>, index: nat)
    ensures enrollment_roster_scan_v1(context, entries, index)
        == enrollment_result_from(logical::enrollment_roster_scan_v1(context, entries_view(entries), index)),
    decreases entries.len() - index,
{
    if index < entries.len() {
        enrollment_entry_correspondence(context, entries[index as int]);
        if index > 0 { enrollment_order_correspondence(entries[index - 1].key, entries[index as int].key); }
        enrollment_scan_correspondence(context, entries, index + 1);
    }
}

proof fn enrollment_header_correspondence(context: u64, capacity: usize, free_len: usize,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>)
    ensures enrollment_header_decision_v1(context, capacity, free_len, entries, output)
        == enrollment_result_from(logical::enrollment_header_decision_v1(context, capacity, free_len,
            entries_view(entries), references_view(output))),
{
    enrollment_scan_correspondence(context, entries, 0);
    let mapped = references_view(output);
    if exists|i: int| 0 <= i < output.len() && output[i].is_some() {
        let i = choose|i: int| 0 <= i < output.len() && output[i].is_some();
        assert(mapped[i].is_some());
    }
    if exists|i: int| 0 <= i < mapped.len() && mapped[i].is_some() {
        let i = choose|i: int| 0 <= i < mapped.len() && mapped[i].is_some();
        assert(output[i].is_some());
    }
    match logical::enrollment_roster_scan_v1(context, entries_view(entries), 0) {
        Ok(value) => { assert(value == ()); }, Err(_) => {},
    }
}

}
