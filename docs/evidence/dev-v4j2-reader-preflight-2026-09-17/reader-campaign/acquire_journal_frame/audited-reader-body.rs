// V4-J2 development: concrete reader contents and ordered preflight only.
// Commit preservation, Rust correspondence, storage and native authority are separate.


use issuance::*;
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy)]
pub enum ReadErrorV1 {
    InvalidContextGeneration, InvalidCapacity, ForeignContext, InvalidWriterId,
    WriterReplay, WriterCapacity, InvalidReference, InvalidState,
    InvalidAllocationReference, AllocationDeviceMismatch, AllocationExtentMismatch,
    InvalidExtent, AllocationBusy, RosterCapacity, MemberCapacity, EpochExhausted,
    NonCanonicalRoster, SettlementEvidenceMismatch,
}

#[derive(Clone, Copy)]
pub struct AllocationReadV1 {
    pub allocation: AllocationReferenceV1,
    pub device: DeviceKeyV1,
    pub byte_extent: u64,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub attempt_epoch: u64,
    pub content_lineage: u64,
}

#[derive(Clone, Copy)]
pub struct ReadReferenceV1 {
    pub slot: usize,
    pub incarnation: u64,
    pub consumer: WriterKeyV1,
}

#[derive(Clone, Copy)]
pub struct ReadLeaseV1 {
    pub reference: ReadReferenceV1,
    pub request: AllocationReadV1,
}

pub struct ReadContentsV1 {
    pub journal: JournalContentsV1,
    pub leases: Vec<Option<ReadLeaseV1>>,
    pub free_reads: Vec<usize>,
    pub readers: Vec<usize>,
    pub next_incarnation: u64,
}

pub open spec fn exact_decision_v1<T>(actual: Result<T, ReadErrorV1>, expected: Result<T, ReadErrorV1>) -> bool {
    actual == expected
}

pub open spec fn lift_error_v1(error: JournalErrorV1) -> ReadErrorV1 {
    match error {
        JournalErrorV1::InvalidContextGeneration => ReadErrorV1::InvalidContextGeneration,
        JournalErrorV1::InvalidCapacity => ReadErrorV1::InvalidCapacity,
        JournalErrorV1::ForeignContext => ReadErrorV1::ForeignContext,
        JournalErrorV1::InvalidWriterId => ReadErrorV1::InvalidWriterId,
        JournalErrorV1::WriterReplay => ReadErrorV1::WriterReplay,
        JournalErrorV1::WriterCapacity => ReadErrorV1::WriterCapacity,
        JournalErrorV1::InvalidReference => ReadErrorV1::InvalidReference,
        JournalErrorV1::InvalidState => ReadErrorV1::InvalidState,
    }
}

pub fn lift_error_exec_v1(error: JournalErrorV1) -> (result: ReadErrorV1)
    ensures result == lift_error_v1(error),
{
    match error {
        JournalErrorV1::InvalidContextGeneration => ReadErrorV1::InvalidContextGeneration,
        JournalErrorV1::InvalidCapacity => ReadErrorV1::InvalidCapacity,
        JournalErrorV1::ForeignContext => ReadErrorV1::ForeignContext,
        JournalErrorV1::InvalidWriterId => ReadErrorV1::InvalidWriterId,
        JournalErrorV1::WriterReplay => ReadErrorV1::WriterReplay,
        JournalErrorV1::WriterCapacity => ReadErrorV1::WriterCapacity,
        JournalErrorV1::InvalidReference => ReadErrorV1::InvalidReference,
        JournalErrorV1::InvalidState => ReadErrorV1::InvalidState,
    }
}

pub open spec fn reader_constructor_admission_v1(
    context: u64, allocations: usize, writers: usize, reads: usize,
) -> Option<ReadErrorV1> {
    if reads == 0 || reads > 1_048_576 { Some(ReadErrorV1::InvalidCapacity) }
    else {
        match constructor_admission_v1(context, allocations, writers) {
            Some(error) => Some(lift_error_v1(error)),
            None => None,
        }
    }
}

