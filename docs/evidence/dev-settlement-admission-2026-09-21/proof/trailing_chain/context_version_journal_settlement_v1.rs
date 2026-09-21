// Settlement admission and issuance composition; physical/native refinement remains separate.
include!("context_version_journal_begin_custody_v1.rs");

verus! {

pub proof fn settle_preserves_issued_custody_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, success: bool, storage: StorageCapacitiesV1,
    history: Seq<WriterReferenceV1>)
    requires issued_custody_v1(before, storage, history), settle_chain_relation_v1(before, after, writer, chain, success),
    ensures issued_custody_v1(after, storage, history),
{
    settle_preserves_pending_custody_v1(before, after, writer, chain, success);
    let pre = issuance_contents_projection_v1(before, storage, history);
    let post = issuance_contents_projection_v1(after, storage, history);
    prefix_update_v1(pre.writers, writer.slot as int, None, pre.writers.len() as int);
    assert(partition_v1(post));
    assert forall|w: int| #![trigger post.writers[w]] 0 <= w < post.writers.len() implies match post.writers[w] {
        Some(entry) => {
            &&& exists|i: int| 0 <= i < history.len() && history[i].slot == w
                && same_key_v1(history[i].key, writer_key_v1(entry))
            &&& forall|i: int| 0 <= i < history.len() && history[i].slot == w
                ==> history[i].key.local <= writer_key_v1(entry).local
        },
        None => true,
    } by { assert(post.writers[w] == pre.writers[w] || w == writer.slot); }
}

pub proof fn settle_preserves_issued_producer_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, success: bool, storage: StorageCapacitiesV1,
    history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history), producer_storage_frame_v1(before, after),
        settle_chain_relation_v1(before.stable.journal, after.stable.journal, writer, chain, success),
    ensures issued_producer_v1(after, storage, history),
{
    settle_preserves_issued_custody_v1(before.stable.journal, after.stable.journal, writer, chain, success, storage, history);
    settle_preserves_producer_invariant_v1(before, after, writer, chain, success);
}

pub proof fn unknown_preserves_issued_custody_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_custody_v1(before, storage, history), mark_unknown_relation_v1(before, after, writer),
    ensures issued_custody_v1(after, storage, history),
{
    unknown_preserves_pending_custody_v1(before, after, writer);
    let pre = issuance_contents_projection_v1(before, storage, history);
    let post = issuance_contents_projection_v1(after, storage, history);
    prefix_update_v1(pre.writers, writer.slot as int, post.writers[writer.slot as int], pre.writers.len() as int);
    assert forall|w: int| #![trigger post.writers[w]] 0 <= w < post.writers.len() implies match post.writers[w] {
        Some(entry) => {
            &&& exists|i: int| 0 <= i < history.len() && history[i].slot == w
                && same_key_v1(history[i].key, writer_key_v1(entry))
            &&& forall|i: int| 0 <= i < history.len() && history[i].slot == w
                ==> history[i].key.local <= writer_key_v1(entry).local
        },
        None => true,
    } by { assert(pre.writers[w].is_some() == post.writers[w].is_some()); }
}

pub proof fn unknown_preserves_issued_producer_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history), producer_storage_frame_v1(before, after),
        mark_unknown_relation_v1(before.stable.journal, after.stable.journal, writer),
    ensures issued_producer_v1(after, storage, history),
{
    unknown_preserves_issued_custody_v1(before.stable.journal, after.stable.journal, writer, storage, history);
    unknown_preserves_producer_invariant_v1(before, after, writer);
}

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
    if head.is_some() { return Ok(()); }
    Ok(())
}

pub open spec fn settlement_scratch_scan_v1(journal: JournalContentsV1, count: usize, index: nat) -> Result<(), ReadErrorV1>
    recommends count <= journal.scratch@.len(),
    decreases count - index,
{
    if index >= count { Ok(()) }
    else if journal.scratch@[index as int].is_some() { Err(ReadErrorV1::InvalidState) }
    else { settlement_scratch_scan_v1(journal, count, index + 1) }
}

pub open spec fn settlement_return_decision_v1(journal: JournalContentsV1, count: usize,
    free_storage: usize, member_free_storage: usize) -> Result<(), ReadErrorV1>
{
    let writers = journal.free@.len() + 1;
    let members = journal.member_free@.len() + count;
    if writers > usize::MAX || members > usize::MAX || writers > journal.writer_capacity || writers > free_storage
        || members > journal.allocation_capacity || members > member_free_storage || count > journal.scratch@.len() {
        Err(ReadErrorV1::InvalidState)
    } else { settlement_scratch_scan_v1(journal, count, 0) }
}

