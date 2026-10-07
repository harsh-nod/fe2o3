use vstd::prelude::*;

include!("../../fe2o3-runtime/src/kfd_backend/compute_quiescence_body.rs");

verus! {

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuiescenceActionV1 { Invalid, Complete, Advance, Wait, Poll }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuiescenceStepV1 {
    pub cursor: usize,
    pub polled: bool,
    pub action: QuiescenceActionV1,
}

pub open spec fn expected_action(cursor: usize, len: usize, polled: bool, exact: bool)
    -> QuiescenceActionV1
{
    if cursor > len { QuiescenceActionV1::Invalid }
    else if cursor == len { QuiescenceActionV1::Complete }
    else if exact { QuiescenceActionV1::Advance }
    else if polled { QuiescenceActionV1::Wait }
    else { QuiescenceActionV1::Poll }
}

pub fn quiescence_step_v1(cursor: usize, len: usize, polled: bool, exact: bool)
    -> (out: QuiescenceStepV1)
    ensures
        out.action == expected_action(cursor, len, polled, exact),
        out.cursor as int == if cursor < len && exact { cursor + 1 } else { cursor as int },
        out.polled == (polled || (cursor < len && !exact)),
        out.cursor >= cursor,
        cursor <= len ==> out.cursor <= len,
        polled ==> out.action != QuiescenceActionV1::Poll,
{
    compute_quiescence_step_body!(verus_exec_expr, cursor, len, polled, exact)
}

pub fn quiescence_complete_v1(cursor: usize, len: usize) -> (out: bool)
    ensures out == (cursor == len),
{
    compute_quiescence_complete_body!(verus_exec_expr, cursor, len)
}

pub fn empty_roster_witness(polled: bool, exact: bool) -> (out: QuiescenceStepV1)
    ensures out.action == QuiescenceActionV1::Complete, out.cursor == 0, out.polled == polled,
{
    quiescence_step_v1(0, 0, polled, exact)
}

pub fn final_entry_witness(len: usize, polled: bool) -> (out: bool)
    requires len > 0,
    ensures out,
{
    let next = quiescence_step_v1(len - 1, len, polled, true);
    assert(next.action == QuiescenceActionV1::Advance);
    assert(next.cursor == len);
    assert(next.polled == polled);
    quiescence_complete_v1(next.cursor, len)
}

pub fn exhausted_budget_witness(cursor: usize, len: usize, exact: bool) -> (out: QuiescenceStepV1)
    requires cursor < len,
    ensures out.action != QuiescenceActionV1::Poll, out.polled,
        out.action == if exact { QuiescenceActionV1::Advance } else { QuiescenceActionV1::Wait },
{
    quiescence_step_v1(cursor, len, true, exact)
}

pub fn poll_then_pending_witness(cursor: usize, len: usize) -> (out: QuiescenceStepV1)
    requires cursor < len,
    ensures out.action == QuiescenceActionV1::Wait, out.cursor == cursor, out.polled,
{
    let first = quiescence_step_v1(cursor, len, false, false);
    assert(first.action == QuiescenceActionV1::Poll);
    assert(first.cursor == cursor);
    quiescence_step_v1(first.cursor, len, first.polled, false)
}

pub fn poll_then_quiescent_witness(cursor: usize, len: usize) -> (out: QuiescenceStepV1)
    requires cursor < len,
    ensures out.action == QuiescenceActionV1::Advance, out.cursor == cursor + 1, out.polled,
{
    let first = quiescence_step_v1(cursor, len, false, false);
    quiescence_step_v1(first.cursor, len, first.polled, true)
}

pub fn invalid_cursor_witness(cursor: usize, len: usize, polled: bool, exact: bool)
    -> (out: QuiescenceStepV1)
    requires cursor > len,
    ensures out.action == QuiescenceActionV1::Invalid, out.cursor == cursor, out.polled == polled,
{
    let ready = quiescence_complete_v1(cursor, len);
    assert(!ready);
    quiescence_step_v1(cursor, len, polled, exact)
}

pub fn readiness_agrees_with_completion_witness(cursor: usize, len: usize, polled: bool, exact: bool) {
    let ready = quiescence_complete_v1(cursor, len);
    let next = quiescence_step_v1(cursor, len, polled, exact);
    assert(ready <==> next.action == QuiescenceActionV1::Complete);
    assert(next.action == QuiescenceActionV1::Poll ==> !ready);
    assert(next.action == QuiescenceActionV1::Invalid ==> !ready);
}

pub fn maximum_cursor_witness(polled: bool, exact: bool) -> (out: QuiescenceStepV1)
    ensures out.action == QuiescenceActionV1::Complete, out.cursor == usize::MAX,
{
    quiescence_step_v1(usize::MAX, usize::MAX, polled, exact)
}

}
