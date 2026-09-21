// Producer-read custody candidate: logical contents, not Rust/native refinement.


verus! {

#[derive(Clone, Copy)]
pub struct ProducerReadV1 {
    pub read: AllocationReadV1,
    pub producer: WriterReferenceV1,
}

#[derive(Clone, Copy)]
pub struct ProducerReadReferenceV1 {
    pub slot: usize,
    pub incarnation: u64,
    pub consumer: WriterKeyV1,
}

#[derive(Clone, Copy)]
pub struct ProducerReservationV1 {
    pub reference: ProducerReadReferenceV1,
    pub request: ProducerReadV1,
}

#[derive(Clone, Copy)]
pub enum ProducerStatusV1 { Pending, Success, NoEffect, Unknown }

pub struct ProducerReadContentsV1 {
    pub stable: ReadContentsV1,
    pub reservations: Vec<Option<ProducerReservationV1>>,
    pub free: Vec<usize>,
    pub counts: Vec<usize>,
    pub next_incarnation: u64,
}

pub open spec fn same_producer_v1(a: WriterReferenceV1, b: WriterReferenceV1) -> bool {
    a.slot == b.slot && same_key_v1(a.key, b.key)
}

// None projects every validation error; exact public error precedence is separate.
pub open spec fn producer_status_v1(journal: JournalContentsV1, request: ProducerReadV1)
    -> Option<ProducerStatusV1>
{
    match allocation_decision_v1(journal, request.read.allocation) {
        Err(_) => None,
        Ok(entry) => {
            let read = request.read;
            if entry.device != read.device || entry.byte_extent != read.byte_extent
                || read.byte_len == 0 || read.byte_offset + read.byte_len > u64::MAX
                || read.byte_offset + read.byte_len > read.byte_extent
                || request.producer.key.context_generation != journal.context_generation
                || request.producer.key.kind != WriterKindV1::Submission
                || read.content_lineage >= read.attempt_epoch
                || entry.attempt_epoch != read.attempt_epoch { None }
            else { match entry.pending_member {
                Some(slot) => {
                    let member = journal.members@[slot as int].unwrap();
                    if !same_producer_v1(member.writer, request.producer)
                        || entry.content_lineage != read.content_lineage
                        || request.producer.slot >= journal.writers@.len() { None }
                    else { match journal.writers@[request.producer.slot as int] {
                        Some(WriterEntryV1::Pending { key, .. }) =>
                            if same_key_v1(key, request.producer.key) { Some(ProducerStatusV1::Pending) } else { None },
                        Some(WriterEntryV1::Unknown { key, .. }) =>
                            if same_key_v1(key, request.producer.key) { Some(ProducerStatusV1::Unknown) } else { None },
                        _ => None,
                    } }
                },
                None => if entry.content_lineage == read.attempt_epoch { Some(ProducerStatusV1::Success) }
                    else if entry.content_lineage == read.content_lineage { Some(ProducerStatusV1::NoEffect) }
                    else { None },
            } }
        },
    }
}

pub open spec fn slot_partition_v1<T>(entries: Seq<Option<T>>, free: Seq<usize>) -> bool {
    &&& free.len() <= entries.len()
    &&& free.no_duplicates()
    &&& forall|i: int| 0 <= i < free.len() ==> free[i] < entries.len()
    &&& forall|s: int| 0 <= s < entries.len()
        ==> (#[trigger] entries[s]).is_none() == free.contains(s as usize)
}

pub proof fn producer_concat_contains_v1(left: Seq<usize>, right: Seq<usize>, slot: usize)
    ensures (left + right).contains(slot) == (left.contains(slot) || right.contains(slot)),
{
    if left.contains(slot) {
        let i = choose|i: int| 0 <= i < left.len() && left[i] == slot;
        assert((left + right)[i] == slot);
    }
    if right.contains(slot) {
        let i = choose|i: int| 0 <= i < right.len() && right[i] == slot;
        assert((left + right)[left.len() + i] == slot);
    }
    if (left + right).contains(slot) {
        let i = choose|i: int| 0 <= i < (left + right).len() && (left + right)[i] == slot;
        if i < left.len() { assert(left[i] == slot); }
        else { assert(right[i - left.len()] == slot); }
    }
}

pub open spec fn writer_retains_v1(journal: JournalContentsV1, writer: WriterReferenceV1) -> bool {
    &&& writer.slot < journal.writers@.len()
    &&& match journal.writers@[writer.slot as int] {
        Some(WriterEntryV1::Pending { key, .. }) | Some(WriterEntryV1::Unknown { key, .. }) =>
            same_key_v1(key, writer.key),
        _ => false,
    }
}

#[verifier::opaque]
pub open spec fn chain_link_v1(journal: JournalContentsV1, writer: WriterReferenceV1, chain: Seq<usize>, i: int) -> bool {
    let slot = chain[i];
    &&& slot < journal.members@.len()
    &&& journal.members@[slot as int].is_some()
    &&& same_producer_v1(journal.members@[slot as int].unwrap().writer, writer)
    &&& journal.members@[slot as int].unwrap().next ==
        if i + 1 == chain.len() { None } else { Some(chain[i + 1]) }
    &&& i == 0 || journal.members@[chain[i - 1] as int].unwrap().allocation.key.local
        < journal.members@[slot as int].unwrap().allocation.key.local
}

pub open spec fn retained_chain_v1(journal: JournalContentsV1, writer: WriterReferenceV1, chain: Seq<usize>) -> bool {
    &&& writer_retains_v1(journal, writer)
    &&& chain.len() <= journal.allocation_capacity
    &&& chain.no_duplicates()
    &&& match journal.writers@[writer.slot as int].unwrap() {
        WriterEntryV1::Pending { head, count, .. } | WriterEntryV1::Unknown { head, count, .. } => {
            &&& count == chain.len()
            &&& head == if chain.len() == 0 { None } else { Some(chain[0]) }
        },
        _ => false,
    }
    &&& forall|i: int| 0 <= i < chain.len() ==> #[trigger] chain_link_v1(journal, writer, chain, i)
    &&& forall|m: int| 0 <= m < journal.members@.len() && (#[trigger] journal.members@[m]).is_some()
        && same_producer_v1(journal.members@[m].unwrap().writer, writer) ==> chain.contains(m as usize)
}

#[verifier::opaque]
pub open spec fn allocation_custody_v1(journal: JournalContentsV1, a: int) -> bool {
    journal.allocations@[a].is_some() ==> {
        let entry = journal.allocations@[a].unwrap();
        &&& entry.key.context_generation == journal.context_generation
        &&& issuable_id_v1(entry.key.local)
        &&& entry.device.context_generation == journal.context_generation
        &&& issuable_id_v1(entry.device.local)
        &&& entry.byte_extent > 0
        &&& entry.content_lineage <= entry.attempt_epoch
        &&& match entry.pending_member {
            None => true,
            Some(m) => {
                &&& m < journal.members@.len()
                &&& journal.members@[m as int].is_some()
                &&& journal.members@[m as int].unwrap().allocation == AllocationReferenceV1 { slot: a as usize, key: entry.key }
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn member_custody_v1(journal: JournalContentsV1, m: int) -> bool {
    journal.members@[m].is_some() ==> {
        let member = journal.members@[m].unwrap();
        &&& member.allocation.slot < journal.allocations@.len()
        &&& journal.allocations@[member.allocation.slot as int].is_some()
        &&& journal.allocations@[member.allocation.slot as int].unwrap().key == member.allocation.key
        &&& journal.allocations@[member.allocation.slot as int].unwrap().pending_member == Some(m as usize)
        &&& journal.allocations@[member.allocation.slot as int].unwrap().attempt_epoch == member.attempt_epoch
        &&& journal.allocations@[member.allocation.slot as int].unwrap().content_lineage == member.prior_lineage
        &&& member.prior_lineage < member.attempt_epoch
        &&& writer_retains_v1(journal, member.writer)
    }
}

#[verifier::opaque]
pub open spec fn writer_custody_v1(journal: JournalContentsV1, w: int) -> bool {
    journal.writers@[w].is_some() ==> {
        let entry = journal.writers@[w].unwrap();
        let key = writer_key_v1(entry);
        &&& key.context_generation == journal.context_generation
        &&& issuable_id_v1(key.local)
        &&& key.local <= journal.registration_watermark
        &&& match entry {
            WriterEntryV1::Reserved(_) => true,
            _ => exists|chain: Seq<usize>| #[trigger] retained_chain_v1(journal, WriterReferenceV1 { slot: w as usize, key }, chain),
        }
    }
}

// Independent of read reservations: exact live custody, not settlement's postcondition.
// This is not the complete issuance invariant: reserved_count/history remain separate.
pub open spec fn pending_custody_v1(journal: JournalContentsV1) -> bool {
    &&& constructor_admission_v1(journal.context_generation, journal.allocation_capacity, journal.writer_capacity).is_none()
    &&& journal.allocations@.len() == journal.allocation_capacity
    &&& journal.members@.len() == journal.allocation_capacity
    &&& journal.scratch@.len() == journal.allocation_capacity
    &&& journal.writers@.len() == journal.writer_capacity
    &&& slot_partition_v1(journal.allocations@, journal.allocation_free@)
    &&& slot_partition_v1(journal.members@, journal.member_free@)
    &&& slot_partition_v1(journal.writers@, journal.free@)
    &&& forall|i: int| 0 <= i < journal.scratch@.len() ==> (#[trigger] journal.scratch@[i]).is_none()
    &&& forall|a: int| 0 <= a < journal.allocations@.len() ==> #[trigger] allocation_custody_v1(journal, a)
    &&& forall|m: int| 0 <= m < journal.members@.len() ==> #[trigger] member_custody_v1(journal, m)
    &&& forall|w: int| 0 <= w < journal.writers@.len() ==> #[trigger] writer_custody_v1(journal, w)
    &&& forall|a: int, b: int| 0 <= a < b < journal.allocations@.len()
        && (#[trigger] journal.allocations@[a]).is_some() && (#[trigger] journal.allocations@[b]).is_some()
        ==> journal.allocations@[a].unwrap().key != journal.allocations@[b].unwrap().key
    &&& forall|w: int, v: int| 0 <= w < v < journal.writers@.len()
        && (#[trigger] journal.writers@[w]).is_some() && (#[trigger] journal.writers@[v]).is_some()
        ==> writer_key_v1(journal.writers@[w].unwrap()).local != writer_key_v1(journal.writers@[v].unwrap()).local
}

pub open spec fn selected_allocation_v1(journal: JournalContentsV1, chain: Seq<usize>, a: int) -> bool {
    journal.allocations@[a].is_some() && journal.allocations@[a].unwrap().pending_member.is_some()
        && chain.contains(journal.allocations@[a].unwrap().pending_member.unwrap())
}

pub open spec fn settled_allocation_v1(entry: AllocationEntryV1, success: bool) -> AllocationEntryV1 {
    AllocationEntryV1 { key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
        attempt_epoch: entry.attempt_epoch,
        content_lineage: if success { entry.attempt_epoch } else { entry.content_lineage },
        pending_member: None }
}

// Exact public-boundary contents under pending custody and successful preflight.
// Error preflight, physical storage, loop refinement and native premises are separate.
pub open spec fn settle_chain_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, success: bool) -> bool
{
    &&& retained_chain_v1(before, writer, chain)
    &&& match before.writers@[writer.slot as int] { Some(WriterEntryV1::Pending { .. }) => true, _ => false }
    &&& before.free@.len() < before.writer_capacity
    &&& before.member_free@.len() + chain.len() <= before.allocation_capacity
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
    &&& after.allocation_free@ == before.allocation_free@
    &&& after.scratch@ == before.scratch@
    &&& after.writers@ == before.writers@.update(writer.slot as int, None)
    &&& after.free@ == before.free@.push(writer.slot)
    &&& after.members@ == Seq::new(before.members@.len(), |m: int|
        if chain.contains(m as usize) { None } else { before.members@[m] })
    &&& after.member_free@ == before.member_free@ + chain
    &&& after.allocations@ == Seq::new(before.allocations@.len(), |a: int|
        if selected_allocation_v1(before, chain, a) {
            Some(settled_allocation_v1(before.allocations@[a].unwrap(), success))
        } else { before.allocations@[a] })
}

pub proof fn selected_member_v1(journal: JournalContentsV1, writer: WriterReferenceV1, chain: Seq<usize>, m: usize)
    requires pending_custody_v1(journal), retained_chain_v1(journal, writer, chain), chain.contains(m),
    ensures m < journal.members@.len(), journal.members@[m as int].is_some(),
        same_producer_v1(journal.members@[m as int].unwrap().writer, writer),
        selected_allocation_v1(journal, chain, journal.members@[m as int].unwrap().allocation.slot as int),
{
    let i = choose|i: int| 0 <= i < chain.len() && chain[i] == m;
    assert(chain[i] == m);
    assert(chain_link_v1(journal, writer, chain, i));
    reveal(chain_link_v1);
    assert(member_custody_v1(journal, m as int));
    reveal(member_custody_v1);
}

pub proof fn surviving_chain_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, success: bool,
    other: WriterReferenceV1, other_chain: Seq<usize>)
    requires pending_custody_v1(before), settle_chain_relation_v1(before, after, writer, chain, success),
        retained_chain_v1(before, other, other_chain), other.slot != writer.slot,
    ensures retained_chain_v1(after, other, other_chain),
{
    assert forall|i: int| 0 <= i < other_chain.len() implies !chain.contains(other_chain[i]) by {
        assert(chain_link_v1(before, other, other_chain, i));
        reveal(chain_link_v1);
        if chain.contains(other_chain[i]) {
            selected_member_v1(before, writer, chain, other_chain[i]);
        }
    }
    assert forall|i: int| 0 <= i < other_chain.len() implies
        after.members@[other_chain[i] as int] == before.members@[other_chain[i] as int] by {
        assert(chain_link_v1(before, other, other_chain, i));
        reveal(chain_link_v1);
    }
    assert forall|i: int| 0 <= i < other_chain.len() implies #[trigger] chain_link_v1(after, other, other_chain, i) by {
        assert(chain_link_v1(before, other, other_chain, i));
        reveal(chain_link_v1);
    }
}

pub proof fn settle_preserves_pending_custody_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, success: bool)
    requires pending_custody_v1(before), settle_chain_relation_v1(before, after, writer, chain, success),
    ensures pending_custody_v1(after),
{
    assert(!before.free@.contains(writer.slot));
    assert forall|i: int| 0 <= i < chain.len() implies !before.member_free@.contains(chain[i]) by {
        assert(chain_link_v1(before, writer, chain, i));
        reveal(chain_link_v1);
        assert(before.members@[chain[i] as int].is_some());
    }
    assert(after.member_free@.no_duplicates());
    assert forall|m: int| 0 <= m < after.members@.len() implies
        after.members@[m].is_none() == after.member_free@.contains(m as usize) by {
        producer_concat_contains_v1(before.member_free@, chain, m as usize);
        assert(after.member_free@.contains(m as usize)
            == (before.member_free@.contains(m as usize) || chain.contains(m as usize)));
    }
    assert forall|i: int| 0 <= i < after.member_free@.len() implies after.member_free@[i] < after.members@.len() by {
        if i < before.member_free@.len() { assert(after.member_free@[i] == before.member_free@[i]); }
        else {
            let index = i - before.member_free@.len();
            assert(chain_link_v1(before, writer, chain, index));
            reveal(chain_link_v1);
            assert(after.member_free@[i] == chain[index]);
        }
    }
    assert(slot_partition_v1(after.members@, after.member_free@));
    assert forall|s: int| 0 <= s < after.writers@.len() implies
        after.writers@[s].is_none() == after.free@.contains(s as usize) by {
        if s == writer.slot { assert(after.free@[before.free@.len() as int] == s); }
        else {
            if before.free@.contains(s as usize) {
                let i = choose|i: int| 0 <= i < before.free@.len() && before.free@[i] == s;
                assert(after.free@[i] == s);
            }
            if after.free@.contains(s as usize) {
                let i = choose|i: int| 0 <= i < after.free@.len() && after.free@[i] == s;
                assert(i < before.free@.len());
                assert(before.free@[i] == s);
            }
        }
    }
    assert(slot_partition_v1(after.writers@, after.free@));
    assert forall|a: int| 0 <= a < after.allocations@.len() implies {
        &&& after.allocations@[a].is_none() == before.allocations@[a].is_none()
        &&& after.allocations@[a].is_some() ==> after.allocations@[a].unwrap().key == before.allocations@[a].unwrap().key
    } by {}
    assert(slot_partition_v1(after.allocations@, after.allocation_free@));
    assert forall|a: int| 0 <= a < after.allocations@.len() implies #[trigger] allocation_custody_v1(after, a) by {
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
    }
    assert forall|m: int| 0 <= m < after.members@.len() implies #[trigger] member_custody_v1(after, m) by {
        assert(member_custody_v1(before, m));
        reveal(member_custody_v1);
        if after.members@[m].is_some() {
            let member = before.members@[m].unwrap();
            if member.writer.slot == writer.slot {
                assert(same_producer_v1(member.writer, writer));
                assert(chain.contains(m as usize));
            }
        }
    }
    assert forall|w: int| 0 <= w < after.writers@.len() implies #[trigger] writer_custody_v1(after, w) by {
        assert(writer_custody_v1(before, w));
        reveal(writer_custody_v1);
        if after.writers@[w].is_some() {
            let entry = before.writers@[w].unwrap();
            if !matches!(entry, WriterEntryV1::Reserved(_)) {
                let other = WriterReferenceV1 { slot: w as usize, key: writer_key_v1(entry) };
                let other_chain = choose|other_chain: Seq<usize>| #[trigger] retained_chain_v1(before, other, other_chain);
                surviving_chain_v1(before, after, writer, chain, success, other, other_chain);
            }
        }
    }
}

pub proof fn settled_producer_status_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, success: bool, request: ProducerReadV1)
    requires pending_custody_v1(before), settle_chain_relation_v1(before, after, writer, chain, success),
        producer_status_v1(before, request).is_some(),
    ensures producer_status_v1(after, request) ==
        if producer_status_v1(before, request) == Some(ProducerStatusV1::Pending)
            && same_producer_v1(request.producer, writer) {
            Some(if success { ProducerStatusV1::Success } else { ProducerStatusV1::NoEffect })
        } else { producer_status_v1(before, request) },
{
    let a = request.read.allocation.slot as int;
    let entry = before.allocations@[a].unwrap();
    if let Some(m) = entry.pending_member {
        assert(member_custody_v1(before, m as int));
        reveal(member_custody_v1);
        if chain.contains(m) { selected_member_v1(before, writer, chain, m); }
        if same_producer_v1(request.producer, writer) { assert(chain.contains(m)); }
        if request.producer.slot == writer.slot { assert(same_producer_v1(request.producer, writer)); }
    }
}

pub proof fn resolved_producer_storage_frame_v1(before: JournalContentsV1, after: JournalContentsV1, request: ProducerReadV1)
    requires producer_status_v1(before, request) == Some(ProducerStatusV1::Success)
        || producer_status_v1(before, request) == Some(ProducerStatusV1::NoEffect),
        after.context_generation == before.context_generation,
        after.allocations@.len() == before.allocations@.len(),
        after.allocations@[request.read.allocation.slot as int] == before.allocations@[request.read.allocation.slot as int],
    ensures producer_status_v1(after, request) == producer_status_v1(before, request),
{}

pub proof fn settlement_preserves_stable_readers_v1(before: ReadContentsV1, after: ReadContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, success: bool)
    requires reader_invariant_v1(before), pending_custody_v1(before.journal),
        settle_chain_relation_v1(before.journal, after.journal, writer, chain, success),
        reader_storage_frame_v1(before, after),
    ensures reader_invariant_v1(after),
{
    assert forall|a: int| 0 <= a < before.readers@.len() && before.readers@[a] > 0 implies
        after.journal.allocations@[a] == before.journal.allocations@[a] by {
        reader_count_consequences_v1(before, a as usize);
    }
    reader_allocation_frame_preserves_v1(before, after);
}

pub open spec fn unknown_writer_v1(entry: WriterEntryV1) -> WriterEntryV1 {
    match entry {
        WriterEntryV1::Pending { key, head, count } | WriterEntryV1::Unknown { key, head, count } =>
            WriterEntryV1::Unknown { key, head, count },
        WriterEntryV1::Reserved(_) => entry,
    }
}

// Successful update under pending_custody_v1; Rust's chain preflight is not refined here.
pub open spec fn mark_unknown_relation_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1) -> bool {
    &&& writer_retains_v1(before, writer)
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
    &&& after.allocations@ == before.allocations@
    &&& after.allocation_free@ == before.allocation_free@
    &&& after.members@ == before.members@
    &&& after.member_free@ == before.member_free@
    &&& after.scratch@ == before.scratch@
    &&& after.free@ == before.free@
    &&& after.writers@ == before.writers@.update(writer.slot as int,
        Some(unknown_writer_v1(before.writers@[writer.slot as int].unwrap())))
}

pub proof fn unknown_preserves_chain_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, other: WriterReferenceV1, chain: Seq<usize>)
    requires mark_unknown_relation_v1(before, after, writer), retained_chain_v1(before, other, chain),
    ensures retained_chain_v1(after, other, chain),
{
    assert forall|i: int| 0 <= i < chain.len() implies #[trigger] chain_link_v1(after, other, chain, i) by {
        assert(chain_link_v1(before, other, chain, i));
        reveal(chain_link_v1);
    }
}