pub fn settlement_return_exec_v1(journal: &JournalContentsV1, count: usize,
    free_storage: usize, member_free_storage: usize) -> (result: Result<(), ReadErrorV1>)
    ensures result == settlement_return_decision_v1(*journal, count, free_storage, member_free_storage),
{
    let writers = match journal.free.len().checked_add(1) { Some(value) => value, None => return Err(ReadErrorV1::InvalidState) };
    let members = match journal.member_free.len().checked_add(count) { Some(value) => value, None => return Err(ReadErrorV1::InvalidState) };
    if writers > journal.writer_capacity || writers > free_storage || members > journal.allocation_capacity
        || members > member_free_storage || count > journal.scratch.len() { return Err(ReadErrorV1::InvalidState); }
    let mut index = 0usize;
    while index < count
        invariant index <= count, count <= journal.scratch@.len(),
            settlement_return_decision_v1(*journal, count, free_storage, member_free_storage)
                == settlement_scratch_scan_v1(*journal, count, index as nat),
        decreases count - index,
    {
        if journal.scratch[index].is_some() { return Err(ReadErrorV1::InvalidState); }
        index += 1;
    }
    Ok(())
}

pub open spec fn settlement_preflight_decision_v1(journal: JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize) -> Result<(Option<usize>, usize), ReadErrorV1>
{
    match retained_header_decision_v1(journal, writer, false) {
        Err(error) => Err(error),
        Ok((head, count, _)) => if evidence != writer { Err(ReadErrorV1::SettlementEvidenceMismatch) }
            else { match retained_chain_decision_v1(journal, writer, head, count) {
                Err(error) => Err(error),
                Ok(()) => match settlement_return_decision_v1(journal, count, free_storage, member_free_storage) {
                    Err(error) => Err(error), Ok(()) => Ok((head, count)),
                },
            } },
    }
}

// Capacity arguments are observations, not an asserted binding to physical Vec storage.
pub fn settlement_preflight_exec_v1(journal: &JournalContentsV1, writer: WriterReferenceV1,
    evidence: WriterReferenceV1, free_storage: usize, member_free_storage: usize)
    -> (result: Result<(Option<usize>, usize), ReadErrorV1>)
    ensures result == settlement_preflight_decision_v1(*journal, writer, evidence, free_storage, member_free_storage),
{
    let (head, count, _) = match retained_header_exec_v1(journal, writer, false) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    if evidence.slot != writer.slot || !same_key_exec_v1(evidence.key, writer.key) { return Err(ReadErrorV1::SettlementEvidenceMismatch); }
    match retained_chain_exec_v1(journal, writer, head, count) { Ok(_) => {}, Err(error) => return Err(error) };
    match settlement_return_exec_v1(journal, count, free_storage, member_free_storage) { Ok(_) => {}, Err(error) => return Err(error) };
    Ok((head, count))
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

pub open spec fn unknown_issued_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, result: Result<(), ReadErrorV1>, storage: StorageCapacitiesV1,
    history: Seq<WriterReferenceV1>) -> bool
{
    &&& unknown_execution_relation_v1(before.stable.journal, after.stable.journal, writer, result)
    &&& producer_storage_frame_v1(before, after)
    &&& issued_producer_v1(after, storage, history)
    &&& result.is_err() ==> after == before
}

pub fn unknown_issued_exec_v1(contents: &mut ProducerReadContentsV1, writer: WriterReferenceV1,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), ReadErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures unknown_issued_relation_v1(*old(contents), *final(contents), writer, result, storage, history),
{
    let ghost before = *contents;
    let result = unknown_exec_v1(&mut contents.stable.journal, writer);
    proof { if result.is_ok() { unknown_preserves_issued_producer_v1(before, *contents, writer, storage, history); } }
    result
}

pub open spec fn settlement_witness_ready_v1(contents: ProducerReadContentsV1, writer: WriterReferenceV1, empty: bool) -> bool {
    let head = if empty { None } else { Some(0usize) };
    let count = if empty { 0usize } else { 2usize };
    &&& issued_producer_v1(contents, witness_storage_v1(3, 2), seq![writer])
    &&& settlement_preflight_decision_v1(contents.stable.journal, writer, writer, 3, 3) == Ok((head, count))
    &&& contents.stable.journal.free@.len() == 1
    &&& retained_header_decision_v1(contents.stable.journal, writer, false) == Ok((head, count, false))
}

