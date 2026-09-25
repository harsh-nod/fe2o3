// Exact retained and Unknown declarations from the historical settlement, in the importing type universe.
use super::*;

verus! {

pub open spec fn retained_header_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1, allow_unknown: bool)
    -> Result<(Option<usize>, usize, bool), ReadErrorV1>
{
    if writer.slot >= journal.writers@.len() { Err(ReadErrorV1::InvalidReference) }
    else { match journal.writers@[writer.slot as int] {
        Some(WriterEntryV1::Pending { key, head, count }) =>
            if key == writer.key && key.context_generation == journal.context_generation { Ok((head, count, false)) }
            else { Err(ReadErrorV1::InvalidReference) },
        Some(WriterEntryV1::Unknown { key, head, count }) =>
            if allow_unknown && key == writer.key && key.context_generation == journal.context_generation { Ok((head, count, true)) }
            else { Err(ReadErrorV1::InvalidReference) },
        _ => Err(ReadErrorV1::InvalidReference),
    } }
}

pub fn retained_header_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, allow_unknown: bool)
    -> (result: Result<(Option<usize>, usize, bool), ReadErrorV1>)
    ensures result == retained_header_decision_v1(*journal, writer, allow_unknown),
{
    if writer.slot >= journal.writers.len() { return Err(ReadErrorV1::InvalidReference); }
    let (key, head, count, unknown) = match journal.writers[writer.slot] {
        Some(WriterEntryV1::Pending { key, head, count }) => (key, head, count, false),
        Some(WriterEntryV1::Unknown { key, head, count }) if allow_unknown => (key, head, count, true),
        _ => return Err(ReadErrorV1::InvalidReference),
    };
    if !same_key_exec_v1(key, writer.key) || key.context_generation != journal.context_generation {
        return Err(ReadErrorV1::InvalidReference);
    }
    Ok((head, count, unknown))
}

pub open spec fn retained_member_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, previous: Option<AllocationKeyV1>) -> Result<MemberEntryV1, ReadErrorV1>
{
    if head.is_none() || head.unwrap() >= journal.members@.len() || journal.members@[head.unwrap() as int].is_none() {
        Err(ReadErrorV1::InvalidState)
    } else {
        let slot = head.unwrap();
        let member = journal.members@[slot as int].unwrap();
        if member.writer != writer || (previous.is_some() && !enrollment_key_less_v1(previous.unwrap(), member.allocation.key)) {
            Err(ReadErrorV1::InvalidState)
        } else { match begin_exact_allocation_v1(journal, member.allocation) {
            Err(_) => Err(ReadErrorV1::InvalidState),
            Ok(allocation) => if allocation.pending_member != Some(slot)
                || allocation.attempt_epoch != member.attempt_epoch
                || allocation.content_lineage != member.prior_lineage
                || member.prior_lineage >= member.attempt_epoch { Err(ReadErrorV1::InvalidState) }
                else { Ok(member) },
        } }
    }
}

pub fn retained_member_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, previous: Option<AllocationKeyV1>) -> (result: Result<MemberEntryV1, ReadErrorV1>)
    ensures result == retained_member_decision_v1(*journal, writer, head, previous),
{
    let slot = match head { Some(slot) => slot, None => return Err(ReadErrorV1::InvalidState) };
    if slot >= journal.members.len() { return Err(ReadErrorV1::InvalidState); }
    let member = match journal.members[slot] { Some(member) => member, None => return Err(ReadErrorV1::InvalidState) };
    if member.writer.slot != writer.slot || !same_key_exec_v1(member.writer.key, writer.key) {
        return Err(ReadErrorV1::InvalidState);
    }
    if let Some(key) = previous {
        if !enrollment_key_less_exec_v1(key, member.allocation.key) { return Err(ReadErrorV1::InvalidState); }
    }
    let allocation = match begin_exact_allocation_exec_v1(journal, member.allocation) {
        Ok(allocation) => allocation, Err(_) => return Err(ReadErrorV1::InvalidState),
    };
    if allocation.pending_member != Some(slot) || allocation.attempt_epoch != member.attempt_epoch
        || allocation.content_lineage != member.prior_lineage || member.prior_lineage >= member.attempt_epoch {
        return Err(ReadErrorV1::InvalidState);
    }
    Ok(member)
}