pub proof fn unknown_preserves_pending_custody_v1(before: JournalContentsV1, after: JournalContentsV1, writer: WriterReferenceV1)
    requires pending_custody_v1(before), mark_unknown_relation_v1(before, after, writer),
    ensures pending_custody_v1(after),
{
    assert forall|a: int| 0 <= a < after.allocations@.len() implies #[trigger] allocation_custody_v1(after, a) by {
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
    }
    assert forall|m: int| 0 <= m < after.members@.len() implies #[trigger] member_custody_v1(after, m) by {
        assert(member_custody_v1(before, m));
        reveal(member_custody_v1);
    }
    assert forall|w: int| 0 <= w < after.writers@.len() implies #[trigger] writer_custody_v1(after, w) by {
        assert(writer_custody_v1(before, w));
        reveal(writer_custody_v1);
        if before.writers@[w].is_some() {
            let entry = before.writers@[w].unwrap();
            if !matches!(entry, WriterEntryV1::Reserved(_)) {
                let other = WriterReferenceV1 { slot: w as usize, key: writer_key_v1(entry) };
                let chain = choose|chain: Seq<usize>| #[trigger] retained_chain_v1(before, other, chain);
                unknown_preserves_chain_v1(before, after, writer, other, chain);
            }
        }
    }
}

