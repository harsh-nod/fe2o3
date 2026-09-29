// Conditional host-controller refinement, starting AFTER the delegate callback.
//
// `storage` is the full opaque, non-Copy owned post-callback storage. Shared Arc
// account interiors are an external environment, not a frozen part of this frame.
// The three typed observations replay values returned by the actual ordered
// getters: queue liveness, then source phase, then destination phase as reached.
// In particular a phase means the account-aware getter's result, not its stored
// phase field. Inclusive composed/session usage takes precedence over local host
// and device fallback; different getter/lock observations need not be simultaneous.
// Linking these replay values to the actual getters is a declared adapter boundary.
// A nonterminal replay is NOT healthy global state or native currentness at return.
//
// Native quarantine is the single external returning-effect adapter below. Its
// relation is deliberately uninterpreted: no release, completion, global-gate,
// session-state, native custody conservation, idempotence, or total-return theorem
// is asserted. It may abort or not return. The exact owner equality theorem applies
// ONLY to the controller branches that do not invoke that adapter.
//
// std catch_unwind/AssertUnwindSafe/resume_unwind and automatic Rust Drop execution
// are outside this root. An Err payload is the ACTUAL caught payload; there is no
// invented callback or pre-call frame. Resume does not reach close-post. Subsequent
// unwind may separately invoke Drop and quarantine again. These are not ISA,
// mapping-validity, kernel, XGMI-currentness, or whole-operation proofs.

#![allow(unused_macros)]

use vstd::prelude::*;

include!("../../fe2o3-kfd/src/sdma/retained_pair_operation_body.rs");

