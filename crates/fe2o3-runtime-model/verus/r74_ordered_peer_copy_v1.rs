// Abstract ordered cursor safety. Native observations, Rust correspondence,
// mapping identity, DMA correctness, deadlines and liveness remain external.
use vstd::prelude::*;
verus! {
pub struct CursorV1 {
    pub count: nat, pub completed: nat, pub ticket: bool, pub ever: bool,
    pub recovered: bool, pub open: bool, pub successful_close: bool, pub outcome: nat,
}
// Outcome: running=0, succeeded=1, failed=2, cancelled=3, terminal=4.
pub open spec fn valid_v1(s: CursorV1) -> bool {
    1 <= s.count <= 4096 && s.completed <= s.count && s.outcome <= 4
    && (s.ticket ==> s.completed < s.count && s.ever && !s.recovered)
    && (s.completed > 0 ==> s.ever)
    && (s.recovered ==> s.ever && !s.ticket && s.completed < s.count)
    && (s.open ==> s.outcome == 0 && !s.successful_close)
    && (s.outcome == 0 && !s.open && s.completed == s.count ==> s.successful_close)
    && (s.outcome == 1 ==> !s.open && !s.ticket && !s.recovered
        && s.completed == s.count && s.successful_close)
    && (s.outcome == 2 ==> !s.ticket)
    && (s.outcome == 3 ==> !s.ever && !s.ticket && s.completed == 0)
}
pub open spec fn initial_v1(count: nat) -> CursorV1 {
    CursorV1 { count, completed: 0, ticket: false, ever: false,
        recovered: false, open: false, successful_close: false, outcome: 0 }
}
// Actions: open=0, publish=1, complete=2, recover=3, close-ok=4,
// close-failed=5, succeed=6, fail=7, cancel=8, quarantine=9.
// Invalid Rust transitions return None; this spec stutters on them.
pub open spec fn step_v1(s: CursorV1, action: nat, segment: nat) -> CursorV1 {
    if s.outcome != 0 { s }
    else if action == 0 && !s.open && !s.recovered {
        CursorV1 { open: true, successful_close: false, ..s }
    } else if action == 1 && s.open && !s.recovered && !s.ticket && s.completed < s.count {
        CursorV1 { ticket: true, ever: true, ..s }
    } else if action == 2 && s.open && s.ticket && segment == s.completed {
        CursorV1 { ticket: false, completed: s.completed + 1, ..s }
    } else if action == 3 && s.open && s.ticket {
        CursorV1 { ticket: false, recovered: true, ..s }
    } else if action == 4 && s.open {
        CursorV1 { open: false, successful_close: true, ..s }
    } else if action == 5 && s.open {
        CursorV1 { open: false, outcome: 4, ..s }
    } else if action == 6 && !s.open && !s.ticket && s.completed == s.count {
        CursorV1 { outcome: 1, ..s }
    } else if action == 7 && !s.open && !s.ticket {
        CursorV1 { outcome: 2, ..s }
    } else if action == 8 && !s.open && !s.ever {
        CursorV1 { outcome: 3, ..s }
    } else if action == 9 && !s.open {
        CursorV1 { outcome: 4, ..s }
    } else { s }
}
pub proof fn initialization_v1(count: nat)
    requires 1 <= count <= 4096,
    ensures valid_v1(initial_v1(count)),
{}
pub proof fn preservation_v1(s: CursorV1, action: nat, segment: nat)
    requires valid_v1(s),
    ensures valid_v1(step_v1(s, action, segment)),
{}
pub proof fn cursor_order_and_publication_history_v1(s: CursorV1, action: nat, segment: nat)
    requires valid_v1(s),
    ensures step_v1(s, action, segment).count == s.count,
        s.completed <= step_v1(s, action, segment).completed <= s.completed + 1,
        s.ever ==> step_v1(s, action, segment).ever,
        step_v1(s, action, segment).completed > s.completed
            ==> s.ticket && s.open && segment == s.completed && action == 2,
{}
pub proof fn no_double_publication_v1(s: CursorV1)
    requires valid_v1(s), s.ticket,
    ensures step_v1(s, 1, 0) == s,
{}
pub proof fn wrong_completion_preserves_custody_v1(s: CursorV1, segment: nat)
    requires valid_v1(s), segment != s.completed,
    ensures step_v1(s, 2, segment) == s,
{}
pub proof fn cancellation_excludes_every_published_prefix_v1(s: CursorV1)
    requires valid_v1(s), s.ever,
    ensures step_v1(s, 8, 0) == s,
{}
pub proof fn recovery_cannot_republish_or_reopen_v1(s: CursorV1)
    requires valid_v1(s), s.recovered,
    ensures step_v1(s, 0, 0) == s, step_v1(s, 1, 0) == s,
{}
pub proof fn failed_close_is_absorbing_v1(s: CursorV1, action: nat, segment: nat)
    requires valid_v1(s), s.open,
    ensures step_v1(s, 5, 0).outcome == 4,
        step_v1(step_v1(s, 5, 0), action, segment) == step_v1(s, 5, 0),
{}
pub proof fn success_requires_complete_prefix_and_successful_close_v1(s: CursorV1, action: nat, segment: nat)
    requires valid_v1(s), step_v1(s, action, segment).outcome == 1,
    ensures !step_v1(s, action, segment).open,
        !step_v1(s, action, segment).ticket,
        step_v1(s, action, segment).completed == s.count,
        step_v1(s, action, segment).successful_close,
{ preservation_v1(s, action, segment); }
pub proof fn terminal_outcomes_absorb_v1(s: CursorV1, action: nat, segment: nat)
    requires valid_v1(s), s.outcome != 0,
    ensures step_v1(s, action, segment) == s,
{}
}