pub proof fn unknown_producer_status_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, request: ProducerReadV1)
    requires producer_status_v1(before, request).is_some(), mark_unknown_relation_v1(before, after, writer),
    ensures producer_status_v1(after, request) ==
        if producer_status_v1(before, request) == Some(ProducerStatusV1::Pending)
            && same_producer_v1(request.producer, writer) { Some(ProducerStatusV1::Unknown) }
        else { producer_status_v1(before, request) },
{}

pub open spec fn producer_weight_v1(entry: Option<ProducerReservationV1>, allocation: usize) -> nat {
    if entry.is_some() && entry.unwrap().request.read.allocation.slot == allocation { 1 } else { 0 }
}

pub open spec fn producer_count_v1(entries: Seq<Option<ProducerReservationV1>>, allocation: usize) -> nat
    decreases entries.len(),
{
    if entries.len() == 0 { 0 }
    else { producer_count_v1(entries.drop_last(), allocation) + producer_weight_v1(entries.last(), allocation) }
}

pub proof fn producer_count_bound_v1(entries: Seq<Option<ProducerReservationV1>>, allocation: usize)
    ensures producer_count_v1(entries, allocation) <= entries.len(),
    decreases entries.len(),
{
    if entries.len() > 0 { producer_count_bound_v1(entries.drop_last(), allocation); }
}

