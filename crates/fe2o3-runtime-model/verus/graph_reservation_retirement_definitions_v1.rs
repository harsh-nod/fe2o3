// Field-complete conditional projection of the Context retirement suffix.
// Opaque owner payloads and untouched subtrees are framed, never interpreted as
// native settlement. Hash-key laws use the pinned standard-library contract as
// an explicit precondition; this file adds no key-model axiom.
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::ops::Deref;
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextGraphReservationV1 {
    context_generation: u64,
    local: u64,
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for ContextGraphReservationV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(PartialEq, Eq)]
enum RuntimeValidationErrorV1 { SubmissionPending, ContextReserved }

struct StreamRecordV1<O> {
    backend_stream: u64,
    device: O,
    unpublished: Option<u64>,
    generated: Option<u64>,
}

enum ReplicaStateV1<O> { Vacant, Pending(O), Settled(O) }
struct ReplicaSlotV1<O> { incarnation: u64, state: ReplicaStateV1<O> }
pub struct HostMetadataTableV1<T, C> { pub slots: Vec<T>, pub credits: Option<C> }
struct RuntimeReplicaStorageV1<O> { slots: HostMetadataTableV1<ReplicaSlotV1<O>, O> }
struct RuntimeReplicaUsageV1 { capacity: usize, pending: usize, settled: usize }

impl<T, C> Deref for HostMetadataTableV1<T, C> {
    type Target = [T];

    fn deref(&self) -> (out: &Self::Target)
        ensures out@ == self.slots@,
    {
        &self.slots
    }
}

// Every native Context field is represented. The generic key is a conditional
// projection of each nominal runtime handle; no lookup or re-keying occurs in
// this suffix. The source guard checks the complete native field/type roster.
struct RuntimeContextV1<K, O> {
    scope_epoch: O,
    replicas: Option<RuntimeReplicaStorageV1<O>>,
    backend: O,
    context_generation: u64,
    devices: Vec<O>,
    streams: HashMap<K, StreamRecordV1<O>>,
    backend_streams: HashSet<u64>,
    allocations: HashMap<K, O>,
    backend_allocations: HashSet<u64>,
    allocation_admission: O,
    versions: Option<O>,
    modules: HashMap<K, O>,
    backend_modules: HashSet<u64>,
    kernels: HashMap<u64, O>,
    events: HashMap<K, O>,
    backend_events: HashSet<u64>,
    submissions: HashMap<K, O>,
    backend_submissions: HashSet<u64>,
    scalar_peer_copies: HashMap<K, O>,
    producer_launches: HashMap<K, O>,
    same_device_copies: HashMap<K, O>,
    segmented_peer_copies: HashMap<K, O>,
    generated_issues: HashMap<K, O>,
    completion_callbacks: HashMap<K, Vec<O>>,
    completion_callback_count: usize,
    completion_callback_panic_count: u64,
    next_identity: u64,
    terminal: bool,
    graph_reservation: Option<ContextGraphReservationV1>,
    native_pair_reservation: Option<u64>,
    graph_issue_closed: bool,
}

mod fe2o3_runtime_model {
    use vstd::prelude::*;
    pub fn r63_graph_can_release_v1(
        terminal: bool, exact_token: bool, issue_closed: bool,
        submissions: usize, events: usize,
    ) -> (out: bool)
        ensures out == (!terminal && exact_token && issue_closed
            && submissions == 0 && events == 0),
    {
        !terminal && exact_token && issue_closed && submissions == 0 && events == 0
    }
}

closed spec fn pending_count<O>(slots: Seq<ReplicaSlotV1<O>>, n: nat) -> nat
    decreases n,
{
    if n == 0 { 0 }
    else { pending_count(slots, (n - 1) as nat)
        + if slots[n as int - 1].state is Pending { 1nat } else { 0nat } }
}

closed spec fn settled_count<O>(slots: Seq<ReplicaSlotV1<O>>, n: nat) -> nat
    decreases n,
{
    if n == 0 { 0 }
    else { settled_count(slots, (n - 1) as nat)
        + if slots[n as int - 1].state is Settled { 1nat } else { 0nat } }
}

impl<K, O> RuntimeContextV1<K, O> {
    closed spec fn retained_frame(&self, before: Self) -> bool {
        &&& self.scope_epoch == before.scope_epoch
        &&& self.replicas == before.replicas
        &&& self.backend == before.backend
        &&& self.context_generation == before.context_generation
        &&& self.devices == before.devices
        &&& self.streams == before.streams
        &&& self.backend_streams == before.backend_streams
        &&& self.allocations == before.allocations
        &&& self.backend_allocations == before.backend_allocations
        &&& self.allocation_admission == before.allocation_admission
        &&& self.versions == before.versions
        &&& self.modules == before.modules
        &&& self.backend_modules == before.backend_modules
        &&& self.kernels == before.kernels
        &&& self.events == before.events
        &&& self.backend_events == before.backend_events
        &&& self.submissions == before.submissions
        &&& self.backend_submissions == before.backend_submissions
        &&& self.scalar_peer_copies == before.scalar_peer_copies
        &&& self.producer_launches == before.producer_launches
        &&& self.same_device_copies == before.same_device_copies
        &&& self.segmented_peer_copies == before.segmented_peer_copies
        &&& self.generated_issues == before.generated_issues
        &&& self.completion_callbacks == before.completion_callbacks
        &&& self.completion_callback_count == before.completion_callback_count
        &&& self.completion_callback_panic_count == before.completion_callback_panic_count
        &&& self.next_identity == before.next_identity
        &&& self.terminal == before.terminal
        &&& self.native_pair_reservation == before.native_pair_reservation
        &&& self.graph_issue_closed == before.graph_issue_closed
    }

    closed spec fn unpublished(&self) -> bool {
        exists|key: K| self.streams@.contains_key(key)
            && self.streams@[key].unpublished.is_some()
    }

    closed spec fn generated(&self) -> bool {
        exists|key: K| self.streams@.contains_key(key)
            && self.streams@[key].generated.is_some()
    }

    closed spec fn pending_replicas(&self) -> nat {
        match self.replicas {
            None => 0,
            Some(table) => pending_count(table.slots.slots@, table.slots.slots@.len()),
        }
    }

    closed spec fn retirement_ready(&self, token: ContextGraphReservationV1) -> bool {
        &&& !self.terminal
        &&& self.graph_reservation == Some(token)
        &&& self.graph_issue_closed
        &&& self.submissions@.dom().is_empty()
        &&& self.events@.dom().is_empty()
        &&& !self.unpublished()
        &&& self.pending_replicas() == 0
        &&& self.generated_issues@.dom().is_empty()
        &&& !self.generated()
        &&& self.completion_callback_count == 0
        &&& self.backend_submissions@.is_empty()
        &&& self.backend_events@.is_empty()
    }
}
}