pub open spec fn reader_constructor_relation_v1(
    context: u64, allocations: usize, writers: usize, reads: usize,
    result: Result<ReadContentsV1, ReadErrorV1>,
) -> bool {
    match reader_constructor_admission_v1(context, allocations, writers, reads) {
        Some(error) => result == Err(error),
        None => match result {
            Err(_) => false,
            Ok(contents) => {
                &&& constructor_contents_initialized_v1(contents.journal, context, allocations, writers)
                &&& contents.leases@ == Seq::new(reads as nat, |i: int| None)
                &&& contents.free_reads@ == reverse_free_v1(reads)
                &&& contents.readers@ == Seq::new(allocations as nat, |i: int| 0usize)
                &&& contents.next_incarnation == 1
            },
        },
    }
}

pub fn zero_readers_exec_v1(capacity: usize) -> (result: Vec<usize>)
    ensures result@ == Seq::new(capacity as nat, |i: int| 0usize),
{
    let mut result = Vec::new();
    let mut index = 0;
    while index < capacity
        invariant index <= capacity, result@ == Seq::new(index as nat, |i: int| 0usize),
        decreases capacity - index,
    {
        result.push(0);
        index += 1;
        proof { assert(result@ =~= Seq::new(index as nat, |i: int| 0usize)); }
    }
    result
}

pub fn reader_constructor_exec_v1(
    context: u64, allocations: usize, writers: usize, reads: usize,
) -> (result: Result<ReadContentsV1, ReadErrorV1>)
    ensures reader_constructor_relation_v1(context, allocations, writers, reads, result),
{
    if reads == 0 || reads > 1_048_576 { return Err(ReadErrorV1::InvalidCapacity); }
    let journal = match constructor_contents_exec_v1(context, allocations, writers) {
        Ok(value) => value,
        Err(error) => return Err(lift_error_exec_v1(error)),
    };
    let leases = vacant_contents_exec_v1(reads);
    let free_reads = free_contents_exec_v1(reads);
    let readers = zero_readers_exec_v1(allocations);
    Ok(ReadContentsV1 { journal, leases, free_reads, readers, next_incarnation: 1 })
}

pub open spec fn same_allocation_v1(left: AllocationReferenceV1, right: AllocationReferenceV1) -> bool {
    left.slot == right.slot && left.key.context_generation == right.key.context_generation
        && left.key.local == right.key.local
}

pub fn same_allocation_exec_v1(left: AllocationReferenceV1, right: AllocationReferenceV1) -> (result: bool)
    ensures result == same_allocation_v1(left, right),
{
    left.slot == right.slot && left.key.context_generation == right.key.context_generation
        && left.key.local == right.key.local
}

pub open spec fn same_read_reference_v1(left: ReadReferenceV1, right: ReadReferenceV1) -> bool {
    left.slot == right.slot && left.incarnation == right.incarnation
        && same_key_v1(left.consumer, right.consumer)
}

pub fn same_read_reference_exec_v1(left: ReadReferenceV1, right: ReadReferenceV1) -> (result: bool)
    ensures result == same_read_reference_v1(left, right),
{
    left.slot == right.slot && left.incarnation == right.incarnation
        && same_key_exec_v1(left.consumer, right.consumer)
}

// Returns the exact allocation only after checking the production pending backlink.
pub open spec fn allocation_decision_v1(
    journal: JournalContentsV1, reference: AllocationReferenceV1,
) -> Result<AllocationEntryV1, ReadErrorV1> {
    if reference.slot >= journal.allocations@.len() { Err(ReadErrorV1::InvalidAllocationReference) }
    else { match journal.allocations@[reference.slot as int] {
        None => Err(ReadErrorV1::InvalidAllocationReference),
        Some(entry) => {
            if entry.key.context_generation != reference.key.context_generation
                || entry.key.local != reference.key.local
                || entry.key.context_generation != journal.context_generation {
                Err(ReadErrorV1::InvalidAllocationReference)
            } else { match entry.pending_member {
                None => Ok(entry),
                Some(slot) => {
                    if slot >= journal.members@.len() { Err(ReadErrorV1::InvalidState) }
                    else { match journal.members@[slot as int] {
                        Some(member) => if same_allocation_v1(member.allocation, reference) {
                            Ok(entry)
                        } else { Err(ReadErrorV1::InvalidState) },
                        None => Err(ReadErrorV1::InvalidState),
                    } }
                },
            } }
        },
    } }
}

