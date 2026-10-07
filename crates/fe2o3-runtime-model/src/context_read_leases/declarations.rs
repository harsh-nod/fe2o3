// The same owner and value declaration tokens are compiled by Rust and Verus.
context_read_declarations_v1! {
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationReadV1 {
    pub allocation: ContextAllocationReferenceV1,
    pub device: ContextJournalDeviceKeyV1,
    pub byte_extent: u64,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub attempt_epoch: u64,
    pub content_lineage: u64,
}

/// Descriptive identity only. Dropping it never releases a retained reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextReadLeaseReferenceV1 {
    pub slot: usize,
    pub incarnation: u64,
    pub consumer: ContextWriterKeyV1,
}

/// An inert model premise; the caller must authenticate exact consumer quiescence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextReadQuiescenceEvidenceV1 {
    pub consumer: ContextWriterKeyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReadLeaseV1 {
    reference: ContextReadLeaseReferenceV1,
    request: ContextAllocationReadV1,
}

/// Owns the writer journal so its mutation/retirement operations cannot bypass
/// reader exclusion. Immutable inspection is available through `Deref`, but no
/// mutable journal access or extraction is exposed. Reader composition is not
/// covered by the original journal's issuance-only verification receipt.
/// Transitions preserve a unique free/occupied slot partition and exact
/// per-allocation reader counts from the constructor's valid initial state.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{ContextReadLeasedJournalV1, ContextVersionJournalV1};
/// let mut leased = ContextReadLeasedJournalV1::new(1, 4, 4, 4).unwrap();
/// let bypass: &mut ContextVersionJournalV1 = &mut *leased;
/// ```
#[derive(Debug)]
pub struct ContextReadLeasedJournalV1 {
    journal: ContextVersionJournalV1,
    leases: Vec<Option<ReadLeaseV1>>,
    free_reads: Vec<usize>,
    readers: Vec<usize>,
    next_incarnation: u64,
}
}
