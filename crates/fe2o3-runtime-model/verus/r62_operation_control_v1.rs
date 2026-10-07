// Abstract host decisions only. Atomic/Rust/executor refinement is external.
// Phase ABI: queued=0, cancelled=1, started=2, observing=3, finished=4,
// stopped-before=5, stopped-after=6. Action ABI: cancel=0, start=1,
// accept=2, finish=3, stop=4, timeout=5, drop-observer=6.
use vstd::prelude::*;
verus! {
pub open spec fn next_v1(p: nat, a: nat) -> nat {
    if p == 0 && a == 0 { 1 }
    else if p == 0 && a == 1 { 2 }
    else if p == 2 && a == 2 { 3 }
    else if (p == 2 || p == 3) && a == 3 { 4 }
    else if p == 0 && a == 4 { 5 }
    else if (p == 2 || p == 3) && a == 4 { 6 }
    else { p }
}
pub struct ControlV1 { pub phase: nat, pub starts: nat }
pub open spec fn valid_v1(s: ControlV1) -> bool {
    s.phase <= 6 && s.starts == if s.phase == 0 || s.phase == 1 || s.phase == 5 { 0nat } else { 1nat }
}
pub open spec fn step_v1(s: ControlV1, a: nat) -> ControlV1 {
    ControlV1 { phase: next_v1(s.phase, a),
        starts: s.starts + if s.phase == 0 && a == 1 { 1nat } else { 0nat } }
}
pub struct ObservationV1 {
    pub operation: nat, pub control: ControlV1, pub retained: bool,
    pub gpu_complete: bool, pub observing: bool,
}
pub open spec fn abandon_v1(s: ObservationV1) -> ObservationV1 {
    ObservationV1 { operation: s.operation, control: s.control, retained: s.retained,
        gpu_complete: s.gpu_complete, observing: false }
}
pub proof fn cancel_start_are_exclusive_v1()
    ensures next_v1(next_v1(0, 0), 1) == 1,
            next_v1(next_v1(0, 1), 0) == 2,
{}
pub proof fn cancellation_prevents_later_submission_v1(a: nat)
    requires a <= 6,
    ensures next_v1(1, a) == 1,
{}
pub proof fn submission_starts_at_most_once_v1(s: ControlV1, a: nat)
    requires valid_v1(s), a <= 6,
    ensures valid_v1(step_v1(s, a)), step_v1(s, a).starts <= 1,
{}
pub proof fn no_transition_reopens_queued_v1(p: nat, a: nat)
    requires 0 < p <= 6, a <= 6,
    ensures next_v1(p, a) != 0,
{}
pub proof fn stop_preserves_submission_disposition_v1(p: nat)
    requires p == 0 || p == 1 || p == 2 || p == 3,
    ensures (p == 0 ==> next_v1(p, 4) == 5),
            (p == 1 ==> next_v1(p, 4) == 1),
            (p == 2 || p == 3 ==> next_v1(p, 4) == 6),
{}
pub proof fn terminal_phases_absorb_v1(p: nat, a: nat)
    requires p == 1 || p == 4 || p == 5 || p == 6, a <= 6,
    ensures next_v1(p, a) == p,
{}
pub proof fn finished_observation_requires_started_path_v1(p: nat, a: nat)
    requires p <= 6, a <= 6, p != 4, next_v1(p, a) == 4,
    ensures (p == 2 || p == 3) && a == 3,
{}
pub proof fn timeout_and_drop_preserve_identity_and_custody_v1(s: ObservationV1, a: nat)
    requires s.control.phase <= 6, a == 5 || a == 6,
    ensures next_v1(s.control.phase, a) == s.control.phase,
            abandon_v1(s).operation == s.operation,
            abandon_v1(s).control == s.control,
            abandon_v1(s).retained == s.retained,
            abandon_v1(s).gpu_complete == s.gpu_complete,
{}
}
