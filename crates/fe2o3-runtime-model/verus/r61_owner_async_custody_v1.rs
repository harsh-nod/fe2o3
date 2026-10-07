// Abstract owner policy only: adapter observations and executable refinement are external.
use vstd::prelude::*;
verus! {
pub open spec fn release_v1(normal: bool, cleanup: bool, attempted: bool, native: bool) -> bool {
    normal && cleanup && attempted && native
}
pub open spec fn resolve_v1(resolved: bool, replies: nat) -> nat {
    if resolved { replies } else { replies + 1 }
}
pub open spec fn admits_v1(retained: nat, capacity: nat) -> bool {
    0 < capacity && capacity <= 65536 && retained < capacity
}
pub struct ObservationV1 {
    pub operation: nat,
    pub observing: bool,
    pub retained: bool,
    pub completed: bool,
}
pub open spec fn abandon_v1(state: ObservationV1) -> ObservationV1 {
    ObservationV1 { operation: state.operation, observing: false,
        retained: state.retained, completed: state.completed }
}
pub proof fn release_requires_all_shutdown_premises_v1(n: bool, c: bool, a: bool, s: bool)
    requires release_v1(n, c, a, s),
    ensures n && c && a && s,
{}
pub proof fn incomplete_cleanup_retains_owner_v1(n: bool, a: bool, s: bool)
    ensures !release_v1(n, false, a, s),
{}
pub proof fn native_failure_retains_owner_v1(n: bool, c: bool, a: bool)
    ensures !release_v1(n, c, a, false),
{}
pub proof fn panic_retains_owner_v1(c: bool, a: bool, s: bool)
    ensures !release_v1(false, c, a, s),
{}
pub proof fn reply_is_resolved_once_v1(replies: nat)
    ensures resolve_v1(true, resolve_v1(false, replies)) == replies + 1,
{}
pub proof fn abandon_preserves_exact_operation_custody_v1(state: ObservationV1)
    ensures abandon_v1(state).operation == state.operation,
            abandon_v1(state).retained == state.retained,
{}
pub proof fn stopping_observation_does_not_establish_completion_v1(state: ObservationV1)
    requires !state.completed,
    ensures !abandon_v1(state).completed,
{}
pub proof fn admission_conserves_bounded_records_v1(retained: nat, capacity: nat)
    requires admits_v1(retained, capacity),
    ensures retained + 1 <= capacity,
            (capacity - (retained + 1)) + (retained + 1) == capacity,
{}
}