pub fn allocation_lookup_exec_v1(journal: &JournalContentsV1, reference: AllocationReferenceV1)
    -> (result: Result<AllocationEntryV1, ReadErrorV1>)
    ensures exact_decision_v1(result, allocation_decision_v1(*journal, reference)),
{
    if reference.slot >= journal.allocations.len() { return Err(ReadErrorV1::InvalidAllocationReference); }
    let entry = match journal.allocations[reference.slot] {
        Some(entry) => entry,
        None => return Err(ReadErrorV1::InvalidAllocationReference),
    };
    if entry.key.context_generation != reference.key.context_generation
        || entry.key.local != reference.key.local
        || entry.key.context_generation != journal.context_generation {
        return Err(ReadErrorV1::InvalidAllocationReference);
    }
    if let Some(slot) = entry.pending_member {
        if slot >= journal.members.len() { return Err(ReadErrorV1::InvalidState); }
        match journal.members[slot] {
            Some(member) => {
                if !same_allocation_exec_v1(member.allocation, reference) { return Err(ReadErrorV1::InvalidState); }
            },
            None => return Err(ReadErrorV1::InvalidState),
        }
    }
    Ok(entry)
}

pub open spec fn read_decision_v1(journal: JournalContentsV1, request: AllocationReadV1)
    -> Result<(), ReadErrorV1>
{
    match allocation_decision_v1(journal, request.allocation) {
        Err(error) => Err(error),
        Ok(entry) => {
            if entry.device.context_generation != request.device.context_generation
                || entry.device.local != request.device.local { Err(ReadErrorV1::AllocationDeviceMismatch) }
            else if entry.byte_extent != request.byte_extent { Err(ReadErrorV1::AllocationExtentMismatch) }
            else if request.byte_len == 0 || request.byte_offset + request.byte_len > u64::MAX
                || request.byte_offset + request.byte_len > request.byte_extent { Err(ReadErrorV1::InvalidExtent) }
            else if entry.pending_member.is_some() { Err(ReadErrorV1::AllocationBusy) }
            else if entry.attempt_epoch != request.attempt_epoch
                || entry.content_lineage != request.content_lineage { Err(ReadErrorV1::InvalidState) }
            else { Ok(()) }
        },
    }
}

pub fn validate_read_exec_v1(journal: &JournalContentsV1, request: AllocationReadV1)
    -> (result: Result<(), ReadErrorV1>)
    ensures exact_decision_v1(result, read_decision_v1(*journal, request)),
{
    let entry = match allocation_lookup_exec_v1(journal, request.allocation) {
        Ok(entry) => entry,
        Err(error) => return Err(error),
    };
    if entry.device.context_generation != request.device.context_generation
        || entry.device.local != request.device.local { return Err(ReadErrorV1::AllocationDeviceMismatch); }
    if entry.byte_extent != request.byte_extent { return Err(ReadErrorV1::AllocationExtentMismatch); }
    if request.byte_len == 0 { return Err(ReadErrorV1::InvalidExtent); }
    let end = match request.byte_offset.checked_add(request.byte_len) {
        Some(value) => value,
        None => return Err(ReadErrorV1::InvalidExtent),
    };
    if end > request.byte_extent { return Err(ReadErrorV1::InvalidExtent); }
    if entry.pending_member.is_some() { return Err(ReadErrorV1::AllocationBusy); }
    if entry.attempt_epoch != request.attempt_epoch
        || entry.content_lineage != request.content_lineage { return Err(ReadErrorV1::InvalidState); }
    Ok(())
}

