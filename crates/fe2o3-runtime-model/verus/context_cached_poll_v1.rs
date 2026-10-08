// Conditional correspondence for the actual shared cached-poll prefix only.
// The outer graph/epoch gate and fresh backend tail are not modeled here.
// std HashMap contracts and custom-key laws are explicit dependencies.
use std::collections::HashMap;
use vstd::prelude::*;

include!("../../fe2o3-runtime/src/context/cached_poll_body.rs");

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
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RuntimeStreamIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeDeviceIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeAllocationIdV1 { context_generation: u64, local: u64 }
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeCompletionFailureV1 { BackendCode(i64), Cancelled }
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeCompletionStatusV1 {
    Pending, Succeeded, Failed(RuntimeCompletionFailureV1), QuiescentWithoutResult,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimePollV1 { Pending, Succeeded, Failed { code: i64 } }
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeValidationErrorV1 { UnknownSubmission, UnknownStream, WrongDevice, ContextReserved }
const RUNTIME_CANCELLED_CODE_V1: i64 = -2;
const RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1: i64 = -3;

#[derive(Clone, Copy)]
struct StreamRecordV1 {
    backend_stream: u64, device: RuntimeDeviceIdV1,
    unpublished: Option<u64>, generated: Option<u64>,
}

// Three unread Copy marker types are independent generic parameters. No marker
// is inspected, interpreted as authority, or discarded by the shared body.
#[derive(Clone, Copy)]
struct SubmissionRecordV1<W: Copy, R: Copy, P: Copy> {
    backend_submission: u64, stream: RuntimeStreamIdV1, device: RuntimeDeviceIdV1,
    quiescent: bool, status: RuntimeCompletionStatusV1,
    journal_writer: Option<W>, journal_read: Option<R>, journal_producer_read: Option<P>,
    scalar_peer_copy: bool, directed_peer_copy: bool, producer_launch: bool,
    same_device_copy: bool, segmented_peer_copy: bool,
    segmented_destination: Option<RuntimeAllocationIdV1>, dependency_retains: usize,
}

// Each unread field remains a distinct arbitrary non-Copy value. O is a
// projection payload, not an implementation of RuntimeBackend or native owners.
struct RuntimeContextV1<O, W: Copy, R: Copy, P: Copy> {
    scope_epoch: O,
    replicas: O,
    backend: O,
    context_generation: u64,
    devices: O,
    streams: HashMap<RuntimeStreamIdV1, StreamRecordV1>,
    backend_streams: O,
    allocations: O,
    backend_allocations: O,
    allocation_admission: O,
    versions: O,
    modules: O,
    backend_modules: O,
    kernels: O,
    events: O,
    backend_events: O,
    submissions: HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1<W, R, P>>,
    backend_submissions: O,
    scalar_peer_copies: O,
    producer_launches: O,
    same_device_copies: O,
    segmented_peer_copies: O,
    generated_issues: O,
    completion_callbacks: O,
    completion_callback_count: usize,
    completion_callback_panic_count: u64,
    next_identity: u64,
    terminal: bool,
    graph_reservation: O,
    native_pair_reservation: Option<u64>,
    graph_issue_closed: bool,
}

struct RuntimeSubmissionV1<A, T> {
    id: RuntimeSubmissionIdV1, backend_submission: u64,
    stream: RuntimeStreamIdV1, device: RuntimeDeviceIdV1,
    completion: Option<RuntimePollV1>, peer_transfer: T, marker: A,
}
}

structural_eq!(RuntimeSubmissionIdV1, RuntimeStreamIdV1, RuntimeDeviceIdV1,
    RuntimeAllocationIdV1, RuntimeCompletionFailureV1, RuntimeCompletionStatusV1,
    RuntimePollV1, RuntimeValidationErrorV1);

