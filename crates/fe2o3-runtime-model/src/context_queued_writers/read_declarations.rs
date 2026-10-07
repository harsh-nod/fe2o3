// The actual child-module declarations retain their original visibility.
context_queued_read_declarations_v1! {
/// An exact queued-producer binding, without a speculative epoch or lineage.
/// The adapter must authenticate producer dependencies and consumer quiescence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextQueuedProducerReadV1 {
    pub allocation: ContextAllocationWriteV1,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub producer: ContextWriterReferenceV1,
}

/// Descriptive identity; dropping this value does not release the lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextQueuedProducerReadReferenceV1 {
    pub slot: usize,
    pub incarnation: u64,
    pub consumer: ContextWriterKeyV1,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Reservation {
    reference: ContextQueuedProducerReadReferenceV1,
    request: ContextQueuedProducerReadV1,
    status: ContextProducerReadStatusV1,
    version: Option<(u64, u64)>,
    previous: Option<usize>,
    next: Option<usize>,
}

}
