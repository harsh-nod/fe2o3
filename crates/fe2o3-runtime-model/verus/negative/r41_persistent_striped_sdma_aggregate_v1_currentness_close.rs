// Expected-negative R41 mutation: full publication commits while currentness is open.
use vstd::prelude::*;
verus! {
pub enum CurrentnessPhaseV1 { Open, Closed }
pub enum CursorPhaseV1 { Retained, Committed }
pub open spec fn mutated_currentness_phase_v1() -> CurrentnessPhaseV1 {
    CurrentnessPhaseV1::Open
}
pub open spec fn mutated_cursor_phase_v1() -> CursorPhaseV1 {
    CursorPhaseV1::Committed
}
pub proof fn mutated_cursor_commit_requires_close_v1()
    ensures mutated_cursor_phase_v1() == CursorPhaseV1::Committed
        ==> mutated_currentness_phase_v1() == CurrentnessPhaseV1::Closed,
{}
}