pub proof fn producer_count_zero_v1(entries: Seq<Option<ProducerReservationV1>>, allocation: usize)
    ensures (producer_count_v1(entries, allocation) == 0) <==>
        (forall|s: int| 0 <= s < entries.len() ==> producer_weight_v1(#[trigger] entries[s], allocation) == 0),
    decreases entries.len(),
{
    if entries.len() > 0 {
        producer_count_zero_v1(entries.drop_last(), allocation);
        if producer_count_v1(entries, allocation) == 0 {
            assert forall|s: int| 0 <= s < entries.len() implies producer_weight_v1(#[trigger] entries[s], allocation) == 0 by {
                if s < entries.len() - 1 { assert(entries.drop_last()[s] == entries[s]); }
            }
        } else if forall|s: int| 0 <= s < entries.len() ==> producer_weight_v1(#[trigger] entries[s], allocation) == 0 {
            assert forall|s: int| 0 <= s < entries.drop_last().len() implies
                producer_weight_v1(#[trigger] entries.drop_last()[s], allocation) == 0 by {
                assert(entries[s] == entries.drop_last()[s]);
            }
            assert(producer_weight_v1(entries[entries.len() - 1], allocation) == 0);
        }
    }
}

#[verifier::opaque]
pub open spec fn producer_entry_valid_v1(journal: JournalContentsV1, entry: ProducerReservationV1, slot: int, next: u64) -> bool {
    &&& entry.reference.slot == slot
    &&& entry.reference.consumer.context_generation == journal.context_generation
    &&& issuable_id_v1(entry.reference.consumer.local)
    &&& entry.reference.consumer.kind == WriterKindV1::Submission
    &&& issuable_id_v1(entry.request.producer.key.local)
    &&& entry.request.producer.slot < journal.writers@.len()
    &&& entry.request.producer.key.local < entry.reference.consumer.local
    &&& 0 < entry.reference.incarnation < next
    &&& entry.request.read.allocation.slot < journal.allocations@.len()
    &&& producer_status_v1(journal, entry.request).is_some()
}

pub open spec fn producer_arena_v1(journal: JournalContentsV1, entries: Seq<Option<ProducerReservationV1>>,
    free: Seq<usize>, counts: Seq<usize>, next: u64) -> bool
{
    &&& 0 < entries.len() <= 1_048_576
    &&& counts.len() == journal.allocations@.len()
    &&& 0 < next
    &&& slot_partition_v1(entries, free)
    &&& forall|a: int| 0 <= a < counts.len() ==> counts[a] == producer_count_v1(entries, a as usize)
    &&& forall|s: int| 0 <= s < entries.len() && (#[trigger] entries[s]).is_some()
        ==> producer_entry_valid_v1(journal, entries[s].unwrap(), s, next)
    &&& forall|s: int, t: int| 0 <= s < t < entries.len()
        && (#[trigger] entries[s]).is_some() && (#[trigger] entries[t]).is_some()
        ==> entries[s].unwrap().reference.incarnation != entries[t].unwrap().reference.incarnation
}

pub open spec fn producer_invariant_v1(contents: ProducerReadContentsV1) -> bool {
    &&& pending_custody_v1(contents.stable.journal)
    &&& reader_invariant_v1(contents.stable)
    &&& producer_arena_v1(contents.stable.journal, contents.reservations@, contents.free@, contents.counts@, contents.next_incarnation)
    &&& contents.reservations@.len() == contents.stable.leases@.len()
    &&& contents.stable.leases@.len() - contents.stable.free_reads@.len()
        + contents.reservations@.len() - contents.free@.len() <= contents.reservations@.len()
}

pub open spec fn producer_storage_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1) -> bool {
    &&& reader_storage_frame_v1(before.stable, after.stable)
    &&& after.reservations@ == before.reservations@
    &&& after.free@ == before.free@
    &&& after.counts@ == before.counts@
    &&& after.next_incarnation == before.next_incarnation
}

pub proof fn settle_preserves_producer_invariant_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>, success: bool)
    requires producer_invariant_v1(before), producer_storage_frame_v1(before, after),
        settle_chain_relation_v1(before.stable.journal, after.stable.journal, writer, chain, success),
    ensures producer_invariant_v1(after),
{
    settle_preserves_pending_custody_v1(before.stable.journal, after.stable.journal, writer, chain, success);
    settlement_preserves_stable_readers_v1(before.stable, after.stable, writer, chain, success);
    assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some() implies
        producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
        let entry = before.reservations@[s].unwrap();
        assert(producer_entry_valid_v1(before.stable.journal, entry, s, before.next_incarnation));
        reveal(producer_entry_valid_v1);
        settled_producer_status_v1(before.stable.journal, after.stable.journal, writer, chain, success, entry.request);
    }
}

pub proof fn unknown_preserves_producer_invariant_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1)
    requires producer_invariant_v1(before), producer_storage_frame_v1(before, after),
        mark_unknown_relation_v1(before.stable.journal, after.stable.journal, writer),
    ensures producer_invariant_v1(after),
{
    unknown_preserves_pending_custody_v1(before.stable.journal, after.stable.journal, writer);
    reader_allocation_frame_preserves_v1(before.stable, after.stable);
    assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some() implies
        producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
        let entry = before.reservations@[s].unwrap();
        assert(producer_entry_valid_v1(before.stable.journal, entry, s, before.next_incarnation));
        reveal(producer_entry_valid_v1);
        unknown_producer_status_v1(before.stable.journal, after.stable.journal, writer, entry.request);
    }
}

