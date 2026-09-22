verus! {

spec fn begin_roster_view(roster: Seq<AllocationWriteV1>) -> Seq<logical::AllocationWriteV1> {
    roster.map(|_i, value| allocation_write_view(value))
}

spec fn begin_result_from<T>(value: Result<T, logical::ReadErrorV1>) -> Result<T, ReadErrorV1> {
    match value { Ok(result) => Ok(result), Err(error) => Err(read_error_embed(error)) }
}

spec fn begin_allocation_result_from(value: Result<logical::AllocationEntryV1, logical::ReadErrorV1>)
    -> Result<AllocationEntryV1, ReadErrorV1>
{
    match value { Ok(entry) => Err(ReadErrorV1::InvalidAllocationReference), Err(error) => Err(read_error_embed(error)) }
}

spec fn begin_reserved_result_from(value: Result<logical::WriterKeyV1, logical::JournalErrorV1>)
    -> Result<WriterKeyV1, ReadErrorV1>
{
    match value { Ok(key) => Ok(writer_key_from(key)), Err(error) => Err(journal_error_embed(error)) }
}

proof fn begin_reserved_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1)
    requires represents(journal, model),
    ensures begin_reserved_decision_v1(journal, writer)
        == begin_reserved_result_from(logical::lookup_decision_v1(model.context_generation, model.writers@,
            writer_reference_view(writer))),
{
    writer_key_round_trip(writer.key, writer_key_view(writer.key));
    if writer.slot < journal.writers@.len() {
        if let Some(WriterEntryV1::Reserved(key)) = journal.writers@[writer.slot as int] {
            writer_key_round_trip(key, writer_key_view(key));
        }
    }
}

proof fn begin_allocation_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    reference: AllocationReferenceV1)
    requires represents(journal, model),
    ensures begin_exact_allocation_v1(journal, reference)
        == begin_allocation_result_from(logical::begin_exact_allocation_v1(model, allocation_reference_view(reference))),
{
    allocation_key_round_trip(reference.key, allocation_key_view(reference.key));
    if reference.slot < journal.allocations@.len() {
        if let Some(entry) = journal.allocations@[reference.slot as int] {
            allocation_entry_round_trip(entry, allocation_entry_view(entry));
            allocation_key_round_trip(entry.key, allocation_key_view(entry.key));
        }
    }
}

proof fn begin_destination_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    destination: AllocationWriteV1)
    requires represents(journal, model),
    ensures begin_destination_decision_v1(journal, destination)
        == begin_result_from(logical::begin_destination_decision_v1(model, allocation_write_view(destination))),
{
    begin_allocation_correspondence(journal, model, destination.allocation);
    device_key_round_trip(destination.device, device_key_view(destination.device));
}

proof fn begin_canonical_correspondence(roster: Seq<AllocationWriteV1>, index: nat)
    ensures begin_canonical_scan_v1(roster, index)
        == begin_result_from(logical::begin_canonical_scan_v1(begin_roster_view(roster), index)),
    decreases roster.len() - index,
{
    if index < roster.len() { begin_canonical_correspondence(roster, index + 1); }
}

proof fn begin_destinations_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    roster: Seq<AllocationWriteV1>, index: nat)
    requires represents(journal, model),
    ensures begin_destination_scan_v1(journal, roster, index)
        == begin_result_from(logical::begin_destination_scan_v1(model, begin_roster_view(roster), index)),
    decreases roster.len() - index,
{
    if index < roster.len() {
        begin_destination_correspondence(journal, model, roster[index as int]);
        begin_destinations_correspondence(journal, model, roster, index + 1);
    }
}

proof fn begin_slots_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    count: nat, index: nat)
    requires represents(journal, model), count <= journal.member_free@.len(), count <= journal.scratch@.len(),
    ensures begin_slot_scan_v1(journal, count, index)
        == begin_result_from(logical::begin_slot_scan_v1(model, count, index)),
    decreases count - index,
{
    if index < count { begin_slots_correspondence(journal, model, count, index + 1); }
}

