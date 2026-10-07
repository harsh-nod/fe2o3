// Shared specification of the fold controller. Errors and stored owners do not
// need Copy or Clone; credit observations describe only individual reached calls.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextProducerReadStatusV1 { Success, Pending, NoEffect, Unknown }

#[derive(Clone, Copy)]
pub(crate) enum Family { Active, Queued }

pub(crate) enum CreditObservation {
    NotReached,
    Returned {
        allocation_generation: u64, allocation_local: u64,
        device_generation: u64, device_local: u64,
        byte_len: u64, matched: bool,
    },
}

pub(crate) struct InputObservation<E> {
    pub(crate) family: Family,
    pub(crate) advanced: bool,
    pub(crate) result: Result<ContextProducerReadStatusV1, E>,
    pub(crate) credit: CreditObservation,
}

pub(crate) open spec fn active_before<E>(history: Seq<InputObservation<E>>, end: int) -> nat
    recommends 0 <= end <= history.len(),
    decreases end,
{
    if end <= 0 { 0 } else {
        active_before(history, end - 1)
            + if history[end - 1].advanced && (match history[end - 1].family { Family::Active => true, Family::Queued => false }) { 1nat } else { 0nat }
    }
}

pub(crate) open spec fn queued_before<E>(history: Seq<InputObservation<E>>, end: int) -> nat
    recommends 0 <= end <= history.len(),
    decreases end,
{
    if end <= 0 { 0 } else {
        queued_before(history, end - 1)
            + if history[end - 1].advanced && (match history[end - 1].family { Family::Active => false, Family::Queued => true }) { 1nat } else { 0nat }
    }
}

pub(crate) proof fn cursor_bounds<E>(history: Seq<InputObservation<E>>, end: int)
    requires 0 <= end <= history.len(),
    ensures active_before(history, end) <= end, queued_before(history, end) <= end,
    decreases end,
{
    if end > 0 { cursor_bounds(history, end - 1); }
}

pub(crate) open spec fn combine(left: ContextProducerReadStatusV1, right: ContextProducerReadStatusV1)
    -> ContextProducerReadStatusV1
{
    match (left, right) {
        (ContextProducerReadStatusV1::Unknown, _) | (_, ContextProducerReadStatusV1::Unknown) => ContextProducerReadStatusV1::Unknown,
        (ContextProducerReadStatusV1::NoEffect, _) | (_, ContextProducerReadStatusV1::NoEffect) => ContextProducerReadStatusV1::NoEffect,
        (ContextProducerReadStatusV1::Pending, _) | (_, ContextProducerReadStatusV1::Pending) => ContextProducerReadStatusV1::Pending,
        _ => ContextProducerReadStatusV1::Success,
    }
}

pub(crate) open spec fn fold_result<E>(history: Seq<InputObservation<E>>, start: int,
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

pub(crate) open spec fn reached<E>(history: Seq<InputObservation<E>>, start: int) -> nat
    recommends 0 <= start <= history.len(),
    decreases history.len() - start,
{
    if start >= history.len() { history.len() }
    else if history[start].result.is_err() { (start + 1) as nat }
    else { reached(history, start + 1) }
}
}
