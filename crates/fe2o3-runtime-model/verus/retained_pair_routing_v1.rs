// Conditional normal-return refinement of the actual retained routing matches.
// Each consumed receipt must describe the corresponding reached callback and
// its return. The replay does not establish native submit/wait safety,
// success, retryability, currentness, ticket validity, deadline expiry or progress.
// O frames only an opaque value stored locally by this replay, not native/Arc
// interiors. Generic payloads are not Copy or Clone. Vec/Option/Result and borrow
// semantics use pinned vstd contracts; allocation, pointers, Drop, unwind, abort,
// compiler and ISA behavior are outside this theorem.
use vstd::prelude::*;

include!("../../fe2o3-runtime/src/kfd_backend/xgmi_retained/routing_declarations.rs");
include!("../../fe2o3-runtime/src/kfd_backend/xgmi_retained/routing_bodies.rs");
retained_pair_routing_declarations!(verus, pub);

verus! {
spec fn submits<R, T, S, W>(phase: BatchPhase<R, T, S, W>) -> bool {
    match phase { BatchPhase::Prepared(_) => true, _ => false }
}

spec fn published<R, T, S, W>(phase: BatchPhase<R, T, S, W>,
    receipt: Option<Result<Vec<T>, S>>) -> BatchPhase<R, T, S, W>
{
    match (phase, receipt) {
        (BatchPhase::Prepared(_), Some(Ok(tickets))) => BatchPhase::Pending(tickets),
        (BatchPhase::Prepared(_), Some(Err(error))) => BatchPhase::SubmitFailure(error),
        (other, _) => other,
    }
}

spec fn waits<R, T, S, W>(phase: BatchPhase<R, T, S, W>) -> bool {
    match phase { BatchPhase::Pending(_) => true, _ => false }
}

spec fn advanced<R, T, S, W, C>(phase: BatchPhase<R, T, S, W>,
    submit: Option<Result<Vec<T>, S>>, wait: Option<Result<C, W>>)
    -> Advanced<BatchPhase<R, T, S, W>, C, W>
    recommends !waits(published(phase, submit)) || wait.is_some(),
{
    match published(phase, submit) {
        BatchPhase::Pending(_) => Advanced::Waited(wait.unwrap()),
        other => Advanced::Unchanged(other),
    }
}

// A receipt has no success contract. Its complete opaque return is consumed
// once, and the exact reached arguments are retained for the routing theorem.
// D is an identity label for the passed Instant, with no clock arithmetic.
struct Observations<O, R, T, S, W, C, D> {
    owner: O,
    submit_return: Option<Result<Vec<T>, S>>,
    wait_return: Option<Result<C, W>>,
    submitted: Option<Vec<R>>,
    waited: Option<Vec<T>>,
    deadline: Option<D>,
}

impl<O, R, T, S, W, C, D> Observations<O, R, T, S, W, C, D> {
    closed spec fn cold(&self) -> bool {
        self.submitted.is_none() && self.waited.is_none() && self.deadline.is_none()
    }

    fn submit(&mut self, requests: Vec<R>) -> (out: Result<Vec<T>, S>)
        requires old(self).submitted.is_none(), old(self).submit_return.is_some(),
        ensures
            out == old(self).submit_return.unwrap(),
            final(self).submitted == Some(requests), final(self).submit_return.is_none(),
            final(self).owner == old(self).owner,
            final(self).wait_return == old(self).wait_return,
            final(self).waited == old(self).waited, final(self).deadline == old(self).deadline,
    {
        self.submitted = Some(requests);
        self.submit_return.take().unwrap()
    }

    fn wait(&mut self, tickets: Vec<T>, deadline: D) -> (out: Result<C, W>)
        requires old(self).waited.is_none(), old(self).deadline.is_none(), old(self).wait_return.is_some(),
        ensures
            out == old(self).wait_return.unwrap(),
            final(self).waited == Some(tickets), final(self).deadline == Some(deadline),
            final(self).wait_return.is_none(), final(self).owner == old(self).owner,
            final(self).submit_return == old(self).submit_return,
            final(self).submitted == old(self).submitted,
    {
        self.waited = Some(tickets);
        self.deadline = Some(deadline);
        self.wait_return.take().unwrap()
    }

    fn publish(&mut self, phase: BatchPhase<R, T, S, W>) -> (out: BatchPhase<R, T, S, W>)
        requires
            old(self).submitted.is_none(),
            !submits(phase) || old(self).submit_return.is_some(),
        ensures
            out == published(phase, old(self).submit_return),
            final(self).owner == old(self).owner,
            final(self).wait_return == old(self).wait_return,
            final(self).waited == old(self).waited, final(self).deadline == old(self).deadline,
            match phase {
                BatchPhase::Prepared(requests) => final(self).submitted == Some(requests)
                    && final(self).submit_return.is_none(),
                _ => final(self).submitted == old(self).submitted
                    && final(self).submit_return == old(self).submit_return,
            },
    {
        retained_pair_publish_once_body!(verus_exec_expr, phase, requests, self.submit(requests))
    }

    fn advance(&mut self, phase: BatchPhase<R, T, S, W>, deadline: D)
        -> (out: Advanced<BatchPhase<R, T, S, W>, C, W>)
        requires
            old(self).cold(),
            !submits(phase) || old(self).submit_return.is_some(),
            !waits(published(phase, old(self).submit_return)) || old(self).wait_return.is_some(),
        ensures
            out == advanced(phase, old(self).submit_return, old(self).wait_return),
            final(self).owner == old(self).owner,
            match phase {
                BatchPhase::Prepared(requests) => final(self).submitted == Some(requests)
                    && final(self).submit_return.is_none(),
                _ => final(self).submitted.is_none()
                    && final(self).submit_return == old(self).submit_return,
            },
            match published(phase, old(self).submit_return) {
                BatchPhase::Pending(tickets) => final(self).waited == Some(tickets)
                    && final(self).deadline == Some(deadline) && final(self).wait_return.is_none(),
                _ => final(self).waited.is_none() && final(self).deadline.is_none()
                    && final(self).wait_return == old(self).wait_return,
            },
    {
        retained_pair_advance_body!(verus_exec_expr, self.publish(phase), self, deadline, tickets)
    }
}
}
