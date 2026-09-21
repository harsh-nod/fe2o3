// Sequence decisions cover every production state, without historical Vec existence.
verus! {

spec fn allocation_decision(journal: LogicalJournalViewV1, reference: logical::AllocationReferenceV1)
    -> Result<logical::AllocationEntryV1, logical::ReadErrorV1>
{
    if reference.slot >= journal.allocations.len() { Err(logical::ReadErrorV1::InvalidAllocationReference) }
    else { match journal.allocations[reference.slot as int] {
        Some(entry) => if entry.key == reference.key && entry.key.context_generation == journal.context_generation {
            Ok(entry)
        } else { Err(logical::ReadErrorV1::InvalidAllocationReference) },
        None => Err(logical::ReadErrorV1::InvalidAllocationReference),
    } }
}

spec fn header_decision(journal: LogicalJournalViewV1, writer: logical::WriterReferenceV1, allow_unknown: bool)
    -> Result<(Option<usize>, usize, bool), logical::ReadErrorV1>
{
    if writer.slot >= journal.writers.len() { Err(logical::ReadErrorV1::InvalidReference) }
    else { match journal.writers[writer.slot as int] {
        Some(logical::WriterEntryV1::Pending { key, head, count }) =>
            if key == writer.key && key.context_generation == journal.context_generation { Ok((head, count, false)) }
            else { Err(logical::ReadErrorV1::InvalidReference) },
        Some(logical::WriterEntryV1::Unknown { key, head, count }) =>
            if allow_unknown && key == writer.key && key.context_generation == journal.context_generation { Ok((head, count, true)) }
            else { Err(logical::ReadErrorV1::InvalidReference) },
        _ => Err(logical::ReadErrorV1::InvalidReference),
    } }
}

spec fn member_decision(journal: LogicalJournalViewV1, writer: logical::WriterReferenceV1,
    head: Option<usize>, previous: Option<logical::AllocationKeyV1>)
    -> Result<logical::MemberEntryV1, logical::ReadErrorV1>
{
    if head.is_none() || head.unwrap() >= journal.members.len() || journal.members[head.unwrap() as int].is_none() {
        Err(logical::ReadErrorV1::InvalidState)
    } else {
        let slot = head.unwrap();
        let member = journal.members[slot as int].unwrap();
        if member.writer != writer || (previous.is_some() && !logical::enrollment_key_less_v1(previous.unwrap(), member.allocation.key)) {
            Err(logical::ReadErrorV1::InvalidState)
        } else { match allocation_decision(journal, member.allocation) {
            Err(_) => Err(logical::ReadErrorV1::InvalidState),
            Ok(allocation) => if allocation.pending_member != Some(slot)
                || allocation.attempt_epoch != member.attempt_epoch
                || allocation.content_lineage != member.prior_lineage
                || member.prior_lineage >= member.attempt_epoch { Err(logical::ReadErrorV1::InvalidState) }
                else { Ok(member) },
        } }
    }
}

spec fn scan_decision(journal: LogicalJournalViewV1, writer: logical::WriterReferenceV1,
    head: Option<usize>, remaining: nat, previous: Option<logical::AllocationKeyV1>)
    -> Result<(), logical::ReadErrorV1>
    decreases remaining,
{
    if remaining == 0 { if head.is_none() { Ok(()) } else { Err(logical::ReadErrorV1::InvalidState) } }
    else { match member_decision(journal, writer, head, previous) {
        Err(error) => Err(error),
        Ok(member) => scan_decision(journal, writer, member.next, (remaining - 1) as nat, Some(member.allocation.key)),
    } }
}

spec fn chain_decision(journal: LogicalJournalViewV1, writer: logical::WriterReferenceV1,
    head: Option<usize>, count: usize) -> Result<(), logical::ReadErrorV1>
{
    if count > journal.allocation_capacity || (count == 0) != head.is_none() { Err(logical::ReadErrorV1::InvalidState) }
    else { scan_decision(journal, writer, head, count as nat, None) }
}

spec fn allocation_result_from(result: Result<logical::AllocationEntryV1, logical::ReadErrorV1>)
    -> Result<AllocationEntryV1, ContextVersionJournalErrorV1>
{
    match result { Ok(value) => Ok(allocation_entry_from(value)), Err(error) => Err(read_error_embed(error)) }
}

spec fn member_result_from(result: Result<logical::MemberEntryV1, logical::ReadErrorV1>)
    -> Result<MemberEntryV1, ContextVersionJournalErrorV1>
{
    match result { Ok(value) => Ok(member_entry_from(value)), Err(error) => Err(read_error_embed(error)) }
}

spec fn read_result_from<T>(result: Result<T, logical::ReadErrorV1>) -> Result<T, ContextVersionJournalErrorV1> {
    match result { Ok(value) => Ok(value), Err(error) => Err(read_error_embed(error)) }
}

spec fn previous_view(previous: Option<ContextAllocationKeyV1>) -> Option<logical::AllocationKeyV1> {
    match previous { Some(value) => Some(allocation_key_view(value)), None => None }
}

proof fn allocation_historical(journal: logical::JournalContentsV1, reference: logical::AllocationReferenceV1)
    ensures allocation_decision(logical_contents(journal), reference) == logical::begin_exact_allocation_v1(journal, reference),
{}

proof fn header_historical(journal: logical::JournalContentsV1, writer: logical::WriterReferenceV1, allow_unknown: bool)
    ensures header_decision(logical_contents(journal), writer, allow_unknown) == logical::retained_header_decision_v1(journal, writer, allow_unknown),
{}

proof fn member_historical(journal: logical::JournalContentsV1, writer: logical::WriterReferenceV1,
    head: Option<usize>, previous: Option<logical::AllocationKeyV1>)
    ensures member_decision(logical_contents(journal), writer, head, previous) == logical::retained_member_decision_v1(journal, writer, head, previous),
{
    if head.is_some() && head.unwrap() < journal.members@.len() && journal.members@[head.unwrap() as int].is_some() {
        allocation_historical(journal, journal.members@[head.unwrap() as int].unwrap().allocation);
    }
}

proof fn scan_historical(journal: logical::JournalContentsV1, writer: logical::WriterReferenceV1,
    head: Option<usize>, remaining: nat, previous: Option<logical::AllocationKeyV1>)
    ensures scan_decision(logical_contents(journal), writer, head, remaining, previous)
        == logical::retained_scan_v1(journal, writer, head, remaining, previous),
    decreases remaining,
{
    if remaining > 0 {
        member_historical(journal, writer, head, previous);
        if let Ok(member) = logical::retained_member_decision_v1(journal, writer, head, previous) {
            scan_historical(journal, writer, member.next, (remaining - 1) as nat, Some(member.allocation.key));
        }
    }
}

proof fn chain_historical(journal: logical::JournalContentsV1, writer: logical::WriterReferenceV1, head: Option<usize>, count: usize)
    ensures chain_decision(logical_contents(journal), writer, head, count) == logical::retained_chain_decision_v1(journal, writer, head, count),
{
    scan_historical(journal, writer, head, count as nat, None);
}

}