pub open spec fn lease_decision_v1(contents: ReadContentsV1, reference: ReadReferenceV1)
    -> Result<AllocationReadV1, ReadErrorV1>
{
    if reference.slot >= contents.leases@.len() { Err(ReadErrorV1::InvalidReference) }
    else { match contents.leases@[reference.slot as int] {
        None => Err(ReadErrorV1::InvalidReference),
        Some(entry) => {
            if !same_read_reference_v1(entry.reference, reference) { Err(ReadErrorV1::InvalidReference) }
            else { match read_decision_v1(contents.journal, entry.request) {
                Err(error) => Err(error),
                Ok(()) => Ok(entry.request),
            } }
        },
    } }
}

pub fn lease_lookup_exec_v1(contents: &ReadContentsV1, reference: ReadReferenceV1)
    -> (result: Result<AllocationReadV1, ReadErrorV1>)
    ensures exact_decision_v1(result, lease_decision_v1(*contents, reference)),
{
    if reference.slot >= contents.leases.len() { return Err(ReadErrorV1::InvalidReference); }
    let entry = match contents.leases[reference.slot] {
        Some(value) => value,
        None => return Err(ReadErrorV1::InvalidReference),
    };
    if !same_read_reference_exec_v1(entry.reference, reference) { return Err(ReadErrorV1::InvalidReference); }
    match validate_read_exec_v1(&contents.journal, entry.request) {
        Err(error) => Err(error),
        Ok(()) => Ok(entry.request),
    }
}

pub open spec fn capacity_decision_v1(contents: ReadContentsV1, count: usize) -> Result<(), ReadErrorV1> {
    if count > contents.free_reads@.len() { Err(ReadErrorV1::MemberCapacity) }
    else if contents.next_incarnation == 0 || contents.next_incarnation + count > u64::MAX {
        Err(ReadErrorV1::EpochExhausted)
    } else { Ok(()) }
}

