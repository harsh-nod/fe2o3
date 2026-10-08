// Read-only correspondence for the shared pre-effect writer lookup.
// This does not model queued-journal transitions, native effects, or unwind.
use std::collections::HashMap;
use vstd::prelude::*;

include!("../../fe2o3-runtime/src/context/versions/submissions/writer_lookup_body.rs");

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
struct RuntimeDeviceIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RuntimeAllocationIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextWriterKindV1 { Synchronous, Submission }
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextWriterKeyV1 {
    context_generation: u64, local: u64, kind: ContextWriterKindV1,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct ContextWriterReferenceV1 { slot: usize, key: ContextWriterKeyV1 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum SubmissionWriterDomainV1 {
    Ordinary,
    Generated { stream: RuntimeStreamIdV1, hold: u64, shell_key: u64 },
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeCompletionFailureV1 { BackendCode(i64), Cancelled }
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeCompletionStatusV1 {
    Pending, Succeeded, Failed(RuntimeCompletionFailureV1), QuiescentWithoutResult,
}
// These are exactly the two error variants produced by this shared body.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextVersionJournalErrorV1 { InvalidReference, InvalidState }

#[derive(Clone, Copy)]
struct SubmissionRecordV1<R: Copy, P: Copy> {
    backend_submission: u64, stream: RuntimeStreamIdV1, device: RuntimeDeviceIdV1,
    quiescent: bool, status: RuntimeCompletionStatusV1,
    journal_writer: Option<ContextWriterReferenceV1>,
    journal_read: Option<R>, journal_producer_read: Option<P>,
    scalar_peer_copy: bool, directed_peer_copy: bool, producer_launch: bool,
    same_device_copy: bool, segmented_peer_copy: bool,
    segmented_destination: Option<RuntimeAllocationIdV1>, dependency_retains: usize,
}

// O is arbitrary non-Copy data. Every unread field is independently retained;
// no flat JournalContents value is substituted for the actual queued journal.
struct RetainedSubmissionWriterV1<O> {
    writer: ContextWriterReferenceV1,
    domain: SubmissionWriterDomainV1,
    allocations: O,
    members: O,
    queued: Option<O>,
    disposal_quiescent: bool,
    disposal_group: Option<ContextWriterReferenceV1>,
    disposal_started: bool,
    disposed_count: usize,
    journal_disposed: bool,
}

struct ContextVersionsV1<O> {
    journal: O,
    phases: O,
    submission_writers: HashMap<RuntimeSubmissionIdV1, RetainedSubmissionWriterV1<O>>,
    submission_readers: HashMap<RuntimeSubmissionIdV1, O>,
    producer_readers: HashMap<RuntimeSubmissionIdV1, O>,
    disposal_groups: HashMap<RuntimeSubmissionIdV1, O>,
    disposal_allocations: HashMap<RuntimeAllocationIdV1, O>,
}
}

structural_eq!(RuntimeSubmissionIdV1, RuntimeStreamIdV1, RuntimeDeviceIdV1,
    RuntimeAllocationIdV1, ContextWriterKindV1, ContextWriterKeyV1,
    ContextWriterReferenceV1, SubmissionWriterDomainV1,
    RuntimeCompletionFailureV1, RuntimeCompletionStatusV1, ContextVersionJournalErrorV1);

verus! {
broadcast use vstd::std_specs::hash::group_hash_axioms;

spec fn key_contract_v1() -> bool {
    vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>()
}

spec fn borrowed_versions_v1<O>(
    versions: Option<&ContextVersionsV1<O>>,
) -> Option<ContextVersionsV1<O>> {
    match versions { None => None, Some(owner) => Some(*owner) }
}

spec fn writer_lookup_selection_v1<O, R: Copy, P: Copy>(
    submissions: Map<RuntimeSubmissionIdV1, SubmissionRecordV1<R, P>>,
    versions: Option<ContextVersionsV1<O>>,
    id: RuntimeSubmissionIdV1,
    domain: SubmissionWriterDomainV1,
) -> Result<Option<ContextWriterReferenceV1>, ContextVersionJournalErrorV1> {
    let present = submissions.contains_key(id);
    let expected = if present { submissions[id].journal_writer } else { None };
    let absent = if expected.is_some() {
        Err(ContextVersionJournalErrorV1::InvalidReference)
    } else { Ok(None) };
    match versions {
        None => absent,
        Some(owner) => {
            if owner.submission_readers@.contains_key(id)
                || owner.producer_readers@.contains_key(id)
                || (present && submissions[id].journal_read.is_some())
                || (present && submissions[id].journal_producer_read.is_some()) {
                Err(ContextVersionJournalErrorV1::InvalidState)
            } else if !owner.submission_writers@.contains_key(id) {
                absent
            } else {
                let root = owner.submission_writers@[id];
                let writer = root.writer;
                if root.domain != domain
                    || (present && expected != Some(writer))
                    || writer.key.context_generation != id.context_generation
                    || writer.key.local != id.local
                    || writer.key.kind != ContextWriterKindV1::Submission {
                    Err(ContextVersionJournalErrorV1::InvalidReference)
                } else { Ok(Some(writer)) }
            }
        },
    }
}

fn preflight_settlement_writer_v1<O, R: Copy, P: Copy>(
    submissions: &HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1<R, P>>,
    versions: Option<&ContextVersionsV1<O>>,
    id: RuntimeSubmissionIdV1,
    domain: SubmissionWriterDomainV1,
) -> (out: Result<Option<ContextWriterReferenceV1>, ContextVersionJournalErrorV1>)
    requires key_contract_v1(),
    ensures out == writer_lookup_selection_v1(submissions@, borrowed_versions_v1(versions), id, domain),
{
    completion_writer_lookup_body_v1!(verus_exec_expr, submissions, versions, id, domain)
}

fn writer_lookup_frames_original_owners_v1<O, R: Copy, P: Copy>(
    submissions: &mut HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1<R, P>>,
    versions: &mut Option<ContextVersionsV1<O>>,
    id: RuntimeSubmissionIdV1,
    domain: SubmissionWriterDomainV1,
) -> (out: Result<Option<ContextWriterReferenceV1>, ContextVersionJournalErrorV1>)
    requires key_contract_v1(),
    ensures
        *final(submissions) == *old(submissions),
        *final(versions) == *old(versions),
        out == writer_lookup_selection_v1(old(submissions)@, *old(versions), id, domain),
{
    preflight_settlement_writer_v1(submissions, versions.as_ref(), id, domain)
}

proof fn selected_writer_is_exact_original_v1<O, R: Copy, P: Copy>(
    submissions: Map<RuntimeSubmissionIdV1, SubmissionRecordV1<R, P>>,
    versions: Option<ContextVersionsV1<O>>,
    id: RuntimeSubmissionIdV1,
    domain: SubmissionWriterDomainV1,
    writer: ContextWriterReferenceV1,
)
    requires writer_lookup_selection_v1(submissions, versions, id, domain) == Ok(Some(writer)),
    ensures
        versions.is_some(),
        versions.unwrap().submission_writers@.contains_key(id),
        versions.unwrap().submission_writers@[id].writer == writer,
        versions.unwrap().submission_writers@[id].domain == domain,
        writer.key.context_generation == id.context_generation,
        writer.key.local == id.local,
        writer.key.kind == ContextWriterKindV1::Submission,
        !versions.unwrap().submission_readers@.contains_key(id),
        !versions.unwrap().producer_readers@.contains_key(id),
        submissions.contains_key(id) ==> (
            submissions[id].journal_writer == Some(writer)
            && submissions[id].journal_read.is_none()
            && submissions[id].journal_producer_read.is_none()
        ),
{
}

proof fn reader_conflict_precedes_missing_writer_v1<O, R: Copy, P: Copy>(
    submissions: Map<RuntimeSubmissionIdV1, SubmissionRecordV1<R, P>>,
    owner: ContextVersionsV1<O>,
    id: RuntimeSubmissionIdV1,
    domain: SubmissionWriterDomainV1,
)
    requires
        owner.submission_readers@.contains_key(id) || owner.producer_readers@.contains_key(id),
    ensures
        writer_lookup_selection_v1(submissions, Some(owner), id, domain)
            == Err(ContextVersionJournalErrorV1::InvalidState),
{
}
}