// Accounting premise only; concrete mutator rejection and temporal framing are separate.
pub proof fn retained_producer_positive_count_v1(contents: ProducerReadContentsV1, slot: usize)
    requires producer_invariant_v1(contents), slot < contents.reservations@.len(), contents.reservations@[slot as int].is_some(),
    ensures contents.stable.readers@[contents.reservations@[slot as int].unwrap().request.read.allocation.slot as int]
        + contents.counts@[contents.reservations@[slot as int].unwrap().request.read.allocation.slot as int] > 0,
{
    let entry = contents.reservations@[slot as int].unwrap();
    assert(producer_entry_valid_v1(contents.stable.journal, entry, slot as int, contents.next_incarnation));
    reveal(producer_entry_valid_v1);
    producer_count_zero_v1(contents.reservations@, entry.request.read.allocation.slot);
    assert(producer_weight_v1(contents.reservations@[slot as int], entry.request.read.allocation.slot) == 1);
}

pub proof fn producer_empty_partition_v1<T>(capacity: usize)
    ensures slot_partition_v1(Seq::<Option<T>>::new(capacity as nat, |i: int| None), reverse_free_v1(capacity)),
{
    let free = reverse_free_v1(capacity);
    assert forall|s: int| 0 <= s < capacity implies #[trigger] free.contains(s as usize) by {
        assert(free[capacity - 1 - s] == s);
    }
}