pub fn validate_capacity_exec_v1(contents: &ReadContentsV1, count: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires count <= u64::MAX,
    ensures exact_decision_v1(result, capacity_decision_v1(*contents, count)),
{
    if count > contents.free_reads.len() { return Err(ReadErrorV1::MemberCapacity); }
    if contents.next_incarnation == 0 || contents.next_incarnation.checked_add(count as u64).is_none() {
        return Err(ReadErrorV1::EpochExhausted);
    }
    Ok(())
}

pub type ReadOrderV1 = (u64, u64, u64, u64);

#[derive(Clone, Copy)]
pub struct ReadScanV1 { pub previous: Option<ReadOrderV1>, pub group: usize }

pub open spec fn read_order_v1(request: AllocationReadV1, incarnation: u64) -> ReadOrderV1 {
    (request.allocation.key.local, request.byte_offset, request.byte_len, incarnation)
}

pub open spec fn order_lt_v1(left: ReadOrderV1, right: ReadOrderV1) -> bool {
    left.0 < right.0 || (left.0 == right.0 && (left.1 < right.1
        || (left.1 == right.1 && (left.2 < right.2 || (left.2 == right.2 && left.3 < right.3)))))
}

pub fn order_lt_exec_v1(left: ReadOrderV1, right: ReadOrderV1) -> (result: bool)
    ensures result == order_lt_v1(left, right),
{
    left.0 < right.0 || (left.0 == right.0 && (left.1 < right.1
        || (left.1 == right.1 && (left.2 < right.2 || (left.2 == right.2 && left.3 < right.3)))))
}

pub open spec fn next_group_v1(previous: Option<ReadOrderV1>, group: usize, key: ReadOrderV1) -> usize {
    if previous.is_some() && previous.unwrap().0 == key.0 { (group + 1) as usize } else { 1 }
}

pub fn next_group_exec_v1(previous: Option<ReadOrderV1>, group: usize, key: ReadOrderV1) -> (result: usize)
    requires group < usize::MAX,
    ensures result == next_group_v1(previous, group, key), result <= group + 1,
{
    if let Some(prior) = previous {
        if prior.0 == key.0 { return group + 1; }
    }
    1
}

pub fn output_vacant_exec_v1(output: &[Option<ReadReferenceV1>]) -> (result: bool)
    ensures result == (forall|i: int| 0 <= i < output@.len() ==> output@[i].is_none()),
{
    let mut index = 0;
    while index < output.len()
        invariant index <= output.len(), forall|i: int| 0 <= i < index ==> output@[i].is_none(),
        decreases output.len() - index,
    {
        if output[index].is_some() { return false; }
        index += 1;
    }
    true
}

pub open spec fn acquire_header_v1(
    contents: ReadContentsV1, consumer: WriterKeyV1, count: usize, output: Seq<Option<ReadReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    if consumer.context_generation != contents.journal.context_generation { Err(ReadErrorV1::ForeignContext) }
    else if !issuable_id_v1(consumer.local) { Err(ReadErrorV1::InvalidWriterId) }
    else if count == 0 || count != output.len() { Err(ReadErrorV1::RosterCapacity) }
    else if exists|i: int| 0 <= i < output.len() && output[i].is_some() { Err(ReadErrorV1::InvalidState) }
    else { capacity_decision_v1(contents, count) }
}

pub fn acquire_header_exec_v1(
    contents: &ReadContentsV1, consumer: WriterKeyV1, count: usize, output: &[Option<ReadReferenceV1>],
) -> (result: Result<(), ReadErrorV1>)
    requires count <= u64::MAX,
    ensures exact_decision_v1(result, acquire_header_v1(*contents, consumer, count, output@)),
{
    if consumer.context_generation != contents.journal.context_generation { return Err(ReadErrorV1::ForeignContext); }
    if !issuable_id_exec_v1(consumer.local) { return Err(ReadErrorV1::InvalidWriterId); }
    if count == 0 || count != output.len() { return Err(ReadErrorV1::RosterCapacity); }
    if !output_vacant_exec_v1(output) { return Err(ReadErrorV1::InvalidState); }
    validate_capacity_exec_v1(contents, count)
}

pub open spec fn acquire_item_v1(
    contents: ReadContentsV1, request: AllocationReadV1, index: usize, state: ReadScanV1,
) -> Result<ReadScanV1, ReadErrorV1> {
    match read_decision_v1(contents.journal, request) {
        Err(error) => Err(error),
        Ok(()) => {
            let key = read_order_v1(request, 0);
            if state.previous.is_some() && !order_lt_v1(state.previous.unwrap(), key) {
                Err(ReadErrorV1::NonCanonicalRoster)
            } else {
                let group = next_group_v1(state.previous, state.group, key);
                let count = contents.readers@[request.allocation.slot as int] + group;
                if count > usize::MAX || count > contents.leases@.len() { Err(ReadErrorV1::InvalidState) }
                else {
                    let slot = contents.free_reads@[contents.free_reads@.len() - index - 1];
                    if slot >= contents.leases@.len() || contents.leases@[slot as int].is_some() {
                        Err(ReadErrorV1::InvalidState)
                    } else { Ok(ReadScanV1 { previous: Some(key), group }) }
                }
            }
        },
    }
}

pub fn acquire_item_exec_v1(
    contents: &ReadContentsV1, request: AllocationReadV1, index: usize, state: ReadScanV1,
) -> (result: Result<ReadScanV1, ReadErrorV1>)
    requires contents.readers@.len() == contents.journal.allocations@.len(),
        index < contents.free_reads@.len(), state.group < usize::MAX,
    ensures exact_decision_v1(result, acquire_item_v1(*contents, request, index, state)),
        match result { Ok(next) => next.group <= state.group + 1, Err(_) => true },
{
    match validate_read_exec_v1(&contents.journal, request) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    let key = (request.allocation.key.local, request.byte_offset, request.byte_len, 0);
    if let Some(prior) = state.previous {
        if !order_lt_exec_v1(prior, key) { return Err(ReadErrorV1::NonCanonicalRoster); }
    }
    let group = next_group_exec_v1(state.previous, state.group, key);
    let count = match contents.readers[request.allocation.slot].checked_add(group) {
        Some(value) => value, None => return Err(ReadErrorV1::InvalidState),
    };
    if count > contents.leases.len() { return Err(ReadErrorV1::InvalidState); }
    let slot = contents.free_reads[contents.free_reads.len() - index - 1];
    if slot >= contents.leases.len() { return Err(ReadErrorV1::InvalidState); }
    if contents.leases[slot].is_some() { return Err(ReadErrorV1::InvalidState); }
    Ok(ReadScanV1 { previous: Some(key), group })
}

// A left-to-right first-error scan, not a conjunction of unordered validity checks.
pub open spec fn acquire_scan_v1(
    contents: ReadContentsV1, requests: Seq<AllocationReadV1>, index: nat, state: ReadScanV1,
) -> Result<(), ReadErrorV1>
    decreases requests.len() - index,
{
    if index >= requests.len() { Ok(()) }
    else { match acquire_item_v1(contents, requests[index as int], index as usize, state) {
        Err(error) => Err(error),
        Ok(next) => acquire_scan_v1(contents, requests, index + 1, next),
    } }
}

pub open spec fn acquire_decision_v1(
    contents: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>,
    output: Seq<Option<ReadReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    match acquire_header_v1(contents, consumer, requests.len() as usize, output) {
        Err(error) => Err(error),
        Ok(()) => acquire_scan_v1(contents, requests, 0, ReadScanV1 { previous: None, group: 0 }),
    }
}

pub fn acquire_preflight_exec_v1(
    contents: &ReadContentsV1, consumer: WriterKeyV1, requests: &[AllocationReadV1],
    output: &[Option<ReadReferenceV1>],
) -> (result: Result<(), ReadErrorV1>)
    requires contents.readers@.len() == contents.journal.allocations@.len(), requests@.len() <= u64::MAX,
    ensures exact_decision_v1(result, acquire_decision_v1(*contents, consumer, requests@, output@)),
{
    match acquire_header_exec_v1(contents, consumer, requests.len(), output) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    let mut index = 0;
    let mut state = ReadScanV1 { previous: None, group: 0 };
    while index < requests.len()
        invariant index <= requests.len(), state.group <= index,
            requests@.len() <= contents.free_reads@.len(),
            contents.readers@.len() == contents.journal.allocations@.len(),
            acquire_decision_v1(*contents, consumer, requests@, output@)
                == acquire_scan_v1(*contents, requests@, index as nat, state),
        decreases requests.len() - index,
    {
        match acquire_item_exec_v1(contents, requests[index], index, state) {
            Err(error) => return Err(error),
            Ok(next) => state = next,
        }
        index += 1;
    }
    Ok(())
}

pub open spec fn release_header_v1(
    contents: ReadContentsV1, consumer: WriterKeyV1, evidence_consumer: WriterKeyV1,
    count: usize, observed_free_capacity: usize,
) -> Result<(), ReadErrorV1> {
    if !same_key_v1(evidence_consumer, consumer) { Err(ReadErrorV1::SettlementEvidenceMismatch) }
    else if count == 0 { Err(ReadErrorV1::RosterCapacity) }
    else if contents.free_reads@.len() + count > usize::MAX
        || contents.free_reads@.len() + count > contents.leases@.len()
        || contents.free_reads@.len() + count > observed_free_capacity { Err(ReadErrorV1::InvalidState) }
    else { Ok(()) }
}

pub fn release_header_exec_v1(
    contents: &ReadContentsV1, consumer: WriterKeyV1, evidence_consumer: WriterKeyV1,
    count: usize, observed_free_capacity: usize,
) -> (result: Result<(), ReadErrorV1>)
    ensures exact_decision_v1(result, release_header_v1(*contents, consumer, evidence_consumer, count, observed_free_capacity)),
{
    if !same_key_exec_v1(evidence_consumer, consumer) { return Err(ReadErrorV1::SettlementEvidenceMismatch); }
    if count == 0 { return Err(ReadErrorV1::RosterCapacity); }
    let count = match contents.free_reads.len().checked_add(count) {
        Some(value) => value, None => return Err(ReadErrorV1::InvalidState),
    };
    if count > contents.leases.len() || count > observed_free_capacity { return Err(ReadErrorV1::InvalidState); }
    Ok(())
}

pub open spec fn release_item_v1(
    contents: ReadContentsV1, consumer: WriterKeyV1, reference: ReadReferenceV1, state: ReadScanV1,
) -> Result<ReadScanV1, ReadErrorV1> {
    if !same_key_v1(reference.consumer, consumer) { Err(ReadErrorV1::InvalidReference) }
    else { match lease_decision_v1(contents, reference) {
        Err(error) => Err(error),
        Ok(request) => {
            let key = read_order_v1(request, reference.incarnation);
            if state.previous.is_some() && !order_lt_v1(state.previous.unwrap(), key) {
                Err(ReadErrorV1::NonCanonicalRoster)
            } else {
                let group = next_group_v1(state.previous, state.group, key);
                if contents.readers@[request.allocation.slot as int] < group { Err(ReadErrorV1::InvalidState) }
                else { Ok(ReadScanV1 { previous: Some(key), group }) }
            }
        },
    } }
}

pub fn release_item_exec_v1(
    contents: &ReadContentsV1, consumer: WriterKeyV1, reference: ReadReferenceV1, state: ReadScanV1,
) -> (result: Result<ReadScanV1, ReadErrorV1>)
    requires contents.readers@.len() == contents.journal.allocations@.len(), state.group < usize::MAX,
    ensures exact_decision_v1(result, release_item_v1(*contents, consumer, reference, state)),
        match result { Ok(next) => next.group <= state.group + 1, Err(_) => true },
{
    if !same_key_exec_v1(reference.consumer, consumer) { return Err(ReadErrorV1::InvalidReference); }
    let request = match lease_lookup_exec_v1(contents, reference) {
        Err(error) => return Err(error), Ok(request) => request,
    };
    let key = (request.allocation.key.local, request.byte_offset, request.byte_len, reference.incarnation);
    if let Some(prior) = state.previous {
        if !order_lt_exec_v1(prior, key) { return Err(ReadErrorV1::NonCanonicalRoster); }
    }
    let group = next_group_exec_v1(state.previous, state.group, key);
    if contents.readers[request.allocation.slot] < group { return Err(ReadErrorV1::InvalidState); }
    Ok(ReadScanV1 { previous: Some(key), group })
}

pub open spec fn release_scan_v1(
    contents: ReadContentsV1, consumer: WriterKeyV1, references: Seq<ReadReferenceV1>, index: nat, state: ReadScanV1,
) -> Result<(), ReadErrorV1>
    decreases references.len() - index,
{
    if index >= references.len() { Ok(()) }
    else { match release_item_v1(contents, consumer, references[index as int], state) {
        Err(error) => Err(error),
        Ok(next) => release_scan_v1(contents, consumer, references, index + 1, next),
    } }
}

pub open spec fn release_decision_v1(
    contents: ReadContentsV1, consumer: WriterKeyV1, references: Seq<ReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
) -> Result<(), ReadErrorV1> {
    match release_header_v1(contents, consumer, evidence_consumer, references.len() as usize, observed_free_capacity) {
        Err(error) => Err(error),
        Ok(()) => release_scan_v1(contents, consumer, references, 0, ReadScanV1 { previous: None, group: 0 }),
    }
}

pub fn release_preflight_exec_v1(
    contents: &ReadContentsV1, consumer: WriterKeyV1, references: &[ReadReferenceV1],
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
) -> (result: Result<(), ReadErrorV1>)
    requires contents.readers@.len() == contents.journal.allocations@.len(),
    ensures exact_decision_v1(result, release_decision_v1(*contents, consumer, references@, evidence_consumer, observed_free_capacity)),
{
    match release_header_exec_v1(contents, consumer, evidence_consumer, references.len(), observed_free_capacity) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    let mut index = 0;
    let mut state = ReadScanV1 { previous: None, group: 0 };
    while index < references.len()
        invariant index <= references.len(), state.group <= index,
            contents.readers@.len() == contents.journal.allocations@.len(),
            release_decision_v1(*contents, consumer, references@, evidence_consumer, observed_free_capacity)
                == release_scan_v1(*contents, consumer, references@, index as nat, state),
        decreases references.len() - index,
    {
        match release_item_exec_v1(contents, consumer, references[index], state) {
            Err(error) => return Err(error),
            Ok(next) => state = next,
        }
        index += 1;
    }
    Ok(())
}

pub open spec fn unread_scan_v1(
    contents: ReadContentsV1, references: Seq<AllocationReferenceV1>, index: nat,
) -> Result<(), ReadErrorV1>
    decreases references.len() - index,
{
    if index >= references.len() { Ok(()) }
    else { match allocation_decision_v1(contents.journal, references[index as int]) {
        Err(error) => Err(error),
        Ok(_) => {
            if contents.readers@[references[index as int].slot as int] != 0 { Err(ReadErrorV1::AllocationBusy) }
            else { unread_scan_v1(contents, references, index + 1) }
        },
    } }
}

pub fn require_unread_exec_v1(contents: &ReadContentsV1, references: &[AllocationReferenceV1])
    -> (result: Result<(), ReadErrorV1>)
    requires contents.readers@.len() == contents.journal.allocations@.len(),
    ensures exact_decision_v1(result, unread_scan_v1(*contents, references@, 0)),
{
    let mut index = 0;
    while index < references.len()
        invariant index <= references.len(),
            contents.readers@.len() == contents.journal.allocations@.len(),
            unread_scan_v1(*contents, references@, 0) == unread_scan_v1(*contents, references@, index as nat),
        decreases references.len() - index,
    {
        let reference = references[index];
        match allocation_lookup_exec_v1(&contents.journal, reference) {
            Err(error) => return Err(error), Ok(_) => {},
        }
        if contents.readers[reference.slot] != 0 { return Err(ReadErrorV1::AllocationBusy); }
        index += 1;
    }
    Ok(())
}

// Project all actual fields, including the base journal's issuance state.
pub open spec fn reader_contents_frame_v1(before: ReadContentsV1, after: ReadContentsV1) -> bool {
    &&& issuance_contents_frame_v1(before.journal, after.journal)
    &&& after.journal.registration_watermark == before.journal.registration_watermark
    &&& after.journal.reserved_count == before.journal.reserved_count
    &&& after.journal.writers@ == before.journal.writers@
    &&& after.journal.free@ == before.journal.free@
    &&& after.leases@ == before.leases@
    &&& after.free_reads@ == before.free_reads@
    &&& after.readers@ == before.readers@
    &&& after.next_incarnation == before.next_incarnation
}

pub open spec fn acquire_preflight_relation_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>,
    output_before: Seq<Option<ReadReferenceV1>>, output_after: Seq<Option<ReadReferenceV1>>,
    result: Result<(), ReadErrorV1>,
) -> bool {
    reader_contents_frame_v1(before, after) && output_after == output_before
        && result == acquire_decision_v1(before, consumer, requests, output_before)
}

