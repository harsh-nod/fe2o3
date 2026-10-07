// Compared journal schemas for the conditional root only. The concrete root
// includes the actual runtime-model declarations instead.
verus! {
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextWriterKindV1 { Synchronous, Submission }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextWriterKeyV1 { context_generation: u64, local: u64, kind: ContextWriterKindV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextWriterReferenceV1 { slot: usize, key: ContextWriterKeyV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextAllocationKeyV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextJournalDeviceKeyV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextAllocationReferenceV1 { slot: usize, key: ContextAllocationKeyV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ContextAllocationWriteV1 {
    allocation: ContextAllocationReferenceV1, device: ContextJournalDeviceKeyV1, byte_extent: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextAllocationReadV1 {
    allocation: ContextAllocationReferenceV1, device: ContextJournalDeviceKeyV1,
    byte_extent: u64, byte_offset: u64, byte_len: u64, attempt_epoch: u64, content_lineage: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextProducerReadV1 { read: ContextAllocationReadV1, producer: ContextWriterReferenceV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextQueuedProducerReadV1 {
    allocation: ContextAllocationWriteV1, byte_offset: u64, byte_len: u64,
    producer: ContextWriterReferenceV1,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextProducerReadReferenceV1 { slot: usize, incarnation: u64, consumer: ContextWriterKeyV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextQueuedProducerReadReferenceV1 { slot: usize, incarnation: u64, consumer: ContextWriterKeyV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextProducerReadStatusV1 { Pending, Success, NoEffect, Unknown }
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextVersionJournalErrorV1 {
    InvalidContextGeneration, InvalidCapacity, StorageAllocationFailed, ForeignContext,
    InvalidWriterId, WriterReplay, WriterCapacity, InvalidReference, InvalidState,
    InvalidAllocationId, InvalidDeviceId, InvalidExtent, AllocationReplay, AllocationCapacity,
    InvalidAllocationReference, AllocationDeviceMismatch, AllocationExtentMismatch,
    AllocationBusy, RosterCapacity, NonCanonicalRoster, MemberCapacity, EpochExhausted,
    SettlementEvidenceMismatch,
}
struct ContextAllocationEnrollmentV1 {
    key: ContextAllocationKeyV1, device: ContextJournalDeviceKeyV1, byte_extent: u64,
}
}