pub proof fn producer_empty_custody_v1(journal: JournalContentsV1, context: u64, allocations: usize, writers: usize)
    requires constructor_admission_v1(context, allocations, writers).is_none(),
        constructor_contents_initialized_v1(journal, context, allocations, writers),
    ensures pending_custody_v1(journal),
{
    producer_empty_partition_v1::<AllocationEntryV1>(allocations);
    producer_empty_partition_v1::<MemberEntryV1>(allocations);
    producer_empty_partition_v1::<WriterEntryV1>(writers);
    reveal(allocation_custody_v1);
    reveal(member_custody_v1);
    reveal(writer_custody_v1);
}

pub open spec fn producer_constructor_relation_v1(context: u64, allocations: usize, writers: usize, reads: usize,
    result: Result<ProducerReadContentsV1, ReadErrorV1>) -> bool
{
    match reader_constructor_admission_v1(context, allocations, writers, reads) {
        Some(error) => result == Err(error),
        None => match result {
            Err(_) => false,
            Ok(contents) => {
                &&& reader_constructor_relation_v1(context, allocations, writers, reads, Ok(contents.stable))
                &&& contents.reservations@ == Seq::new(reads as nat, |i: int| None)
                &&& contents.free@ == reverse_free_v1(reads)
                &&& contents.counts@ == Seq::new(allocations as nat, |i: int| 0usize)
                &&& contents.next_incarnation == 1
            },
        },
    }
}

