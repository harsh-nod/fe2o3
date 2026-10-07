// Runtime declarations shared by conditional and actual-journal proof roots.
use std::collections::{HashMap, HashSet};
use vstd::prelude::*;

include!("../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs");
include!("../../fe2o3-runtime/src/context/versions/live_validation_bodies.rs");

macro_rules! structural_eq {
    ($($name:ident),+ $(,)?) => { $(verus! {
        impl vstd::std_specs::cmp::PartialEqSpecImpl for $name {
            open spec fn obeys_eq_spec() -> bool { true }
            open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
        }
    })+ };
}

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RuntimeSubmissionIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct RuntimeAllocationIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeDeviceIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeStreamIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeEventIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeMemoryKindV1 { DeviceLocal, HostVisible }
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeAccessV1 { Read, Write, ReadWrite }
#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeMemoryRegionV1 {
    allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1, byte_offset: u64, byte_len: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct AllocationRecordV1 {
    backend_allocation: u64, device: RuntimeDeviceIdV1, kind: RuntimeMemoryKindV1,
    byte_len: u64, journal: Option<ContextAllocationReferenceV1>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextReadSourceV1 { region: RuntimeMemoryRegionV1, record: AllocationRecordV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ScalarPeerDependencyV1 {
    ordinal: usize, event: RuntimeEventIdV1, backend_event: u64,
    submission: RuntimeSubmissionIdV1, backend_submission: u64,
    stream: RuntimeStreamIdV1, device: RuntimeDeviceIdV1,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ProducerReadRequestV1 { Active(ContextProducerReadV1), Queued(ContextQueuedProducerReadV1) }
struct ProducerInputV1 {
    source: ContextReadSourceV1, dependency: ScalarPeerDependencyV1, request: ProducerReadRequestV1,
}

// All unread owner fields are opaque, not silently declared absent or Copy.
struct Launch<L> {
    dependencies_held: bool, dependencies: Vec<ScalarPeerDependencyV1>,
    sources: Vec<ContextReadSourceV1>, custody: L,
}
struct Peer<P, D> {
    directed: Option<D>, dependencies_held: bool, dependencies: Vec<ScalarPeerDependencyV1>,
    source: ContextReadSourceV1, custody: P,
}
struct Context<C, L, P, D> {
    producer_launches: HashMap<RuntimeSubmissionIdV1, Launch<L>>,
    scalar_peer_copies: HashMap<RuntimeSubmissionIdV1, Peer<P, D>>,
    allocations: HashMap<RuntimeAllocationIdV1, AllocationRecordV1>,
    backend_allocations: HashSet<u64>, custody: C,
}
struct Root<R> {
    inputs: Vec<ProducerInputV1>, requests: Vec<ContextProducerReadV1>,
    references: Vec<ContextProducerReadReferenceV1>, queued_requests: Vec<ContextQueuedProducerReadV1>,
    queued_references: Vec<ContextQueuedProducerReadReferenceV1>, custody: R,
}
struct Owner<C, L, P, D, R> { context: Context<C, L, P, D>, root: Root<R> }

// These are possible per-call returns, never a precomputed validation answer.
// One validate invocation reaches each helper at most once. Repeated invocations
// may use different returns, including a different credit observation.
#[derive(Clone, Copy)]
struct Returns {
    active_lookup: Result<ContextProducerReadV1, ContextVersionJournalErrorV1>,
    active_status: Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1>,
    queued_lookup: Result<ContextQueuedProducerReadV1, ContextVersionJournalErrorV1>,
    queued_status: Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1>,
    credit: bool, live: Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1>,
}
#[derive(Clone, Copy)]
enum Call {
    ActiveLookup(ContextProducerReadReferenceV1), ActiveStatus(ContextProducerReadReferenceV1),
    QueuedLookup(ContextQueuedProducerReadReferenceV1), QueuedStatus(ContextQueuedProducerReadReferenceV1),
    Credit(RuntimeAllocationIdV1, RuntimeDeviceIdV1, u64),
    Live(RuntimeAllocationIdV1, AllocationRecordV1),
}
}

// This is the same structural equality bridge used by other source-bound
// proofs. Rust derive lowering and vstd comparison contracts remain trusted.
structural_eq!(RuntimeSubmissionIdV1, RuntimeAllocationIdV1, RuntimeDeviceIdV1,
    RuntimeStreamIdV1, RuntimeEventIdV1, RuntimeMemoryKindV1, RuntimeAccessV1,
    RuntimeMemoryRegionV1, ContextWriterKindV1, ContextWriterKeyV1,
    ContextWriterReferenceV1, ContextAllocationKeyV1, ContextJournalDeviceKeyV1,
    ContextAllocationReferenceV1, ContextAllocationWriteV1, ContextAllocationReadV1,
    ContextProducerReadV1, ContextQueuedProducerReadV1, ContextProducerReadReferenceV1,
    ContextQueuedProducerReadReferenceV1, ContextProducerReadStatusV1,
    ContextVersionJournalErrorV1, AllocationRecordV1, ContextReadSourceV1,
    ScalarPeerDependencyV1, ProducerReadRequestV1);

// Preserve the production qualified name, without importing a second body.
mod fe2o3_runtime_model { pub(super) use super::ContextAllocationWriteV1; }