proof fn begin_preflight_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>)
    requires represents(journal, model),
    ensures begin_preflight_decision_v1(journal, writer, roster)
        == begin_result_from(logical::begin_preflight_decision_v1(model, writer_reference_view(writer), begin_roster_view(roster))),
{
    begin_reserved_correspondence(journal, model, writer);
    let lookup = logical::lookup_decision_v1(model.context_generation, model.writers@, writer_reference_view(writer));
    if let Err(error) = lookup { journal_error_lift_commutes(error); }
    begin_canonical_correspondence(roster, 0);
    begin_destinations_correspondence(journal, model, roster, 0);
    if roster.len() <= journal.member_free@.len() && roster.len() <= journal.scratch@.len() {
        begin_slots_correspondence(journal, model, roster.len(), 0);
    }
}

proof fn begin_ready_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    roster: Seq<AllocationWriteV1>)
    requires represents(journal, model),
    ensures begin_storage_ready_v1(journal, roster) == logical::begin_storage_ready_v1(model, begin_roster_view(roster)),
{
    assert forall|i: int| 0 <= i < roster.len() implies
        begin_destination_decision_v1(journal, #[trigger] roster[i])
        == begin_result_from(logical::begin_destination_decision_v1(model, begin_roster_view(roster)[i])) by {
        begin_destination_correspondence(journal, model, roster[i]);
    }
    if begin_storage_ready_v1(journal, roster) {
        assert forall|i: int| 0 <= i < roster.len() implies
            #[trigger] logical::begin_destination_decision_v1(model, begin_roster_view(roster)[i]) == Ok(()) by {
            assert(begin_destination_decision_v1(journal, roster[i]) == Ok(()));
        }
        assert forall|i: int| 0 <= i < roster.len() implies {
            &&& model.member_free@[model.member_free@.len() - 1 - i] < model.members@.len()
            &&& model.members@[model.member_free@[model.member_free@.len() - 1 - i] as int].is_none()
            &&& model.scratch@[i].is_none()
        } by {
            assert(journal.member_free@[journal.member_free@.len() - 1 - i] < journal.members@.len());
            assert(journal.members@[journal.member_free@[journal.member_free@.len() - 1 - i] as int].is_none());
            assert(journal.scratch@[i].is_none());
        }
    }
    if logical::begin_storage_ready_v1(model, begin_roster_view(roster)) {
        assert forall|i: int| 0 <= i < roster.len() implies
            #[trigger] begin_destination_decision_v1(journal, roster[i]) == Ok(()) by {
            assert(logical::begin_destination_decision_v1(model, begin_roster_view(roster)[i]) == Ok(()));
        }
        assert forall|i: int| 0 <= i < roster.len() implies {
            &&& journal.member_free@[journal.member_free@.len() - 1 - i] < journal.members@.len()
            &&& journal.members@[journal.member_free@[journal.member_free@.len() - 1 - i] as int].is_none()
            &&& journal.scratch@[i].is_none()
        } by {
            assert(model.member_free@[model.member_free@.len() - 1 - i] < model.members@.len());
            assert(model.members@[model.member_free@[model.member_free@.len() - 1 - i] as int].is_none());
            assert(model.scratch@[i].is_none());
        }
    }
}

proof fn begin_plan_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    roster: Seq<AllocationWriteV1>, index: int)
    requires represents(journal, model), begin_storage_ready_v1(journal, roster), 0 <= index < roster.len(),
    ensures begin_plan_view(begin_plan_v1(journal, roster, index))
        == logical::begin_plan_v1(model, begin_roster_view(roster), index),
{
    assert(begin_destination_decision_v1(journal, roster[index]) == Ok(()));
}

proof fn begin_scratch_correspondence(journal: JournalContentsV1, model: logical::JournalContentsV1,
    roster: Seq<AllocationWriteV1>, filled: nat, cleared: nat)
    requires represents(journal, model), begin_storage_ready_v1(journal, roster), cleared <= filled <= roster.len(),
    ensures begin_scratch_v1(journal, roster, filled, cleared).map(|_i, value| begin_plan_slot_view(value))
        == logical::begin_scratch_v1(model, begin_roster_view(roster), filled, cleared),
{
    assert forall|i: int| 0 <= i < journal.scratch@.len() implies
        begin_plan_slot_view(begin_scratch_v1(journal, roster, filled, cleared)[i])
        == logical::begin_scratch_v1(model, begin_roster_view(roster), filled, cleared)[i] by {
        if cleared <= i < filled { begin_plan_correspondence(journal, model, roster, i); }
    }
    assert(begin_scratch_v1(journal, roster, filled, cleared).map(|_i, value| begin_plan_slot_view(value))
        =~= logical::begin_scratch_v1(model, begin_roster_view(roster), filled, cleared));
}

}