pub proof fn producer_constructor_invariant_v1(context: u64, allocations: usize, writers: usize, reads: usize, contents: ProducerReadContentsV1)
    requires producer_constructor_relation_v1(context, allocations, writers, reads, Ok(contents)),
    ensures producer_invariant_v1(contents),
{
    reader_constructor_invariant_v1(context, allocations, writers, reads, contents.stable);
    producer_empty_custody_v1(contents.stable.journal, context, allocations, writers);
    producer_empty_partition_v1::<ProducerReservationV1>(reads);
    assert forall|a: int| 0 <= a < contents.counts@.len() implies
        contents.counts@[a] == producer_count_v1(contents.reservations@, a as usize) by {
        producer_count_zero_v1(contents.reservations@, a as usize);
    }
}

// Logical constructor only: fallible Rust allocation and physical storage are not refined.
pub fn producer_constructor_exec_v1(context: u64, allocations: usize, writers: usize, reads: usize)
    -> (result: Result<ProducerReadContentsV1, ReadErrorV1>)
    ensures producer_constructor_relation_v1(context, allocations, writers, reads, result),
        match result { Ok(contents) => producer_invariant_v1(contents), Err(_) => true },
{
    let stable = match reader_constructor_exec_v1(context, allocations, writers, reads) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    let reservations = vacant_contents_exec_v1(reads);
    let free = free_contents_exec_v1(reads);
    let counts = zero_readers_exec_v1(allocations);
    let contents = ProducerReadContentsV1 { stable, reservations, free, counts, next_incarnation: 1 };
    proof { producer_constructor_invariant_v1(context, allocations, writers, reads, contents); }
    Ok(contents)
}