#[verifier::spinoff_prover]
pub fn settlement_witness_setup_v1(empty: bool)
    -> (result: Result<(ProducerReadContentsV1, WriterReferenceV1), ReadErrorV1>)
    ensures match result { Ok((contents, writer)) => settlement_witness_ready_v1(contents, writer, empty), Err(_) => false },
{
    proof {
        reveal_with_fuel(producer_unread_scan_v1, 4);
        reveal_with_fuel(unread_scan_v1, 4);
        reveal_with_fuel(enrollment_roster_scan_v1, 4);
        reveal_with_fuel(enrollment_prefix_v1, 4);
        reveal_with_fuel(begin_canonical_scan_v1, 4);
        reveal_with_fuel(begin_destination_scan_v1, 4);
        reveal_with_fuel(begin_slot_scan_v1, 4);
        reveal_with_fuel(begin_allocations_prefix_v1, 4);
        reveal_with_fuel(begin_members_prefix_v1, 4);
        reveal_with_fuel(retained_scan_v1, 4);
        reveal_with_fuel(settlement_scratch_scan_v1, 4);
    }
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = Seq::<WriterReferenceV1>::empty();
    let mut contents = match issued_producer_constructor_exec_v1(7, 3, 2, 2, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return Err(ReadErrorV1::InvalidState),
    };
    let entries = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 },
        EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
        device: DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let mut output = vec![None, None];
    let enrolled = enrollment_issued_exec_v1(&mut contents, &entries, &mut output, Ghost(storage), Ghost(history));
    assert(enrolled == Ok(()));
    let key = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let writer = match register_issued_producer_exec_v1(&mut contents, key, Ghost(storage), Ghost(history)) {
        Ok(reference) => reference, Err(_) => return Err(ReadErrorV1::InvalidState),
    };
    let ghost history = seq![writer];
    let mut roster = vec![];
    if !empty {
        roster.push(AllocationWriteV1 { allocation: output[0].unwrap(), device: entries[0].device, byte_extent: 16 });
        roster.push(AllocationWriteV1 { allocation: output[1].unwrap(), device: entries[1].device, byte_extent: 32 });
    }
    let begun = begin_issued_exec_v1(&mut contents, writer, &roster, Ghost(storage), Ghost(history));
    assert(begun == Ok(()));
    Ok((contents, writer))
}

#[verifier::spinoff_prover]
pub fn settlement_constructor_witness_v1(empty: bool) -> (result: bool)
    ensures result,
{
    proof { reveal_with_fuel(retained_scan_v1, 4); }
    let (mut contents, writer) = match settlement_witness_setup_v1(empty) {
        Ok(value) => value, Err(_) => return false,
    };
    let ghost storage = witness_storage_v1(3, 2);
    let ghost history = seq![writer];
    let head = if empty { None } else { Some(0usize) };
    let count = if empty { 0usize } else { 2usize };
    let admitted = settlement_preflight_exec_v1(&contents.stable.journal, writer, writer, 3, 3);
    assert(admitted == Ok((head, count)));
    let rejected = settlement_preflight_exec_v1(&contents.stable.journal, writer, writer, 0, 3);
    assert(rejected == Err(ReadErrorV1::InvalidState));
    let wrong = WriterReferenceV1 { slot: usize::MAX, key: writer.key };
    let rejected = settlement_preflight_exec_v1(&contents.stable.journal, writer, wrong, 0, 0);
    assert(rejected == Err(ReadErrorV1::SettlementEvidenceMismatch));
    assert(retained_chain_decision_v1(contents.stable.journal, writer, head, count) == Ok(()));
    assert(unknown_decision_v1(contents.stable.journal, writer) == Ok(()));
    let marked = unknown_issued_exec_v1(&mut contents, writer, Ghost(storage), Ghost(history));
    assert(marked == Ok(()));
    assert(contents.stable.journal.writers@[writer.slot as int] == Some(WriterEntryV1::Unknown { key: writer.key, head, count }));
    assert(issued_producer_v1(contents, storage, history));
    let rejected = settlement_preflight_exec_v1(&contents.stable.journal, writer, wrong, 0, 0);
    assert(rejected == Err(ReadErrorV1::InvalidReference));
    let ghost before = contents.stable.journal;
    let repeated = unknown_exec_v1(&mut contents.stable.journal, writer);
    assert(repeated == Ok(()) && contents.stable.journal == before);
    true
}

}