pub fn acquire_preflight_contents_exec_v1(
    contents: &mut ReadContentsV1, consumer: WriterKeyV1, requests: &[AllocationReadV1],
    output: &mut Vec<Option<ReadReferenceV1>>,
) -> (result: Result<(), ReadErrorV1>)
    requires old(contents).readers@.len() == old(contents).journal.allocations@.len(), requests@.len() <= u64::MAX,
    ensures acquire_preflight_relation_v1(*old(contents), *final(contents), consumer, requests@,
        old(output)@, final(output)@, result),
{
    let result = acquire_preflight_exec_v1(contents, consumer, requests, output.as_slice());
    contents.journal.registration_watermark = 0;
    result
}

pub open spec fn release_preflight_relation_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1, references: Seq<ReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize, result: Result<(), ReadErrorV1>,
) -> bool {
    reader_contents_frame_v1(before, after)
        && result == release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity)
}

pub fn release_preflight_contents_exec_v1(
    contents: &mut ReadContentsV1, consumer: WriterKeyV1, references: &[ReadReferenceV1],
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
) -> (result: Result<(), ReadErrorV1>)
    requires old(contents).readers@.len() == old(contents).journal.allocations@.len(),
    ensures release_preflight_relation_v1(*old(contents), *final(contents), consumer, references@,
        evidence_consumer, observed_free_capacity, result),
{
    release_preflight_exec_v1(contents, consumer, references, evidence_consumer, observed_free_capacity)
}

}
