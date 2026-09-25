verus! {

spec fn owner_retained_header_v1(journal: JournalContentsV1, writer: WriterReferenceV1, allow_unknown: bool)
    -> Result<(Option<usize>, usize, bool), ReadErrorV1>
{
    if writer.slot >= journal.writers@.len() { Err(ReadErrorV1::InvalidReference) }
    else {
        let header = match journal.writers@[writer.slot as int] {
            Some(WriterEntryV1::Pending { key, head, count }) => Ok((key, head, count, false)),
            Some(WriterEntryV1::Unknown { key, head, count }) => if allow_unknown {
                Ok((key, head, count, true))
            } else { Err(ReadErrorV1::InvalidReference) },
            _ => Err(ReadErrorV1::InvalidReference),
        };
        match header {
            Err(error) => Err(error),
            Ok((key, head, count, unknown)) => if key != writer.key || key.context_generation != journal.context_generation {
                Err(ReadErrorV1::InvalidReference)
            } else { Ok((head, count, unknown)) },
        }
    }
}

spec fn owner_retained_member_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, previous: Option<AllocationKeyV1>) -> Result<MemberEntryV1, ReadErrorV1>
{
    match head {
        None => Err(ReadErrorV1::InvalidState),
        Some(slot) => if slot >= journal.members@.len() { Err(ReadErrorV1::InvalidState) }
        else { match journal.members@[slot as int] {
            None => Err(ReadErrorV1::InvalidState),
            Some(member) => {
                if member.writer != writer || (previous.is_some() && !begin_key_less_v1(previous.unwrap(), member.allocation.key)) {
                    Err(ReadErrorV1::InvalidState)
                } else { match begin_exact_allocation_v1(journal, member.allocation) {
                    Err(_) => Err(ReadErrorV1::InvalidState),
                    Ok(allocation) => if allocation.pending_member != Some(slot)
                        || allocation.attempt_epoch != member.attempt_epoch
                        || allocation.content_lineage != member.prior_lineage
                        || member.prior_lineage >= member.attempt_epoch {
                        Err(ReadErrorV1::InvalidState)
                    } else { Ok(member) },
                } }
            },
        } },
    }
}

spec fn owner_retained_scan_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, remaining: nat, previous: Option<AllocationKeyV1>) -> Result<(), ReadErrorV1>
    decreases remaining,
{
    if remaining == 0 {
        if head.is_some() { Err(ReadErrorV1::InvalidState) } else { Ok(()) }
    } else { match owner_retained_member_v1(journal, writer, head, previous) {
        Err(error) => Err(error),
        Ok(member) => owner_retained_scan_v1(journal, writer, member.next, (remaining - 1) as nat, Some(member.allocation.key)),
    } }
}

spec fn owner_retained_chain_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, count: usize) -> Result<(), ReadErrorV1>
{
    if count > journal.allocation_capacity || ((count == 0) != head.is_none()) { Err(ReadErrorV1::InvalidState) }
    else { owner_retained_scan_v1(journal, writer, head, count as nat, None) }
}

spec fn owner_unknown_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1) -> Result<(), ReadErrorV1> {
    match owner_retained_header_v1(journal, writer, true) {
        Err(error) => Err(error),
        Ok((head, count, _)) => owner_retained_chain_v1(journal, writer, head, count),
    }
}

spec fn owner_unknown_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
    &&& after.free == before.free
    &&& after.allocations == before.allocations
    &&& after.allocation_free == before.allocation_free
    &&& after.members == before.members
    &&& after.member_free == before.member_free
    &&& after.scratch == before.scratch
}

spec fn owner_unknown_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == owner_unknown_decision_v1(before, writer)
    &&& match owner_retained_header_v1(before, writer, true) {
        Ok((head, count, false)) => if result.is_ok() {
            &&& owner_unknown_frame_v1(before, after)
            &&& after.writers@ == before.writers@.update(writer.slot as int,
                Some(WriterEntryV1::Unknown { key: writer.key, head, count }))
        } else { after == before },
        _ => after == before,
    }
}

spec fn stable_unknown_relation_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1,
    writer: WriterReferenceV1, result: Result<(), ReadErrorV1>) -> bool
{
    &&& owner_unknown_relation_v1(before.journal, after.journal, writer, result)
    &&& after.leases == before.leases
    &&& after.free_reads == before.free_reads
    &&& after.readers == before.readers
    &&& after.next_incarnation == before.next_incarnation
}

spec fn producer_unknown_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    writer: WriterReferenceV1, result: Result<(), ReadErrorV1>) -> bool
{
    &&& stable_unknown_relation_v1(before.stable, after.stable, writer, result)
    &&& after.reservations == before.reservations
    &&& after.free == before.free
    &&& after.counts == before.counts
    &&& after.next_incarnation == before.next_incarnation
}

proof fn owner_retained_scan_frame_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, head: Option<usize>, remaining: nat, previous: Option<AllocationKeyV1>)
    requires owner_unknown_frame_v1(before, after),
    ensures owner_retained_scan_v1(before, writer, head, remaining, previous)
        == owner_retained_scan_v1(after, writer, head, remaining, previous),
    decreases remaining,
{
    if remaining > 0 {
        if let Ok(member) = owner_retained_member_v1(before, writer, head, previous) {
            owner_retained_scan_frame_v1(before, after, writer, member.next, (remaining - 1) as nat, Some(member.allocation.key));
        }
    }
}

proof fn owner_retained_chain_frame_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, head: Option<usize>, count: usize)
    requires owner_unknown_frame_v1(before, after),
    ensures owner_retained_chain_v1(before, writer, head, count) == owner_retained_chain_v1(after, writer, head, count),
{
    owner_retained_scan_frame_v1(before, after, writer, head, count as nat, None);
}

}