pub open spec fn retained_scan_v1(journal: JournalContentsV1, writer: WriterReferenceV1, head: Option<usize>,
    remaining: nat, previous: Option<AllocationKeyV1>) -> Result<(), ReadErrorV1>
    decreases remaining,
{
    if remaining == 0 { if head.is_none() { Ok(()) } else { Err(ReadErrorV1::InvalidState) } }
    else { match retained_member_decision_v1(journal, writer, head, previous) {
        Err(error) => Err(error),
        Ok(member) => retained_scan_v1(journal, writer, member.next, (remaining - 1) as nat, Some(member.allocation.key)),
    } }
}

pub open spec fn retained_chain_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    head: Option<usize>, count: usize) -> Result<(), ReadErrorV1>
{
    if count > journal.allocation_capacity || (count == 0) != head.is_none() { Err(ReadErrorV1::InvalidState) }
    else { retained_scan_v1(journal, writer, head, count as nat, None) }
}

pub fn retained_chain_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1, initial: Option<usize>, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    ensures result == retained_chain_decision_v1(*journal, writer, initial, count),
{
    let mut head = initial;
    if count > journal.allocation_capacity || (count == 0) != head.is_none() { return Err(ReadErrorV1::InvalidState); }
    let ghost original = head;
    let mut previous = None;
    let mut index = 0usize;
    while index < count
        invariant index <= count, count <= journal.allocation_capacity, (count == 0) == original.is_none(), original == initial,
            retained_scan_v1(*journal, writer, original, count as nat, None)
                == retained_scan_v1(*journal, writer, head, (count - index) as nat, previous),
        decreases count - index,
    {
        let member = match retained_member_exec_v1(journal, writer, head, previous) {
            Ok(member) => member, Err(error) => return Err(error),
        };
        previous = Some(member.allocation.key);
        head = member.next;
        index += 1;
    }
    if head.is_some() { return Err(ReadErrorV1::InvalidState); }
    Ok(())
}

pub open spec fn unknown_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1) -> Result<(), ReadErrorV1> {
    match retained_header_decision_v1(journal, writer, true) {
        Err(error) => Err(error),
        Ok((head, count, _)) => retained_chain_decision_v1(journal, writer, head, count),
    }
}

pub open spec fn unknown_execution_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == unknown_decision_v1(before, writer)
    &&& result.is_err() ==> after == before
    &&& result.is_ok() ==> mark_unknown_relation_v1(before, after, writer)
    &&& match retained_header_decision_v1(before, writer, true) {
        Ok((_, _, true)) => after == before, _ => true,
    }
}

pub fn unknown_exec_v1(journal: &mut JournalContentsV1, writer: WriterReferenceV1) -> (result: Result<(), ReadErrorV1>)
    ensures unknown_execution_relation_v1(*old(journal), *final(journal), writer, result),
{
    let ghost before = *journal;
    let (head, count, unknown) = match retained_header_exec_v1(journal, writer, true) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    match retained_chain_exec_v1(journal, writer, head, count) { Ok(value) => { assert(value == ()); }, Err(error) => return Err(error) };
    assert(unknown_decision_v1(before, writer) == Ok(()));
    if !unknown { journal.writers.set(writer.slot, Some(WriterEntryV1::Unknown { key: writer.key, head, count })); }
    else {
        assert(journal.writers@ =~= before.writers@.update(writer.slot as int,
            Some(unknown_writer_v1(before.writers@[writer.slot as int].unwrap()))));
    }
    assert(mark_unknown_relation_v1(before, *journal, writer));
    Ok(())
}

}