// Explicit fixture population proves nonvacuity, not production begin-write/acquire refinement.
pub fn producer_nonempty_witness_v1() -> (result: Option<ProducerReadContentsV1>)
    ensures result.is_some(), producer_invariant_v1(result.unwrap()),
        result.unwrap().counts@ == seq![2usize], result.unwrap().free@.len() == 0,
        result.unwrap().reservations@[0].is_some(), result.unwrap().reservations@[1].is_some(),
        producer_status_v1(result.unwrap().stable.journal, result.unwrap().reservations@[0].unwrap().request) == Some(ProducerStatusV1::Pending),
{
    let mut contents = match producer_constructor_exec_v1(7, 1, 1, 2) { Ok(value) => value, Err(_) => return None };
    let ghost empty = contents.stable;
    let key = AllocationKeyV1 { context_generation: 7, local: 1 };
    let device = DeviceKeyV1 { context_generation: 7, local: 2 };
    let allocation = AllocationReferenceV1 { slot: 0, key };
    let producer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let _allocation_slot = contents.stable.journal.allocation_free.pop();
    let _member_slot = contents.stable.journal.member_free.pop();
    let _writer_slot = contents.stable.journal.free.pop();
    contents.stable.journal.registration_watermark = 10;
    contents.stable.journal.allocations.set(0, Some(AllocationEntryV1 {
        key, device, byte_extent: 16, attempt_epoch: 1, content_lineage: 0, pending_member: Some(0),
    }));
    contents.stable.journal.members.set(0, Some(MemberEntryV1 {
        writer: producer, allocation, prior_lineage: 0, attempt_epoch: 1, next: None,
    }));
    contents.stable.journal.writers.set(0, Some(WriterEntryV1::Pending { key: producer.key, head: Some(0), count: 1 }));
    let request = ProducerReadV1 { read: AllocationReadV1 {
        allocation, device, byte_extent: 16, byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0,
    }, producer };
    let first = ProducerReservationV1 { request, reference: ProducerReadReferenceV1 {
        slot: 0, incarnation: 1, consumer: WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission },
    } };
    let second = ProducerReservationV1 { request, reference: ProducerReadReferenceV1 {
        slot: 1, incarnation: 2, consumer: WriterKeyV1 { context_generation: 7, local: 21, kind: WriterKindV1::Submission },
    } };
    contents.reservations.set(0, Some(first));
    contents.reservations.set(1, Some(second));
    contents.free.clear();
    contents.counts.set(0, 2);
    contents.next_incarnation = 3;
    proof {
        reader_allocation_frame_preserves_v1(empty, contents.stable);
        reveal(chain_link_v1);
        assert(chain_link_v1(contents.stable.journal, producer, seq![0usize], 0));
        assert forall|m: int| 0 <= m < contents.stable.journal.members@.len()
            && (#[trigger] contents.stable.journal.members@[m]).is_some()
            && same_producer_v1(contents.stable.journal.members@[m].unwrap().writer, producer)
            implies seq![0usize].contains(m as usize) by {
            assert(m == 0);
        }
        assert(retained_chain_v1(contents.stable.journal, producer, seq![0usize]));
        reveal(allocation_custody_v1);
        reveal(member_custody_v1);
        reveal(writer_custody_v1);
        assert(pending_custody_v1(contents.stable.journal));
        reveal(producer_entry_valid_v1);
        assert(contents.reservations@ =~= seq![Some(first), Some(second)]);
        reveal_with_fuel(producer_count_v1, 3);
        assert(producer_arena_v1(contents.stable.journal, contents.reservations@, contents.free@, contents.counts@, contents.next_incarnation));
    }
    Some(contents)
}

}

verus! {
pub open spec fn producer_mutation_prestate_v1(contents: ProducerReadContentsV1) -> bool { producer_invariant_v1(contents) }
pub fn producer_invariant_subject_v1(contents: &mut ProducerReadContentsV1)
    requires producer_mutation_prestate_v1(*old(contents)),
    ensures producer_invariant_v1(*final(contents)),
{
    if let Some(mut entry) = contents.reservations[0] { entry.request.read.attempt_epoch = 0; contents.reservations.set(0, Some(entry)); }
    return ();
}
}