verus! {

pub enum Gfx942SdmaErrorV1<M, K, D> {
    Memory(M),
    Packet(K),
    Contract(&'static str),
    QueueCreationIndeterminate,
    QueueDestroyIndeterminate,
    Doorbell(D),
    QueueFull,
    Pending,
    Timeout,
    PublishedCancellationUnsupported,
}

// Request/ticket/mapping elements are opaque non-Copy values. Production moves
// whole Vec payloads without allocating or inspecting their contents. The proof
// uses the pinned verifier's standard Vec value model, not an allocator/ISA proof.
pub enum Gfx942XgmiBatchSubmissionFailureV1<E, Requests, Tickets> {
    Recoverable { error: E, requests: Requests },
    Retained { error: E, tickets: Tickets },
}

pub enum Gfx942XgmiBatchWaitFailureV1<E, Tickets, Completed> {
    Retained { error: E, tickets: Tickets },
    CompletedCurrentnessIndeterminate { error: E, completed: Completed },
}

pub enum Settled<R, P> {
    Return(R),
    Resume(P),
}

pub trait TerminalOutcome: Sized {
    spec fn normalized(self) -> Self;

    fn refuse_terminal_success(self) -> (result: Self)
        ensures result == self.normalized();
}

pub open spec fn terminal_error<M, K, D>() -> Gfx942SdmaErrorV1<M, K, D> {
    Gfx942SdmaErrorV1::Contract("retained XGMI success has terminal custody")
}

pub fn terminal_success_error<M, K, D>() -> (result: Gfx942SdmaErrorV1<M, K, D>)
    ensures result == terminal_error::<M, K, D>(),
{
    Gfx942SdmaErrorV1::Contract("retained XGMI success has terminal custody")
}

impl<M, K, D> TerminalOutcome for Result<(), Gfx942SdmaErrorV1<M, K, D>> {
    open spec fn normalized(self) -> Self {
        match self {
            Ok(()) => Err(terminal_error::<M, K, D>()),
            Err(error) => Err(error),
        }
    }

    fn refuse_terminal_success(self) -> (result: Self)
        ensures result == self.normalized(),
    {
        normalize_unit(self)
    }
}

impl<M, K, D, Requests, Tickets> TerminalOutcome
    for Result<Vec<Tickets>, Gfx942XgmiBatchSubmissionFailureV1<Gfx942SdmaErrorV1<M, K, D>, Vec<Requests>, Vec<Tickets>>>
{
    open spec fn normalized(self) -> Self {
        match self {
            Ok(tickets) => Err(Gfx942XgmiBatchSubmissionFailureV1::Retained {
                error: terminal_error::<M, K, D>(), tickets,
            }),
            Err(error) => Err(error),
        }
    }

    fn refuse_terminal_success(self) -> (result: Self)
        ensures result == self.normalized(),
    {
        normalize_tickets(self)
    }
}

impl<M, K, D, Tickets, Completed> TerminalOutcome
    for Result<Vec<Completed>, Gfx942XgmiBatchWaitFailureV1<Gfx942SdmaErrorV1<M, K, D>, Vec<Tickets>, Vec<Completed>>>
{
    open spec fn normalized(self) -> Self {
        match self {
            Ok(completed) => Err(Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate {
                error: terminal_error::<M, K, D>(), completed,
            }),
            Err(error) => Err(error),
        }
    }

    fn refuse_terminal_success(self) -> (result: Self)
        ensures result == self.normalized(),
    {
        normalize_completed(self)
    }
}

pub fn normalize_unit<M, K, D>(value: Result<(), Gfx942SdmaErrorV1<M, K, D>>)
    -> (result: Result<(), Gfx942SdmaErrorV1<M, K, D>>)
    ensures result == value.normalized(),
{
    retained_pair_unit_outcome_body!(value)
}

pub fn normalize_tickets<M, K, D, Requests, Tickets>(
    value: Result<Vec<Tickets>, Gfx942XgmiBatchSubmissionFailureV1<Gfx942SdmaErrorV1<M, K, D>, Vec<Requests>, Vec<Tickets>>>,
) -> (result: Result<Vec<Tickets>, Gfx942XgmiBatchSubmissionFailureV1<Gfx942SdmaErrorV1<M, K, D>, Vec<Requests>, Vec<Tickets>>>)
    ensures result == value.normalized(),
{
    retained_pair_tickets_outcome_body!(value)
}

pub fn normalize_completed<M, K, D, Tickets, Completed>(
    value: Result<Vec<Completed>, Gfx942XgmiBatchWaitFailureV1<Gfx942SdmaErrorV1<M, K, D>, Vec<Tickets>, Vec<Completed>>>,
) -> (result: Result<Vec<Completed>, Gfx942XgmiBatchWaitFailureV1<Gfx942SdmaErrorV1<M, K, D>, Vec<Tickets>, Vec<Completed>>>)
    ensures result == value.normalized(),
{
    retained_pair_completed_outcome_body!(value)
}

#[derive(PartialEq, Eq)]
pub enum SharedMemorySessionPhaseV1 {
    Active,
    Quarantined,
}

pub enum QueueGetterResult {
    Live,
    Refused,
}

pub struct QueueObservation {
    returned: QueueGetterResult,
}

impl QueueObservation {
    // The real error is discarded by `.is_err()` in the actual shared terminal
    // body. This projection erases no callback error or returned custody payload.
    fn require_live_queue_state_v1(&self) -> (result: Result<(), ()>)
        ensures result == match self.returned {
            QueueGetterResult::Live => Ok(()),
            QueueGetterResult::Refused => Err(()),
        },
    {
        match self.returned {
            QueueGetterResult::Live => Ok(()),
            QueueGetterResult::Refused => Err(()),
        }
    }
}

pub struct PhaseObservation {
    returned: SharedMemorySessionPhaseV1,
}

impl PhaseObservation {
    fn phase(&self) -> (result: SharedMemorySessionPhaseV1)
        ensures result == self.returned,
    {
        match self.returned {
            SharedMemorySessionPhaseV1::Active => SharedMemorySessionPhaseV1::Active,
            SharedMemorySessionPhaseV1::Quarantined => SharedMemorySessionPhaseV1::Quarantined,
        }
    }
}

pub struct PostCallbackCustody<C> {
    storage: C,
    queue: QueueObservation,
    source: PhaseObservation,
    destination: PhaseObservation,
    ghost quarantine_calls: nat,
}

pub uninterp spec fn returning_quarantine_effect<C>(before: C, after: C) -> bool;

pub open spec fn terminal_observation<C>(owner: PostCallbackCustody<C>) -> bool {
    matches!(owner.queue.returned, QueueGetterResult::Refused)
        || owner.source.returned != SharedMemorySessionPhaseV1::Active
        || owner.destination.returned != SharedMemorySessionPhaseV1::Active
}

pub open spec fn one_returning_quarantine<C>(before: PostCallbackCustody<C>, after: PostCallbackCustody<C>) -> bool {
    &&& returning_quarantine_effect(before.storage, after.storage)
    &&& after.queue == before.queue
    &&& after.source == before.source
    &&& after.destination == before.destination
    &&& after.quarantine_calls == before.quarantine_calls + 1
}

impl<C> PostCallbackCustody<C> {
    fn terminal(&self) -> (result: bool)
        ensures result == terminal_observation(*self),
    {
        retained_pair_terminal_body!(self)
    }

    // Conditional adapter contract, NOT verified native quarantine. Replay fields
    // are historical observations, so preserving them does not freeze live state.
    #[verifier::external_body]
    fn quarantine(&mut self)
        ensures one_returning_quarantine(*old(self), *final(self)),
    {
        unimplemented!()
    }
}

pub open spec fn settled_value<R: TerminalOutcome, P>(outcome: Result<R, P>, terminal: bool) -> Settled<R, P> {
    match outcome {
        Ok(value) => Settled::Return(if terminal { value.normalized() } else { value }),
        Err(payload) => Settled::Resume(payload),
    }
}

pub open spec fn settles_with_quarantine<R, P>(outcome: Result<R, P>, terminal: bool) -> bool {
    match outcome {
        Ok(_) => terminal,
        Err(_) => true,
    }
}

// An arbitrary concrete callback outcome and post-callback owner are accepted;
// neither a successful callback nor a healthy owner is a theorem precondition.
pub fn settle_given_returning_quarantine_contract<C, R: TerminalOutcome, P>(
    context: &mut PostCallbackCustody<C>, outcome: Result<R, P>,
) -> (result: Settled<R, P>)
    ensures
        result == settled_value(outcome, terminal_observation(*old(context))),
        if settles_with_quarantine(outcome, terminal_observation(*old(context))) {
            one_returning_quarantine(*old(context), *final(context))
        } else {
            *final(context) == *old(context)
        },
{
    retained_pair_settle_body!(context, outcome)
}

pub struct Scope<C> {
    context: PostCallbackCustody<C>,
    finished: bool,
}

pub fn close_post_given_returning_quarantine_contract<C, M, K, D>(
    scope: &mut Scope<C>, result: Result<(), Gfx942SdmaErrorV1<M, K, D>>,
) -> (returned: Result<(), Gfx942SdmaErrorV1<M, K, D>>)
    ensures
        returned == result,
        final(scope).finished,
        if result.is_err() {
            one_returning_quarantine(old(scope).context, final(scope).context)
        } else {
            final(scope).context == old(scope).context
        },
{
    retained_pair_close_post_body!(scope, result)
}

pub fn finish_terminal_given_returning_quarantine_contract<C>(scope: &mut Scope<C>)
    ensures
        final(scope).finished,
        one_returning_quarantine(old(scope).context, final(scope).context),
{
    retained_pair_finish_terminal_body!(scope)
}

// One explicit invocation of the actual Drop body. Automatic Drop/unwind timing
// is external. The original finished flag is retained even on the abandoned path.
pub fn drop_once_given_returning_quarantine_contract<C>(scope: &mut Scope<C>)
    ensures
        final(scope).finished == old(scope).finished,
        if old(scope).finished {
            *final(scope) == *old(scope)
        } else {
            one_returning_quarantine(old(scope).context, final(scope).context)
        },
{
    retained_pair_drop_body!(scope)
}

}
