// Conditional refinement of the actual Context fold controller, not yet the
// native per-input validator. Each receipt must correspond to one reached
// validation, including whether its family cursor advanced before it returned.
// Credit receipts describe individual reached calls and can disagree. No credit
// snapshot, Arc interior frame, native freshness, Mutex poison recovery, unwind,
// physical Vec allocation, Rust compiler or ISA correctness is claimed.
#![allow(unused_macros)]
use vstd::prelude::*;

include!("../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs");

verus! {
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextProducerReadStatusV1 { Success, Pending, NoEffect, Unknown }

#[derive(Clone, Copy)]
enum Family { Active, Queued }

// These are observations, not a persistent credit_ok field of the owner.
enum CreditObservation {
    NotReached,
    Returned {
        allocation_generation: u64, allocation_local: u64,
        device_generation: u64, device_local: u64,
        byte_len: u64, matched: bool,
    },
}

struct InputObservation<E> {
    family: Family,
    advanced: bool,
    result: Result<ContextProducerReadStatusV1, E>,
    credit: CreditObservation,
}

// C and E are intentionally opaque and need not be Copy or Clone. Only this
// replay's receipts are consumed; C models local stored ownership, not whatever
// an Arc or native handle may refer to outside that stored value.
struct Observations<C, E> {
    owner: C,
    active: usize,
    queued: usize,
    records: Vec<Option<InputObservation<E>>>,
    consumed: usize,
    ghost original: Seq<InputObservation<E>>,
}

spec fn active_before<E>(history: Seq<InputObservation<E>>, end: int) -> nat
    recommends 0 <= end <= history.len(),
    decreases end,
{
    if end <= 0 { 0 } else {
        active_before(history, end - 1)
            + if history[end - 1].advanced && (match history[end - 1].family { Family::Active => true, Family::Queued => false }) { 1nat } else { 0nat }
    }
}

spec fn queued_before<E>(history: Seq<InputObservation<E>>, end: int) -> nat
    recommends 0 <= end <= history.len(),
    decreases end,
{
    if end <= 0 { 0 } else {
        queued_before(history, end - 1)
            + if history[end - 1].advanced && (match history[end - 1].family { Family::Active => false, Family::Queued => true }) { 1nat } else { 0nat }
    }
}

proof fn cursor_bounds<E>(history: Seq<InputObservation<E>>, end: int)
    requires 0 <= end <= history.len(),
    ensures active_before(history, end) <= end, queued_before(history, end) <= end,
    decreases end,
{
    if end > 0 { cursor_bounds(history, end - 1); }
}

spec fn combine(left: ContextProducerReadStatusV1, right: ContextProducerReadStatusV1)
    -> ContextProducerReadStatusV1
{
    match (left, right) {
        (ContextProducerReadStatusV1::Unknown, _) | (_, ContextProducerReadStatusV1::Unknown) => ContextProducerReadStatusV1::Unknown,
        (ContextProducerReadStatusV1::NoEffect, _) | (_, ContextProducerReadStatusV1::NoEffect) => ContextProducerReadStatusV1::NoEffect,
        (ContextProducerReadStatusV1::Pending, _) | (_, ContextProducerReadStatusV1::Pending) => ContextProducerReadStatusV1::Pending,
        _ => ContextProducerReadStatusV1::Success,
    }
}

spec fn fold_result<E>(history: Seq<InputObservation<E>>, start: int,
    aggregate: ContextProducerReadStatusV1, active: usize, queued: usize, invalid: E)
    -> Result<ContextProducerReadStatusV1, E>
    recommends 0 <= start <= history.len(),
    decreases history.len() - start,
{
    if start >= history.len() {
        if active_before(history, history.len() as int) == active
            && queued_before(history, history.len() as int) == queued { Ok(aggregate) }
        else { Err(invalid) }
    } else {
        match history[start].result {
            Err(error) => Err(error),
            Ok(status) => fold_result(history, start + 1, combine(aggregate, status), active, queued, invalid),
        }
    }
}

spec fn reached<E>(history: Seq<InputObservation<E>>, start: int) -> nat
    recommends 0 <= start <= history.len(),
    decreases history.len() - start,
{
    if start >= history.len() { history.len() }
    else if history[start].result.is_err() { (start + 1) as nat }
    else { reached(history, start + 1) }
}

impl<C, E> Observations<C, E> {
    closed spec fn wf(&self) -> bool {
        &&& self.original.len() == self.records@.len()
        &&& self.consumed <= self.records@.len()
        &&& forall|i: int| 0 <= i < self.records@.len() ==>
            self.records@[i] == if i < self.consumed { None } else { Some(self.original[i]) }
    }

    fn input_count(&self) -> (out: usize)
        requires self.wf(),
        ensures out == self.original.len(),
    { self.records.len() }

    fn active_count(&self) -> (out: usize)
        ensures out == self.active,
    { self.active }

    fn queued_count(&self) -> (out: usize)
        ensures out == self.queued,
    { self.queued }

    fn validate(&mut self, index: usize, active: &mut usize, queued: &mut usize)
        -> (out: Result<ContextProducerReadStatusV1, E>)
        requires
            old(self).wf(), index == old(self).consumed,
            index < old(self).original.len(),
            *old(active) == active_before(old(self).original, index as int),
            *old(queued) == queued_before(old(self).original, index as int),
        ensures
            final(self).wf(), final(self).owner == old(self).owner,
            final(self).original == old(self).original,
            final(self).active == old(self).active, final(self).queued == old(self).queued,
            final(self).consumed == index + 1,
            *final(active) == active_before(final(self).original, index as int + 1),
            *final(queued) == queued_before(final(self).original, index as int + 1),
            out == final(self).original[index as int].result,
    {
        proof {
            vstd::std_specs::vec::axiom_spec_len(&self.records);
            assert(self.consumed == index && index < usize::MAX);
            cursor_bounds(self.original, index as int);
        }
        let observation = self.records[index].take().unwrap();
        if observation.advanced {
            match observation.family {
                Family::Active => *active += 1,
                Family::Queued => *queued += 1,
            }
        }
        self.consumed += 1;
        observation.result
    }

    fn reconcile(&mut self, invalid_reference: E)
        -> (out: Result<ContextProducerReadStatusV1, E>)
        requires old(self).wf(), old(self).consumed == 0,
        ensures
            final(self).wf(), final(self).owner == old(self).owner,
            final(self).original == old(self).original,
            final(self).active == old(self).active, final(self).queued == old(self).queued,
            final(self).consumed == reached(old(self).original, 0),
            out == fold_result(old(self).original, 0, ContextProducerReadStatusV1::Success,
                old(self).active, old(self).queued, invalid_reference),
    {
        let ghost before = *self;
        producer_input_fold_body!(verus_exec_expr, self, invalid_reference,
            (index, aggregate, active_index, queued_index, input_count),
            [invariant
                before == *old(self),
                self.wf(), self.owner == before.owner,
                self.original == before.original,
                self.active == before.active, self.queued == before.queued,
                input_count == before.original.len(), index <= input_count,
                self.consumed == index,
                active_index == active_before(before.original, index as int),
                queued_index == queued_before(before.original, index as int),
                fold_result(before.original, 0, ContextProducerReadStatusV1::Success,
                    before.active, before.queued, invalid_reference)
                    == fold_result(before.original, index as int, aggregate,
                        before.active, before.queued, invalid_reference),
                reached(before.original, 0) == reached(before.original, index as int),
             decreases input_count - index],
            [proof {
                reveal_with_fuel(fold_result, 2);
                reveal_with_fuel(reached, 2);
            }],
            [])
    }
}
}