verus! {
broadcast use vstd::std_specs::hash::group_hash_axioms;

spec fn key_contracts_v1() -> bool {
    &&& vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>()
    &&& vstd::std_specs::hash::obeys_key_model::<RuntimeStreamIdV1>()
}

spec fn terminal_v1(status: RuntimeCompletionStatusV1) -> bool {
    status != RuntimeCompletionStatusV1::Pending
}

spec fn legacy_v1(status: RuntimeCompletionStatusV1) -> RuntimePollV1 {
    match status {
        RuntimeCompletionStatusV1::Pending => RuntimePollV1::Pending,
        RuntimeCompletionStatusV1::Succeeded => RuntimePollV1::Succeeded,
        RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(code)) =>
            RuntimePollV1::Failed { code },
        RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled) =>
            RuntimePollV1::Failed { code: RUNTIME_CANCELLED_CODE_V1 },
        RuntimeCompletionStatusV1::QuiescentWithoutResult =>
            RuntimePollV1::Failed { code: RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1 },
    }
}

spec fn token_frame_v1<A, T>(before: RuntimeSubmissionV1<A, T>, after: RuntimeSubmissionV1<A, T>) -> bool {
    &&& before.id == after.id
    &&& before.backend_submission == after.backend_submission
    &&& before.stream == after.stream
    &&& before.device == after.device
    &&& before.peer_transfer == after.peer_transfer
    &&& before.marker == after.marker
}

spec fn record_selection_v1<O, W: Copy, R: Copy, P: Copy, A, T>(
    context: &RuntimeContextV1<O, W, R, P>, submission: &RuntimeSubmissionV1<A, T>,
) -> Result<SubmissionRecordV1<W, R, P>, RuntimeValidationErrorV1> {
    if submission.id.context_generation != context.context_generation
        || !context.submissions@.contains_key(submission.id) {
        Err(RuntimeValidationErrorV1::UnknownSubmission)
    } else {
        let record = context.submissions@[submission.id];
        if record.backend_submission != submission.backend_submission
            || record.stream != submission.stream || record.device != submission.device {
            Err(RuntimeValidationErrorV1::UnknownSubmission)
        } else { Ok(record) }
    }
}

spec fn live_selection_v1<O, W: Copy, R: Copy, P: Copy, A, T>(
    context: &RuntimeContextV1<O, W, R, P>, submission: &RuntimeSubmissionV1<A, T>,
) -> Result<SubmissionRecordV1<W, R, P>, RuntimeValidationErrorV1> {
    match record_selection_v1(context, submission) {
        Err(error) => Err(error),
        Ok(record) => if !context.streams@.contains_key(record.stream) {
            Err(RuntimeValidationErrorV1::UnknownStream)
        } else if context.streams@[record.stream].device != record.device {
            Err(RuntimeValidationErrorV1::WrongDevice)
        } else { Ok(record) },
    }
}

spec fn unheld_selection_v1<O, W: Copy, R: Copy, P: Copy>(
    context: &RuntimeContextV1<O, W, R, P>, stream: RuntimeStreamIdV1,
) -> Result<StreamRecordV1, RuntimeValidationErrorV1> {
    if !context.streams@.contains_key(stream) { Err(RuntimeValidationErrorV1::UnknownStream) }
    else if context.streams@[stream].unpublished.is_some() { Err(RuntimeValidationErrorV1::ContextReserved) }
    else { Ok(context.streams@[stream]) }
}

spec fn prefix_selection_v1<O, W: Copy, R: Copy, P: Copy, A, T>(
    context: &RuntimeContextV1<O, W, R, P>, submission: &RuntimeSubmissionV1<A, T>,
) -> Result<Option<RuntimePollV1>, RuntimeValidationErrorV1> {
    match live_selection_v1(context, submission) {
        Err(error) => Err(error),
        Ok(record) => match unheld_selection_v1(context, record.stream) {
            Err(error) => Err(error),
            Ok(_) => if terminal_v1(record.status) { Ok(Some(legacy_v1(record.status))) }
                else { Ok(None) },
        },
    }
}

impl RuntimeCompletionStatusV1 {
    fn is_terminal(self) -> (out: bool)
        ensures out == terminal_v1(self),
    { cached_status_terminal_body_v1!(verus_exec_expr, self) }

