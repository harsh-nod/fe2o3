// Conditional refinement of the actual per-input production body. Every helper
// answer below describes one reached call, not a frozen journal or account.
// The five native forwarding methods, lazy credit method and complete compared
// value schemas are source-calibrated separately. Their native semantics are
// not established by this theorem. Local immutable storage is framed; shared
// Arc interiors, locking, allocation, unwinding and native freshness are not.
// Membership scans use the actual bounded slice bodies below. Pinned slice
// indexing and structural comparison contracts remain library/compiler trust.
use std::collections::{HashMap, HashSet};
use vstd::prelude::*;

include!("../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs");

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
struct Observations<'a, C, L, P, D, R> {
    owner: &'a Owner<C, L, P, D, R>, returns: Returns, calls: Ghost<Seq<Call>>,
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
mod fe2o3_runtime_model { pub use super::ContextAllocationWriteV1; }

verus! {
broadcast use vstd::std_specs::hash::group_hash_axioms;

spec fn allocation_less(a: RuntimeAllocationIdV1, b: RuntimeAllocationIdV1) -> bool {
    a.context_generation < b.context_generation
        || a.context_generation == b.context_generation && a.local < b.local
}
impl vstd::std_specs::cmp::PartialOrdSpecImpl for RuntimeAllocationIdV1 {
    open spec fn obeys_partial_cmp_spec() -> bool { true }
    closed spec fn partial_cmp_spec(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self.context_generation < other.context_generation
            || self.context_generation == other.context_generation && self.local < other.local
        { Some(core::cmp::Ordering::Less) }
        else if *self == *other { Some(core::cmp::Ordering::Equal) }
        else { Some(core::cmp::Ordering::Greater) }
    }
}

struct ContextAllocationEnrollmentV1 {
    key: ContextAllocationKeyV1, device: ContextJournalDeviceKeyV1, byte_extent: u64,
}
fn enrollment(id: RuntimeAllocationIdV1, device: RuntimeDeviceIdV1, byte_extent: u64)
    -> (out: ContextAllocationEnrollmentV1)
    ensures out == (ContextAllocationEnrollmentV1 {
        key: ContextAllocationKeyV1 { context_generation: id.context_generation, local: id.local },
        device: ContextJournalDeviceKeyV1 { context_generation: device.context_generation, local: device.local },
        byte_extent }),
{
    ContextAllocationEnrollmentV1 {
        key: ContextAllocationKeyV1 {
            context_generation: id.context_generation,
            local: id.local,
        },
        device: ContextJournalDeviceKeyV1 {
            context_generation: device.context_generation,
            local: device.local,
        },
        byte_extent,
    }
}

type StatusResult = Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1>;
type Payload = (ContextAllocationWriteV1, u64, u64, ContextWriterReferenceV1, ContextProducerReadStatusV1);
struct Header {
    result: Result<Payload, ContextVersionJournalErrorV1>, active: usize, queued: usize, calls: Seq<Call>,
}
struct Outcome { result: StatusResult, active: usize, queued: usize, calls: Seq<Call> }

fn producer_dependency_contains_v1(dependencies: &[ScalarPeerDependencyV1], dependency: &ScalarPeerDependencyV1)
    -> (found: bool)
    ensures found == dependencies@.contains(*dependency),
{
    producer_dependency_contains_body!(verus_exec_expr, dependencies, dependency, index,
        [invariant
            index <= dependencies.len(),
            forall|j: int| 0 <= j < index ==> dependencies@[j] != *dependency,
         decreases dependencies.len() - index,])
}

fn producer_source_pair_contains_v1(sources: &[ContextReadSourceV1], source: &ContextReadSourceV1)
    -> (found: bool)
    ensures found == (exists|j: int| 0 <= j < sources@.len()
        && sources@[j].region == source.region && sources@[j].record == source.record),
{
    producer_source_pair_contains_body!(verus_exec_expr, sources, source, index,
        [invariant
            index <= sources.len(),
            forall|j: int| 0 <= j < index
                ==> !(sources@[j].region == source.region && sources@[j].record == source.record),
         decreases sources.len() - index,])
}

spec fn header<R>(root: &Root<R>, index: usize, active: usize, queued: usize,
    consumer: ContextWriterKeyV1, returns: Returns) -> Header
    recommends index < root.inputs@.len(),
{
    let invalid = ContextVersionJournalErrorV1::InvalidReference;
    match root.inputs@[index as int].request {
        ProducerReadRequestV1::Active(request) => {
            if active >= root.references@.len() { Header { result: Err(invalid), active, queued, calls: seq![] } }
            else {
                let reference = root.references@[active as int];
                if active >= root.requests@.len() || root.requests@[active as int] != request
                    || reference.consumer != consumer
                    || root.references@[0].incarnation as int + (active as u64) as int != reference.incarnation as int
                { Header { result: Err(invalid), active, queued, calls: seq![] } }
                else {
                    let calls = seq![Call::ActiveLookup(reference)];
                    match returns.active_lookup {
                        Err(error) => Header { result: Err(error), active, queued, calls },
                        Ok(found) => if found != request { Header { result: Err(invalid), active, queued, calls } }
                        else {
                            let calls = calls.push(Call::ActiveStatus(reference));
                            let result = match returns.active_status {
                                Err(error) => Err(error),
                                Ok(status) => Ok((ContextAllocationWriteV1 { allocation: request.read.allocation,
                                    device: request.read.device, byte_extent: request.read.byte_extent },
                                    request.read.byte_offset, request.read.byte_len, request.producer, status)),
                            };
                            Header { result, active: (active + 1) as usize, queued, calls }
                        },
                    }
                }
            }
        },
        ProducerReadRequestV1::Queued(request) => {
            if queued >= root.queued_references@.len() { Header { result: Err(invalid), active, queued, calls: seq![] } }
            else {
                let reference = root.queued_references@[queued as int];
                if queued >= root.queued_requests@.len() || root.queued_requests@[queued as int] != request
                    || reference.consumer != consumer
                    || root.queued_references@[0].incarnation as int + (queued as u64) as int != reference.incarnation as int
                { Header { result: Err(invalid), active, queued, calls: seq![] } }
                else {
                    let calls = seq![Call::QueuedLookup(reference)];
                    match returns.queued_lookup {
                        Err(error) => Header { result: Err(error), active, queued, calls },
                        Ok(found) => if found != request { Header { result: Err(invalid), active, queued, calls } }
                        else {
                            let calls = calls.push(Call::QueuedStatus(reference));
                            let result = match returns.queued_status {
                                Err(error) => Err(error),
                                Ok(status) => Ok((request.allocation, request.byte_offset, request.byte_len, request.producer, status)),
                            };
                            Header { result, active, queued: (queued + 1) as usize, calls }
                        },
                    }
                }
            }
        },
    }
}

spec fn locally_bound<C, L, P, D>(context: &Context<C, L, P, D>, id: RuntimeSubmissionIdV1,
    input: ProducerInputV1, launch: bool) -> bool
{
    if launch {
        context.producer_launches@.contains_key(id)
        && context.producer_launches@[id].dependencies_held
        && context.producer_launches@[id].dependencies@.contains(input.dependency)
        && exists|i: int| 0 <= i < context.producer_launches@[id].sources@.len()
            && context.producer_launches@[id].sources@[i].region == input.source.region
            && context.producer_launches@[id].sources@[i].record == input.source.record
    } else {
        context.scalar_peer_copies@.contains_key(id)
        && context.scalar_peer_copies@[id].directed.is_some()
        && context.scalar_peer_copies@[id].dependencies_held
        && context.scalar_peer_copies@[id].dependencies@.contains(input.dependency)
        && context.scalar_peer_copies@[id].source.region == input.source.region
        && context.scalar_peer_copies@[id].source.record == input.source.record
    }
}

spec fn outcome<C, L, P, D, R>(owner: &Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1, launch: bool, index: usize, active: usize, queued: usize,
    returns: Returns) -> Outcome
    recommends index < owner.root.inputs@.len(),
{
    let h = header(&owner.root, index, active, queued, consumer, returns);
    let input = owner.root.inputs@[index as int];
    let source = input.source;
    let invalid = ContextVersionJournalErrorV1::InvalidReference;
    match h.result {
        Err(error) => Outcome { result: Err(error), active: h.active, queued: h.queued, calls: h.calls },
        Ok((allocation, offset, bytes, producer, status)) => {
            let local = locally_bound(&owner.context, id, input, launch)
                && producer.key == (ContextWriterKeyV1 { context_generation: input.dependency.submission.context_generation,
                    local: input.dependency.submission.local, kind: ContextWriterKindV1::Submission })
                && input.dependency.submission.local < id.local
                && (index == 0 || allocation_less(owner.root.inputs@[index as int - 1].source.region.allocation, source.region.allocation))
                && owner.context.allocations@.contains_key(source.region.allocation)
                && owner.context.allocations@[source.region.allocation] == source.record
                && owner.context.backend_allocations@.contains(source.record.backend_allocation);
            if !local { Outcome { result: Err(invalid), active: h.active, queued: h.queued, calls: h.calls } }
            else {
                let calls = h.calls.push(Call::Credit(source.region.allocation, source.record.device, source.record.byte_len));
                if !returns.credit { Outcome { result: Err(invalid), active: h.active, queued: h.queued, calls } }
                else {
                    let calls = calls.push(Call::Live(source.region.allocation, source.record));
                    let result = match returns.live {
                        Err(error) => Err(error),
                        Ok(live) => if live == allocation.allocation
                            && allocation.device == (ContextJournalDeviceKeyV1 { context_generation: source.record.device.context_generation,
                                local: source.record.device.local })
                            && allocation.byte_extent == source.record.byte_len
                            && offset == source.region.byte_offset && bytes == source.region.byte_len
                        { Ok(status) } else { Err(invalid) },
                    };
                    Outcome { result, active: h.active, queued: h.queued, calls }
                }
            }
        },
    }
}

proof fn allocation_order_correspondence(a: RuntimeAllocationIdV1, b: RuntimeAllocationIdV1)
    ensures
        (vstd::std_specs::cmp::PartialOrdSpec::partial_cmp_spec(&a, &b)
            matches Some(core::cmp::Ordering::Greater | core::cmp::Ordering::Equal))
            <==> !allocation_less(a, b),
{
}

broadcast proof fn trace_push_after_prefix(prefix: Seq<Call>, suffix: Seq<Call>, call: Call)
    ensures #[trigger] (prefix + suffix.push(call)) == (prefix + suffix).push(call),
{
    Seq::push_distributes_over_add(prefix, suffix, call);
}

impl<'a, C, L, P, D, R> Observations<'a, C, L, P, D, R> {
    fn observe_active_lookup(&mut self, reference: ContextProducerReadReferenceV1)
        -> (out: Result<ContextProducerReadV1, ContextVersionJournalErrorV1>)
        ensures out == old(self).returns.active_lookup,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::ActiveLookup(reference)),
    { proof { self.calls@ = self.calls@.push(Call::ActiveLookup(reference)); } self.returns.active_lookup }

    fn observe_active_status(&mut self, reference: ContextProducerReadReferenceV1) -> (out: StatusResult)
        ensures out == old(self).returns.active_status,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::ActiveStatus(reference)),
    { proof { self.calls@ = self.calls@.push(Call::ActiveStatus(reference)); } self.returns.active_status }

    fn observe_queued_lookup(&mut self, reference: ContextQueuedProducerReadReferenceV1)
        -> (out: Result<ContextQueuedProducerReadV1, ContextVersionJournalErrorV1>)
        ensures out == old(self).returns.queued_lookup,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::QueuedLookup(reference)),
    { proof { self.calls@ = self.calls@.push(Call::QueuedLookup(reference)); } self.returns.queued_lookup }

    fn observe_queued_status(&mut self, reference: ContextQueuedProducerReadReferenceV1) -> (out: StatusResult)
        ensures out == old(self).returns.queued_status,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::QueuedStatus(reference)),
    { proof { self.calls@ = self.calls@.push(Call::QueuedStatus(reference)); } self.returns.queued_status }

    fn observe_expected_credit(&mut self, allocation: RuntimeAllocationIdV1, device: RuntimeDeviceIdV1, bytes: u64) -> (out: bool)
        ensures out == old(self).returns.credit,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::Credit(allocation, device, bytes)),
    { proof { self.calls@ = self.calls@.push(Call::Credit(allocation, device, bytes)); } self.returns.credit }

    fn observe_live(&mut self, allocation: RuntimeAllocationIdV1, record: &AllocationRecordV1)
        -> (out: Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1>)
        ensures out == old(self).returns.live,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::Live(allocation, *record)),
    { proof { self.calls@ = self.calls@.push(Call::Live(allocation, *record)); } self.returns.live }

    fn validate(&mut self, id: RuntimeSubmissionIdV1, consumer: ContextWriterKeyV1,
        launch: bool, index: usize, active: &mut usize, queued: &mut usize) -> (out: StatusResult)
        requires index < old(self).owner.root.inputs@.len(),
            vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
            vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
        ensures final(self).owner == old(self).owner,
            final(self).returns == old(self).returns,
            out == outcome(old(self).owner, id, consumer, launch, index, *old(active), *old(queued), old(self).returns).result,
            *final(active) == outcome(old(self).owner, id, consumer, launch, index, *old(active), *old(queued), old(self).returns).active,
            *final(queued) == outcome(old(self).owner, id, consumer, launch, index, *old(active), *old(queued), old(self).returns).queued,
            final(self).calls@ == old(self).calls@ + outcome(old(self).owner, id, consumer, launch, index, *old(active), *old(queued), old(self).returns).calls,
    {
        broadcast use trace_push_after_prefix;
        let owner = self.owner;
        let context = &owner.context;
        let root = &owner.root;
        proof {
            if index > 0 {
                allocation_order_correspondence(
                    root.inputs@[index as int - 1].source.region.allocation,
                    root.inputs@[index as int].source.region.allocation,
                );
            }
        }
        producer_input_validate_body!(verus_exec_expr, context, root, id,
            consumer, launch, index, active, queued, self)
    }
}
}
