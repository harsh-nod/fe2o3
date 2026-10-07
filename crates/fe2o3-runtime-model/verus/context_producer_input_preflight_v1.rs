// Read-only Context root selection, before any journal query or status fold.
// Actual std HashMaps retain full two-u64 keys. The theorem explicitly requires
// vstd's key-model law for that derived key type; Hash/Eq implementation and std
// collection contracts are trusted, not native identity authority.
// Unread metadata is packed into independent opaque non-Copy payloads. The
// successor source guard pins every observed field and every packed field set.
use std::collections::HashMap;
use vstd::prelude::*;

include!("../../fe2o3-runtime/src/context/versions/producer_input_preflight_body.rs");

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
#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeStreamIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextWriterKindV1 { Synchronous, Submission }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextWriterKeyV1 { context_generation: u64, local: u64, kind: ContextWriterKindV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextWriterReferenceV1 { slot: usize, key: ContextWriterKeyV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextProducerReadReferenceV1 { slot: usize, incarnation: u64, consumer: ContextWriterKeyV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextQueuedProducerReadReferenceV1 { slot: usize, incarnation: u64, consumer: ContextWriterKeyV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum ProducerReadReferenceV1 { Active(ContextProducerReadReferenceV1), Queued(ContextQueuedProducerReadReferenceV1) }
#[derive(Clone, Copy, PartialEq, Eq)]
struct FirstProducerReadV1 { consumer: ContextWriterKeyV1, reference: ProducerReadReferenceV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct SubmissionProducerReaderMarkerV1 {
    first: FirstProducerReadV1, count: usize,
    active: Option<(ContextProducerReadReferenceV1, usize)>,
    queued: Option<(ContextQueuedProducerReadReferenceV1, usize)>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ProducerReadDomainV1 { DirectedPeer, Launch }
#[derive(Clone, Copy, PartialEq, Eq)]
enum SubmissionWriterDomainV1 {
    Ordinary, Generated { stream: RuntimeStreamIdV1, hold: u64, shell_key: u64 },
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextVersionJournalErrorV1 { InvalidReference }

enum ProducerReadRequestV1<A, Q> { Active(A), Queued(Q) }
struct ProducerInputV1<A, Q, I> { request: ProducerReadRequestV1<A, Q>, custody: I }
struct RetainedProducerReadV1<A, Q, I> {
    domain: ProducerReadDomainV1, inputs: Vec<ProducerInputV1<A, Q, I>>,
    requests: Vec<A>, references: Vec<ContextProducerReadReferenceV1>,
    queued_requests: Vec<Q>, queued_references: Vec<ContextQueuedProducerReadReferenceV1>,
    marker: Option<SubmissionProducerReaderMarkerV1>,
}
struct RetainedSubmissionWriterV1<W> {
    writer: ContextWriterReferenceV1, domain: SubmissionWriterDomainV1, custody: W,
}
struct SubmissionRecordV1<S, T> {
    producer_launch: bool, directed_peer_copy: bool, journal_read: Option<T>,
    journal_producer_read: Option<SubmissionProducerReaderMarkerV1>,
    journal_writer: Option<ContextWriterReferenceV1>, custody: S,
}
struct ContextVersionsV1<A, Q, I, W, R, V> {
    producer_readers: HashMap<RuntimeSubmissionIdV1, RetainedProducerReadV1<A, Q, I>>,
    submission_writers: HashMap<RuntimeSubmissionIdV1, RetainedSubmissionWriterV1<W>>,
    submission_readers: HashMap<RuntimeSubmissionIdV1, R>, custody: V,
}
struct RuntimeContextV1<A, Q, I, W, R, V, S, T, C> {
    submissions: HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1<S, T>>,
    versions: Option<ContextVersionsV1<A, Q, I, W, R, V>>, custody: C,
}
struct ProducerInputRootV1<'a, A, Q, I, W, R, V> {
    versions: &'a ContextVersionsV1<A, Q, I, W, R, V>,
    root: &'a RetainedProducerReadV1<A, Q, I>, consumer: ContextWriterKeyV1, launch: bool,
}
}

structural_eq!(RuntimeSubmissionIdV1, RuntimeStreamIdV1, ContextWriterKindV1,
    ContextWriterKeyV1, ContextWriterReferenceV1, ContextProducerReadReferenceV1,
    ContextQueuedProducerReadReferenceV1, ProducerReadReferenceV1,
    FirstProducerReadV1, SubmissionProducerReaderMarkerV1,
    ProducerReadDomainV1, SubmissionWriterDomainV1, ContextVersionJournalErrorV1);

verus! {
broadcast use vstd::std_specs::hash::group_hash_axioms;

spec fn consumer_v1(id: RuntimeSubmissionIdV1) -> ContextWriterKeyV1 {
    ContextWriterKeyV1 { context_generation: id.context_generation,
        local: id.local, kind: ContextWriterKindV1::Submission }
}

spec fn first_reference_v1<A, Q, I>(root: &RetainedProducerReadV1<A, Q, I>)
    -> Option<FirstProducerReadV1>
{
    if root.inputs@.len() == 0 { None }
    else { match root.inputs@[0].request {
        ProducerReadRequestV1::Active(_) => if root.references@.len() == 0 { None } else {
            Some(FirstProducerReadV1 { consumer: root.references@[0].consumer,
                reference: ProducerReadReferenceV1::Active(root.references@[0]) })
        },
        ProducerReadRequestV1::Queued(_) => if root.queued_references@.len() == 0 { None } else {
            Some(FirstProducerReadV1 { consumer: root.queued_references@[0].consumer,
                reference: ProducerReadReferenceV1::Queued(root.queued_references@[0]) })
        },
    } }
}

impl<A, Q, I> RetainedProducerReadV1<A, Q, I> {
    fn first_reference(&self) -> (out: Option<FirstProducerReadV1>)
        ensures out == first_reference_v1(self),
    { producer_input_first_reference_body!(verus_exec_expr, self) }
}

spec fn envelope_valid_v1<A, Q, I, W, R, V, S, T>(
    versions: &ContextVersionsV1<A, Q, I, W, R, V>,
    record: Option<&SubmissionRecordV1<S, T>>, id: RuntimeSubmissionIdV1,
    root: &RetainedProducerReadV1<A, Q, I>, marker: SubmissionProducerReaderMarkerV1,
) -> bool {
    let launch = root.domain == ProducerReadDomainV1::Launch;
    let writer = if versions.submission_writers@.contains_key(id) {
        Some(versions.submission_writers@[id].writer)
    } else { None };
    &&& marker.count > 0
    &&& marker.count == root.inputs@.len()
    &&& marker.count as int == root.references@.len() + root.queued_references@.len()
    &&& root.references@.len() == root.requests@.len()
    &&& root.queued_references@.len() == root.queued_requests@.len()
    &&& Some(marker.first) == first_reference_v1(root)
    &&& marker.active == if root.references@.len() == 0 { None }
        else { Some((root.references@[0], root.references@.len() as usize)) }
    &&& marker.queued == if root.queued_references@.len() == 0 { None }
        else { Some((root.queued_references@[0], root.queued_references@.len() as usize)) }
    &&& marker.first.consumer == consumer_v1(id)
    &&& launch || root.queued_requests@.len() == 0
    &&& launch || !versions.submission_readers@.contains_key(id)
    &&& match record {
        None => true,
        Some(record) => record.producer_launch == launch
            && (launch || record.directed_peer_copy && record.journal_read.is_none())
            && record.journal_producer_read == Some(marker) && record.journal_writer == writer,
    }
    &&& !versions.submission_writers@.contains_key(id)
        || versions.submission_writers@[id].domain == SubmissionWriterDomainV1::Ordinary
    &&& launch || versions.submission_writers@.contains_key(id)
}

spec fn selection_v1<A, Q, I, W, R, V, S, T, C>(
    owner: &RuntimeContextV1<A, Q, I, W, R, V, S, T, C>, id: RuntimeSubmissionIdV1,
) -> Result<bool, ContextVersionJournalErrorV1> {
    let record = if owner.submissions@.contains_key(id) { Some(&owner.submissions@[id]) } else { None };
    let missing = match record {
        Some(record) => record.journal_producer_read.is_some(), None => false,
    };
    match &owner.versions {
        None => if missing { Err(ContextVersionJournalErrorV1::InvalidReference) } else { Ok(false) },
        Some(versions) => if !versions.producer_readers@.contains_key(id) {
            if missing { Err(ContextVersionJournalErrorV1::InvalidReference) } else { Ok(false) }
        } else {
            let root = &versions.producer_readers@[id];
            match root.marker {
                Some(marker) => if envelope_valid_v1(versions, record, id, root, marker) { Ok(true) }
                    else { Err(ContextVersionJournalErrorV1::InvalidReference) },
                None => Err(ContextVersionJournalErrorV1::InvalidReference),
            }
        },
    }
}

impl<A, Q, I, W, R, V, S, T, C> RuntimeContextV1<A, Q, I, W, R, V, S, T, C> {
    fn producer_input_root_v1(&self, id: RuntimeSubmissionIdV1)
        -> (out: Result<Option<ProducerInputRootV1<'_, A, Q, I, W, R, V>>, ContextVersionJournalErrorV1>)
        requires vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
        ensures match out {
            Err(error) => selection_v1(self, id) == Err(error),
            Ok(None) => selection_v1(self, id) == Ok(false),
            Ok(Some(selected)) => {
                &&& selection_v1(self, id) == Ok(true)
                &&& self.versions.is_some()
                &&& *selected.versions == self.versions.unwrap()
                &&& selected.versions.producer_readers@.contains_key(id)
                &&& *selected.root == selected.versions.producer_readers@[id]
                &&& selected.consumer == consumer_v1(id)
                &&& selected.launch == (selected.root.domain == ProducerReadDomainV1::Launch)
            },
        },
    {
        producer_input_preflight_body!(verus_exec_expr, self, id)
    }
}
}
