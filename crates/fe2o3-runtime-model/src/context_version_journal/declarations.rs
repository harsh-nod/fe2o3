// The same declaration tokens are compiled by ordinary Rust and Verus.
context_journal_declarations_v1! {
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextWriterKindV1 {
    Synchronous,
    Submission,
}

/// Inert projection of an existing identity, not an identity-issuance API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextWriterKeyV1 {
    pub context_generation: u64,
    pub local: u64,
    pub kind: ContextWriterKindV1,
}

/// A descriptive slot reference; discarding it does not abort its reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use = "discarding a reference does not release its retained writer slot"]
pub struct ContextWriterReferenceV1 {
    pub slot: usize,
    pub key: ContextWriterKeyV1,
}

/// An inert projection of the complete existing allocation identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ContextAllocationKeyV1 {
    pub context_generation: u64,
    pub local: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextJournalDeviceKeyV1 {
    pub context_generation: u64,
    pub local: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationReferenceV1 {
    pub slot: usize,
    pub key: ContextAllocationKeyV1,
}

/// One canonical whole-allocation destination, not an aliased input view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationWriteV1 {
    pub allocation: ContextAllocationReferenceV1,
    pub device: ContextJournalDeviceKeyV1,
    pub byte_extent: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextWriterStateV1 {
    Reserved,
    Pending { member_count: usize },
    Unknown { member_count: usize },
}

/// Inert model premise, not authenticated runtime completion authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextWriterSuccessEvidenceV1 {
    pub writer: ContextWriterReferenceV1,
}

/// Inert model premise; an ordinary backend error does not authenticate it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextWriterNoEffectEvidenceV1 {
    pub writer: ContextWriterReferenceV1,
}

/// Inert premise that every exact member has been disposed. This is neither
/// native disposal authority nor successful content-settlement evidence.
#[derive(Debug)]
pub struct ContextWriterDisposalEvidenceV1<'a> {
    pub writer: ContextWriterReferenceV1,
    pub allocations: &'a [ContextAllocationWriteV1],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationStateV1 {
    pub device: ContextJournalDeviceKeyV1,
    pub byte_extent: u64,
    pub attempt_epoch: u64,
    pub content_lineage: u64,
    pub pending_writer: Option<ContextWriterReferenceV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriterEntryV1 {
    Reserved(ContextWriterKeyV1),
    Pending {
        key: ContextWriterKeyV1,
        head: Option<usize>,
        count: usize,
    },
    Unknown {
        key: ContextWriterKeyV1,
        head: Option<usize>,
        count: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationEntryV1 {
    key: ContextAllocationKeyV1,
    device: ContextJournalDeviceKeyV1,
    byte_extent: u64,
    attempt_epoch: u64,
    content_lineage: u64,
    pending_member: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MemberEntryV1 {
    writer: ContextWriterReferenceV1,
    allocation: ContextAllocationReferenceV1,
    prior_lineage: u64,
    attempt_epoch: u64,
    next: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BeginMemberPlanV1 {
    member_slot: usize,
    allocation: ContextAllocationReferenceV1,
    prior_lineage: u64,
    attempt_epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextVersionJournalErrorV1 {
    InvalidContextGeneration,
    InvalidCapacity,
    StorageAllocationFailed,
    ForeignContext,
    InvalidWriterId,
    WriterReplay,
    WriterCapacity,
    InvalidReference,
    InvalidState,
    InvalidAllocationId,
    InvalidDeviceId,
    InvalidExtent,
    AllocationReplay,
    AllocationCapacity,
    InvalidAllocationReference,
    AllocationDeviceMismatch,
    AllocationExtentMismatch,
    AllocationBusy,
    RosterCapacity,
    NonCanonicalRoster,
    MemberCapacity,
    EpochExhausted,
    SettlementEvidenceMismatch,
}

/// Construction is O(A + W), enrollment O(A), and canonical Begin O(k).
/// Writer issuance, exact lookup and pre-effect abort remain O(1).
/// Settlement and Unknown validation examine only the retained k members.
#[derive(Debug)]
pub struct ContextVersionJournalV1 {
    context_generation: u64,
    allocation_capacity: usize,
    writer_capacity: usize,
    registration_watermark: u64,
    reserved_count: usize,
    writers: Vec<Option<WriterEntryV1>>,
    free: Vec<usize>,
    allocations: Vec<Option<AllocationEntryV1>>,
    allocation_free: Vec<usize>,
    members: Vec<Option<MemberEntryV1>>,
    member_free: Vec<usize>,
    scratch: Vec<Option<BeginMemberPlanV1>>,
    #[cfg(test)]
    indexed_accesses: Cell<usize>,
}
}