    fn legacy_poll(self) -> (out: RuntimePollV1)
        ensures out == legacy_v1(self),
    { cached_status_legacy_body_v1!(verus_exec_expr, self) }
}

impl<A, T> RuntimeSubmissionV1<A, T> {
    fn observe_status(&mut self, status: RuntimeCompletionStatusV1) -> (out: RuntimePollV1)
        ensures
            out == legacy_v1(status),
            token_frame_v1(*old(self), *final(self)),
            final(self).completion == if terminal_v1(status) { Some(out) } else { old(self).completion },
    { cached_observe_status_body_v1!(verus_exec_expr, self, status) }
}

impl<O, W: Copy, R: Copy, P: Copy> RuntimeContextV1<O, W, R, P> {
    fn submission_record<A, T>(&self, submission: &RuntimeSubmissionV1<A, T>)
        -> (out: Result<SubmissionRecordV1<W, R, P>, RuntimeValidationErrorV1>)
        requires vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
        ensures out == record_selection_v1(self, submission),
    { cached_submission_record_body_v1!(verus_exec_expr, self, submission) }

    fn live_submission_record<A, T>(&self, submission: &RuntimeSubmissionV1<A, T>)
        -> (out: Result<SubmissionRecordV1<W, R, P>, RuntimeValidationErrorV1>)
        requires key_contracts_v1(),
        ensures out == live_selection_v1(self, submission),
    { cached_live_submission_body_v1!(verus_exec_expr, self, submission) }

    fn unheld_stream_v1(&self, stream: RuntimeStreamIdV1)
        -> (out: Result<&StreamRecordV1, RuntimeValidationErrorV1>)
        requires vstd::std_specs::hash::obeys_key_model::<RuntimeStreamIdV1>(),
        ensures match out {
            Err(error) => unheld_selection_v1(self, stream) == Err(error),
            Ok(record) => unheld_selection_v1(self, stream) == Ok(*record),
        },
    { cached_unheld_stream_body_v1!(verus_exec_expr, self, stream) }

    fn require_stream_unheld_v1(&self, stream: RuntimeStreamIdV1)
        -> (out: Result<(), RuntimeValidationErrorV1>)
        requires vstd::std_specs::hash::obeys_key_model::<RuntimeStreamIdV1>(),
        ensures out == match unheld_selection_v1(self, stream) {
            Err(error) => Err(error), Ok(_) => Ok(()),
        },
    { self.unheld_stream_v1(stream).map(|_held_stream| ()) }

    fn poll_cached_status_v1<A, T>(&self, submission: &mut RuntimeSubmissionV1<A, T>)
        -> (out: Result<Option<RuntimePollV1>, RuntimeValidationErrorV1>)
        requires key_contracts_v1(),
        ensures
            out == prefix_selection_v1(self, old(submission)),
            token_frame_v1(*old(submission), *final(submission)),
            final(submission).completion == match out {
                Ok(Some(status)) => Some(status), _ => old(submission).completion,
            },
    { cached_poll_prefix_body_v1!(verus_exec_expr, self, submission) }
}

// A verification wrapper, not a replacement for the actual graph-access gate.
// It makes the whole projected owner frame an explicit pre/post obligation.
fn cached_prefix_frames_owner_v1<O, W: Copy, R: Copy, P: Copy, A, T>(
    context: &mut RuntimeContextV1<O, W, R, P>, submission: &mut RuntimeSubmissionV1<A, T>,
) -> (out: Result<Option<RuntimePollV1>, RuntimeValidationErrorV1>)
    requires key_contracts_v1(),
    ensures
        *final(context) == *old(context),
        out == prefix_selection_v1(old(context), old(submission)),
        token_frame_v1(*old(submission), *final(submission)),
        final(submission).completion == match out {
            Ok(Some(status)) => Some(status), _ => old(submission).completion,
        },
{
    context.poll_cached_status_v1(submission)
}
}
