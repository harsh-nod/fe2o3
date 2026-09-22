// The same owner and value declaration tokens are compiled by Rust and Verus.
context_producer_read_declarations_v1! {
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextProducerReadV1 {
    pub read: ContextAllocationReadV1,
    pub producer: ContextWriterReferenceV1,
}

/// Descriptive identity; discarding it does not release consumer custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextProducerReadReferenceV1 {
    pub slot: usize,
    pub incarnation: u64,
    pub consumer: ContextWriterKeyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextProducerReadStatusV1 {
    Pending,
    Success,
    NoEffect,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReservationV1 {
    reference: ContextProducerReadReferenceV1,
    request: ContextProducerReadV1,
}

/// Owns the stable-reader journal without mutable extraction. The two arenas
/// each have `reads` slots, but share one total limit of `reads` live records.
/// Construction allocates O(allocations + writers + reads) metadata. Producer
/// acquisition/release is O(k), lookup O(1), and settlement needs no arena scan.
/// Existing stable-reader rules remain unchanged. This composition requires
/// separate verification from the stable-reader journal's original artifact.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{ContextProducerReadJournalV1, ContextReadLeasedJournalV1};
/// let mut journal = ContextProducerReadJournalV1::new(1, 4, 4, 4).unwrap();
/// let bypass: &mut ContextReadLeasedJournalV1 = &mut *journal;
/// ```
#[derive(Debug)]
pub struct ContextProducerReadJournalV1 {
    stable: ContextReadLeasedJournalV1,
    reservations: Vec<Option<ReservationV1>>,
    free: Vec<usize>,
    counts: Vec<usize>,
    next_incarnation: u64,
}
}
